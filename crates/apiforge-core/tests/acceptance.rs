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
    use apiforge_script::print_scenario;

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
        next.graph = added.graph;
        (next, added.node_id)
    }

    fn printed(project: &Project, scenario: &Scenario) -> String {
        print_scenario(project, scenario).expect("the scenario prints")
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

    /// Criterion 2: the source shows `@after(cart)` on the new request,
    /// written after `cart` and the checks and values that follow it and
    /// before the next step; nothing else moves, and text → window → text is
    /// unchanged.
    #[test]
    fn criterion_2_the_step_is_written_after_its_source_and_its_checks() {
        let (project, scenario) = fixture("shop-30");
        let before = printed(&project, &scenario);
        let (next, added) = added_from(&scenario, "cart", ORDER_PORT);
        let handle = handle_of(&next.graph, &added).to_owned();
        let text = printed(&project, &next);
        let at = line_of(&text, &format!("{handle} = "));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[at - 1].trim(), "@after(cart)", "{text}");
        assert!(line_of(&text, "cart_total = ") < at - 1, "{text}");
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
  @after(begin)
  cart = POST "/carts" {
    sample 201 { id: "c-1", total: 0 }
  }
  @after(cart)
  paid = POST "/payments" {
    sample 201 { id: "p-1" }
  }
  created = assert cart.status == 201
  total = cart.body.total
  cleanup {
    removed = DELETE "/carts/{{cart.body.id}}" {
      sample 204 {}
    }
    @after(removed)
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
