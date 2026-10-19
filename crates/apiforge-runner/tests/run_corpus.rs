//! Differential tests against run reports recorded from the TypeScript reference.
//!
//! The corpus is produced by `scripts/export-compatibility-fixtures.ts` (see
//! `scripts/run-fixtures.ts`). Every run uses simulation mode and a seeded random
//! source shared with the generator. A mismatch means the port changed observable
//! behaviour; fix the port or record the divergence below with its reason. Never
//! edit the corpus by hand.

use apiforge_core::{
    format::Project,
    js::strict_equals,
    run::{RunMode, RunResult},
};
use apiforge_runner::{HttpTransport, RunLimits, RunOptions, RunRequest, run_scenario};
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::{fs, path::PathBuf};
use tokio_util::sync::CancellationToken;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    description: String,
    random_seed: u32,
    request: FixtureRequest,
    expected: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureRequest {
    project: Project,
    scenario_id: String,
    mode: RunMode,
    #[serde(default)]
    target_node_id: Option<String>,
    #[serde(default)]
    single_node_id: Option<String>,
    #[serde(default)]
    previous_run: Option<RunResult>,
    #[serde(default)]
    secrets: IndexMap<String, String>,
}

/// Deliberate divergences: (fixture, JSON pointer into the normalised report, text
/// the reference publishes that the port masks).
const MASKED_LEAKS: &[(&str, &str, &str)] = &[(
    // The secret goes raw into a URL template; setting a query parameter re-serialises
    // the query as a form, so `my+session secret` becomes `my+session+secret`, which
    // none of the reference's three masked forms matches.
    "redaction",
    "/steps/0/request/url",
    "my+session+secret",
)];

/// Fixtures the reference runs but the port rejects before running, with the reason.
const REJECTED: &[(&str, &str)] = &[(
    // The core validator limits component nesting to 16 levels at import; the
    // reference only detects it at run time. The runtime guard is tested separately.
    "component-depth",
    "Component nesting exceeds 16 levels",
)];

/// The `Math.random` replacement of the generator: a 32-bit LCG.
fn lcg(seed: u32) -> impl FnMut() -> f64 + Send {
    let mut state = seed;
    move || {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f64::from(state) / 4_294_967_296.0
    }
}

fn normalize_step(step: &Value) -> Value {
    let mut output = Map::new();
    for key in [
        "nodeId", "name", "kind", "status", "phase", "inputs", "outputs", "actual", "expected",
        "request",
    ] {
        if let Some(value) = step.get(key) {
            output.insert(key.to_owned(), value.clone());
        }
    }
    if let Some(Value::Array(children)) = step.get("children") {
        output.insert(
            "children".into(),
            Value::Array(children.iter().map(normalize_step).collect()),
        );
    }
    Value::Object(output)
}

fn normalize_run(run: &RunResult, single: bool) -> Value {
    let report = serde_json::to_value(run).expect("reports serialise");
    let steps = report
        .get("steps")
        .and_then(Value::as_array)
        .map(|steps| steps.iter().map(normalize_step).collect())
        .unwrap_or_default();
    let mut output = Map::new();
    output.insert("status".into(), report["status"].clone());
    output.insert("diagnostic".into(), json!(run.diagnostic));
    if !single {
        output.insert("hasError".into(), json!(run.error.is_some()));
    }
    output.insert("steps".into(), Value::Array(steps));
    Value::Object(output)
}

fn fixtures() -> Vec<(String, Fixture)> {
    let directory =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/compatibility/runs");
    let mut entries: Vec<_> = fs::read_dir(&directory)
        .expect("run corpus exists")
        .map(|entry| entry.expect("readable directory").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    entries.sort();
    entries
        .into_iter()
        .map(|path| {
            let slug = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("UTF-8 file name")
                .to_owned();
            let text = fs::read_to_string(&path).expect("readable fixture");
            let fixture = serde_json::from_str(&text)
                .unwrap_or_else(|error| panic!("{slug}: invalid fixture: {error}"));
            (slug, fixture)
        })
        .collect()
}

#[tokio::test]
async fn simulation_runs_match_the_reference() {
    let fixtures = fixtures();
    assert!(fixtures.len() >= 20, "corpus too small: {}", fixtures.len());
    let transport = HttpTransport::new().expect("client builds");
    let mut failures = Vec::new();
    for (slug, fixture) in fixtures {
        let request = fixture.request;
        let single = request.single_node_id.is_some();
        let run_request = RunRequest {
            project: request.project,
            scenario_id: request.scenario_id,
            mode: request.mode,
            target_node_id: request.target_node_id,
            single_node_id: request.single_node_id,
            previous_run: request.previous_run,
            secrets: request.secrets,
        };
        let options = RunOptions {
            limits: RunLimits::default(),
            transport: transport.clone(),
            random: Box::new(lcg(fixture.random_seed)),
        };
        let result = run_scenario(run_request, options, CancellationToken::new(), |_| {}).await;
        if let Some((_, reason)) = REJECTED.iter().find(|(rejected, _)| *rejected == slug) {
            let error = result
                .expect_err("the port rejects this project")
                .to_string();
            assert!(
                error.contains(reason),
                "{slug}: unexpected rejection: {error}"
            );
            continue;
        }
        let run = result.unwrap_or_else(|error| panic!("{slug}: request rejected: {error}"));
        assert!(run.finished_at.is_some(), "{slug}: unfinished report");
        if single {
            assert!(
                run.notice.is_some(),
                "{slug}: provenance belongs in `notice`"
            );
        }
        let mut expected = fixture.expected;
        for (fixture_slug, pointer, leaked) in MASKED_LEAKS {
            if *fixture_slug == slug {
                let field = expected
                    .pointer_mut(pointer)
                    .unwrap_or_else(|| panic!("{slug}: no field at {pointer}"));
                let text = field.as_str().expect("text field").to_owned();
                assert!(
                    text.contains(leaked),
                    "{slug}: divergence no longer present"
                );
                *field = Value::String(text.replace(leaked, "[REDACTED]"));
            }
        }
        let actual = normalize_run(&run, single);
        if !strict_equals(Some(&actual), Some(&expected)) {
            failures.push(format!(
                "{slug} ({}):\nexpected {}\nactual   {}",
                fixture.description,
                serde_json::to_string(&expected).unwrap_or_default(),
                serde_json::to_string(&actual).unwrap_or_default()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
