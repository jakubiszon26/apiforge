//! `{{name}}` text templates and `:name` path parameters of requests.
//!
//! A template token names a node input (an alias declared in `templateInputs` or an
//! ordinary input) or, failing that, an environment variable. The caller supplies the
//! lookup so that input precedence and secret handling stay with the runner.

use crate::js;
use serde_json::Value;
use std::ops::Range;
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TemplateError {
    #[error("missing template argument: {0}")]
    MissingArgument(String),
    #[error("missing body argument: {0}")]
    MissingBodyArgument(String),
    #[error("missing path argument: {0}")]
    MissingPathArgument(String),
}

/// A `{{ key }}` occurrence: the byte range of the whole token and the trimmed key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token<'a> {
    pub range: Range<usize>,
    pub key: &'a str,
}

/// Every token, left to right, matching `\{\{\s*([^{}]+?)\s*\}\}` of the reference
/// implementation: keys contain no braces and are trimmed.
pub fn tokens(text: &str) -> Vec<Token<'_>> {
    let mut result = Vec::new();
    let mut start = 0;
    while let Some(offset) = text[start..].find("{{") {
        let open = start + offset;
        let content_start = open + 2;
        let content_end = text[content_start..]
            .find(['{', '}'])
            .map_or(text.len(), |end| content_start + end);
        let content = &text[content_start..content_end];
        if !content.is_empty() && text[content_end..].starts_with("}}") {
            let trimmed = content.trim();
            // An all-whitespace content still matches: the lazy group takes the last
            // whitespace character after the greedy prefix backtracks.
            let key = if trimmed.is_empty() {
                let last = content.char_indices().last().map_or(0, |(index, _)| index);
                &content[last..]
            } else {
                trimmed
            };
            result.push(Token {
                range: open..content_end + 2,
                key,
            });
            start = content_end + 2;
        } else {
            start = open + 1;
        }
    }
    result
}

/// Text substituted for a template value: JSON for objects and arrays, otherwise
/// JavaScript `String(value)`.
pub fn template_text(value: &Value) -> String {
    match value {
        Value::Array(_) | Value::Object(_) => js::json_stringify(value),
        other => js::to_display_string(other),
    }
}

/// Replaces every token using `lookup`; a missing value is an error.
pub fn expand(text: &str, lookup: &dyn Fn(&str) -> Option<Value>) -> Result<String, TemplateError> {
    let mut output = String::with_capacity(text.len());
    let mut last = 0;
    for token in tokens(text) {
        output.push_str(&text[last..token.range.start]);
        let value = lookup(token.key)
            .ok_or_else(|| TemplateError::MissingArgument(token.key.to_owned()))?;
        output.push_str(&template_text(&value));
        last = token.range.end;
    }
    output.push_str(&text[last..]);
    Ok(output)
}

/// Expands templates inside a JSON body. A string that is exactly one token takes
/// the argument's JSON value and type; tokens embedded in text produce text.
pub fn expand_body(
    value: &Value,
    lookup: &dyn Fn(&str) -> Option<Value>,
) -> Result<Value, TemplateError> {
    match value {
        Value::String(text) => {
            let found = tokens(text);
            if let [token] = found.as_slice()
                && token.range == (0..text.len())
            {
                return lookup(token.key)
                    .ok_or_else(|| TemplateError::MissingBodyArgument(token.key.to_owned()));
            }
            expand(text, lookup).map(Value::String)
        }
        Value::Array(items) => items
            .iter()
            .map(|item| expand_body(item, lookup))
            .collect::<Result<_, _>>()
            .map(Value::Array),
        Value::Object(fields) => fields
            .iter()
            .map(|(key, item)| Ok((key.clone(), expand_body(item, lookup)?)))
            .collect::<Result<_, _>>()
            .map(Value::Object),
        other => Ok(other.clone()),
    }
}

/// Replaces `/:name` segments with the percent-encoded argument value.
///
/// Names start with a letter or `_` and continue with word characters or `-`, as in
/// `/users/:user-id`. Only node inputs are consulted, never the environment.
pub fn expand_path_parameters(
    path: &str,
    lookup: &dyn Fn(&str) -> Option<Value>,
) -> Result<String, TemplateError> {
    let mut output = String::with_capacity(path.len());
    let mut rest = path;
    while let Some(index) = rest.find("/:") {
        output.push_str(&rest[..index]);
        let after = &rest[index + 2..];
        let name_length = parameter_name_length(after);
        if name_length == 0 {
            output.push_str("/:");
            rest = after;
            continue;
        }
        let name = &after[..name_length];
        let value =
            lookup(name).ok_or_else(|| TemplateError::MissingPathArgument(name.to_owned()))?;
        output.push('/');
        output.push_str(&encode_uri_component(&js::to_display_string(&value)));
        rest = &after[name_length..];
    }
    output.push_str(rest);
    Ok(output)
}

fn parameter_name_length(text: &str) -> usize {
    let mut characters = text.char_indices();
    match characters.next() {
        Some((_, first)) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return 0,
    }
    characters
        .find(|(_, next)| !(next.is_ascii_alphanumeric() || matches!(next, '_' | '-')))
        .map_or(text.len(), |(index, _)| index)
}

/// JavaScript `encodeURIComponent`: everything except `A-Z a-z 0-9 - _ . ! ~ * ' ( )`
/// is percent-encoded as UTF-8.
pub fn encode_uri_component(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            output.push(byte as char);
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

/// Names referenced by tokens in `text`, in order.
pub fn token_keys(text: &str) -> impl Iterator<Item = &str> {
    tokens(text).into_iter().map(|token| token.key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn lookup(key: &str) -> Option<Value> {
        match key {
            "id" => Some(json!(123)),
            "name" => Some(json!("Zażółć & co")),
            "object" => Some(json!({"a": [1, null]})),
            "missing" => None,
            other => Some(json!(format!("<{other}>"))),
        }
    }

    #[test]
    fn tokens_match_the_reference_pattern() {
        let keys: Vec<_> = token_keys("{{ a }}{{b}} {{c d}} {{x{y}} {{}} {{  }} {{{e}}").collect();
        assert_eq!(keys, ["a", "b", "c d", " ", "e"]);
        assert!(tokens("{{ unterminated").is_empty());
    }

    #[test]
    fn expansion_uses_json_for_structures_and_text_otherwise() {
        assert_eq!(
            expand("/items/{{id}}?q={{object}}&n={{name}}", &lookup),
            Ok("/items/123?q={\"a\":[1,null]}&n=Zażółć & co".to_owned())
        );
        assert_eq!(
            expand("{{missing}}", &lookup),
            Err(TemplateError::MissingArgument("missing".into()))
        );
    }

    #[test]
    fn a_whole_body_token_keeps_its_type() {
        let body =
            json!({"amount": "{{id}}", "label": "#{{id}}", "list": ["{{object}}"], "n": null});
        assert_eq!(
            expand_body(&body, &lookup),
            Ok(json!({"amount": 123, "label": "#123", "list": [{"a": [1, null]}], "n": null}))
        );
        assert_eq!(
            expand_body(&json!("{{missing}}"), &lookup),
            Err(TemplateError::MissingBodyArgument("missing".into()))
        );
    }

    #[test]
    fn path_parameters_are_percent_encoded_segments() {
        assert_eq!(
            expand_path_parameters("/users/:name/items/:id", &lookup),
            Ok("/users/Za%C5%BC%C3%B3%C5%82%C4%87%20%26%20co/items/123".to_owned())
        );
        assert_eq!(
            expand_path_parameters("/a/:/b", &lookup),
            Ok("/a/:/b".to_owned())
        );
        assert_eq!(
            expand_path_parameters("/x/:missing", &lookup),
            Err(TemplateError::MissingPathArgument("missing".into()))
        );
    }

    #[test]
    fn uri_component_encoding_matches_javascript() {
        assert_eq!(
            encode_uri_component("a b/c?d=e&f~*'()!"),
            "a%20b%2Fc%3Fd%3De%26f~*'()!"
        );
    }
}
