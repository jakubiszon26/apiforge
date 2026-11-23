//! LAY-08, criterion 6: `sequence` replays the runner's order without running
//! anything. The runner's own loop is the reference, so this test sits outside
//! the unit tests of `graph`, where a dependency on the runner would see another
//! build of this crate.

use apiforge_core::{
    demos::shop_project,
    format::{Graph, Node, Project, Scenario},
    graph::{relations, sequence},
    model::ValueType,
    operator::{Operator, OperatorGroup, RandomSource},
    run::{NotRunCause, Phase, RunMode, RunResult, StepStatus},
};
use apiforge_runner::{HttpTransport, RunOptions, RunRequest, run_scenario};
use apiforge_script::{Document, LowerContext, lower, parse};
use std::collections::BTreeSet;
use std::{fs, path::Path};
use tokio_util::sync::CancellationToken;

/// A reference scenario lowered in the demo shop project, whose components it
/// calls.
fn fixture(name: &str) -> (Project, Scenario) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/scenarios/lay")
        .join(format!("{name}.afs"));
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the fixture {}: {error}", path.display()));
    let project = shop_project();
    let lowered = lower(&parse(&text), &LowerContext::folder(&project));
    assert!(!lowered.has_errors(), "{name}: {:#?}", lowered.diagnostics);
    let Some(Document::Scenario(scenario)) = lowered.document else {
        panic!("{name} is not a scenario");
    };
    (project, scenario)
}

async fn simulate(mut project: Project, scenario: &Scenario) -> RunResult {
    project.scenarios.push(scenario.clone());
    let request = RunRequest {
        project,
        scenario_id: scenario.id.clone(),
        target: apiforge_runner::RunTarget::Scenario,
        mode: RunMode::Simulation,
        target_node_id: None,
        single_node_id: None,
        previous_run: None,
        secrets: Default::default(),
    };
    let options = RunOptions::new(HttpTransport::new().expect("client builds"));
    run_scenario(request, options, CancellationToken::new(), |_| {})
        .await
        .expect("the request is valid")
}

fn executed(run: &RunResult, phase: Phase) -> Vec<&str> {
    run.steps
        .iter()
        .filter(|step| step.phase == phase)
        .map(|step| step.node_id.as_str())
        .collect()
}

fn reversed(graph: &mut Graph) {
    graph.nodes.reverse();
}

#[tokio::test]
async fn sequence_agrees_with_the_engine_on_the_fixtures() {
    for name in ["order-flow", "shop-30", "shop-50"] {
        let (project, mut scenario) = fixture(name);
        // The document as written, and turned around: a document in engine
        // order would pass with no scheduling at all.
        for turn in ["as written", "reversed"] {
            let run = simulate(project.clone(), &scenario).await;
            assert_eq!(
                sequence(&scenario.graph),
                executed(&run, Phase::Main),
                "{name}, main, {turn}"
            );
            assert_eq!(
                sequence(&scenario.cleanup),
                executed(&run, Phase::Cleanup),
                "{name}, cleanup, {turn}"
            );
            assert_eq!(
                sequence(&scenario.graph).len(),
                scenario.graph.nodes.len(),
                "{name}: every step has a place"
            );
            reversed(&mut scenario.graph);
            reversed(&mut scenario.cleanup);
        }
    }
}

// LAY-22 (#312) ---------------------------------------------------------------

fn handle_of<'a>(graph: &'a Graph, id: &str) -> &'a str {
    graph
        .nodes
        .iter()
        .find(|node| node.id == id)
        .map_or_else(|| panic!("no node `{id}`"), |node| node.handle.as_str())
}

fn node_by_handle<'a>(graph: &'a Graph, handle: &str) -> &'a Node {
    graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("no step `{handle}`"))
}

/// Criterion 4: a step's producers and readers with the field and the input
/// between them, its checks and its gates; a check's subjects and values.
#[test]
fn relations_of_cart_on_shop_30() {
    let (_, scenario) = fixture("shop-30");
    let graph = &scenario.graph;
    let handles = |ids: Vec<&str>| -> BTreeSet<String> {
        ids.into_iter()
            .map(|id| handle_of(graph, id).to_owned())
            .collect()
    };
    let set = |names: &[&str]| -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    };

    let cart = relations(graph, node_by_handle(graph, "cart"));
    assert_eq!(
        handles(cart.producers.iter().map(|feed| feed.step).collect()),
        set(&["login", "first", "second"])
    );
    let login = &node_by_handle(graph, "login").id;
    assert!(
        cart.producers.iter().any(|feed| feed.step == login
            && feed.output == "body"
            && feed.path == "token"
            && feed.input == "headers"),
        "{:#?}",
        cart.producers
    );
    let checks = handles(cart.checks.iter().map(|check| check.check).collect());
    assert!(checks.contains("cart_total"), "{checks:?}");
    assert_eq!(
        checks,
        set(&["cart_created", "cart_total", "receipt_total"])
    );
    let shared: Vec<_> = cart
        .checks
        .iter()
        .filter(|check| check.shared)
        .map(|check| handle_of(graph, check.check))
        .collect();
    assert_eq!(shared, ["receipt_total"]);
    let readers = handles(cart.readers.iter().map(|feed| feed.step).collect());
    assert!(readers.contains("order"), "{readers:?}");
    assert!(
        readers.iter().all(|reader| !checks.contains(reader)),
        "checks are listed apart: {readers:?}"
    );
    let order = &node_by_handle(graph, "order").id;
    assert!(
        cart.readers
            .iter()
            .any(|feed| feed.step == order && feed.output == "body" && feed.input == "query"),
        "{:#?}",
        cart.readers
    );
    assert!(cart.gates.is_empty() && cart.subjects.is_empty() && cart.values.is_empty());

    // A check: its subjects and its value sources.
    let cart_total = relations(graph, node_by_handle(graph, "cart_total"));
    assert_eq!(
        handles(
            cart_total
                .subjects
                .iter()
                .map(|subject| subject.step)
                .collect()
        ),
        set(&["cart"])
    );
    assert_eq!(handles(cart_total.values.clone()), set(&["expected_total"]));

    // A gate: the Data condition a step reads, and the step it checks.
    let checkout = relations(graph, node_by_handle(graph, "checkout"));
    assert_eq!(handles(checkout.gates.clone()), set(&["valid_cart"]));
    let cart_view = relations(graph, node_by_handle(graph, "cart_view"));
    assert!(
        cart_view
            .checks
            .iter()
            .any(|check| handle_of(graph, check.check) == "valid_cart" && check.gate),
        "{:#?}",
        cart_view.checks
    );
}

/// Criterion 5: a report written before causes existed loads, and writing it
/// again adds neither a cause nor the running step.
#[test]
fn a_report_without_causes_still_loads() {
    // A whole report from the run corpus, with a skipped step written the way
    // the runner wrote one before this change.
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/runs/single-with-history.json");
    let text = fs::read_to_string(&path).expect("the recorded run");
    let fixture: serde_json::Value = serde_json::from_str(&text).expect("JSON");
    let mut recorded = fixture["request"]["previousRun"].clone();
    recorded["steps"]
        .as_array_mut()
        .expect("steps")
        .push(serde_json::json!({
            "nodeId": "reads-failed", "handle": "reads_failed", "kind": "value",
            "status": "skipped", "phase": "main", "inputs": {}, "outputs": {},
            "durationMs": 0, "message": "Stopped after an error."
        }));
    assert!(!recorded.to_string().contains("\"cause\""));
    let report: RunResult = serde_json::from_value(recorded).expect("an earlier report loads");
    assert!(
        report
            .steps
            .iter()
            .any(|step| step.status == StepStatus::Skipped)
    );
    assert!(report.steps.iter().all(|step| step.cause.is_none()));
    assert_eq!(report.running, None);
    let written = serde_json::to_string(&report).expect("the report serialises");
    assert!(!written.contains("\"cause\"") && !written.contains("\"running\""));

    // A cause written now reads back as written.
    let mut later = report.clone();
    later.steps[0].cause = Some(NotRunCause::Stopped {
        by: "create".into(),
    });
    let reloaded: RunResult =
        serde_json::from_str(&serde_json::to_string(&later).expect("serialises")).expect("loads");
    assert_eq!(reloaded, later);
}

// DIAG-07 (#392) --------------------------------------------------------------

mod diag07 {
    use super::*;
    use apiforge_core::{
        block::Block,
        edit::{OutputRef, add_node},
        format::Position,
        graph::{ORDER_PORT, TRUE_PORT},
        model::NodeKind,
    };
    use apiforge_script::print_folder_document;

    const AT: Position = Position { x: 0.0, y: 0.0 };

    fn id_of(graph: &Graph, handle: &str) -> String {
        node_by_handle(graph, handle).id.clone()
    }

    /// `scenario` with a request added from the output `port` of `handle`,
    /// and the new request's identifier.
    fn added_from(scenario: &Scenario, handle: &str, port: &str) -> (Scenario, String) {
        let source = id_of(&scenario.graph, handle);
        let added = add_node(
            &scenario.graph,
            Block::Request,
            AT,
            None,
            Some(OutputRef {
                node_id: &source,
                port,
            }),
        )
        .expect("the request is added");
        let mut next = scenario.clone();
        // As the session writes a board edit: the chain is kept (LANG-13).
        next.graph = apiforge_core::chain::edited(&scenario.graph, &added.graph);
        (next, added.node_id)
    }

    fn printed(project: &Project, scenario: &Scenario) -> String {
        print_folder_document(project, &Document::Scenario(scenario.clone()))
            .expect("the scenario prints")
    }

    /// The text lowered again and printed: what a window shows of it.
    fn reprinted(project: &Project, text: &str) -> String {
        let lowered = lower(&parse(text), &LowerContext::folder(project));
        assert!(!lowered.has_errors(), "{:#?}", lowered.diagnostics);
        let Some(Document::Scenario(scenario)) = lowered.document else {
            panic!("a scenario");
        };
        printed(project, &scenario)
    }

    /// The lines of `text` without those of the statement written for
    /// `handle`: its attributes before it and its block after it.
    fn without(text: &str, handle: &str) -> String {
        let lines: Vec<&str> = text.lines().collect();
        let first = line_of(text, &format!("{handle} = "));
        let indent = |line: &str| line.len() - line.trim_start().len();
        let mut start = first;
        while start > 0 && lines[start - 1].trim_start().starts_with('@') {
            start -= 1;
        }
        let mut end = first + 1;
        if lines[first].trim_end().ends_with('{') {
            while !(indent(lines[end]) == indent(lines[first]) && lines[end].trim() == "}") {
                end += 1;
            }
            end += 1;
        }
        let mut kept: Vec<&str> = lines[..start].to_vec();
        kept.extend(&lines[end..]);
        kept.join("\n") + if text.ends_with('\n') { "\n" } else { "" }
    }

    /// The line on which `prefix` starts a statement.
    fn line_of(text: &str, prefix: &str) -> usize {
        text.lines()
            .position(|line| line.trim_start().starts_with(prefix))
            .unwrap_or_else(|| panic!("`{prefix}` is written:\n{text}"))
    }

    /// Criterion 1: on `shop-30`, a request added from `cart`'s *Next* is the
    /// step the simulated run executes next after `cart`: after its checks
    /// and values, before `cart_view` and `order`.
    #[tokio::test]
    async fn criterion_1_a_step_added_from_next_runs_directly_after_its_source() {
        let (project, scenario) = fixture("shop-30");
        let (next, added) = added_from(&scenario, "cart", ORDER_PORT);
        let run = simulate(project, &next).await;
        let order = executed(&run, Phase::Main);
        let graph = &next.graph;
        let handles = |ids: &[&str]| -> Vec<String> {
            ids.iter()
                .map(|id| handle_of(graph, id).to_owned())
                .collect()
        };
        let position = |id: &str| {
            order
                .iter()
                .position(|step| *step == id)
                .unwrap_or_else(|| {
                    let step = handle_of(graph, id);
                    panic!("{step} runs: {:?}", handles(&order))
                })
        };
        let cart = position(&id_of(graph, "cart"));
        let new = position(&added);
        assert!(cart < new, "{:?}", handles(&order));
        let between = handles(&order[cart + 1..new]);
        assert!(
            between.iter().all(|handle| matches!(
                node_by_handle(graph, handle).kind,
                NodeKind::Assert | NodeKind::Condition | NodeKind::Value | NodeKind::Export
            )),
            "only checks and values run between cart and the new step: {between:?}"
        );
        for later in ["cart_view", "address", "checkout", "order"] {
            assert!(
                new < position(&id_of(graph, later)),
                "the new step runs before {later}: {:?}",
                handles(&order)
            );
        }
        assert_eq!(sequence(graph), order, "the board's order is the run's");
    }

    /// Criterion 2: the new request is written after `cart` and the checks
    /// and values that follow it and before the next step, without `@after`
    /// since LANG-13; nothing else moves, and text → window → text is
    /// unchanged.
    #[test]
    fn criterion_2_the_step_is_written_after_its_source_and_its_checks() {
        let (project, scenario) = fixture("shop-30");
        let before = printed(&project, &scenario);
        let (next, added) = added_from(&scenario, "cart", ORDER_PORT);
        let handle = handle_of(&next.graph, &added).to_owned();
        let text = printed(&project, &next);
        let at = line_of(&text, &format!("{handle} = "));
        assert!(!text.contains("@after"), "{text}");
        assert!(line_of(&text, "cart_total = ") < at, "{text}");
        assert!(at < line_of(&text, "cart_view = "), "{text}");
        assert_eq!(without(&text, &handle), before, "nothing else moves");
        assert_eq!(reprinted(&project, &text), text, "text → window → text");
    }

    /// Criterion 3: from `paid.then` the step is written inside that branch,
    /// last, as before; nothing else in the file moves.
    #[test]
    fn criterion_3_a_step_from_a_branch_is_written_inside_it() {
        let (project, scenario) = fixture("shop-30");
        let before = printed(&project, &scenario);
        let (next, added) = added_from(&scenario, "paid", TRUE_PORT);
        let handle = handle_of(&next.graph, &added).to_owned();
        let text = printed(&project, &next);
        let at = line_of(&text, &format!("{handle} = "));
        assert!(line_of(&text, "receipt_total = ") < at, "{text}");
        assert!(at < line_of(&text, "} else {"), "{text}");
        assert_eq!(without(&text, &handle), before, "nothing else moves");
        assert_eq!(reprinted(&project, &text), text, "text → window → text");
    }

    /// Criterion 4: a block added with no source, or from a data port, is
    /// appended.
    #[test]
    fn criterion_4_without_a_source_or_from_data_the_block_is_appended() {
        let (_, scenario) = fixture("shop-30");
        let graph = &scenario.graph;
        let cart = id_of(graph, "cart");
        for source in [
            None,
            Some(OutputRef {
                node_id: &cart,
                port: "body",
            }),
        ] {
            for block in [Block::Request, Block::Comparison, Block::Value] {
                let added = add_node(graph, block, AT, None, source).expect("added");
                assert_eq!(
                    added.graph.nodes.last().map(|node| node.id.as_str()),
                    Some(added.node_id.as_str()),
                    "{block:?} from {source:?}"
                );
                assert_eq!(
                    added.graph.nodes[..graph.nodes.len()],
                    graph.nodes[..],
                    "{block:?} from {source:?}: nothing else moves"
                );
            }
        }
    }
}

// RUN-03 (#389) ---------------------------------------------------------------

/// A check and a value of `cart`, written after the step that follows it.
const CHECK_AFTER_THE_NEXT_STEP: &str = r#"scenario "Checks before the next step" {
  begin = start
  cart = POST "/carts" {
    sample 201 { id: "c-1", total: 0 }
  }
  paid = POST "/payments" {
    sample 201 { id: "p-1" }
  }
  created = assert cart.status == 201
  total = cart.body.total
  cleanup {
    removed = DELETE "/carts/{{cart.body.id}}" {
      sample 204 {}
    }
    again = GET "/carts/{{cart.body.id}}" {
      sample 404 {}
    }
    gone = assert removed.status == 204
  }
}
"#;

fn lowered_scenario(text: &str) -> (Project, Scenario) {
    let project = shop_project();
    let lowered = lower(&parse(text), &LowerContext::folder(&project));
    assert!(!lowered.has_errors(), "{:#?}", lowered.diagnostics);
    let Some(Document::Scenario(scenario)) = lowered.document else {
        panic!("not a scenario");
    };
    (project, scenario)
}

fn handles<'a>(graph: &'a Graph, ids: &[&str]) -> Vec<&'a str> {
    ids.iter().map(|id| handle_of(graph, id)).collect()
}

/// Criterion 5: the static order behind the badge numbers puts a step's
/// checks and values before the next step, in the main flow and the cleanup.
#[test]
fn run03_criterion_5_sequence_puts_checks_and_values_before_the_next_step() {
    let (_, scenario) = lowered_scenario(CHECK_AFTER_THE_NEXT_STEP);
    assert_eq!(
        handles(&scenario.graph, &sequence(&scenario.graph)),
        ["begin", "cart", "created", "total", "paid"]
    );
    assert_eq!(
        handles(&scenario.cleanup, &sequence(&scenario.cleanup)),
        ["removed", "gone", "again"]
    );
}

/// Criterion 5: the static order agrees with the engine on that scenario and
/// on every reference scenario, as written and turned around.
#[tokio::test]
async fn run03_criterion_5_sequence_agrees_with_the_engine() {
    let mut cases = vec![(
        "checks after the next step".to_owned(),
        lowered_scenario(CHECK_AFTER_THE_NEXT_STEP),
    )];
    for name in [
        "fan-out",
        "far-value",
        "far-value-saved",
        "order-flow",
        "shop-30",
        "shop-50",
    ] {
        cases.push((name.to_owned(), fixture(name)));
    }
    for (name, (project, mut scenario)) in cases {
        for turn in ["as written", "reversed"] {
            let run = simulate(project.clone(), &scenario).await;
            assert_eq!(
                sequence(&scenario.graph),
                executed(&run, Phase::Main),
                "{name}, main, {turn}"
            );
            assert_eq!(
                sequence(&scenario.cleanup),
                executed(&run, Phase::Cleanup),
                "{name}, cleanup, {turn}"
            );
            reversed(&mut scenario.graph);
            reversed(&mut scenario.cleanup);
        }
    }
}

// RUN-04 (#418) ---------------------------------------------------------------

/// A value written before *Start* that reads no step, read by a later request.
const VALUE_BEFORE_START: &str = r#"scenario "A value before Start" {
  bad = "1" + 1
  begin = start
  first = GET "/a" {
    sample 200 {}
  }
  second = GET "/b" {
    query { limit: bad }
    sample 200 {}
  }
}
"#;

/// Criterion 6: the board's order is the run's; *Start* comes first, before
/// the value that reads no step, as written and turned around.
#[tokio::test]
async fn run04_criterion_6_sequence_puts_start_first_and_agrees_with_the_engine() {
    let (project, mut scenario) = lowered_scenario(VALUE_BEFORE_START);
    for turn in ["as written", "reversed"] {
        assert_eq!(
            handles(&scenario.graph, &sequence(&scenario.graph)),
            ["begin", "bad", "first", "second"],
            "{turn}"
        );
        let run = simulate(project.clone(), &scenario).await;
        assert_eq!(
            sequence(&scenario.graph),
            executed(&run, Phase::Main),
            "{turn}"
        );
        reversed(&mut scenario.graph);
    }
}

// LANG-10 (#354) --------------------------------------------------------------

/// The built-in `uuid`, by the name a document writes it with.
fn uuid_operator() -> Operator {
    serde_json::from_value(serde_json::json!("uuid")).expect("`uuid` is a built-in")
}

/// Units in `[0, 1)` from SplitMix64 with a fixed seed, as a seeded run draws them.
fn seeded(seed: u64) -> impl FnMut() -> f64 {
    let mut state = seed;
    move || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// `xxxxxxxx-xxxx-4xxx-[89ab]xxx-xxxxxxxxxxxx` in lowercase hexadecimal.
fn is_uuid_v4(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_digit() || (b'a'..=b'f').contains(byte),
        })
        && bytes[14] == b'4'
        && b"89ab".contains(&bytes[19])
}

/// Criterion 1: `uuid` is a text built-in of the Text group with no operand.
#[test]
fn lang10_criterion_1_uuid_is_a_text_built_in_without_operands() {
    let uuid = uuid_operator();
    assert!(Operator::ALL.contains(&uuid));
    assert_eq!(uuid.label(), "New UUID");
    let spec = uuid.spec();
    assert_eq!(spec.group, OperatorGroup::Text);
    assert_eq!(spec.output, ValueType::String);
    assert!(spec.operands.is_empty(), "{spec:?}");
}

/// Criterion 1: with a seeded source, `uuid()` is a valid version 4 text; the
/// same seed gives the same value, and every call a new one.
#[test]
fn lang10_criterion_1_a_seeded_uuid_is_version_4_reproducible_and_new_per_call() {
    let uuid = uuid_operator();
    let draw = |source: &mut dyn RandomSource| match uuid.evaluate(None, None, source) {
        Ok(Some(serde_json::Value::String(text))) => text,
        other => panic!("uuid() gave {other:?}"),
    };
    let mut source = seeded(7);
    let drawn: Vec<String> = (0..1000).map(|_| draw(&mut source)).collect();
    for text in &drawn {
        assert!(is_uuid_v4(text), "{text}");
    }
    assert_eq!(
        drawn.iter().collect::<BTreeSet<_>>().len(),
        drawn.len(),
        "two calls gave the same value"
    );
    assert_eq!(draw(&mut seeded(7)), drawn[0]);
    assert_ne!(draw(&mut seeded(8)), drawn[0]);
}

// #433 ------------------------------------------------------------------------

/// A check added from Run results or the Inspector on a step is written in
/// the braces that step stands in, right after the step's checks, and runs
/// on its branch only; on a top-level step it is written as before.
mod issue433 {
    use super::*;
    use apiforge_core::{
        edit::{add_check, field_assertion},
        run::NotRunCause,
    };
    use serde_json::json;

    /// The room-booking scenario of the LAY acceptance trial, without its
    /// sign-in component.
    const SOURCE: &str = r#"scenario "Double booking is refused" {
  begin = start
  first = POST "/bookings" {
    sample 201 { id: "bk-1" }
  }
  booked = if first.status == 201 {
    second = POST "/bookings" {
      sample 409 { error: { code: "room_unavailable" } }
    }
    refused = assert second.status == 409
    calendar = GET "/bookings" {
      sample 200 { items: [{ id: "bk-1" }] }
    }
    one_booking = assert calendar.body.items[0].id == first.body.id
  } else {
    not_booked = assert first.status == 201
  }
  cleanup {
    cancelled = DELETE "/bookings/{{first.body.id}}" {
      sample 204 {}
    }
    gone = assert cancelled.status == 204
  }
}
"#;

    /// `scenario` with a check of `path` in `subject`'s `body` added as Run
    /// results adds it, and the check's identifier.
    fn checked(scenario: &Scenario, subject: &str, path: &str, value: &str) -> (Scenario, String) {
        let source = node_by_handle(&scenario.graph, subject).clone();
        let check = field_assertion(&source, "body", path, &json!(value));
        let id = check.id.clone();
        let mut next = scenario.clone();
        next.graph = add_check(&scenario.graph, &source.id, check);
        (next, id)
    }

    /// `SOURCE` as a board edit writes `next` into it (LAY-34).
    fn written(project: &Project, scenario: &Scenario, next: &Scenario) -> String {
        apiforge_script::edit_folder_document(
            project,
            SOURCE,
            &Document::Scenario(scenario.clone()),
            &Document::Scenario(next.clone()),
        )
        .expect("the edit is written")
    }

    /// A check on `second`, which stands in `booked`'s `then`, is written in
    /// those braces right after `refused`, before `@after(refused) calendar`,
    /// and nothing else in the file changes.
    #[test]
    fn a_check_on_a_step_in_a_branch_is_written_in_its_braces() {
        let (project, scenario) = lowered_scenario(SOURCE);
        let (next, _) = checked(&scenario, "second", "error.code", "slot_conflict");
        assert_eq!(
            written(&project, &scenario, &next),
            SOURCE.replace(
                "    refused = assert second.status == 409\n",
                "    refused = assert second.status == 409\n    \
                 @label(\"Check error.code\")\n    \
                 check_error_code = assert second.body.error.code == \"slot_conflict\"\n",
            )
        );
    }

    /// The check follows `booked`'s `then` as `refused` does, so it runs right
    /// after `refused` on that branch, and reads *outside path* like its
    /// siblings when the other branch runs.
    #[tokio::test]
    async fn a_check_on_a_step_in_a_branch_runs_on_that_branch_only() {
        let (project, scenario) = lowered_scenario(SOURCE);
        let (next, check) = checked(&scenario, "second", "error.code", "slot_conflict");
        let run = simulate(project, &next).await;
        let order = executed(&run, Phase::Main);
        let at = |id: &str| {
            order
                .iter()
                .position(|step| *step == id)
                .unwrap_or_else(|| panic!("{} ran", handle_of(&next.graph, id)))
        };
        let refused = node_by_handle(&next.graph, "refused").id.clone();
        let calendar = node_by_handle(&next.graph, "calendar").id.clone();
        assert_eq!(at(&check), at(&refused) + 1, "right after refused");
        assert!(at(&check) < at(&calendar), "before calendar");

        let other = SOURCE.replace("sample 201 { id: \"bk-1\" }", "sample 500 {}");
        let (project, scenario) = lowered_scenario(&other);
        let (next, check) = checked(&scenario, "second", "error.code", "slot_conflict");
        let run = simulate(project, &next).await;
        let cause = |id: &str| {
            run.steps
                .iter()
                .find(|step| step.node_id == id)
                .unwrap_or_else(|| panic!("{} is reported", handle_of(&next.graph, id)))
                .cause
                .clone()
        };
        let refused = node_by_handle(&next.graph, "refused").id.clone();
        assert_eq!(cause(&refused), Some(NotRunCause::OutsidePath));
        assert_eq!(cause(&check), Some(NotRunCause::OutsidePath));
    }

    /// A check on `first`, a top-level step, is added as before: last on the
    /// board without an ordering edge, written after the whole `if`.
    #[test]
    fn a_check_on_a_top_level_step_is_added_as_before() {
        let (project, scenario) = lowered_scenario(SOURCE);
        let first = node_by_handle(&scenario.graph, "first").clone();
        let check = field_assertion(&first, "body", "id", &json!("bk-1"));
        let mut before = scenario.graph.clone();
        before.nodes.push(check.clone());
        assert_eq!(add_check(&scenario.graph, &first.id, check), before);

        let (next, _) = checked(&scenario, "first", "id", "bk-1");
        assert_eq!(
            written(&project, &scenario, &next),
            SOURCE.replace(
                "    not_booked = assert first.status == 201\n  }\n",
                "    not_booked = assert first.status == 201\n  }\n  \
                 @label(\"Check id\")\n  \
                 check_id = assert first.body.id == \"bk-1\"\n",
            )
        );
    }
}

// DIAG-08 (#521) --------------------------------------------------------------

/// The ✓ on a field of a step's data offers *equals ‹value›*, *is a ‹type›*,
/// *exists* and *is not empty*; each is written as an ordinary check under
/// the step, in the language's words.
mod diag08 {
    use super::*;
    use apiforge_core::{
        edit::{FieldCheck, add_check, field_assertion, field_check},
        model::{NodeKind, Predicate},
    };
    use serde_json::{Value, json};

    const SOURCE: &str = r#"scenario "A cart" {
  begin = start
  cart = POST "/carts" {
    sample 201 { id: "c-1", total: 172.5, paid: false, owner: { name: "Ann" }, items: ["coffee"], coupon: null }
  }
}
"#;

    /// `SOURCE` with `checks` of `cart`'s body added as the window adds
    /// them, written as a board edit writes them (LAY-34). The text passes
    /// `check`: it lowers without an error.
    fn written(checks: &[(&str, FieldCheck)]) -> String {
        let (project, scenario) = lowered_scenario(SOURCE);
        let cart = node_by_handle(&scenario.graph, "cart").clone();
        let mut next = scenario.clone();
        for (path, check) in checks {
            let check = field_check(&cart, "body", path, check);
            next.graph = add_check(&next.graph, &cart.id, check);
        }
        let text = apiforge_script::edit_folder_document(
            &project,
            SOURCE,
            &Document::Scenario(scenario),
            &Document::Scenario(next),
        )
        .expect("the edit is written");
        lowered_scenario(&text);
        text
    }

    /// `SOURCE` with `lines` written after `cart`, at the end of the flow.
    fn with(lines: &str) -> String {
        SOURCE.replace("  }\n}\n", &format!("  }}\n{lines}}}\n"))
    }

    /// Criterion 1: *is a number* from `total` is one check, written
    /// `… = assert cart.body.total is "number"` under the step.
    #[test]
    fn diag08_criterion_1_type_check_from_a_field() {
        let (_, scenario) = lowered_scenario(SOURCE);
        let cart = node_by_handle(&scenario.graph, "cart").clone();
        let check = field_check(&cart, "body", "total", &FieldCheck::Type("number".into()));
        assert_eq!(check.kind, NodeKind::Assert);
        assert_eq!(check.config.predicate, Some(Predicate::Type));
        assert_eq!(
            written(&[("total", FieldCheck::Type("number".into()))]),
            with(
                "  @label(\"Check total\")\n  \
                 check_total = assert cart.body.total is \"number\"\n"
            )
        );
    }

    /// Criterion 2: the type offered is the type of the value shown, in the
    /// language's six words, and the check it writes passes on that value.
    #[test]
    fn diag08_criterion_2_the_type_is_the_values() {
        for (value, word) in [
            (json!("c-1"), "string"),
            (json!(172.5), "number"),
            (json!(false), "boolean"),
            (json!({ "name": "Ann" }), "object"),
            (json!(["coffee"]), "array"),
            (Value::Null, "null"),
        ] {
            assert_eq!(
                FieldCheck::offered(&value),
                [
                    FieldCheck::Equals(value.clone()),
                    FieldCheck::Type(word.into()),
                    FieldCheck::Exists,
                    FieldCheck::NotEmpty,
                ],
                "the checks offered for {value}"
            );
            assert_eq!(
                Predicate::Type.check(Some(&value), Some(&json!(word)), 0.0),
                Ok(true),
                "{value} is a {word}"
            );
        }
    }

    /// Criterion 3: *exists* and *is not empty* write `assert exists …` and
    /// `assert nonempty …`; *equals* writes what *Add comparison* wrote.
    #[test]
    fn diag08_criterion_3_the_other_checks() {
        assert_eq!(
            written(&[
                ("total", FieldCheck::Equals(json!(172.5))),
                ("id", FieldCheck::Exists),
                ("items", FieldCheck::NotEmpty),
            ]),
            with(
                "  @label(\"Check total\")\n  \
                 check_total = assert cart.body.total == 172.5\n  \
                 @label(\"Check id\")\n  \
                 check_id = assert exists cart.body.id\n  \
                 @label(\"Check items\")\n  \
                 check_items = assert nonempty cart.body.items\n"
            )
        );
        let (_, scenario) = lowered_scenario(SOURCE);
        let cart = node_by_handle(&scenario.graph, "cart").clone();
        let mut equals = field_check(&cart, "body", "total", &FieldCheck::Equals(json!(172.5)));
        let mut before = field_assertion(&cart, "body", "total", &json!(172.5));
        equals.id.clear();
        before.id.clear();
        assert_eq!(equals, before, "equals is the comparison of today");
        for check in [FieldCheck::Exists, FieldCheck::NotEmpty] {
            let node = field_check(&cart, "body", "id", &check);
            assert!(
                !node.inputs.contains_key("expected"),
                "{check:?} expects nothing"
            );
        }
    }
}
