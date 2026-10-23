//! Text of interpolated values, URL encoding, and the version-one `{{name}}` token
//! syntax, which the development converter reads from old documents.

use crate::js;
use serde_json::Value;
use std::ops::Range;

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tokens_match_the_reference_pattern() {
        let keys: Vec<_> = tokens("{{ a }}{{b}} {{c d}} {{x{y}} {{}} {{  }} {{{e}}")
            .into_iter()
            .map(|token| token.key)
            .collect();
        assert_eq!(keys, ["a", "b", "c d", " ", "e"]);
        assert!(tokens("{{ unterminated").is_empty());
    }

    #[test]
    fn values_become_json_or_javascript_text() {
        assert_eq!(template_text(&json!({"a": [1, null]})), "{\"a\":[1,null]}");
        assert_eq!(template_text(&json!(1.5)), "1.5");
        assert_eq!(template_text(&json!(null)), "null");
        assert_eq!(template_text(&json!("Zażółć")), "Zażółć");
    }

    #[test]
    fn uri_component_encoding_matches_javascript() {
        assert_eq!(
            encode_uri_component("a b/c?d=e&f~*'()!"),
            "a%20b%2Fc%3Fd%3De%26f~*'()!"
        );
    }
}
