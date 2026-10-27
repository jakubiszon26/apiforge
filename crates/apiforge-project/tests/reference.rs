//! MCP-02: the `.afs` language reference is proven against the grammar it
//! describes. Every fenced `afs` block parses without a diagnostic and is
//! already in canonical form, the examples show every predicate, method and
//! statement form, and the complete component and scenario lower against a
//! folder project that publishes the component the scenario calls. A grammar
//! change that the reference does not follow fails here.
use apiforge_core::{format::Project, model::NodeKind};
use apiforge_project::*;
use apiforge_script::ast::{Check, Expr, ExprKind, FileKind, Form, Part, RequestItem, Statement};
use std::collections::BTreeSet;

const REFERENCE: &str = include_str!("../../../docs/language/reference.md");

/// Second-level headings of the reference, in this order. Each topic of the
/// ticket's first criterion is one section.
const SECTIONS: &[&str] = &[
    "File shape",
    "Parameters and inputs",
    "Start and ordering",
    "Requests",
    "Expressions and operators",
    "Values and var",
    "Assertions and predicates",
    "Branches: if and else",
    "Cleanup",
    "Exports",
    "Component calls",
    "Comments and labels",
    "What cannot be expressed",
    "A complete component",
    "A complete scenario",
    "Diagnostics and what to do",
];

/// Sections that show their topic with at least one `afs` example.
const WITH_EXAMPLES: &[&str] = &[
    "File shape",
    "Parameters and inputs",
    "Start and ordering",
    "Requests",
    "Expressions and operators",
    "Values and var",
    "Assertions and predicates",
    "Branches: if and else",
    "Cleanup",
    "Exports",
    "Component calls",
    "Comments and labels",
];

/// A fenced `afs` block: the section it is in, the reference line of its
/// first source line (1-based) and its text.
struct Block {
    section: String,
    line: usize,
    text: String,
}

fn is_fence(line: &str) -> bool {
    line.trim_start().starts_with("```")
}

fn blocks() -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut section = String::new();
    // (first line, text, tagged afs) of the fenced block being read.
    let mut open: Option<(usize, String, bool)> = None;
    for (index, line) in REFERENCE.lines().enumerate() {
        if let Some((start, text, afs)) = &mut open {
            if is_fence(line) {
                if *afs {
                    blocks.push(Block {
                        section: section.clone(),
                        line: *start,
                        text: std::mem::take(text),
                    });
                }
                open = None;
            } else {
                text.push_str(line);
                text.push('\n');
            }
        } else if let Some(heading) = line.strip_prefix("## ") {
            heading.trim().clone_into(&mut section);
        } else if is_fence(line) {
            let tag = line.trim_start().trim_start_matches('`').trim();
            open = Some((index + 2, String::new(), tag == "afs"));
        }
    }
    assert!(open.is_none(), "the reference ends inside a fenced block");
    blocks
}

fn sections() -> Vec<String> {
    let mut fenced = false;
    let mut headings = Vec::new();
    for line in REFERENCE.lines() {
        if is_fence(line) {
            fenced = !fenced;
        } else if !fenced && let Some(heading) = line.strip_prefix("## ") {
            headings.push(heading.trim().to_owned());
        }
    }
    headings
}

fn the_block(section: &str) -> String {
    let mut found: Vec<_> = blocks()
        .into_iter()
        .filter(|block| block.section == section)
        .collect();
    assert_eq!(
        found.len(),
        1,
        "the section `{section}` has exactly one afs block"
    );
    found.remove(0).text
}

#[test]
fn the_reference_opens_with_its_version_line() {
    let first = REFERENCE.lines().next().unwrap_or_default();
    let date = first
        .strip_prefix("Reference version: 1 · grammar of ADR 0005 as of ")
        .unwrap_or_else(|| panic!("unexpected first line: {first:?}"));
    let parts: Vec<_> = date.split('-').collect();
    let iso = parts.len() == 3
        && [4, 2, 2].iter().zip(&parts).all(|(width, part)| {
            part.len() == *width && part.bytes().all(|byte| byte.is_ascii_digit())
        });
    assert!(iso, "the version line ends with an ISO date: {date:?}");
}

#[test]
fn the_reference_is_under_50_000_characters() {
    let characters = REFERENCE.chars().count();
    assert!(
        characters < 50_000,
        "{characters} characters; Claude Code shows the reference inline only below 50 000"
    );
}

#[test]
fn the_reference_has_its_sections_in_order_each_topic_with_an_example() {
    let wanted: Vec<String> = SECTIONS.iter().map(|s| (*s).to_owned()).collect();
    let found: Vec<String> = sections()
        .into_iter()
        .filter(|heading| wanted.contains(heading))
        .collect();
    assert_eq!(found, wanted, "the required sections, in order");
    let blocks = blocks();
    for section in WITH_EXAMPLES {
        assert!(
            blocks.iter().any(|block| block.section == *section),
            "the section `{section}` has an afs example"
        );
    }
}

#[test]
fn every_afs_block_parses_without_a_diagnostic_and_is_canonical() {
    let blocks = blocks();
    assert!(!blocks.is_empty(), "the reference has afs examples");
    let mut failures = Vec::new();
    for block in &blocks {
        let tree = apiforge_script::parse(&block.text);
        if !tree.diagnostics.is_empty() {
            let found: Vec<_> = tree
                .diagnostics
                .iter()
                .map(|d| (&block.text[d.range.clone()], d.message.as_str()))
                .collect();
            failures.push(format!("line {}: diagnostics {found:?}", block.line));
            continue;
        }
        match apiforge_script::format_tree(&tree) {
            Ok(formatted) if formatted == block.text => {}
            Ok(formatted) => failures.push(format!(
                "line {}: not canonical; the formatter writes:\n{formatted}",
                block.line
            )),
            Err(diagnostics) => failures.push(format!(
                "line {}: does not format: {diagnostics:?}",
                block.line
            )),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// What the examples use, collected from their syntax trees rather than from
/// the prose, so a list in the text cannot stand in for an example.
#[derive(Default)]
struct Coverage {
    words: BTreeSet<String>,
}

impl Coverage {
    fn add(&mut self, word: impl Into<String>) {
        self.words.insert(word.into());
    }

    fn check(&mut self, check: &Check) {
        self.add(format!("predicate {}", check.predicate));
        self.expr(&check.actual);
        if let Some(expected) = &check.expected {
            self.expr(expected);
        }
    }

    fn expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Literal(_) | ExprKind::Missing | ExprKind::Name(_) => {}
            ExprKind::Object(fields) => fields.iter().for_each(|(_, value)| self.expr(value)),
            ExprKind::Array(items) => items.iter().for_each(|item| self.expr(item)),
            ExprKind::Template(parts) => {
                self.add("template");
                for part in parts {
                    if let Part::Value(value) = part {
                        self.expr(value);
                    }
                }
            }
            ExprKind::Field { base, .. } => {
                if let ExprKind::Name(root) = &base.kind
                    && matches!(root.as_str(), "env" | "param" | "input")
                {
                    self.add(format!("namespace {root}"));
                }
                self.expr(base);
            }
            ExprKind::Call {
                revision,
                arguments,
                ..
            } => {
                self.add(if revision.is_some() {
                    "call with revision"
                } else {
                    "function call"
                });
                arguments
                    .iter()
                    .for_each(|argument| self.expr(&argument.value));
            }
            ExprKind::Binary {
                operator,
                left,
                right,
            } => {
                self.add(format!("operator {operator}"));
                self.expr(left);
                self.expr(right);
            }
            ExprKind::Not(value) => {
                self.add("operator !");
                self.expr(value);
            }
            ExprKind::Group(value) => self.expr(value),
        }
    }

    fn request_item(&mut self, item: &RequestItem) {
        let (word, value) = match item {
            RequestItem::Argument { value, .. } => ("request argument", Some(value)),
            RequestItem::Headers(value) => ("request headers", Some(value)),
            RequestItem::Query(value) => ("request query", Some(value)),
            RequestItem::Body { text: true, value } => ("request body text", Some(value)),
            RequestItem::Body { value, .. } => ("request body", Some(value)),
            RequestItem::Timeout { .. } => ("request timeout", None),
            RequestItem::Contract(_) => ("request contract", None),
            RequestItem::Sample { .. } => ("request sample", None),
        };
        self.add(word);
        if let Some(value) = value {
            self.expr(value);
        }
    }

    fn statements(&mut self, statements: &[Statement]) {
        for statement in statements {
            for attribute in &statement.attributes {
                self.add(format!("attribute {}", attribute.name));
            }
            if statement.note.is_some() {
                self.add("comment");
            }
            if statement.variable {
                self.add("var");
            }
            match &statement.form {
                Form::Start => self.add("start"),
                Form::Value(value) => {
                    self.add("value");
                    self.expr(value);
                }
                Form::Request { method, url, items } => {
                    self.add(format!("method {method}"));
                    self.expr(url);
                    items.iter().for_each(|item| self.request_item(item));
                }
                Form::Assert(check) => {
                    self.add("assert");
                    self.check(check);
                }
                Form::Condition {
                    mode,
                    checks,
                    value,
                } => {
                    self.add(format!("condition {mode}"));
                    checks.iter().for_each(|check| self.check(check));
                    if let Some(value) = value {
                        self.add("condition with");
                        self.expr(value);
                    }
                }
                Form::If {
                    check,
                    then,
                    otherwise,
                } => {
                    self.add("if");
                    self.check(check);
                    self.statements(then);
                    if let Some(otherwise) = otherwise {
                        self.add("else");
                        self.statements(otherwise);
                    }
                }
                Form::Export { value, .. } => {
                    self.add("export");
                    self.expr(value);
                }
            }
        }
    }
}

#[test]
fn the_examples_cover_every_predicate_method_and_statement_form() {
    let mut coverage = Coverage::default();
    for block in blocks() {
        let tree = apiforge_script::parse(&block.text);
        let Some(file) = &tree.file else { continue };
        coverage.add(match &file.kind {
            FileKind::Scenario { .. } => "scenario",
            FileKind::Component { .. } => "component",
        });
        if file.description.is_some() {
            coverage.add("description");
        }
        if let Some(policy) = &file.on_error {
            coverage.add(format!("on_error {policy}"));
        }
        if !file.parameters.is_empty() {
            coverage.add("param");
        }
        for input in &file.inputs {
            coverage.add("input");
            if input.default.is_some() {
                coverage.add("input default");
            }
            for attribute in &input.attributes {
                coverage.add(format!("input attribute {}", attribute.name));
            }
        }
        if file.action.is_some() {
            coverage.add("action");
        }
        coverage.statements(&file.statements);
        if let Some(cleanup) = &file.cleanup {
            coverage.add("cleanup");
            coverage.statements(cleanup);
        }
    }
    let mut wanted: Vec<String> = [
        "scenario",
        "component",
        "description",
        "on_error stop",
        "on_error continue",
        "param",
        "input",
        "input default",
        "input attribute optional",
        "input attribute label",
        "action",
        "start",
        "attribute after",
        "attribute label",
        "comment",
        "var",
        "value",
        "assert",
        "condition all",
        "condition any",
        "condition with",
        "if",
        "else",
        "cleanup",
        "export",
        "call with revision",
        "function call",
        "template",
        "namespace env",
        "namespace param",
        "namespace input",
        "request argument",
        "request headers",
        "request query",
        "request body",
        "request body text",
        "request timeout",
        "request contract",
        "request sample",
        "operator +",
        "operator -",
        "operator *",
        "operator /",
        "operator %",
        "operator ==",
        "operator !=",
        "operator <",
        "operator >",
        "operator <=",
        "operator >=",
        "operator &&",
        "operator ||",
        "operator !",
    ]
    .iter()
    .map(|word| (*word).to_owned())
    .collect();
    wanted.extend(
        apiforge_script::complete::predicate_words()
            .into_iter()
            .map(|(_, word)| format!("predicate {word}")),
    );
    wanted.extend(
        apiforge_script::complete::method_words()
            .into_iter()
            .map(|(_, word)| format!("method {word}")),
    );
    let missing: Vec<_> = wanted
        .iter()
        .filter(|word| !coverage.words.contains(*word))
        .collect();
    assert!(missing.is_empty(), "no example shows: {missing:?}");
}

#[test]
fn the_reference_explains_what_the_server_answers() {
    let diagnostics = REFERENCE
        .split("\n## Diagnostics and what to do")
        .nth(1)
        .expect("a diagnostics section");
    for code in [
        "stale_source",
        "comments_would_be_lost",
        "secret_in_text",
        "check_text",
        "run",
    ] {
        assert!(
            diagnostics.contains(&format!("`{code}`")),
            "the diagnostics section explains `{code}`"
        );
    }
    assert!(
        diagnostics.contains("`check_text` checks a proposal")
            && diagnostics.contains("`run` executes the saved source"),
        "the diagnostics section says that check_text checks a proposal while run executes the saved source"
    );
}

/// A folder project with a base URL, one environment variable and one secret
/// name, into which the reference's complete component is created and
/// published as revision 1.
fn fixture(directory: &std::path::Path) -> (ProjectSession, Published) {
    let project: Project = serde_json::from_value(serde_json::json!({
        "version": apiforge_core::format::FORMAT_VERSION,
        "id": "reference",
        "name": "Reference",
        "scenarios": [],
        "components": [],
        "contracts": [],
        "environment": {
            "baseUrl": "http://127.0.0.1:4318/demo/shop",
            "variables": { "currency": "EUR" },
            "secretNames": ["apiToken"],
        },
    }))
    .unwrap();
    prepare_project(directory, &project).unwrap();
    let mut session = ProjectSession::open(directory).unwrap();
    let component = session
        .create_document(
            DocumentKind::Component,
            "components/reference.afs",
            &the_block("A complete component"),
        )
        .unwrap();
    let diagnostics = session.document(&component).unwrap().diagnostics().to_vec();
    assert!(
        diagnostics.is_empty(),
        "the complete component is a valid working document: {diagnostics:?}"
    );
    let published = session.publish(&component, Origin::SESSION).unwrap();
    assert_eq!(published.revision, 1);
    (session, published)
}

#[test]
fn the_complete_component_and_scenario_lower_against_a_project_that_publishes_the_component() {
    let directory = tempfile::tempdir().unwrap();
    let (session, published) = fixture(directory.path());
    let context = session.context().as_ref().clone();
    let folder = apiforge_script::LowerContext::folder(&context);

    let component = the_block("A complete component");
    let lowered = apiforge_script::lower(&apiforge_script::parse(&component), &folder);
    assert!(
        lowered.diagnostics.is_empty(),
        "the complete component lowers without a diagnostic: {:?}",
        lowered.diagnostics
    );
    assert!(matches!(lowered.document, Some(Document::Component(_))));

    let scenario = the_block("A complete scenario");
    let lowered = apiforge_script::lower(&apiforge_script::parse(&scenario), &folder);
    assert!(
        lowered.diagnostics.is_empty(),
        "the complete scenario lowers without a diagnostic: {:?}",
        lowered.diagnostics
    );
    let Some(Document::Scenario(scenario)) = lowered.document else {
        panic!("the complete scenario is a scenario")
    };
    let calls: Vec<_> = scenario
        .graph
        .nodes
        .iter()
        .chain(&scenario.cleanup.nodes)
        .filter(|node| node.kind == NodeKind::Component)
        .collect();
    assert!(
        !calls.is_empty(),
        "the complete scenario calls the complete component"
    );
    for call in calls {
        assert_eq!(
            call.config.component_id.as_deref(),
            Some(published.component_id.as_str()),
            "the call resolves to the published component"
        );
        assert_eq!(call.config.revision, Some(published.revision));
    }
}
