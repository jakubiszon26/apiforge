use crate::{
    ast::*,
    parser::{parse, precedence},
};
use std::collections::HashMap;

pub fn format(source: &str) -> Result<String, Vec<Diagnostic>> {
    format_tree(&parse(source))
}

pub fn format_tree(tree: &SyntaxTree) -> Result<String, Vec<Diagnostic>> {
    if tree.has_errors() || tree.file.is_none() {
        return Err(tree.diagnostics.clone());
    }
    let mut writer = Writer::new(&tree.source, &tree.comments, true);
    writer.file(tree.file.as_ref().unwrap());
    let formatted = writer.out;
    if formatted.len() > crate::MAX_SOURCE_BYTES {
        let mut diagnostics = tree.diagnostics.clone();
        diagnostics.push(Diagnostic::error(
            0..tree.source.len(),
            "Formatted source would exceed the 1 MiB limit",
        ));
        return Err(diagnostics);
    }
    Ok(formatted)
}

pub(crate) fn quote(text: &str) -> String {
    serde_json::to_string(text)
        .expect("strings serialize")
        .replace("{{", "\\{{")
}

fn key(name: &str) -> String {
    let mut chars = name.chars();
    if chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        name.into()
    } else {
        quote(name)
    }
}

/// How a check that is one Boolean expression is written (LANG-05).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Checks {
    /// The written language: a compound check as the author wrote it.
    Short,
    /// The stored spelling of a published revision and of the legacy
    /// workspace: every plain expression in full, `expr == true`, as 0.1.0
    /// wrote it, so a revision prints back to its own bytes.
    Full,
}

/// `file` in the canonical layout, as `print` writes it, with checks
/// written as `checks` says.
pub(crate) fn print_file_with(file: &File, checks: Checks) -> String {
    let mut writer = Writer::new("", &[], true);
    writer.checks = checks;
    writer.file(file);
    writer.out
}

/// The first line of a file, up to its opening brace.
pub(crate) fn opening(kind: &FileKind) -> String {
    match kind {
        FileKind::Scenario { name } => format!("scenario {} {{", quote(name)),
        FileKind::Component { handle, name } => format!("component {handle} {} {{", quote(name)),
    }
}

/// The parts of the file before its statements, in the formatter's order,
/// each with its text at depth 0: `description`, `on_error`, `param` lines,
/// `input` lines with their attributes, `action`.
pub(crate) fn head(file: &File) -> Vec<(Item, String)> {
    head_in(file, false)
}

/// As [`head`], the attributes of each `input` in the canonical order when
/// `canonical` is set.
fn head_in(file: &File, canonical: bool) -> Vec<(Item, String)> {
    let mut parts = vec![];
    if let Some(description) = &file.description {
        parts.push((
            Item::Description,
            format!("description {}", quote(description)),
        ));
    }
    if let Some(policy) = &file.on_error {
        parts.push((Item::OnError, format!("on_error {policy}")));
    }
    for (name, value) in &file.parameters {
        parts.push((
            Item::Parameter(name.clone()),
            format!("param {} = {}", key(name), expression(value)),
        ));
    }
    for input in &file.inputs {
        let mut out = String::new();
        attributes(&mut out, &input.attributes, 0, canonical.then_some(true));
        let default = input
            .default
            .as_ref()
            .map_or(String::new(), |value| format!(" = {}", expression(value)));
        line(
            &mut out,
            0,
            &format!("input {}: {}{default}", key(&input.name), input.value_type),
        );
        out.pop();
        parts.push((Item::Input(input.name.clone()), out));
    }
    if let Some(action) = &file.action {
        let value_type = file
            .action_type
            .as_ref()
            .map_or(String::new(), |t| format!(": {t}"));
        let contract = file
            .action_contract
            .as_ref()
            .map_or(String::new(), |c| format!(" contract {}", quote(c)));
        parts.push((
            Item::Action,
            format!("action {}{value_type}{contract}", key(action)),
        ));
    }
    parts
}

fn line(out: &mut String, depth: usize, text: &str) {
    out.push_str(&"  ".repeat(depth));
    out.push_str(text);
    out.push('\n');
}

/// The canonical place of an attribute among a statement's, or among an
/// `input`'s when `input` is set (KTLO-37): the order `print` builds them in,
/// `@label` first and `@note` after it. One the language does not know goes
/// last. `fmt` and `print` write attributes in this order; a board edit keeps
/// the order its author wrote.
pub(crate) fn attribute_rank(name: &str, input: bool) -> usize {
    const STATEMENT: &[&str] = &["label", "note", "contract", "after", "config", "inputs"];
    const INPUT: &[&str] = &[
        "label", "contract", "required", "optional", "exposed", "hidden",
    ];
    let order = if input { INPUT } else { STATEMENT };
    order
        .iter()
        .position(|known| *known == name)
        .unwrap_or(order.len())
}

/// Writes `attributes` at `depth`: in the canonical order of an `input`'s
/// (`Some(true)`) or a statement's (`Some(false)`), or as given.
fn attributes(out: &mut String, attributes: &[Attribute], depth: usize, canonical: Option<bool>) {
    let mut attributes: Vec<&Attribute> = attributes.iter().collect();
    if let Some(input) = canonical {
        attributes.sort_by_key(|attribute| attribute_rank(&attribute.name, input));
    }
    for attribute in attributes {
        let arguments = if attribute.arguments.is_empty() {
            String::new()
        } else {
            format!(
                "({})",
                attribute
                    .arguments
                    .iter()
                    .map(expression)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        line(out, depth, &format!("@{}{arguments}", attribute.name));
    }
}

/// `nodes` written at `depth`, each with its note and attributes, in the
/// order of the attributes given.
pub(crate) fn statements(out: &mut String, nodes: &[Statement], depth: usize) {
    let mut writer = Writer::new("", &[], false);
    writer.out = std::mem::take(out);
    writer.statements(nodes, depth);
    *out = writer.out;
}

/// Writes a file's lines, with the free comments of its source at their
/// places (LANG-06).
struct Writer<'a> {
    out: String,
    source: &'a str,
    /// The free comments, in source order.
    comments: &'a [Comment],
    /// The last comment of each place, by its index in `comments`.
    last: HashMap<Place, usize>,
    /// The first comment not written yet.
    next: usize,
    /// Whether attributes are written in the canonical order.
    canonical: bool,
    /// How a check that is one Boolean expression is written (LANG-05).
    checks: Checks,
}

impl<'a> Writer<'a> {
    fn new(source: &'a str, comments: &'a [Comment], canonical: bool) -> Self {
        let last = comments
            .iter()
            .enumerate()
            .map(|(index, comment)| (comment.place, index))
            .collect();
        Self {
            out: String::new(),
            source,
            comments,
            last,
            next: 0,
            canonical,
            checks: Checks::Short,
        }
    }

    /// Writes the comments of `place` at `depth`, with any written before
    /// them in the source that are still to be written, so that free
    /// comments keep their order when Format moves what they stand at (a
    /// part of the head written after a statement). Whether it wrote any.
    fn comments(&mut self, place: Place, depth: usize) -> bool {
        let Some(&last) = self.last.get(&place) else {
            return false;
        };
        if last < self.next {
            return false;
        }
        for comment in &self.comments[self.next..=last] {
            let text = self.source[comment.range.clone()].trim_end();
            line(&mut self.out, depth, text);
        }
        self.next = last + 1;
        true
    }

    fn file(&mut self, file: &File) {
        self.comments(Place::Top, 0);
        self.out.push_str(&opening(&file.kind));
        self.out.push('\n');
        let start = |wanted: &Item| {
            file.items
                .iter()
                .find(|(item, _)| item == wanted)
                .map(|(_, range)| range.start)
        };
        for (item, text) in head_in(file, self.canonical) {
            if let Some(start) = start(&item) {
                self.comments(Place::Head(start), 1);
            }
            for text in text.lines() {
                line(&mut self.out, 1, text);
            }
        }
        self.statements(&file.statements, 1);
        if let Some(cleanup) = &file.cleanup {
            if let Some(start) = start(&Item::Cleanup) {
                self.comments(Place::Head(start), 1);
            }
            line(&mut self.out, 1, "cleanup {");
            self.statements(cleanup, 2);
            self.comments(Place::End(Braces::Cleanup), 2);
            line(&mut self.out, 1, "}");
        }
        self.comments(Place::End(Braces::File), 1);
        self.out.push_str("}\n");
        self.comments(Place::Bottom, 0);
        // Every comment has a place written above; none is ever lost.
        for comment in &self.comments[self.next.min(self.comments.len())..] {
            line(
                &mut self.out,
                0,
                self.source[comment.range.clone()].trim_end(),
            );
        }
        self.next = self.comments.len();
    }

    fn statements(&mut self, nodes: &[Statement], depth: usize) {
        for node in nodes {
            let start = node.range.start;
            if self.comments(Place::Statement(start), depth) {
                self.out.push('\n');
            }
            if let Some(note) = &node.note {
                for note in note.split('\n') {
                    line(&mut self.out, depth, &format!("// {note}"));
                }
            }
            attributes(
                &mut self.out,
                &node.attributes,
                depth,
                self.canonical.then_some(false),
            );
            let handle = &node.handle;
            let prefix = if node.variable {
                format!("var {handle} = ")
            } else {
                format!("{handle} = ")
            };
            match &node.form {
                Form::Start => line(&mut self.out, depth, &format!("{prefix}start")),
                Form::Value(value) => line(
                    &mut self.out,
                    depth,
                    &format!("{prefix}{}", expression(value)),
                ),
                Form::Export { value_type, value } => line(
                    &mut self.out,
                    depth,
                    &format!("export {handle}: {value_type} = {}", expression(value)),
                ),
                Form::Assert(check) => line(
                    &mut self.out,
                    depth,
                    &format!("{prefix}assert {}", check_text(check, self.checks)),
                ),
                Form::If {
                    check,
                    then,
                    otherwise,
                } => {
                    line(
                        &mut self.out,
                        depth,
                        &format!("{prefix}if {} {{", check_text(check, self.checks)),
                    );
                    self.statements(then, depth + 1);
                    self.comments(Place::End(Braces::Then(start)), depth + 1);
                    if let Some(otherwise) = otherwise {
                        line(&mut self.out, depth, "} else {");
                        self.statements(otherwise, depth + 1);
                        self.comments(Place::End(Braces::Else(start)), depth + 1);
                    }
                    line(&mut self.out, depth, "}");
                }
                Form::Condition {
                    mode,
                    checks,
                    value,
                } => {
                    line(
                        &mut self.out,
                        depth,
                        &format!("{prefix}condition {mode} {{"),
                    );
                    for (index, check) in checks.iter().enumerate() {
                        self.comments(
                            Place::Line {
                                statement: start,
                                index,
                            },
                            depth + 1,
                        );
                        line(
                            &mut self.out,
                            depth + 1,
                            &format!("{},", check_text(check, self.checks)),
                        );
                    }
                    self.comments(Place::End(Braces::Lines(start)), depth + 1);
                    line(
                        &mut self.out,
                        depth,
                        &value.as_ref().map_or_else(
                            || "}".into(),
                            |value| format!("}} with {}", expression(value)),
                        ),
                    );
                }
                Form::Request { method, url, items } => {
                    let first = format!("{prefix}{method} {}", expression(url));
                    if items.is_empty() {
                        line(&mut self.out, depth, &first);
                        continue;
                    }
                    line(&mut self.out, depth, &format!("{first} {{"));
                    for (index, item) in items.iter().enumerate() {
                        self.comments(
                            Place::Line {
                                statement: start,
                                index,
                            },
                            depth + 1,
                        );
                        line(&mut self.out, depth + 1, &request_item(item));
                    }
                    self.comments(Place::End(Braces::Lines(start)), depth + 1);
                    line(&mut self.out, depth, "}");
                }
            }
        }
    }
}

fn request_item(item: &RequestItem) -> String {
    match item {
        RequestItem::Argument { name, value } => {
            format!("argument {} = {}", key(name), expression(value))
        }
        RequestItem::Headers(value) => format!("headers {}", expression(value)),
        RequestItem::Query(value) => format!("query {}", expression(value)),
        RequestItem::Body { text, value } => format!(
            "body {}{}",
            if *text { "text " } else { "" },
            expression(value)
        ),
        RequestItem::Timeout { milliseconds } => {
            if milliseconds % 1000.0 == 0.0 {
                format!("timeout {}s", milliseconds / 1000.0)
            } else {
                format!("timeout {milliseconds}ms")
            }
        }
        RequestItem::Contract(name) => format!("contract {}", quote(name)),
        RequestItem::Sample { status, value } => {
            format!("sample {status} {}", expression(value))
        }
    }
}

fn check_operand(value: &Expr) -> String {
    if matches!(&value.kind, ExprKind::Binary { operator, .. } if precedence(operator).is_some_and(|p| p <= 40))
    {
        format!("({})", expression(value))
    } else {
        expression(value)
    }
}

/// The expression of `check` when it may be written alone (LANG-05): the check
/// is `expr == true` and `expr` is a comparison or a logical operation that,
/// written without `== true`, reads back as this same check. Those are `>=`,
/// `<=`, `&&`, `||` and `!` at its top. A bare value keeps `== true`, and so
/// does an expression whose own operator is a predicate (`(a == b) == true`).
fn written_alone(check: &Check) -> Option<&Expr> {
    if check.predicate != "=="
        || check.tolerance.is_some()
        || !check
            .expected
            .as_ref()
            .is_some_and(|e| matches!(e.kind, ExprKind::Literal(serde_json::Value::Bool(true))))
    {
        return None;
    }
    let mut actual = &check.actual;
    while let ExprKind::Group(inner) = &actual.kind {
        actual = inner;
    }
    match &actual.kind {
        ExprKind::Not(_) => Some(actual),
        ExprKind::Binary { operator, .. }
            if matches!(operator.as_str(), ">=" | "<=" | "&&" | "||") =>
        {
            Some(actual)
        }
        _ => None,
    }
}

fn check_text(check: &Check, checks: Checks) -> String {
    if matches!(check.predicate.as_str(), "exists" | "nonempty") {
        return format!("{} {}", check.predicate, expression(&check.actual));
    }
    if checks == Checks::Short
        && let Some(alone) = written_alone(check)
    {
        return expression(alone);
    }
    let expected = check
        .expected
        .as_ref()
        .map_or_else(|| "?".into(), check_operand);
    let mut text = format!(
        "{} {} {expected}",
        check_operand(&check.actual),
        check.predicate
    );
    if let Some(tolerance) = &check.tolerance {
        text.push_str(&format!(" within {}", expression(tolerance)));
    }
    text
}

pub fn expression(value: &Expr) -> String {
    print_expr(value, 0)
}

fn print_expr(expr: &Expr, parent_power: u8) -> String {
    match &expr.kind {
        ExprKind::Literal(serde_json::Value::String(text)) => quote(text),
        ExprKind::Literal(value) => serde_json::to_string(value).expect("JSON value serializes"),
        ExprKind::Name(name) => name.clone(),
        ExprKind::Missing => "?".into(),
        ExprKind::Group(value) => format!("({})", expression(value)),
        ExprKind::Not(value) => {
            let text = format!("!{}", print_expr(value, 70));
            if parent_power > 70 {
                format!("({text})")
            } else {
                text
            }
        }
        ExprKind::Binary {
            operator,
            left,
            right,
        } => {
            let power = precedence(operator).unwrap_or(0);
            let text = format!(
                "{} {operator} {}",
                print_expr(left, power),
                print_expr(right, power + 1)
            );
            if power < parent_power {
                format!("({text})")
            } else {
                text
            }
        }
        ExprKind::Object(fields) => {
            if fields.is_empty() {
                return "{}".into();
            }
            format!(
                "{{ {} }}",
                fields
                    .iter()
                    .map(|(name, value)| format!("{}: {}", key(name), expression(value)))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        ExprKind::Array(items) => format!(
            "[{}]",
            items.iter().map(expression).collect::<Vec<_>>().join(", ")
        ),
        ExprKind::Field {
            base,
            key: field,
            index,
        } => {
            let base = print_expr(base, 80);
            if *index {
                format!("{base}[{field}]")
            } else if key(field) == *field {
                format!("{base}.{field}")
            } else {
                format!("{base}[{}]", quote(field))
            }
        }
        ExprKind::Call {
            name,
            revision,
            arguments,
        } => {
            let revision = revision.map_or(String::new(), |revision| format!("@{revision}"));
            format!(
                "{name}{revision}({})",
                arguments
                    .iter()
                    .map(|argument| {
                        let prefix = argument
                            .name
                            .as_ref()
                            .map_or(String::new(), |name| format!("{}: ", key(name)));
                        prefix + &expression(&argument.value)
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        ExprKind::Template(parts) => {
            let mut text = String::from("\"");
            for part in parts {
                match part {
                    Part::Text(value) => {
                        let quoted = quote(value);
                        text.push_str(&quoted[1..quoted.len() - 1]);
                    }
                    Part::Value(value) => {
                        text.push_str("{{");
                        text.push_str(&expression(value));
                        text.push_str("}}");
                    }
                }
            }
            text.push('"');
            text
        }
    }
}

#[cfg(test)]
mod tests {
    use super::format;
    use crate::{Document, LowerContext, lower, parse, print_folder_document};

    /// LANG-28, criterion 2: a title, a description, a string value and a
    /// key made of any 1 to 5 of these characters print to a text that reads
    /// back to the same model, and the formatter writes that text as it is.
    #[test]
    fn braces_in_a_text_are_written_so_that_they_read_back() {
        let project = apiforge_core::demos::shop_project();
        let read = |source: &str| {
            let lowered = lower(&parse(source), &LowerContext::folder(&project));
            assert!(!lowered.has_errors(), "{source}\n{:?}", lowered.diagnostics);
            let Some(Document::Scenario(scenario)) = lowered.document else {
                panic!("a scenario")
            };
            scenario
        };
        let base = read("scenario \"S\" {\n  begin = start\n  a = GET \"/a\"\n}\n");
        let mut texts = vec![String::new()];
        let mut failed = vec![];
        for _ in 0..5 {
            texts = texts
                .iter()
                .flat_map(|text| ['{', '}', '\\', '"'].map(|c| format!("{text}{c}")))
                .collect();
            for text in &texts {
                let mut scenario = base.clone();
                scenario.name = text.clone();
                scenario.description = text.clone();
                scenario
                    .parameters
                    .insert("value".into(), text.as_str().into());
                scenario
                    .parameters
                    .insert("keyed".into(), serde_json::json!({ text.as_str(): text }));
                let document = Document::Scenario(scenario.clone());
                let Ok(printed) = print_folder_document(&project, &document) else {
                    failed.push(text.clone());
                    continue;
                };
                let back = read(&printed);
                assert_eq!(back.name, *text);
                assert_eq!(back.description, *text);
                assert_eq!(back.parameters, scenario.parameters);
                assert_eq!(format(&printed).unwrap(), printed);
            }
        }
        assert_eq!(failed, Vec::<String>::new());
    }
}
