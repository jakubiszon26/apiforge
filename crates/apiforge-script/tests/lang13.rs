//! LANG-13 (#386): the steps of a block run in the order written; `@after`
//! leaves the language.
//!
//! A folder source is read in the written language: the steps of a scenario,
//! a component, a branch and the cleanup run in the order they are written,
//! checks and values stand anywhere, and `@after` is an error with a fix that
//! deletes its line when the file is written in the order it runs today. A
//! published revision keeps the stored spelling.
use apiforge_core::{
    format::{Component, Graph, Project, Scenario},
    graph::sequence,
    model::{Branch, NodeKind},
};
use apiforge_script::{
    AFTER_REMOVED, Diagnostic, Document, LowerContext, NOT_IN_PLACE, RUNS_NOWHERE, START_MISSING,
    START_NOT_FIRST, Severity,
    complete::{Mode, Request, Scope, complete},
    edit_folder_document, fix_source, lower, parse, print_folder_document, print_revision,
    semantic,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

const SIGN_IN_REVISION: &str =
    include_str!("../../../fixtures/scenarios/lay/rooms/.apiforge/revisions/sign-in/1.afs");

/// The demo shop with the room booking's published `sign_in`, read in its
/// stored spelling: every component a fixture calls.
fn project() -> Project {
    let mut project = apiforge_core::demos::shop_project();
    let lowered = lower(
        &parse(SIGN_IN_REVISION),
        &LowerContext::folder(&project).stored(),
    );
    assert!(!lowered.has_errors(), "{:#?}", lowered.diagnostics);
    let Some(Document::Component(sign_in)) = lowered.document else {
        panic!("a component");
    };
    project.components.push(sign_in);
    project
}

fn lowered_with(project: &Project, text: &str, context: LowerContext<'_>) -> Document {
    let lowered = lower(&parse(text), &context);
    let errors: Vec<_> = errors(&lowered.diagnostics);
    assert!(errors.is_empty(), "{text}\n{errors:#?}");
    let _ = project;
    lowered.document.expect("a document")
}

/// `text` lowered as a folder source, without an error.
fn written(project: &Project, text: &str) -> Document {
    lowered_with(project, text, LowerContext::folder(project))
}

/// `text` lowered in its stored spelling, as 0.1.0 reads it.
fn stored(project: &Project, text: &str) -> Document {
    lowered_with(project, text, LowerContext::folder(project).stored())
}

fn errors(diagnostics: &[Diagnostic]) -> Vec<&Diagnostic> {
    diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect()
}

fn main_graph(document: &Document) -> &Graph {
    match document {
        Document::Scenario(scenario) => &scenario.graph,
        Document::Component(component) => &component.graph,
    }
}

fn cleanup_graph(document: &Document) -> &Graph {
    match document {
        Document::Scenario(scenario) => &scenario.cleanup,
        Document::Component(_) => panic!("a component has no cleanup"),
    }
}

fn is_step(kind: NodeKind) -> bool {
    !kind.is_data()
}

/// The handles of the steps in the order the engine reaches them.
fn steps(graph: &Graph) -> Vec<String> {
    sequence(graph)
        .into_iter()
        .map(|id| graph.nodes.iter().find(|node| node.id == id).unwrap())
        .filter(|node| is_step(node.kind))
        .map(|node| node.handle.clone())
        .collect()
}

/// What a statement follows, as `@after` would spell each source.
fn follows(graph: &Graph, handle: &str) -> BTreeSet<String> {
    let handle_of = |id: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.id == id)
            .map(|node| node.handle.clone())
            .unwrap_or_else(|| panic!("no node {id}"))
    };
    let target = graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("no statement `{handle}`"));
    graph
        .edges
        .iter()
        .filter(|edge| edge.target == target.id)
        .map(|edge| {
            let source = handle_of(&edge.source);
            match edge.branch {
                None => source,
                Some(Branch::True) => format!("{source}.then"),
                Some(Branch::False) => format!("{source}.else"),
            }
        })
        .collect()
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

fn request(handle: &str, indent: &str) -> String {
    format!(
        "{indent}{handle} = GET \"/{handle}\" {{\n{indent}  sample 200 {{ id: 1 }}\n{indent}}}\n"
    )
}

// Criterion 1 ----------------------------------------------------------------

/// Three requests none of which reads another, with a check of the first
/// written after the second: a chain in the order written.
const SCENARIO_BODY: &str = r#"scenario "Order written" {
  begin = start
  first = GET "/first" {
    sample 200 { id: 1 }
  }
  second = GET "/second" {
    sample 200 { id: 2 }
  }
  first_ok = assert first.status == 200
  third = GET "/third" {
    sample 200 { id: 3 }
  }
}
"#;

#[test]
fn lang13_criterion_1_a_scenario_body_runs_in_the_order_written() {
    let project = project();
    let document = written(&project, SCENARIO_BODY);
    let graph = main_graph(&document);
    assert_eq!(steps(graph), ["begin", "first", "second", "third"]);
    assert_eq!(follows(graph, "first"), set(&["begin"]));
    assert_eq!(follows(graph, "second"), set(&["first"]));
    assert_eq!(follows(graph, "third"), set(&["second"]));
    // A check is ordered by what it reads only.
    assert!(follows(graph, "first_ok").is_empty());
}

#[test]
fn lang13_criterion_1_a_component_body_runs_in_the_order_written() {
    let project = project();
    // A component needs no `start`: its body runs from its first step.
    let text = "component pair \"Pair\" {\n  b = GET \"/b\" {\n    sample 200 { id: 1 }\n  }\n  a = GET \"/a\" {\n    sample 200 { id: 2 }\n  }\n  export id: number = a.body.id\n}\n";
    let document = written(&project, text);
    let graph = main_graph(&document);
    assert_eq!(steps(graph), ["b", "a"]);
    assert!(follows(graph, "b").is_empty());
    assert_eq!(follows(graph, "a"), set(&["b"]));
}

#[test]
fn lang13_criterion_1_a_branch_runs_in_the_order_written() {
    let project = project();
    let text = format!(
        "scenario \"Branch\" {{\n  begin = start\n{}  x = if probe.status == 200 {{\n{}{}    seen = assert one.status == 200\n{}  }}\n}}\n",
        request("probe", "  "),
        request("one", "    "),
        request("two", "    "),
        request("three", "    "),
    );
    let document = written(&project, &text);
    let graph = main_graph(&document);
    assert_eq!(steps(graph), ["begin", "probe", "x", "one", "two", "three"]);
    // Every statement in braces keeps the edge of its braces.
    assert_eq!(follows(graph, "one"), set(&["x.then"]));
    assert_eq!(follows(graph, "two"), set(&["x.then", "one"]));
    assert_eq!(follows(graph, "three"), set(&["x.then", "two"]));
    assert_eq!(follows(graph, "seen"), set(&["x.then"]));
}

#[test]
fn lang13_criterion_1_the_cleanup_runs_in_the_order_written() {
    let project = project();
    let text = format!(
        "scenario \"Cleanup\" {{\n  begin = start\n{}  cleanup {{\n{}{}{}  }}\n}}\n",
        request("made", "  "),
        request("drop_c", "    "),
        request("drop_a", "    "),
        request("drop_b", "    "),
    );
    let document = written(&project, &text);
    let graph = cleanup_graph(&document);
    assert_eq!(steps(graph), ["drop_c", "drop_a", "drop_b"]);
    assert!(follows(graph, "drop_c").is_empty());
    assert_eq!(follows(graph, "drop_a"), set(&["drop_c"]));
    assert_eq!(follows(graph, "drop_b"), set(&["drop_a"]));
}

// Criterion 2 ----------------------------------------------------------------

#[test]
fn lang13_criterion_2_the_step_after_an_if_runs_after_the_whole_if() {
    let project = project();
    // `then` and `else` both end in a step.
    let text = format!(
        "scenario \"After\" {{\n  begin = start\n{}  x = if probe.status == 200 {{\n{}{}  }} else {{\n{}  }}\n{}}}\n",
        request("probe", "  "),
        request("a", "    "),
        request("b", "    "),
        request("c", "    "),
        request("after", "  "),
    );
    let graph = main_graph(&written(&project, &text)).clone();
    assert_eq!(follows(&graph, "after"), set(&["b", "c"]));

    // A branch without a step: the step follows the branch itself.
    let text = format!(
        "scenario \"After\" {{\n  begin = start\n{}  x = if probe.status == 200 {{\n{}  }} else {{\n    nothing = assert probe.status != 200\n  }}\n{}}}\n",
        request("probe", "  "),
        request("a", "    "),
        request("after", "  "),
    );
    let graph = main_graph(&written(&project, &text)).clone();
    assert_eq!(follows(&graph, "after"), set(&["a", "x.else"]));

    // An `if` inside a branch: the step follows where the inner `if` ends.
    let text = format!(
        "scenario \"After\" {{\n  begin = start\n{}  x = if probe.status == 200 {{\n{}    y = if a.status == 200 {{\n{}    }}\n  }} else {{\n{}  }}\n{}}}\n",
        request("probe", "  "),
        request("a", "    "),
        request("inner", "      "),
        request("c", "    "),
        request("after", "  "),
    );
    let graph = main_graph(&written(&project, &text)).clone();
    assert_eq!(follows(&graph, "after"), set(&["inner", "y.else", "c"]));
    assert_eq!(
        steps(&graph),
        ["begin", "probe", "x", "a", "y", "inner", "c", "after"]
    );
}

// Criterion 3 ----------------------------------------------------------------

/// A file written in the order it runs today: every `@after` names the
/// statement above it.
const IN_ORDER: &str = r#"scenario "In order" {
  begin = start
  // The first request.
  @after(begin)
  first = GET "/first" {
    sample 200 { id: 1 }
  }
  ok = assert first.status == 200
  @after(ok)
  @label("Second")
  second = GET "/second" {
    sample 200 { id: 2 }
  }
}
"#;

#[test]
fn lang13_criterion_3_an_after_is_an_error_whose_fix_deletes_its_line() {
    let project = project();
    let lowered = lower(&parse(IN_ORDER), &LowerContext::folder(&project));
    let found = errors(&lowered.diagnostics);
    assert_eq!(found.len(), 2, "{found:#?}");
    for (error, written) in found.iter().zip(["@after(begin)", "@after(ok)"]) {
        assert_eq!(error.message, AFTER_REMOVED);
        assert_eq!(&IN_ORDER[error.range.clone()], written);
        let fix = error.fix.as_ref().expect("a fix");
        assert_eq!(fix.replacement, "");
        assert_eq!(&IN_ORDER[fix.range.clone()], format!("  {written}\n"));
    }
}

/// The fix of one `@after` applied to `text`.
fn fixed_once(text: &str) -> String {
    let project = project();
    let lowered = lower(&parse(text), &LowerContext::folder(&project));
    let after = lowered
        .diagnostics
        .iter()
        .find(|d| d.message == AFTER_REMOVED)
        .unwrap_or_else(|| panic!("an @after error: {:#?}", lowered.diagnostics));
    let fix = after.fix.as_ref().expect("a fix");
    let mut fixed = text.to_owned();
    fixed.replace_range(fix.range.clone(), &fix.replacement);
    fixed
}

#[test]
fn lang13_criterion_3_the_fix_removes_the_attribute_not_its_neighbours() {
    // On one line with its statement: only the attribute goes.
    let text = "scenario \"One line\" {\n  begin = start\n  @after(begin) b = GET \"/b\" {\n    sample 200 {}\n  }\n}\n";
    assert_eq!(
        fixed_once(text),
        "scenario \"One line\" {\n  begin = start\n  b = GET \"/b\" {\n    sample 200 {}\n  }\n}\n"
    );
    // With a comment beside it: the comment stays.
    let text = "scenario \"Comment\" {\n  begin = start\n  @after(begin) // the first request\n  b = GET \"/b\" {\n    sample 200 {}\n  }\n}\n";
    assert_eq!(
        fixed_once(text),
        "scenario \"Comment\" {\n  begin = start\n  // the first request\n  b = GET \"/b\" {\n    sample 200 {}\n  }\n}\n"
    );
    // Broken over two lines: both lines go, nothing else.
    let text = "scenario \"Two lines\" {\n  begin = start\n  @after(\n    begin)\n  b = GET \"/b\" {\n    sample 200 {}\n  }\n}\n";
    assert_eq!(
        fixed_once(text),
        "scenario \"Two lines\" {\n  begin = start\n  b = GET \"/b\" {\n    sample 200 {}\n  }\n}\n"
    );
}

#[test]
fn lang13_criterion_3_out_of_order_no_after_has_a_fix_and_the_statement_is_named() {
    let project = project();
    // `second` is written above `first`, which it runs after.
    let text = "scenario \"Out of order\" {\n  begin = start\n  @after(first)\n  second = GET \"/second\" {\n    sample 200 { id: 2 }\n  }\n  @after(begin)\n  first = GET \"/first\" {\n    sample 200 { id: 1 }\n  }\n}\n";
    let lowered = lower(&parse(text), &LowerContext::folder(&project));
    let found = errors(&lowered.diagnostics);
    let afters: Vec<_> = found
        .iter()
        .filter(|d| d.message == AFTER_REMOVED)
        .collect();
    assert_eq!(afters.len(), 2, "{found:#?}");
    assert!(afters.iter().all(|d| d.fix.is_none()), "{afters:#?}");
    let named: Vec<_> = found.iter().filter(|d| d.message == NOT_IN_PLACE).collect();
    assert_eq!(named.len(), 1, "{found:#?}");
    assert!(text[named[0].range.clone()].contains("second = GET"));
}

// Criterion 4 ----------------------------------------------------------------

#[test]
fn lang13_criterion_4_a_scenario_with_a_step_needs_a_start() {
    let project = project();
    let text = "scenario \"No start\" {\n  value = 1\n  // The request.\n  ping = GET \"/ping\" {\n    sample 200 {}\n  }\n}\n";
    let lowered = lower(&parse(text), &LowerContext::folder(&project));
    let found = errors(&lowered.diagnostics);
    assert_eq!(found.len(), 1, "{text}\n{found:#?}");
    assert_eq!(found[0].message, START_MISSING);
    assert!(text[found[0].range.clone()].starts_with("ping = GET"));
    let fix = found[0].fix.as_ref().expect("a fix");
    let mut fixed = text.to_owned();
    fixed.replace_range(fix.range.clone(), &fix.replacement);
    let document = written(&project, &fixed);
    assert_eq!(steps(main_graph(&document)), ["begin", "ping"], "{fixed}");
    assert!(fixed.contains("  begin = start\n"), "{fixed}");
    // The fix writes the start as the first statement, above its comment.
    let text = "scenario \"No start\" {\n  // The request.\n  ping = GET \"/ping\" {\n    sample 200 {}\n  }\n}\n";
    assert_eq!(
        fix_source(text, &LowerContext::folder(&project)).text,
        "scenario \"No start\" {\n  begin = start\n  // The request.\n  ping = GET \"/ping\" {\n    sample 200 {}\n  }\n}\n"
    );
}

#[test]
fn lang13_criterion_4_the_start_takes_the_handle_the_board_would_give_it() {
    let project = project();
    let text =
        "scenario \"Taken\" {\n  begin = 1\n  ping = GET \"/ping\" {\n    sample 200 {}\n  }\n}\n";
    let fixed = fix_source(text, &LowerContext::folder(&project));
    assert_eq!(fixed.refused, None);
    assert_eq!(
        fixed.text,
        "scenario \"Taken\" {\n  begin_2 = start\n  begin = 1\n  ping = GET \"/ping\" {\n    sample 200 {}\n  }\n}\n"
    );
    let document = written(&project, &fixed.text);
    assert_eq!(steps(main_graph(&document)), ["begin_2", "ping"]);
}

#[test]
fn lang13_criterion_4_a_component_needs_no_start() {
    let project = project();
    let text = "component ping \"Ping\" {\n  ping = GET \"/ping\" {\n    sample 200 {}\n  }\n  export status: number = ping.status\n}\n";
    let document = written(&project, text);
    assert_eq!(steps(main_graph(&document)), ["ping"]);
    assert_eq!(fix_source(text, &LowerContext::folder(&project)).text, text);
    // A `start` a component has stays valid.
    let text = "component ping \"Ping\" {\n  begin = start\n  ping = GET \"/ping\" {\n    sample 200 {}\n  }\n  export status: number = ping.status\n}\n";
    let document = written(&project, text);
    assert_eq!(steps(main_graph(&document)), ["begin", "ping"]);
}

#[test]
fn lang13_criterion_4_a_step_above_start_is_an_error() {
    let project = project();
    let text = format!(
        "scenario \"Late start\" {{\n{}  begin = start\n{}}}\n",
        request("early", "  "),
        request("later", "  ")
    );
    let lowered = lower(&parse(&text), &LowerContext::folder(&project));
    let found = errors(&lowered.diagnostics);
    assert!(
        found
            .iter()
            .any(|d| d.message == START_NOT_FIRST && &text[d.range.clone()] == "begin = start"),
        "{found:#?}"
    );
}

#[test]
fn lang13_criterion_4_a_cleanup_and_a_branch_need_no_start() {
    let project = project();
    let text = format!(
        "scenario \"Braces\" {{\n  begin = start\n{}  x = if probe.status == 200 {{\n{}  }}\n  cleanup {{\n{}  }}\n}}\n",
        request("probe", "  "),
        request("inside", "    "),
        request("tidy", "    "),
    );
    written(&project, &text);
}

// Criterion 5 ----------------------------------------------------------------

fn lang1(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/lang1")
        .join(format!("{name}.afs"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// `text` without the lines `fixed` deleted and with the lines it added:
/// the fix's own lines.
fn line_changes(text: &str, fixed: &str) -> (Vec<String>, Vec<String>) {
    let before: Vec<&str> = text.split_inclusive('\n').collect();
    let after: Vec<&str> = fixed.split_inclusive('\n').collect();
    let (mut deleted, mut added) = (vec![], vec![]);
    let (mut i, mut j) = (0, 0);
    while i < before.len() || j < after.len() {
        if i < before.len() && j < after.len() && before[i] == after[j] {
            i += 1;
            j += 1;
        } else if i < before.len() && before[i].trim_start().starts_with("@after(") {
            deleted.push(before[i].to_owned());
            i += 1;
        } else if j < after.len() {
            added.push(after[j].to_owned());
            j += 1;
        } else {
            deleted.push(before[i].to_owned());
            i += 1;
        }
    }
    (deleted, added)
}

#[test]
fn lang13_criterion_5_the_fix_deletes_the_after_lines_of_a_file_in_run_order() {
    let project = project();
    let context = LowerContext::folder(&project);
    for (name, start) in [
        ("checkout", false),
        ("lookup", false),
        ("booking-day", false),
        ("labels", false),
        ("sign-in", false),
    ] {
        let text = lang1(name);
        let fixed = fix_source(&text, &context);
        assert_eq!(fixed.refused, None, "{name}");
        // Only `@after` lines go; a missing `begin = start` is the one line added.
        let (deleted, added) = line_changes(&text, &fixed.text);
        assert_eq!(
            deleted.len(),
            text.matches("@after(").count(),
            "{name}: {deleted:#?}"
        );
        assert!(
            deleted
                .iter()
                .all(|line| line.trim_start().starts_with("@after(")),
            "{name}: {deleted:#?}"
        );
        if start {
            assert_eq!(added, ["  begin = start\n"], "{name}");
        } else {
            assert!(added.is_empty(), "{name}: {added:#?}");
        }
        // Every comment stays.
        for line in text
            .lines()
            .filter(|line| line.trim_start().starts_with("//"))
        {
            assert!(fixed.text.contains(line), "{name}: {line}");
        }
        // It lowers, in the order of steps the 0.1.0 reading gave.
        let before = stored(&project, &text);
        let after = written(&project, &fixed.text);
        let doing = |graph: &Graph| {
            steps(graph)
                .into_iter()
                .filter(|handle| handle != "begin")
                .collect::<Vec<_>>()
        };
        assert_eq!(
            doing(main_graph(&after)),
            doing(main_graph(&before)),
            "{name}"
        );
        if let (Document::Scenario(before), Document::Scenario(after)) = (&before, &after) {
            assert_eq!(steps(&after.cleanup), steps(&before.cleanup), "{name}");
        }
        // Applied again it changes nothing.
        let again = fix_source(&fixed.text, &context);
        assert_eq!(again.text, fixed.text, "{name}");
        assert_eq!(again.refused, None, "{name}");
    }
}

// Criterion 6 ----------------------------------------------------------------

#[test]
fn lang13_criterion_6_a_step_that_runs_nowhere_is_not_fixed() {
    let project = project();
    let text = format!(
        "scenario \"Nowhere\" {{\n  begin = start\n  @after(begin)\n{}{}}}\n",
        request("ordered", "  "),
        request("loose", "  ")
    );
    let fixed = fix_source(&text, &LowerContext::folder(&project));
    assert_eq!(fixed.text, text);
    let refused = fixed.refused.expect("refused");
    assert_eq!(refused.message, RUNS_NOWHERE);
    assert!(text[refused.range].starts_with("loose = GET"));
}

#[test]
fn lang13_criterion_6_steps_not_written_in_run_order_are_not_fixed() {
    let project = project();
    let text = "scenario \"Out of order\" {\n  begin = start\n  @after(first)\n  second = GET \"/second\" {\n    sample 200 { id: 2 }\n  }\n  @after(begin)\n  first = GET \"/first\" {\n    sample 200 { id: 1 }\n  }\n}\n";
    let fixed = fix_source(text, &LowerContext::folder(&project));
    assert_eq!(fixed.text, text);
    let refused = fixed.refused.expect("refused");
    assert_eq!(refused.message, NOT_IN_PLACE);
    assert!(text[refused.range].starts_with("@after(first)\n  second"));
}

#[test]
fn lang13_criterion_6_a_statement_ordered_into_a_branch_is_not_fixed() {
    let project = project();
    let text = format!(
        "scenario \"Into\" {{\n  begin = start\n  @after(begin)\n{}  @after(probe)\n  x = if probe.status == 200 {{\n{}  }}\n  @after(x.then)\n{}}}\n",
        request("probe", "  "),
        request("inside", "    "),
        request("outside", "  ")
    );
    let fixed = fix_source(&text, &LowerContext::folder(&project));
    assert_eq!(fixed.text, text);
    let refused = fixed.refused.expect("refused");
    assert_eq!(refused.message, NOT_IN_PLACE);
    assert!(text[refused.range].contains("outside = GET"));
}

#[test]
fn lang13_criterion_6_the_language_1_shop_30_is_not_fixed() {
    // `refund` is written after the braces of `paid` and ordered into its
    // `then`: deleting the line would make it run on the `else` path too.
    let project = project();
    let text = lang1("shop-30");
    let fixed = fix_source(&text, &LowerContext::folder(&project));
    assert_eq!(fixed.text, text);
    let refused = fixed.refused.expect("refused");
    assert_eq!(refused.message, NOT_IN_PLACE);
    assert!(text[refused.range].contains("refund = POST"));
}

#[test]
fn lang13_criterion_6_a_step_after_branches_of_two_ifs_is_not_fixed() {
    let project = project();
    let text = format!(
        "scenario \"Two ifs\" {{\n  begin = start\n  @after(begin)\n{}  @after(probe)\n  x = if probe.status == 200 {{\n{}  }}\n  @after(x)\n  y = if probe.status == 201 {{\n{}  }}\n  @after(x.then, y.else)\n{}}}\n",
        request("probe", "  "),
        request("a", "    "),
        request("b", "    "),
        request("joined", "  ")
    );
    let fixed = fix_source(&text, &LowerContext::folder(&project));
    assert_eq!(fixed.text, text);
    let refused = fixed.refused.expect("refused");
    assert_eq!(refused.message, NOT_IN_PLACE);
    assert!(text[refused.range].contains("joined = GET"));
}

// Criterion 7 ----------------------------------------------------------------

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every `.afs` under `fixtures/` but the stored revisions, by path.
fn fixtures() -> Vec<(String, String)> {
    fn walk(directory: &Path, found: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|name| name == ".apiforge") {
                continue;
            }
            if path.is_dir() {
                walk(&path, found);
            } else if path.extension().is_some_and(|e| e == "afs") {
                found.push(path);
            }
        }
    }
    let root = repository();
    let mut found = vec![];
    walk(&root.join("fixtures"), &mut found);
    found.sort();
    found
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path)
                .unwrap()
                .replace("\r\n", "\n");
            let relative = path
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            (relative, text)
        })
        .collect()
}

#[test]
fn lang13_criterion_7_the_window_writes_a_chain_without_after() {
    let project = project();
    for (path, text) in fixtures() {
        let document = written(&project, &text);
        let printed = print_folder_document(&project, &document)
            .unwrap_or_else(|errors| panic!("{path}: {errors:#?}"));
        assert!(!printed.contains("@after"), "{path}");
        let again = written(&project, &printed);
        assert_eq!(
            semantic(&again, &project),
            semantic(&document, &project),
            "{path}"
        );
        // The window writes an unchanged document back as the text it read.
        let back = edit_folder_document(&project, &text, &document, &document)
            .unwrap_or_else(|errors| panic!("{path}: {errors:#?}"));
        assert_eq!(back, text, "{path}: text -> document -> text");
    }
}

// Criterion 8 ----------------------------------------------------------------

/// A revision as 0.1.0 published it: a fan-out from the start, a join and a
/// request nothing orders after the start.
const STORED_REVISION: &str = r#"component stored "Stored" {
  input id: number
  begin = start
  @after(begin)
  left = GET "/left/{{param.id}}" {
    sample 200 { id: 1 }
  }
  @after(begin)
  right = GET "/right" {
    sample 200 { id: 2 }
  }
  @after(left, right)
  joined = GET "/joined" {
    sample 200 { id: 3 }
  }
  loose = GET "/loose" {
    sample 200 { id: 4 }
  }
  export id: number = joined.body.id
}
"#;

#[test]
fn lang13_criterion_8_a_stored_revision_reads_as_main_reads_it() {
    let project = apiforge_core::demos::shop_project();
    let recorded: BTreeMap<String, serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/lang13-revisions.json")).unwrap();
    for (name, text) in [("sign-in", SIGN_IN_REVISION), ("stored", STORED_REVISION)] {
        let document = stored(&project, text);
        assert_eq!(semantic(&document, &project), recorded[name], "{name}");
        // Printed for publication, it keeps that spelling.
        let Document::Component(component) = &document else {
            panic!("a component");
        };
        let printed = print_revision(&project, component).expect("prints");
        assert_eq!(
            semantic(&stored(&project, &printed), &project),
            recorded[name],
            "{name}"
        );
    }
}

#[test]
fn lang13_criterion_8_a_written_component_publishes_as_its_own_graph() {
    let project = project();
    let text = "component pair \"Pair\" {\n  b = GET \"/b\" {\n    sample 200 { id: 1 }\n  }\n  a = GET \"/a\" {\n    sample 200 { id: 2 }\n  }\n  export id: number = a.body.id\n}\n";
    let Document::Component(component) = written(&project, text) else {
        panic!("a component");
    };
    let revision = print_revision(&project, &component).expect("prints");
    let published = stored(&project, &revision);
    assert_eq!(
        semantic(&published, &project),
        semantic(&Document::Component(component.clone()), &project)
    );
    let _: &Component = &component;
}

// Criterion 11 ---------------------------------------------------------------

/// The order recorded from `main`'s reading of every fixture in the
/// acceptance commit: `path: main steps | cleanup steps`, *Start* left out.
fn recorded_orders() -> BTreeMap<String, String> {
    include_str!("fixtures/lang13-step-order.txt")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (path, order) = line.split_once(": ").expect("path: order");
            (path.to_owned(), order.to_owned())
        })
        .collect()
}

/// The steps that do something, without *Start*, which the fix may add: the
/// main flow's, then the cleanup's.
fn order_of(document: &Document) -> String {
    let doing = |graph: &Graph| {
        steps(graph)
            .into_iter()
            .filter(|handle| {
                graph
                    .nodes
                    .iter()
                    .any(|node| node.handle == *handle && node.kind != NodeKind::Start)
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let cleanup = match document {
        Document::Scenario(scenario) => doing(&scenario.cleanup),
        Document::Component(_) => String::new(),
    };
    format!("{} | {cleanup}", doing(main_graph(document)))
}

#[test]
fn lang13_criterion_11_every_fixture_is_language_2_in_the_recorded_order() {
    let project = project();
    let recorded = recorded_orders();
    let found = fixtures();
    assert_eq!(
        found
            .iter()
            .map(|(path, _)| path.clone())
            .collect::<BTreeSet<_>>(),
        recorded.keys().cloned().collect::<BTreeSet<_>>(),
        "every fixture has a recorded order"
    );
    for (path, text) in found {
        assert!(!text.contains("@after"), "{path}");
        let document = written(&project, &text);
        assert_eq!(order_of(&document), recorded[&path], "{path}");
    }
}

#[test]
fn lang13_criterion_11_every_example_of_the_documentation_is_language_2() {
    let root = repository();
    let mut files = vec![root.join("README.md")];
    fn walk(directory: &Path, files: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.is_dir() {
                if !matches!(name.as_str(), "qa" | "tickets" | "sprints") {
                    walk(&path, files);
                }
            } else if name.ends_with(".md") {
                files.push(path);
            }
        }
    }
    walk(&root.join("docs"), &mut files);
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap();
        let mut block: Option<String> = None;
        for line in text.lines() {
            match &mut block {
                None if line.trim_start() == "```afs" => block = Some(String::new()),
                Some(code) if line.trim_start().starts_with("```") => {
                    assert!(
                        !code.contains("@after"),
                        "{}: an example writes @after:\n{code}",
                        path.display()
                    );
                    let tree = parse(code);
                    assert!(
                        tree.file.is_some() && !tree.has_errors(),
                        "{}:\n{code}",
                        path.display()
                    );
                    let starts_and_steps = check_start(code);
                    assert!(
                        starts_and_steps,
                        "{}: a flow with a step has no start:\n{code}",
                        path.display()
                    );
                    block = None;
                }
                Some(code) => {
                    code.push_str(line);
                    code.push('\n');
                }
                None => {}
            }
        }
    }
}

/// Whether the example obeys rule 1: a main flow with a step starts with
/// `start`. An example is not always lowerable alone (it may call a
/// component of its own project), so the rule is read off the syntax.
fn check_start(code: &str) -> bool {
    use apiforge_script::ast::Form;
    let tree = parse(code);
    let Some(file) = &tree.file else {
        return true;
    };
    let step = |form: &Form| match form {
        Form::Start | Form::Request { .. } | Form::If { .. } => true,
        Form::Value(expr) => matches!(
            &expr.kind,
            apiforge_script::ast::ExprKind::Call {
                revision: Some(_),
                ..
            }
        ),
        _ => false,
    };
    match file.statements.iter().position(|s| step(&s.form)) {
        None => true,
        Some(first) => matches!(file.statements[first].form, Form::Start),
    }
}

// Criterion 12 ---------------------------------------------------------------

#[test]
fn lang13_criterion_12_completion_has_no_after() {
    let text = "scenario \"Complete\" {\n  \n}\n";
    let at = text.find("  \n}").unwrap() + 2;
    let found = complete(&Request {
        source: text,
        cursor: at,
        mode: Mode::Script,
        scope: Scope::default(),
    });
    assert!(
        found.iter().all(|item| !item.label.contains("after")),
        "{:#?}",
        found.iter().map(|item| &item.label).collect::<Vec<_>>()
    );
    assert!(
        found.iter().any(|item| item.label == "cleanup"),
        "{:?}",
        found.iter().map(|item| &item.label).collect::<Vec<_>>()
    );
}

#[test]
fn lang13_the_document_model_is_unchanged_in_shape() {
    // The written language adds no field to the model and stores no list of
    // steps: a scenario is still nodes and ordering edges.
    let project = project();
    let Document::Scenario(scenario) = written(&project, SCENARIO_BODY) else {
        panic!("a scenario");
    };
    let value = serde_json::to_value(&scenario).unwrap();
    let keys: BTreeSet<_> = value["graph"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(keys, set(&["nodes", "edges"]));
    let _: &Scenario = &scenario;
}

// Review of #488 -------------------------------------------------------------

/// A comment written inside an `@after` stays: the fix removes the
/// attribute's own text, not the comments in it (LF and CRLF).
#[test]
fn lang13_review_a_comment_inside_an_after_is_kept() {
    let project = project();
    for newline in ["\n", "\r\n"] {
        let text = "scenario \"X\" {\n  begin = start\n  @after( // keep this explanation\n    begin)\n  a = GET \"/a\"\n}\n"
            .replace('\n', newline);
        let expected =
            "scenario \"X\" {\n  begin = start\n  // keep this explanation\n  a = GET \"/a\"\n}\n"
                .replace('\n', newline);
        assert_eq!(fixed_once(&text), expected, "{newline:?}");
        let fixed = fix_source(&text, &LowerContext::folder(&project));
        assert_eq!(fixed.refused, None, "{newline:?}");
        assert_eq!(fixed.text, expected, "{newline:?}");
    }
}

/// A file without `@after` whose order in language 2 is not the order it
/// ran in language 1 is refused and left as it is: a request that reads one
/// written below it, and a request nothing ordered after `start`.
#[test]
fn lang13_review_a_file_without_after_that_would_run_otherwise_is_not_fixed() {
    let project = project();
    let context = LowerContext::folder(&project);
    let forward = "scenario \"X\" {\n  a = GET \"/{{b.body.id}}\"\n  b = GET \"/b\" { sample 200 { id: 1 } }\n}\n";
    let lowered = lower(&parse(forward), &context);
    let start = lowered
        .diagnostics
        .iter()
        .find(|d| d.message == START_MISSING)
        .expect("the start is missing");
    assert!(start.fix.is_none(), "no fix that would make a cycle");
    let fixed = fix_source(forward, &context);
    assert_eq!(fixed.text, forward);
    let refused = fixed.refused.expect("refused");
    assert_eq!(refused.message, NOT_IN_PLACE);
    assert!(forward[refused.range].starts_with("a = GET"));

    // A request nothing ordered after `start` in a file with an `@after` to
    // fix: the request that ran nowhere is named and nothing is written.
    let nowhere = "scenario \"X\" {\n  begin = start\n  @after(begin)\n  first = GET \"/first\"\n  loose = GET \"/loose\"\n}\n";
    let fixed = fix_source(nowhere, &context);
    assert_eq!(fixed.text, nowhere);
    let refused = fixed.refused.expect("refused");
    assert_eq!(refused.message, RUNS_NOWHERE);
    assert!(nowhere[refused.range].starts_with("loose = GET"));
    // With nothing to fix in its text, a file is returned as it is: whether
    // it is a language-1 file whose `loose` ran nowhere or a language-2 file
    // is known only from its project's manifest (LANG-20, A5).
    let plain = "scenario \"X\" {\n  begin = start\n  loose = GET \"/loose\"\n}\n";
    let fixed = fix_source(plain, &context);
    assert_eq!((fixed.text.as_str(), fixed.refused), (plain, None));
}

/// What the fix writes passes the check.
#[test]
fn lang13_review_the_fixed_text_passes_the_check() {
    let project = project();
    let context = LowerContext::folder(&project);
    for name in ["checkout", "lookup", "booking-day", "labels", "sign-in"] {
        let fixed = fix_source(&lang1(name), &context);
        assert_eq!(fixed.refused, None, "{name}");
        written(&project, &fixed.text);
    }
}

/// What the fix writes must not have an error its text did not have. The
/// missing `start` hides the check of the whole graph, so a data cycle in
/// the cleanup shows only once the start is written: the fix is refused,
/// the text is left as it is, and the error it would reveal is named.
#[test]
fn lang13_review_a_fix_that_would_reveal_an_error_is_refused() {
    let project = project();
    let text = "scenario \"X\" {\n  a = GET \"/a\"\n  cleanup {\n    x = y\n    y = x\n  }\n}\n";
    let lowered = lower(&parse(text), &LowerContext::folder(&project));
    let found = errors(&lowered.diagnostics);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].message, START_MISSING);
    assert!(found[0].fix.is_some(), "the start alone would be fixed");
    let fixed = fix_source(text, &LowerContext::folder(&project));
    assert_eq!(fixed.text, text);
    let refused = fixed.refused.expect("refused");
    assert_eq!(refused.message, "Graph contains a data/control cycle");
}
