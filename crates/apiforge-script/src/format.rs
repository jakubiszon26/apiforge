use crate::{
    ast::*,
    parser::{parse, precedence},
};

pub fn format(source: &str) -> Result<String, Vec<Diagnostic>> {
    format_tree(&parse(source))
}

pub fn format_tree(tree: &SyntaxTree) -> Result<String, Vec<Diagnostic>> {
    if tree.has_errors() || tree.file.is_none() {
        return Err(tree.diagnostics.clone());
    }
    let formatted = print_file(tree.file.as_ref().unwrap());
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

pub(crate) fn print_file(file: &File) -> String {
    let mut out = opening(&file.kind);
    out.push('\n');
    for (_, text) in head(file) {
        for text in text.lines() {
            line(&mut out, 1, text);
        }
    }
    statements(&mut out, &file.statements, 1);
    if let Some(cleanup) = &file.cleanup {
        line(&mut out, 1, "cleanup {");
        statements(&mut out, cleanup, 2);
        line(&mut out, 1, "}");
    }
    out.push_str("}\n");
    out
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
        attributes(&mut out, &input.attributes, 0);
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

fn attributes(out: &mut String, attributes: &[Attribute], depth: usize) {
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

pub(crate) fn statements(out: &mut String, nodes: &[Statement], depth: usize) {
    for node in nodes {
        if let Some(note) = &node.note {
            for note in note.split('\n') {
                line(out, depth, &format!("// {note}"));
            }
        }
        attributes(out, &node.attributes, depth);
        let handle = &node.handle;
        let prefix = if node.variable {
            format!("var {handle} = ")
        } else {
            format!("{handle} = ")
        };
        match &node.form {
            Form::Start => line(out, depth, &format!("{prefix}start")),
            Form::Value(value) => line(out, depth, &format!("{prefix}{}", expression(value))),
            Form::Export { value_type, value } => line(
                out,
                depth,
                &format!("export {handle}: {value_type} = {}", expression(value)),
            ),
            Form::Assert(check) => {
                line(out, depth, &format!("{prefix}assert {}", check_text(check)))
            }
            Form::If {
                check,
                then,
                otherwise,
            } => {
                line(out, depth, &format!("{prefix}if {} {{", check_text(check)));
                statements(out, then, depth + 1);
                if let Some(otherwise) = otherwise {
                    line(out, depth, "} else {");
                    statements(out, otherwise, depth + 1);
                }
                line(out, depth, "}");
            }
            Form::Condition {
                mode,
                checks,
                value,
            } => {
                line(out, depth, &format!("{prefix}condition {mode} {{"));
                for check in checks {
                    line(out, depth + 1, &format!("{},", check_text(check)));
                }
                line(
                    out,
                    depth,
                    &value.as_ref().map_or_else(
                        || "}".into(),
                        |value| format!("}} with {}", expression(value)),
                    ),
                );
            }
            Form::Request { method, url, items } => {
                let start = format!("{prefix}{method} {}", expression(url));
                if items.is_empty() {
                    line(out, depth, &start);
                    continue;
                }
                line(out, depth, &format!("{start} {{"));
                for item in items {
                    let text = match item {
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
                    };
                    line(out, depth + 1, &text);
                }
                line(out, depth, "}");
            }
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

fn check_text(check: &Check) -> String {
    if matches!(check.predicate.as_str(), "exists" | "nonempty") {
        return format!("{} {}", check.predicate, expression(&check.actual));
    }
    if let Some(alone) = written_alone(check) {
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
