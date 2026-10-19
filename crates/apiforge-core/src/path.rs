//! Field paths into JSON values, such as `items[0].price` or `["a.b"].id`.
//!
//! The syntax is that of version-one documents:
//!
//! - `name` selects an object field; names cannot contain `.`, `[` or `]`,
//! - `[123]` selects an array element (or an object field named by the number),
//! - `["any key"]` selects a field whose name is a JSON string literal,
//! - segments are separated by `.`; brackets may follow a name directly,
//! - the empty path selects the whole value.
//!
//! Resolution never coerces: a path through a missing field, `null` or a scalar is
//! missing, which is distinct from a present `null`.

use crate::js::number_to_string;
use serde_json::Value;
use std::{borrow::Cow, fmt};
use thiserror::Error;

/// Keys the reference implementation refused to read, since they address the
/// prototype chain in JavaScript. They stay unreadable for compatibility.
const RESERVED_KEYS: [&str; 3] = ["__proto__", "constructor", "prototype"];

/// A parsed path. Every segment is a property key, exactly as JavaScript would use it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JsonPath {
    segments: Vec<String>,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum PathSyntaxError {
    #[error("empty path segment at byte {0}")]
    EmptySegment(usize),
    #[error("path segment must be separated by `.` at byte {0}")]
    MissingSeparator(usize),
    #[error("malformed bracket segment at byte {0}")]
    MalformedBracket(usize),
    #[error("invalid quoted key at byte {0}")]
    InvalidQuotedKey(usize),
}

impl JsonPath {
    /// The empty path, which selects the whole value.
    pub fn root() -> Self {
        Self::default()
    }

    pub fn parse(text: &str) -> Result<Self, PathSyntaxError> {
        let mut segments = Vec::new();
        let mut index = 0;
        let mut expect_segment = true;

        while let Some(character) = text[index..].chars().next() {
            match character {
                '.' => {
                    if expect_segment {
                        return Err(PathSyntaxError::EmptySegment(index));
                    }
                    expect_segment = true;
                    index += 1;
                }
                // A bracket is accepted after a separator too: `a.[0]` equals `a[0]`.
                '[' => {
                    let (key, length) = parse_bracket(&text[index..], index)?;
                    segments.push(key);
                    index += length;
                    expect_segment = false;
                }
                ']' => return Err(PathSyntaxError::MalformedBracket(index)),
                _ => {
                    if !expect_segment {
                        return Err(PathSyntaxError::MissingSeparator(index));
                    }
                    let length = text[index..]
                        .find(['.', '[', ']'])
                        .unwrap_or(text.len() - index);
                    segments.push(text[index..index + length].to_owned());
                    index += length;
                    expect_segment = false;
                }
            }
        }
        if expect_segment && !text.is_empty() {
            return Err(PathSyntaxError::EmptySegment(index));
        }
        Ok(Self { segments })
    }

    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    pub fn is_root(&self) -> bool {
        self.segments.is_empty()
    }

    /// Appends an object field.
    pub fn push_key(&mut self, key: impl Into<String>) {
        self.segments.push(key.into());
    }

    /// Appends an array element.
    pub fn push_index(&mut self, index: usize) {
        self.segments.push(index.to_string());
    }

    /// Selects the addressed value, or `None` when any segment is missing.
    ///
    /// The result is borrowed except for the `length` of an array, which JavaScript
    /// exposes as an own property and version-one paths may therefore read.
    pub fn resolve<'a>(&self, value: &'a Value) -> Option<Cow<'a, Value>> {
        let mut current = value;
        let mut segments = self.segments.iter().peekable();
        while let Some(key) = segments.next() {
            if RESERVED_KEYS.contains(&key.as_str()) {
                return None;
            }
            match current {
                Value::Object(fields) => current = fields.get(key)?,
                Value::Array(items) if key == "length" => {
                    // A number has no fields, so `length` is only valid as the last segment.
                    return segments
                        .peek()
                        .is_none()
                        .then(|| Cow::Owned(Value::from(items.len())));
                }
                Value::Array(items) => current = items.get(array_index(key)?)?,
                _ => return None,
            }
        }
        Some(Cow::Borrowed(current))
    }
}

impl fmt::Display for JsonPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (position, key) in self.segments.iter().enumerate() {
            if array_index(key).is_some() {
                write!(formatter, "[{key}]")?;
            } else if is_identifier(key) {
                if position > 0 {
                    formatter.write_str(".")?;
                }
                formatter.write_str(key)?;
            } else {
                let quoted = serde_json::to_string(key).map_err(|_| fmt::Error)?;
                write!(formatter, "[{quoted}]")?;
            }
        }
        Ok(())
    }
}

/// Resolves `path` in `value`; a syntactically invalid path is treated as missing.
///
/// This is the lookup used during execution. Editors should call
/// [`JsonPath::parse`] to report syntax errors instead.
pub fn resolve<'a>(value: &'a Value, path: &str) -> Option<Cow<'a, Value>> {
    JsonPath::parse(path).ok()?.resolve(value)
}

/// Parses `[digits]` or `["quoted"]` at the start of `text`, returning the key and
/// the number of bytes consumed.
fn parse_bracket(text: &str, offset: usize) -> Result<(String, usize), PathSyntaxError> {
    let inner = &text[1..];
    if inner.starts_with('"') {
        let end = closing_quote(inner).ok_or(PathSyntaxError::MalformedBracket(offset))?;
        if inner.as_bytes().get(end + 1) != Some(&b']') {
            return Err(PathSyntaxError::MalformedBracket(offset));
        }
        let key = serde_json::from_str::<String>(&inner[..=end])
            .map_err(|_| PathSyntaxError::InvalidQuotedKey(offset))?;
        return Ok((key, end + 3));
    }

    let digits = inner.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || inner.as_bytes().get(digits) != Some(&b']') {
        return Err(PathSyntaxError::MalformedBracket(offset));
    }
    // JavaScript converted the digits through a number, so `[007]` addresses "7"
    // and very long indices collapse to their (unreachable) floating-point name.
    let number: f64 = inner[..digits]
        .parse()
        .expect("a run of ASCII digits parses as a double");
    Ok((number_to_string(number), digits + 2))
}

/// Byte index of the quote closing the string literal that starts `text`.
fn closing_quote(text: &str) -> Option<usize> {
    let mut escaped = false;
    for (index, character) in text.char_indices().skip(1) {
        match character {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '"' => return Some(index),
            _ => {}
        }
    }
    None
}

/// The element index named by a canonical array key such as `"0"` or `"12"`.
fn array_index(key: &str) -> Option<usize> {
    let canonical = !key.is_empty()
        && key.bytes().all(|byte| byte.is_ascii_digit())
        && (key == "0" || !key.starts_with('0'));
    canonical.then(|| key.parse().ok()).flatten()
}

fn is_identifier(key: &str) -> bool {
    let mut characters = key.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_' || first == '$')
        && characters.all(|next| next.is_ascii_alphanumeric() || matches!(next, '_' | '$' | '-'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn lookup(value: &Value, path: &str) -> Option<Value> {
        resolve(value, path).map(Cow::into_owned)
    }

    #[test]
    fn resolves_fields_elements_and_quoted_keys() {
        let value = json!({
            "a": {"b": "nested"},
            "a.b": {"id": "literal"},
            "items[0]": 123,
            "": false,
            "zażółć": "unicode",
            "list": [{"value": 42}, null],
            "7": "numeric key",
        });
        assert_eq!(lookup(&value, ""), Some(value.clone()));
        assert_eq!(lookup(&value, "a.b"), Some(json!("nested")));
        assert_eq!(lookup(&value, r#"["a.b"].id"#), Some(json!("literal")));
        assert_eq!(lookup(&value, r#"["items[0]"]"#), Some(json!(123)));
        assert_eq!(lookup(&value, r#"[""]"#), Some(json!(false)));
        assert_eq!(lookup(&value, "zażółć"), Some(json!("unicode")));
        assert_eq!(
            lookup(&value, r#"["za\u017c\u00f3\u0142\u0107"]"#),
            Some(json!("unicode"))
        );
        assert_eq!(lookup(&value, "list[0].value"), Some(json!(42)));
        assert_eq!(lookup(&value, "list.0.value"), Some(json!(42)));
        assert_eq!(lookup(&value, "list[1]"), Some(json!(null)));
        assert_eq!(lookup(&value, "[007]"), Some(json!("numeric key")));
        assert_eq!(lookup(&value, "list.[0].value"), Some(json!(42)));
    }

    #[test]
    fn missing_is_distinct_from_null() {
        let value = json!({"present": null, "list": [1]});
        assert_eq!(lookup(&value, "present"), Some(Value::Null));
        assert_eq!(lookup(&value, "absent"), None);
        assert_eq!(lookup(&value, "present.field"), None);
        assert_eq!(lookup(&value, "list[1]"), None);
        assert_eq!(lookup(&value, "list.01"), None);
        assert_eq!(lookup(&value, "list.first"), None);
        assert_eq!(lookup(&json!("text"), "length"), None);
        assert_eq!(lookup(&json!(5), "[0]"), None);
    }

    #[test]
    fn array_length_is_readable_as_in_the_reference_implementation() {
        let value = json!({"list": [1, 2, 3]});
        assert_eq!(lookup(&value, "list.length"), Some(json!(3)));
        assert_eq!(lookup(&value, r#"list["length"]"#), Some(json!(3)));
        assert_eq!(lookup(&value, "list.length.x"), None);
    }

    #[test]
    fn reserved_prototype_keys_are_never_read() {
        let value = json!({"__proto__": 1, "constructor": {"a": 1}, "prototype": 2});
        for path in ["__proto__", "constructor.a", r#"["prototype"]"#] {
            assert_eq!(lookup(&value, path), None, "{path}");
        }
    }

    #[test]
    fn rejects_malformed_paths() {
        let cases = [
            (".a", PathSyntaxError::EmptySegment(0)),
            ("a.", PathSyntaxError::EmptySegment(2)),
            ("a..b", PathSyntaxError::EmptySegment(2)),
            ("a]", PathSyntaxError::MalformedBracket(1)),
            ("a[x]", PathSyntaxError::MalformedBracket(1)),
            ("a[1", PathSyntaxError::MalformedBracket(1)),
            ("a[-1]", PathSyntaxError::MalformedBracket(1)),
            (r#"a["x"#, PathSyntaxError::MalformedBracket(1)),
            (r#"a["x"x]"#, PathSyntaxError::MalformedBracket(1)),
            ("a[0]b", PathSyntaxError::MissingSeparator(4)),
            ("a[\"\\q\"]", PathSyntaxError::InvalidQuotedKey(1)),
        ];
        for (path, error) in cases {
            assert_eq!(JsonPath::parse(path), Err(error), "{path}");
            assert_eq!(lookup(&json!({"a": [1]}), path), None, "{path}");
        }
    }

    #[test]
    fn display_round_trips_through_parse() {
        let mut path = JsonPath::root();
        path.push_key("items");
        path.push_index(0);
        path.push_key("a.b");
        path.push_key("");
        path.push_key("zażółć");
        path.push_key("price-net");
        let text = path.to_string();
        assert_eq!(text, r#"items[0]["a.b"][""]["zażółć"].price-net"#);
        assert_eq!(JsonPath::parse(&text), Ok(path));
    }
}
