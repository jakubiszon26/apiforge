//! Request body text is an editing projection, never a runtime interpolation
//! pass over literal JSON. Parsed references become ordinary typed bindings.
use crate::{
    format::{Binding, TemplatePart},
    template_text::{Scope, SyntaxError, parse_template, print_reference, print_template},
};
use serde_json::Value;

pub const MAX_BODY_TEXT_BYTES: usize = 1024 * 1024;

fn error(message: &str) -> SyntaxError {
    SyntaxError {
        offset: 0,
        message: message.into(),
    }
}

/// JSON strings may contain reference templates. A whole placeholder is a typed
/// binding; partial interpolation is text. `\\{{` in JSON writes literal braces.
/// A whole-body placeholder may also be entered without JSON quotes.
pub fn parse_json_body(text: &str, scope: Scope<'_>) -> Result<Binding, SyntaxError> {
    if text.len() > MAX_BODY_TEXT_BYTES {
        return Err(error("Body exceeds the 1 MiB limit"));
    }
    let json = quote_placeholders(text)?;
    let value = serde_json::from_str::<crate::strict_json::StrictValue>(&json)
        .map_err(|e| {
            error(&format!(
                "Invalid JSON body at line {}, column {}",
                e.line(),
                e.column()
            ))
        })?
        .0;
    fn convert(
        value: Value,
        scope: Scope<'_>,
        depth: usize,
        count: &mut usize,
    ) -> Result<Binding, SyntaxError> {
        *count += 1;
        if depth >= 32 || *count > 5000 {
            return Err(error("Body exceeds the binding nesting or item limit"));
        }
        Ok(match value {
            Value::Object(fields) => {
                if fields.len() > 1000 {
                    return Err(error("Too many body object fields"));
                }
                Binding::Object {
                    fields: fields
                        .into_iter()
                        .map(|(key, value)| {
                            if ["__proto__", "constructor", "prototype"].contains(&key.as_str()) {
                                return Err(error("Reserved body object key"));
                            }
                            convert(value, scope, depth + 1, count).map(|binding| (key, binding))
                        })
                        .collect::<Result<_, _>>()?,
                }
            }
            Value::Array(items) => {
                if items.len() > 1000 {
                    return Err(error("Too many body array items"));
                }
                Binding::Array {
                    items: items
                        .into_iter()
                        .map(|v| convert(v, scope, depth + 1, count))
                        .collect::<Result<_, _>>()?,
                }
            }
            Value::String(text) => {
                let binding = parse_template(&text, scope).map_err(|_| error("Invalid body reference; use input.name, block.output, param.name or env.name"))?;
                if let Binding::Template { parts } = &binding
                    && let [TemplatePart::Value { binding }] = parts.as_slice()
                {
                    binding.clone()
                } else {
                    binding
                }
            }
            value => Binding::literal(value),
        })
    }
    Ok(convert(value, scope, 0, &mut 0)?.normalized())
}

/// Returns no projection when text cannot retain an advanced expression exactly.
/// Literal braces are escaped so opening this editor never activates literal text.
pub fn print_json_body(binding: &Binding, scope: Scope<'_>) -> Option<String> {
    fn value(binding: &Binding, scope: Scope<'_>) -> Option<Value> {
        Some(match binding {
            Binding::Literal {
                value: Value::String(text),
            } => Value::String(text.replace("{{", "\\{{")),
            Binding::Literal { value } => value.clone(),
            Binding::Object { fields } => Value::Object(
                fields
                    .iter()
                    .map(|(key, binding)| value(binding, scope).map(|v| (key.clone(), v)))
                    .collect::<Option<_>>()?,
            ),
            Binding::Array { items } => Value::Array(
                items
                    .iter()
                    .map(|b| value(b, scope))
                    .collect::<Option<_>>()?,
            ),
            Binding::Template { .. } => Value::String(print_template(binding, scope)?),
            _ => Value::String(format!("{{{{{}}}}}", print_reference(binding, scope)?)),
        })
    }
    let text = serde_json::to_string_pretty(&value(&binding.normalized(), scope)?).ok()?;
    if text.len() > MAX_BODY_TEXT_BYTES {
        return None;
    }
    // A single template value forces text, whereas a whole JSON placeholder
    // preserves its type. Keep the binding controls for that distinction.
    (parse_json_body(&text, scope).ok()? == binding.normalized()).then_some(text)
}

pub fn parse_text_body(text: &str, scope: Scope<'_>) -> Result<Binding, SyntaxError> {
    if text.len() > MAX_BODY_TEXT_BYTES {
        return Err(error("Body exceeds the 1 MiB limit"));
    }
    parse_template(text, scope).map_err(|_| {
        error("Invalid body reference; use input.name, block.output, param.name or env.name")
    })
}

pub fn print_text_body(binding: &Binding, scope: Scope<'_>) -> Option<String> {
    let text = print_template(binding, scope)?;
    (text.len() <= MAX_BODY_TEXT_BYTES).then_some(text)
}

// Unquoted placeholders are accepted at JSON value positions. JSON's own parser
// still validates structure, duplicate keys and everything around the token.
fn quote_placeholders(text: &str) -> Result<String, SyntaxError> {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    let mut quoted = false;
    let mut escaped = false;
    while at < text.len() {
        let rest = &text[at..];
        if !quoted && rest.starts_with("{{") {
            let end = rest
                .find("}}")
                .ok_or_else(|| error("Unclosed body reference"))?
                + 2;
            out.push_str(&serde_json::to_string(&rest[..end]).expect("string serializes"));
            at += end;
            continue;
        }
        let c = rest.chars().next().expect("remaining character");
        out.push(c);
        at += c.len_utf8();
        if escaped {
            escaped = false;
        } else if quoted && c == '\\' {
            escaped = true;
        } else if c == '"' {
            quoted = !quoted;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{demos, format::Position, graph, model::NodeKind};
    use serde_json::json;

    #[test]
    fn body_placeholders_are_typed_bindings_in_nested_json_and_raw_text() {
        let project = demos::empty_project();
        let mut request = graph::create_node(NodeKind::Request, Position { x: 0.0, y: 0.0 }, None);
        for name in ["product", "count", "active", "nullable"] {
            request
                .inputs
                .insert(name.into(), Binding::literal(Value::Null));
        }
        let scope = Scope {
            nodes: &[],
            project: &project,
            request: Some(&request),
        };
        for source in [
            r#"{"id": "{{input.product.id}}", "count": "{{input.count}}", "active": "{{input.active}}", "nullable": "{{input.nullable}}", "label": "id-{{input.product.id}}", "items": ["{{input.product}}"]}"#,
            r#"{"id": {{input.product.id}}, "count": {{input.count}}, "active": {{input.active}}, "nullable": {{input.nullable}}, "label": "id-{{input.product.id}}", "items": [{{input.product}}]}"#,
        ] {
            let body = parse_json_body(source, scope).unwrap();
            let Binding::Object { fields } = &body else {
                panic!()
            };
            assert_eq!(
                fields["id"],
                Binding::Argument {
                    name: "product".into(),
                    path: "id".into()
                }
            );
            assert!(matches!(fields["label"], Binding::Template { .. }));
            let text = print_json_body(&body, scope).unwrap();
            assert_eq!(parse_json_body(&text, scope).unwrap(), body);
            assert_eq!(
                print_json_body(&parse_json_body(&text, scope).unwrap(), scope).unwrap(),
                text
            );
        }
        for source in ["{{input.product}}", r#""{{input.product}}""#] {
            assert_eq!(
                parse_json_body(source, scope).unwrap(),
                Binding::Argument {
                    name: "product".into(),
                    path: String::new()
                }
            );
        }
        let text = "<product>{{input.product.id}}</product>";
        let body = parse_text_body(text, scope).unwrap();
        assert_eq!(print_text_body(&body, scope).as_deref(), Some(text));
        let escaped = Binding::from_json(
            json!({"literal": "{{input.product}}", "slash": "\\{{input.product}}"}),
        );
        assert_eq!(
            parse_json_body(&print_json_body(&escaped, scope).unwrap(), scope).unwrap(),
            escaped
        );
        assert_eq!(
            parse_json_body("null", scope).unwrap(),
            Binding::literal(Value::Null)
        );
    }

    #[test]
    fn body_errors_are_bounded_private_and_do_not_activate_unknown_sources() {
        let project = demos::empty_project();
        let scope = Scope {
            nodes: &[],
            project: &project,
            request: None,
        };
        for source in [
            r#"{"id": "{{input.missing}}"}"#,
            r#"{"id": "{{canary-private-value}}"}"#,
            r#"{"id": {{input.missing}}}"#,
            r#"{"same": 1, "same": 2}"#,
            r#"{"broken": "canary-private-value""#,
            r#"{"constructor": 1}"#,
            "{{input.missing",
        ] {
            let error = parse_json_body(source, scope).unwrap_err();
            assert!(!error.message.contains("canary-private-value"));
        }
        assert!(parse_text_body("{{input.missing}}", scope).is_err());
        assert!(parse_json_body(&" ".repeat(MAX_BODY_TEXT_BYTES + 1), scope).is_err());
        assert!(parse_json_body(&format!("{}0{}", "[".repeat(33), "]".repeat(33)), scope).is_err());
        let forcing_text = Binding::Template {
            parts: vec![TemplatePart::Value {
                binding: Binding::environment("region"),
            }],
        };
        assert!(
            print_json_body(&forcing_text, scope).is_none(),
            "a forced string must not silently become a typed value"
        );
    }
}
