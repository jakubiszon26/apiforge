//! Text form of templates and references, as written in fields and in scenario
//! code ([ADR 0005]): `/orders/{{order.body.id}}?region={{env.region}}`.
//!
//! A reference names a block by handle. When the block has a single output port,
//! the following segments are a path into it (`first.price`); otherwise the first
//! segment names the port (`order.status`, `order.body.id`). `env.name` and
//! `param.name.path` name environment variables and parameters. Keys that are not
//! identifiers use brackets: `env["base-url"]`, `order.body["first name"]`.
//!
//! This module covers references only. Scenario code ([ADR 0005]) extends the
//! same syntax with expressions.
//!
//! [ADR 0005]: ../../../docs/decisions/0005-scenario-language.md

use crate::{
    format::{Binding, Node, Project, TemplatePart},
    graph::output_ports,
    path::JsonPath,
};
use serde_json::Value;
use thiserror::Error;

/// Blocks a reference may name, and the project defining their ports.
#[derive(Clone, Copy)]
pub struct Scope<'a> {
    pub nodes: &'a [Node],
    pub project: &'a Project,
    /// The request whose named arguments are available as `input.name`.
    pub request: Option<&'a Node>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{message} (at character {offset})")]
pub struct SyntaxError {
    /// Character offset of the problem in the parsed text.
    pub offset: usize,
    pub message: String,
}

fn error(offset: usize, message: impl Into<String>) -> SyntaxError {
    SyntaxError {
        offset,
        message: message.into(),
    }
}

/// The text of a string literal or template, or `None` for any other binding or a
/// template with a value that is not a reference.
pub fn print_template(binding: &Binding, scope: Scope<'_>) -> Option<String> {
    match binding {
        Binding::Literal {
            value: Value::String(text),
        } => Some(escape(text)),
        Binding::Template { parts } => {
            let mut text = String::new();
            for part in parts {
                match part {
                    TemplatePart::Text { text: literal } => text.push_str(&escape(literal)),
                    TemplatePart::Value { binding } => {
                        text.push_str("{{");
                        text.push_str(&print_reference(binding, scope)?);
                        text.push_str("}}");
                    }
                }
            }
            Some(text)
        }
        _ => None,
    }
}

/// A string literal or, when the text holds `{{reference}}` tokens, a template.
/// `\{{` writes literal braces.
pub fn parse_template(text: &str, scope: Scope<'_>) -> Result<Binding, SyntaxError> {
    let characters: Vec<char> = text.chars().collect();
    let mut parts = Vec::new();
    let mut literal = String::new();
    let mut index = 0;
    while index < characters.len() {
        let at = |offset: usize| characters.get(index + offset).copied();
        if at(0) == Some('\\') && at(1) == Some('{') && at(2) == Some('{') {
            literal.push_str("{{");
            index += 3;
        } else if at(0) == Some('{') && at(1) == Some('{') {
            let start = index + 2;
            let end = (start..characters.len().saturating_sub(1))
                .find(|&position| characters[position] == '}' && characters[position + 1] == '}')
                .ok_or_else(|| error(index, "`{{` is not closed by `}}`"))?;
            let inner: String = characters[start..end].iter().collect();
            let leading = inner.chars().take_while(|c| c.is_whitespace()).count();
            let binding = parse_reference(inner.trim(), scope)
                .map_err(|problem| error(start + leading + problem.offset, problem.message))?;
            if !literal.is_empty() {
                parts.push(TemplatePart::Text {
                    text: std::mem::take(&mut literal),
                });
            }
            parts.push(TemplatePart::Value { binding });
            index = end + 2;
        } else {
            literal.push(characters[index]);
            index += 1;
        }
    }
    if parts.is_empty() {
        return Ok(Binding::literal(Value::String(literal)));
    }
    if !literal.is_empty() {
        parts.push(TemplatePart::Text { text: literal });
    }
    Ok(Binding::Template { parts })
}

fn escape(text: &str) -> String {
    text.replace("{{", "\\{{")
}

/// The reference form of a reference, parameter or environment binding.
pub fn print_reference(binding: &Binding, scope: Scope<'_>) -> Option<String> {
    let (root, port, path) = match binding {
        Binding::Argument { name, path } => (format!("input{}", key(name, true)), None, &**path),
        Binding::Environment { name } => (format!("env{}", key(name, true)), None, ""),
        Binding::Parameter { name, path } => (format!("param{}", key(name, true)), None, &**path),
        Binding::Reference {
            node_id,
            port,
            path,
        } => {
            let node = scope.nodes.iter().find(|node| &node.id == node_id)?;
            let named_port = (output_ports(node, scope.project).len() != 1).then_some(&**port);
            (node.handle.clone(), named_port, &**path)
        }
        _ => return None,
    };
    let mut text = root;
    if let Some(port) = port {
        text.push_str(&key(port, true));
    }
    let path = JsonPath::parse(path).ok()?;
    for segment in path.segments() {
        text.push_str(&key(segment, false));
    }
    Some(text)
}

/// `.name` for identifiers, `[0]` for digit keys and `["key"]` otherwise. A port or
/// variable name is never written as an index.
fn key(name: &str, named: bool) -> String {
    let identifier = name
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if identifier {
        format!(".{name}")
    } else if !named && !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_digit()) {
        format!("[{name}]")
    } else {
        format!("[{}]", serde_json::Value::String(name.to_owned()))
    }
}

/// Parses `handle.port.path`, `env.name` or `param.name.path`.
pub fn parse_reference(text: &str, scope: Scope<'_>) -> Result<Binding, SyntaxError> {
    let (root, segments) = split(text)?;
    match root.as_str() {
        "input" => {
            let ((_, name), rest) = segments
                .split_first()
                .ok_or_else(|| error(text.len(), "Name the request argument: input.name"))?;
            if !crate::graph::is_request_argument(name)
                || scope
                    .request
                    .is_none_or(|node| !node.inputs.contains_key(name))
            {
                return Err(error(0, "Request argument does not exist"));
            }
            Ok(Binding::Argument {
                name: name.clone(),
                path: join(rest),
            })
        }
        "env" => match segments.as_slice() {
            [(_, name)] => Ok(Binding::environment(name.clone())),
            [] => Err(error(text.len(), "Name the environment variable: env.name")),
            [_, (offset, _), ..] => Err(error(*offset, "An environment variable has no fields")),
        },
        "param" => {
            let ((_, name), rest) = segments
                .split_first()
                .ok_or_else(|| error(text.len(), "Name the parameter: param.name"))?;
            Ok(Binding::parameter_field(name.clone(), join(rest)))
        }
        handle => {
            let node = scope
                .nodes
                .iter()
                .find(|node| node.handle == handle)
                .ok_or_else(|| error(0, format!("No block is named `{handle}`")))?;
            let ports = output_ports(node, scope.project);
            let (port, rest) = match ports.as_slice() {
                [only] => (only.name.clone(), segments.as_slice()),
                _ => {
                    let ((offset, port), rest) = segments.split_first().ok_or_else(|| {
                        let names: Vec<_> = ports.iter().map(|port| port.name.as_str()).collect();
                        error(
                            text.len(),
                            format!("Choose an output: {}", names.join(", ")),
                        )
                    })?;
                    if !ports.iter().any(|candidate| &candidate.name == port) {
                        return Err(error(*offset, format!("`{handle}` has no output `{port}`")));
                    }
                    (port.clone(), rest)
                }
            };
            Ok(Binding::reference(node.id.clone(), port, join(rest)))
        }
    }
}

fn join(segments: &[(usize, String)]) -> String {
    let mut path = JsonPath::root();
    for (_, segment) in segments {
        path.push_key(segment.clone());
    }
    path.to_string()
}

/// The root identifier and the keys after it, each with its character offset.
fn split(text: &str) -> Result<(String, Vec<(usize, String)>), SyntaxError> {
    let characters: Vec<char> = text.chars().collect();
    let identifier = |start: usize| {
        let mut end = start;
        while end < characters.len()
            && (characters[end].is_ascii_alphanumeric() || characters[end] == '_')
        {
            end += 1;
        }
        end
    };
    let end = identifier(0);
    if end == 0 || characters[0].is_ascii_digit() {
        return Err(error(0, "Expected a block, env or param"));
    }
    let root: String = characters[..end].iter().collect();
    let mut segments = Vec::new();
    let mut index = end;
    while index < characters.len() {
        match characters[index] {
            '.' => {
                let stop = identifier(index + 1);
                if stop == index + 1 {
                    return Err(error(index + 1, "Expected a name after `.`"));
                }
                segments.push((index + 1, characters[index + 1..stop].iter().collect()));
                index = stop;
            }
            '[' => {
                let close = characters[index..]
                    .iter()
                    .position(|&c| c == ']')
                    .map(|offset| index + offset)
                    .ok_or_else(|| error(index, "`[` is not closed by `]`"))?;
                let inner: String = characters[index + 1..close].iter().collect();
                let key = if inner.starts_with('"') {
                    serde_json::from_str::<String>(&inner)
                        .map_err(|_| error(index + 1, "Invalid quoted key"))?
                } else if !inner.is_empty() && inner.bytes().all(|byte| byte.is_ascii_digit()) {
                    inner
                } else {
                    return Err(error(index + 1, "Use a number or a quoted key in brackets"));
                };
                segments.push((index + 1, key));
                index = close + 1;
            }
            _ => return Err(error(index, "Expected `.` or `[`")),
        }
    }
    Ok((root, segments))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{block::Block, format::Position};
    use serde_json::json;

    fn nodes() -> Vec<Node> {
        let at = Position { x: 0.0, y: 0.0 };
        let mut order = Block::Request.create(at.clone(), None);
        order.id = "n-order".into();
        order.handle = "order".into();
        let mut first = Block::Value.create(at, None);
        first.id = "n-first".into();
        first.handle = "first".into();
        vec![order, first]
    }

    fn scope<'a>(nodes: &'a [Node], project: &'a Project) -> Scope<'a> {
        Scope {
            nodes,
            project,
            request: None,
        }
    }

    #[test]
    fn references_name_ports_only_when_a_block_has_several() {
        let (nodes, project) = (nodes(), crate::demos::empty_project());
        let scope = scope(&nodes, &project);
        for (text, binding) in [
            ("order.body.id", Binding::reference("n-order", "body", "id")),
            ("order.status", Binding::reference("n-order", "status", "")),
            (
                "first.items[0].price",
                Binding::reference("n-first", "value", "items[0].price"),
            ),
            (
                r#"first["first name"]"#,
                Binding::reference("n-first", "value", r#"["first name"]"#),
            ),
            ("env.region", Binding::environment("region")),
            (r#"env["base-url"]"#, Binding::environment("base-url")),
            (
                "param.product.id",
                Binding::parameter_field("product", "id"),
            ),
            ("param.count", Binding::parameter("count")),
        ] {
            assert_eq!(parse_reference(text, scope), Ok(binding.clone()), "{text}");
            assert_eq!(
                print_reference(&binding, scope).as_deref(),
                Some(text),
                "{text}"
            );
        }
    }

    #[test]
    fn invalid_references_point_at_the_problem() {
        let (nodes, project) = (nodes(), crate::demos::empty_project());
        let scope = scope(&nodes, &project);
        for (text, offset, message) in [
            ("missing.x", 0, "No block is named `missing`"),
            (
                "order",
                5,
                "Choose an output: body, status, headers, duration",
            ),
            ("order.nope", 6, "`order` has no output `nope`"),
            ("order.body.", 11, "Expected a name after `.`"),
            ("first[x]", 6, "Use a number or a quoted key in brackets"),
            ("env.a.b", 6, "An environment variable has no fields"),
        ] {
            assert_eq!(
                parse_reference(text, scope),
                Err(error(offset, message)),
                "{text}"
            );
        }
    }

    #[test]
    fn templates_round_trip_and_plain_text_stays_a_literal() {
        let (nodes, project) = (nodes(), crate::demos::empty_project());
        let scope = scope(&nodes, &project);
        let text = "/orders/{{order.body.id}}?region={{ env.region }}&raw=\\{{x}}";
        let binding = parse_template(text, scope).unwrap();
        assert_eq!(
            binding,
            Binding::Template {
                parts: vec![
                    TemplatePart::Text {
                        text: "/orders/".into()
                    },
                    TemplatePart::Value {
                        binding: Binding::reference("n-order", "body", "id")
                    },
                    TemplatePart::Text {
                        text: "?region=".into()
                    },
                    TemplatePart::Value {
                        binding: Binding::environment("region")
                    },
                    TemplatePart::Text {
                        text: "&raw={{x}}".into()
                    },
                ]
            }
        );
        assert_eq!(
            print_template(&binding, scope).as_deref(),
            Some("/orders/{{order.body.id}}?region={{env.region}}&raw=\\{{x}}")
        );
        assert_eq!(
            parse_template("/plain", scope),
            Ok(Binding::literal(json!("/plain")))
        );
        assert_eq!(
            parse_template("a {{order.body", scope),
            Err(error(2, "`{{` is not closed by `}}`"))
        );
        assert_eq!(parse_template("a {{ nope }}", scope).unwrap_err().offset, 5);
        assert_eq!(print_template(&Binding::literal(json!(1)), scope), None);
    }
}

#[cfg(test)]
mod request_argument_tests {
    use super::*;
    #[test]
    fn local_arguments_round_trip_in_templates_and_validate_their_scope() {
        let project = crate::demos::empty_project();
        let mut request = crate::graph::create_node(
            crate::model::NodeKind::Request,
            crate::format::Position { x: 0.0, y: 0.0 },
            None,
        );
        request
            .inputs
            .insert("product".into(), Binding::literal(serde_json::json!(null)));
        let scope = Scope {
            nodes: &[],
            project: &project,
            request: Some(&request),
        };
        let text = "/products/{{input.product.id}}";
        let binding = parse_template(text, scope).unwrap();
        assert_eq!(print_template(&binding, scope).as_deref(), Some(text));
        assert!(parse_template("{{input.absent}}", scope).is_err());
        assert!(parse_template("{{input.body}}", scope).is_err());
        assert!(
            parse_template(
                text,
                Scope {
                    request: None,
                    ..scope
                }
            )
            .is_err()
        );
    }
}
