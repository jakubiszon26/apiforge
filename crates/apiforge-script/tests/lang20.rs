//! LANG-20 (#481), criterion 3: a source of a language-1 project is judged by
//! its 0.1.0 reading whether or not it holds an `@after`, in its main flow,
//! its cleanup and a component's body. `fix_source` (LANG-13) and the
//! editor's behaviour do not change: it still returns such files unchanged.
use apiforge_core::{
    format::{Graph, Project},
    graph::sequence,
};
use apiforge_script::{
    Blocked, Document, Judged, LowerContext, fix_source, judge_language_1, lower, parse,
};

fn project() -> Project {
    apiforge_core::demos::shop_project()
}

fn judge(text: &str) -> Judged {
    let project = project();
    judge_language_1(text, &LowerContext::folder(&project))
}

fn blockers(text: &str) -> Vec<(String, Blocked, String)> {
    match judge(text) {
        Judged::Blocked(blockers) => blockers
            .into_iter()
            .map(|blocker| {
                let statement = text[blocker.range.clone()].to_owned();
                (blocker.handle, blocker.reason, statement)
            })
            .collect(),
        other => panic!("blocked expected:\n{text}\n{other:?}"),
    }
}

fn converted(text: &str) -> String {
    match judge(text) {
        Judged::Convertible { text, .. } => text,
        other => panic!("convertible expected:\n{text}\n{other:?}"),
    }
}

/// `fix_source` leaves a file without `@after` as it is, refusing nothing.
fn fix_source_leaves_it(text: &str) {
    let project = project();
    let fixed = fix_source(text, &LowerContext::folder(&project));
    assert_eq!(fixed.text, text);
    assert!(!fixed.changed(text));
}

/// The steps of `graph` in the order a run reaches them, by handle.
fn steps(graph: &Graph) -> Vec<String> {
    sequence(graph)
        .into_iter()
        .filter_map(|id| graph.nodes.iter().find(|node| node.id == id))
        .filter(|node| !node.kind.is_data())
        .map(|node| node.handle.clone())
        .collect()
}

fn written_steps(text: &str) -> (Vec<String>, Vec<String>) {
    let project = project();
    let lowered = lower(&parse(text), &LowerContext::folder(&project));
    assert!(!lowered.has_errors(), "{text}\n{:#?}", lowered.diagnostics);
    match lowered.document.unwrap() {
        Document::Scenario(scenario) => (steps(&scenario.graph), steps(&scenario.cleanup)),
        Document::Component(component) => (steps(&component.graph), vec![]),
    }
}

const LOOSE: &str =
    "scenario \"Loose\" {\n  begin = start\n  ping = GET \"/ping\" {\n    sample 200 {}\n  }\n}\n";

/// A scenario with a `start` and a request nothing orders: blocked, the
/// request named; with `@after(<the step above>)` on it the project converts
/// and the request runs there.
#[test]
fn lang20_criterion_3_a_request_nothing_orders_blocks_and_converts_with_after() {
    let found = blockers(LOOSE);
    assert_eq!(found.len(), 1, "{found:?}");
    let (handle, reason, statement) = &found[0];
    assert_eq!(handle, "ping");
    assert_eq!(
        *reason,
        Blocked::RunsNowhere {
            above: Some("begin".into())
        }
    );
    assert!(statement.starts_with("ping = GET"), "{statement}");
    fix_source_leaves_it(LOOSE);

    let ordered = LOOSE.replace("  ping = GET", "  @after(begin)\n  ping = GET");
    let text = converted(&ordered);
    assert!(!text.contains("@after"), "{text}");
    assert_eq!(text, LOOSE, "only the `@after` line goes");
    let (main, _) = written_steps(&text);
    assert_eq!(main, ["begin", "ping"]);
}

/// A working component whose first request reads a request written below
/// it: blocked, and both are named.
#[test]
fn lang20_criterion_3_a_component_reading_below_is_blocked_and_names_both() {
    let text = "component lookup \"Lookup\" {\n  first = GET \"/first/{{second.body.id}}\" {\n    sample 200 {}\n  }\n  second = GET \"/second\" {\n    sample 200 { id: \"x\" }\n  }\n}\n";
    let found = blockers(text);
    assert_eq!(found.len(), 1, "{found:?}");
    let (handle, reason, _) = &found[0];
    assert_eq!(handle, "first");
    assert_eq!(
        *reason,
        Blocked::NotInPlace {
            after: vec!["second".into()]
        }
    );
    fix_source_leaves_it(text);
}

/// A cleanup whose first step reads a step written below it is blocked.
#[test]
fn lang20_criterion_3_a_cleanup_reading_below_is_blocked() {
    let text = "scenario \"Tidy\" {\n  begin = start\n  @after(begin)\n  made = POST \"/things\" {\n    sample 201 { id: \"t\" }\n  }\n  cleanup {\n    gone = DELETE \"/things/{{listed.body.id}}\" {\n      sample 204 {}\n    }\n    listed = GET \"/things/{{made.body.id}}\" {\n      sample 200 { id: \"t\" }\n    }\n  }\n}\n";
    let found = blockers(text);
    assert_eq!(found.len(), 1, "{found:?}");
    let (handle, reason, _) = &found[0];
    assert_eq!(handle, "gone");
    assert_eq!(
        *reason,
        Blocked::NotInPlace {
            after: vec!["listed".into()]
        }
    );
}

/// A working component written in the order it runs converts, unchanged.
#[test]
fn lang20_criterion_3_a_component_in_its_order_converts() {
    let text = "component lookup \"Lookup\" {\n  second = GET \"/second\" {\n    sample 200 { id: \"x\" }\n  }\n  first = GET \"/first/{{second.body.id}}\" {\n    sample 200 {}\n  }\n}\n";
    match judge(text) {
        Judged::Convertible {
            text: converted,
            edits,
        } => {
            assert_eq!(converted, text);
            assert!(edits.is_empty(), "{edits:?}");
        }
        other => panic!("{other:?}"),
    }
    fix_source_leaves_it(text);
    let (main, _) = written_steps(text);
    assert_eq!(main, ["second", "first"]);
}

/// Every statement in the way is named, not only the first: one written
/// above the step it follows and one request nothing orders.
#[test]
fn lang20_criterion_3_every_statement_in_the_way_is_named() {
    let text = "scenario \"Two\" {\n  begin = start\n  @after(begin)\n  first = GET \"/first\" {\n    sample 200 {}\n  }\n  @after(second)\n  third = GET \"/third\" {\n    sample 200 {}\n  }\n  @after(first)\n  second = GET \"/second\" {\n    sample 200 {}\n  }\n  loose = GET \"/loose\" {\n    sample 200 {}\n  }\n}\n";
    let found = blockers(text);
    let handles: Vec<&str> = found.iter().map(|(handle, ..)| handle.as_str()).collect();
    assert_eq!(handles, ["third", "loose"], "{found:?}");
    assert_eq!(
        found[0].1,
        Blocked::NotInPlace {
            after: vec!["second".into()]
        }
    );
    assert_eq!(
        found[1].1,
        Blocked::RunsNowhere {
            above: Some("second".into())
        }
    );
}

/// A file written in the order it runs, with `@after` lines and without
/// `start`: converted as `fix_source` converts it, the edits named.
#[test]
fn lang20_criterion_1_a_convertible_file_is_fixed_and_its_edits_named() {
    let text = "scenario \"Old\" {\n  first = GET \"/first\" {\n    sample 200 {}\n  }\n  @after(first)\n  second = GET \"/second\" {\n    sample 200 {}\n  }\n}\n";
    let project = project();
    let fixed = fix_source(text, &LowerContext::folder(&project));
    assert!(fixed.refused.is_none(), "{fixed:?}");
    match judge(text) {
        Judged::Convertible {
            text: converted,
            edits,
        } => {
            assert_eq!(converted, fixed.text);
            let removed: Vec<&str> = edits
                .iter()
                .map(|edit| text[edit.range.clone()].trim())
                .filter(|removed| !removed.is_empty())
                .collect();
            assert_eq!(removed, ["@after(first)"]);
            let written: Vec<&str> = edits
                .iter()
                .map(|edit| edit.replacement.trim())
                .filter(|written| !written.is_empty())
                .collect();
            assert_eq!(written, ["begin = start"]);
        }
        other => panic!("{other:?}"),
    }
}

/// A text that does not parse is unreadable, with its first syntax error.
#[test]
fn lang20_criterion_4_a_syntax_error_is_unparsable() {
    assert!(matches!(
        judge("scenario \"Broken\" {\n  first = GET\n"),
        Judged::Unparsable(_)
    ));
}
