//! LANG-22 (#522): a failed type check says which type the value has, beside
//! the value, so the reader does not work it out; a type read from a value
//! is still checked by the run. The message is the step's, so Run results,
//! the command line and the MCP server show the same words.
//!
//! The runs here are simulated: nothing is sent.

use apiforge_core::{
    demos::shop_project,
    format::{OptionalJson, Project, Scenario},
    run::{RunMode, RunResult, StepResult, StepStatus},
};
use apiforge_runner::{HttpTransport, RunOptions, RunRequest, run_scenario};
use apiforge_script::{Document, LowerContext, lower, parse};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

/// The scenario lowered as a folder source in the demo shop, which it joins.
fn scenario_in(text: &str) -> (Project, Scenario) {
    let mut project = shop_project();
    let lowered = lower(&parse(text), &LowerContext::folder(&project));
    assert!(!lowered.has_errors(), "{text}\n{:#?}", lowered.diagnostics);
    let Some(Document::Scenario(scenario)) = lowered.document else {
        panic!("not a scenario");
    };
    project.scenarios.push(scenario.clone());
    (project, scenario)
}

async fn simulate(text: &str) -> RunResult {
    let (project, scenario) = scenario_in(text);
    run_scenario(
        RunRequest::new(project, scenario.id.clone(), RunMode::Simulation),
        RunOptions::new(HttpTransport::new().expect("client builds")),
        CancellationToken::new(),
        |_| {},
    )
    .await
    .expect("the run starts")
}

fn step<'a>(run: &'a RunResult, handle: &str) -> &'a StepResult {
    run.steps
        .iter()
        .find(|step| step.handle == handle)
        .unwrap_or_else(|| panic!("a step `{handle}`"))
}

/// A scenario whose `typed` checks `field` of a cart against `expected`.
fn scenario(field: &str, expected: &str) -> String {
    format!(
        r#"scenario "Types" {{
  param kind = "nubmer"
  begin = start
  cart = GET "/carts/c-1" {{
    sample 200 {{ id: "c-1", total: 160, open: true, owner: {{ id: 7 }}, coupon: null, items: [1, 2] }}
  }}
  typed = assert cart.body.{field} is {expected}
}}
"#
    )
}

#[tokio::test]
async fn lang22_criterion_3_failure_names_the_type() {
    for (field, expected, actual, message) in [
        (
            "total",
            "string",
            json!(160),
            "The comparison failed: expected string, actual number (160).",
        ),
        (
            "id",
            "number",
            json!("c-1"),
            r#"The comparison failed: expected number, actual string ("c-1")."#,
        ),
        (
            "open",
            "string",
            json!(true),
            "The comparison failed: expected string, actual boolean (true).",
        ),
        (
            "owner",
            "array",
            json!({ "id": 7 }),
            r#"The comparison failed: expected array, actual object ({"id":7})."#,
        ),
        (
            "items",
            "object",
            json!([1, 2]),
            "The comparison failed: expected object, actual array ([1,2]).",
        ),
        (
            "coupon",
            "object",
            Value::Null,
            "The comparison failed: expected object, actual null.",
        ),
    ] {
        let run = simulate(&scenario(field, &format!("\"{expected}\""))).await;
        let typed = step(&run, "typed");
        assert_eq!(typed.status, StepStatus::Failed, "{field}");
        assert_eq!(typed.message.as_deref(), Some(message), "{field}");
        // The value and the type asked for stay where they were.
        assert_eq!(typed.actual, OptionalJson::Present(actual), "{field}");
        assert_eq!(
            typed.expected,
            OptionalJson::Present(json!(expected)),
            "{field}"
        );
    }

    // A check that passes says nothing.
    let run = simulate(&scenario("total", "\"number\"")).await;
    let typed = step(&run, "typed");
    assert_eq!(typed.status, StepStatus::Passed);
    assert_eq!(typed.message, None);
}

#[tokio::test]
async fn lang22_criterion_4_computed_type() {
    // The parameter holds a name that is no type: the file passes, and the
    // run fails the check, as today.
    let run = simulate(&scenario("total", "param.kind")).await;
    let typed = step(&run, "typed");
    assert_eq!(typed.status, StepStatus::Failed);
    assert_eq!(
        typed.message.as_deref(),
        Some("The comparison failed: expected nubmer, actual number (160).")
    );
    assert_eq!(typed.expected, OptionalJson::Present(json!("nubmer")));
}

#[tokio::test]
async fn lang22_criterion_3_failure_message_hides_secrets() {
    // The value is a secret the run learns only later, from the cleanup's
    // Authorization header, and holds a character JSON escapes: the message
    // masks it as the step's `actual` does.
    for (written, secret) in [
        (r#"demo\"secret"#, r#"demo"secret"#),
        (r"demo\\secret", r"demo\secret"),
    ] {
        let text = format!(
            r#"scenario "Secret" {{
  begin = start
  cart = GET "/carts/c-1" {{
    sample 200 {{ id: "{written}" }}
  }}
  typed = assert cart.body.id is "number"
  cleanup {{
    removed = DELETE "/carts/c-1" {{
      headers {{ Authorization: cart.body.id }}
      sample 200 {{}}
    }}
  }}
}}
"#
        );
        let run = simulate(&text).await;
        let typed = step(&run, "typed");
        assert_eq!(typed.status, StepStatus::Failed, "{secret}");
        assert_eq!(
            typed.actual,
            OptionalJson::Present(json!("[REDACTED]")),
            "{secret}"
        );
        let message = typed.message.as_deref().unwrap();
        assert!(!message.contains("secret"), "{secret}: {message}");
        assert!(
            message.starts_with("The comparison failed: expected number, actual string"),
            "{message}"
        );
    }
}
