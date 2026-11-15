//! Far reads in code (LAY-26): a warning at each place where a step reads a
//! value of a request or a component call directly from far away, with the
//! fix that saves the value in a variable as the board's *Save as variable*
//! does (LAY-25).
//!
//! Which reads are far is `apiforge_core::edit::far_reads`, the board's rule
//! read off the document; the variable's name and the one that already
//! holds the value are `edit::propose`. This pass only finds the places in
//! the text. It is not part of parsing or lowering: the Code view's analysis
//! and `apiforge check` call it, MCP does not.
use crate::{
    Diagnostic, Document, Fix, Severity,
    ast::{Expr, ExprKind, Form, Part, RequestItem, Statement, SyntaxTree},
    lower::{field_path, reference_parts, ungroup},
};
use apiforge_core::{
    edit::{self, ValueRef},
    format::{Graph, Node, Project},
    graph,
};
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
};

/// The warning at a far read. Callers recognise it by this constant rather
/// than by its wording; it names no value, the range shows it.
pub const FAR_READ: &str = "A far step reads this value directly; save it as a variable";

/// A reference written in a statement: the statement's handle, where the
/// reference is, and the name and fields it is written with.
struct Written<'a> {
    reader: &'a str,
    range: Range<usize>,
    name: &'a str,
    fields: Vec<String>,
}

/// The warnings of every far read in `tree`, whose valid model is
/// `document` (lowered or reconciled: steps are matched by handle), in the
/// order of the text. Each carries the fix of its value: one range from the
/// first change to the last, the `var` line added right after the source's
/// statement unless a variable already holds the value, and every direct
/// read of the value in Main and Cleanup renamed to the variable; every
/// other byte is kept.
pub fn far_read_warnings(
    tree: &SyntaxTree,
    document: &Document,
    project: &Project,
) -> Vec<Diagnostic> {
    let Some(file) = &tree.file else {
        return Vec::new();
    };
    let (main, cleanup) = match document {
        Document::Scenario(scenario) => (&scenario.graph, Some(&scenario.cleanup)),
        Document::Component(component) => (&component.graph, None),
    };
    let far: HashSet<(String, ValueRef)> = edit::far_reads(main, cleanup)
        .into_iter()
        .map(|read| (read.reader, read.value))
        .collect();
    if far.is_empty() {
        return Vec::new();
    }
    let by_handle: HashMap<&str, &Node> = main
        .nodes
        .iter()
        .chain(cleanup.into_iter().flat_map(|graph| &graph.nodes))
        .map(|node| (node.handle.as_str(), node))
        .collect();
    let mut statements: HashMap<&str, &Statement> = HashMap::new();
    let mut written = Vec::new();
    for statement in file.statements.iter().chain(file.cleanup.iter().flatten()) {
        collect(statement, &mut statements, &mut written);
    }
    // Each written reference as the value it reads, by the reader's id.
    let reads: Vec<(Range<usize>, &Node, ValueRef)> = written
        .into_iter()
        .filter_map(|reference| {
            let reader = *by_handle.get(reference.reader)?;
            let value = resolve(&reference, &by_handle, project)?;
            Some((reference.range, reader, value))
        })
        .collect();
    let mut fixes: HashMap<ValueRef, Option<Fix>> = HashMap::new();
    let mut warnings: Vec<Diagnostic> = reads
        .iter()
        .filter(|(_, reader, value)| far.contains(&(reader.id.clone(), value.clone())))
        .map(|(range, _, value)| {
            let fix = fixes
                .entry(value.clone())
                .or_insert_with(|| {
                    fix(
                        &tree.source,
                        value,
                        &reads,
                        &statements,
                        &by_handle,
                        (main, cleanup),
                        project,
                    )
                })
                .clone();
            Diagnostic {
                range: range.clone(),
                severity: Severity::Warning,
                message: FAR_READ.into(),
                fix,
            }
        })
        .collect();
    warnings.sort_by_key(|warning| warning.range.start);
    warnings
}

/// The value a written reference reads, when it is a field of a request's
/// or a component call's output, as lowering resolves it.
fn resolve(
    reference: &Written<'_>,
    by_handle: &HashMap<&str, &Node>,
    project: &Project,
) -> Option<ValueRef> {
    if matches!(reference.name, "input" | "param" | "env") {
        return None;
    }
    let source = by_handle.get(reference.name)?;
    let ports = graph::output_ports(source, project);
    let mut fields = reference.fields.clone();
    let port = if let [port] = ports.as_slice() {
        port.name.clone()
    } else if fields.is_empty() {
        return None;
    } else {
        fields.remove(0)
    };
    Some(ValueRef::new(source.id.clone(), port, field_path(&fields)))
}

/// The fix of `value`: the board's edit as a text edit.
fn fix(
    text: &str,
    value: &ValueRef,
    reads: &[(Range<usize>, &Node, ValueRef)],
    statements: &HashMap<&str, &Statement>,
    by_handle: &HashMap<&str, &Node>,
    (main, cleanup): (&Graph, Option<&Graph>),
    project: &Project,
) -> Option<Fix> {
    let proposal = edit::propose(main, cleanup, project, value);
    let mut edits: Vec<(Range<usize>, String)> = reads
        .iter()
        .filter(|(_, reader, read)| {
            read == value && Some(reader.id.as_str()) != proposal.held_by.as_deref()
        })
        .map(|(range, ..)| (range.clone(), proposal.name.clone()))
        .collect();
    if proposal.held_by.is_none() {
        let source = by_handle
            .values()
            .find(|node| node.id == value.node_id)
            .and_then(|node| statements.get(node.handle.as_str()))?;
        // The end of the source's last line, before its line break: a
        // comment that ends the line stays with the source.
        let end = source.range.end.min(text.len());
        let mut at = text[end..]
            .find('\n')
            .map_or(text.len(), |offset| end + offset);
        let mut line_break = "\n";
        if text[..at].ends_with('\r') {
            at -= 1;
            line_break = "\r\n";
        }
        let start = source.range.start.min(text.len());
        let line = text[..start].rfind('\n').map_or(0, |offset| offset + 1);
        let indent: String = text[line..]
            .chars()
            .take_while(|character| matches!(character, ' ' | '\t'))
            .collect();
        edits.push((at..at, format!("{line_break}{indent}{}", proposal.line())));
    }
    edits.sort_by_key(|(range, _)| (range.start, range.end));
    let start = edits.first()?.0.start;
    let end = edits.iter().map(|(range, _)| range.end).max()?;
    let mut replacement = String::new();
    let mut at = start;
    for (range, with) in &edits {
        if range.start < at {
            return None;
        }
        replacement.push_str(&text[at..range.start]);
        replacement.push_str(with);
        at = range.end;
    }
    replacement.push_str(&text[at..end]);
    Some(Fix {
        range: start..end,
        replacement,
    })
}

/// Records `statement` by handle and every reference it reads, its
/// branches' statements included.
fn collect<'a>(
    statement: &'a Statement,
    statements: &mut HashMap<&'a str, &'a Statement>,
    written: &mut Vec<Written<'a>>,
) {
    statements.insert(statement.handle.as_str(), statement);
    let reader = statement.handle.as_str();
    let mut read = |expr: &'a Expr| references(reader, expr, written);
    match &statement.form {
        Form::Start => {}
        Form::Value(value) | Form::Export { value, .. } => read(value),
        Form::Request { url, items, .. } => {
            read(url);
            for item in items {
                match item {
                    RequestItem::Argument { value, .. }
                    | RequestItem::Headers(value)
                    | RequestItem::Query(value)
                    | RequestItem::Body { value, .. } => read(value),
                    RequestItem::Timeout { .. }
                    | RequestItem::Contract(_)
                    | RequestItem::Sample { .. } => {}
                }
            }
        }
        Form::Assert(check) | Form::If { check, .. } => {
            for expr in [
                Some(&check.actual),
                check.expected.as_ref(),
                check.tolerance.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                read(expr);
            }
        }
        Form::Condition { checks, value, .. } => {
            for check in checks {
                for expr in [
                    Some(&check.actual),
                    check.expected.as_ref(),
                    check.tolerance.as_ref(),
                ]
                .into_iter()
                .flatten()
                {
                    read(expr);
                }
            }
            if let Some(value) = value {
                read(value);
            }
        }
    }
    // `@inputs` binds inputs too; the other attributes read no value.
    for attribute in &statement.attributes {
        if attribute.name == "inputs" {
            for argument in &attribute.arguments {
                read(argument);
            }
        }
    }
    if let Form::If {
        then, otherwise, ..
    } = &statement.form
    {
        for nested in then.iter().chain(otherwise.iter().flatten()) {
            collect(nested, statements, written);
        }
    }
}

/// Every reference `expr` writes, whole: `cart.cart.id` once, not `cart`.
fn references<'a>(reader: &'a str, expr: &'a Expr, written: &mut Vec<Written<'a>>) {
    match &expr.kind {
        ExprKind::Name(_) | ExprKind::Field { .. } => {
            let (name, fields) = reference_parts(expr);
            if let Some(name) = name {
                written.push(Written {
                    reader,
                    range: expr.range.clone(),
                    name,
                    fields,
                });
            } else if let ExprKind::Field { base, .. } = &expr.kind {
                references(reader, ungroup(base), written);
            }
        }
        ExprKind::Group(inner) | ExprKind::Not(inner) => references(reader, inner, written),
        ExprKind::Object(fields) => {
            for (_, value) in fields {
                references(reader, value, written);
            }
        }
        ExprKind::Array(items) => {
            for item in items {
                references(reader, item, written);
            }
        }
        ExprKind::Template(parts) => {
            for part in parts {
                if let Part::Value(value) = part {
                    references(reader, value, written);
                }
            }
        }
        ExprKind::Call { arguments, .. } => {
            for argument in arguments {
                references(reader, &argument.value, written);
            }
        }
        ExprKind::Binary { left, right, .. } => {
            references(reader, left, written);
            references(reader, right, written);
        }
        ExprKind::Literal(_) | ExprKind::Missing => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LowerContext, lower, parse};

    fn project() -> Project {
        apiforge_core::import_document(
            include_bytes!("../../../fixtures/workspace.json"),
            &apiforge_core::ImportLimits::default(),
        )
        .unwrap()
        .into_workspace()
        .projects
        .remove(0)
    }

    fn warnings(text: &str) -> Vec<Diagnostic> {
        let project = project();
        let tree = parse(text);
        let lowered = lower(&tree, &LowerContext::new(&project));
        assert!(!lowered.has_errors(), "{:?}", lowered.diagnostics);
        far_read_warnings(&tree, lowered.document.as_ref().unwrap(), &project)
    }

    fn fixed(text: &str, warning: &Diagnostic) -> String {
        let fix = warning.fix.clone().expect("a fix");
        let mut text = text.to_owned();
        text.replace_range(fix.range, &fix.replacement);
        text
    }

    const FAR: &str = "scenario \"S\" {\n  a = GET \"/a\" // made here\n  @after(a)\n  b = GET \"/b\"\n  @after(b)\n  c = GET \"/c/{{a.body.id}}\" {\n    body { again: a.body.id, near: b.body, all: a.body }\n  }\n}\n";

    #[test]
    fn a_far_read_is_warned_and_its_fix_adds_the_line_after_the_comment() {
        let found = warnings(FAR);
        let reads: Vec<&str> = found
            .iter()
            .map(|warning| &FAR[warning.range.clone()])
            .collect();
        assert_eq!(reads, ["a.body.id", "a.body.id", "a.body"]);
        assert!(found.iter().all(|warning| warning.message == FAR_READ));
        let text = fixed(FAR, &found[0]);
        assert_eq!(
            text,
            FAR.replace("a.body.id", "aId")
                .replace("// made here\n", "// made here\n  var aId = a.body.id\n")
        );
        let after = warnings(&text);
        assert_eq!(after.len(), 1, "only a.body is left");
    }

    #[test]
    fn a_crlf_source_gets_a_crlf_line() {
        let crlf = FAR.replace('\n', "\r\n");
        let found = warnings(&crlf);
        let text = fixed(&crlf, &found[0]);
        assert!(
            text.contains("// made here\r\n  var aId = a.body.id\r\n  @after(a)"),
            "{text:?}"
        );
    }

    #[test]
    fn a_near_read_or_a_parameter_is_not_warned() {
        let text = "scenario \"S\" {\n  param p = 1\n  a = GET \"/a\"\n  @after(a)\n  b = GET \"/b/{{a.body.id}}\" {\n    body { p: param.p }\n  }\n}\n";
        assert!(warnings(text).is_empty());
    }
}
