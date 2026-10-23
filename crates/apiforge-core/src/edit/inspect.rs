//! Field paths offered by the response pickers of the inspector.
//!
//! Every offered path uses the canonical [`JsonPath`] syntax and resolves back to the
//! listed value with [`crate::path::resolve`]. Keys the runner refuses to read
//! (`__proto__`, `constructor`, `prototype`) are never offered.
use crate::{js, path::JsonPath};
use serde_json::Value;

/// Maximum number of fields listed for one response.
pub const RESPONSE_FIELD_LIMIT: usize = 1000;
/// Nesting depth below which fields are no longer listed.
pub const RESPONSE_FIELD_DEPTH: usize = 12;
const PREVIEW_UTF16_LENGTH: usize = 160;
const SAMPLE_DEPTH: usize = 6;
const SAMPLE_ENTRIES_PER_LEVEL: usize = 50;
const SAMPLE_PATHS_PER_LEVEL: usize = 150;
const RESERVED_KEYS: [&str; 3] = ["__proto__", "constructor", "prototype"];

/// A selectable field of a response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResponseField {
    /// Canonical path; empty for the whole value.
    pub path: String,
    /// JSON type name, with `null` and `array` distinguished.
    pub value_type: &'static str,
    /// `Object · 3` or `List · 3` for containers, otherwise the start of the JSON text.
    pub preview: String,
    pub depth: usize,
}

fn supported_key(key: &str) -> bool {
    !RESERVED_KEYS.contains(&key)
}

/// Child entries of a container with their paths, in document order.
fn children<'a>(
    value: &'a Value,
    parent: &'a JsonPath,
) -> Box<dyn Iterator<Item = (JsonPath, &'a Value)> + 'a> {
    match value {
        Value::Array(items) => Box::new(items.iter().enumerate().map(move |(index, item)| {
            let mut path = parent.clone();
            path.push_index(index);
            (path, item)
        })),
        Value::Object(fields) => Box::new(fields.iter().filter(|(key, _)| supported_key(key)).map(
            move |(key, item)| {
                let mut path = parent.clone();
                path.push_key(key.clone());
                (path, item)
            },
        )),
        _ => Box::new(std::iter::empty()),
    }
}

/// JSON text cut to at most 160 UTF-16 units, never inside a character.
fn preview(value: &Value) -> String {
    match value {
        Value::Array(items) => format!("List · {}", items.len()),
        Value::Object(fields) => format!("Object · {}", fields.len()),
        scalar => {
            let text = js::json_stringify(scalar);
            let mut units = 0;
            let end = text
                .char_indices()
                .find(|(_, character)| {
                    units += character.len_utf16();
                    units > PREVIEW_UTF16_LENGTH
                })
                .map_or(text.len(), |(index, _)| index);
            text[..end].to_owned()
        }
    }
}

/// Every field of a response, depth first, starting with the whole value.
///
/// Lists at most [`RESPONSE_FIELD_LIMIT`] fields and does not descend below
/// [`RESPONSE_FIELD_DEPTH`]. A missing response has no fields.
pub fn collect_response_fields(value: Option<&Value>) -> Vec<ResponseField> {
    collect_response_fields_bounded(value).fields
}

/// A bounded picker list, including whether any readable fields were omitted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResponseFields {
    pub fields: Vec<ResponseField>,
    pub truncated: bool,
}

pub fn collect_response_fields_bounded(value: Option<&Value>) -> ResponseFields {
    fn visit(value: &Value, path: &JsonPath, depth: usize, out: &mut ResponseFields) {
        if out.fields.len() >= RESPONSE_FIELD_LIMIT {
            out.truncated = true;
            return;
        }
        out.fields.push(ResponseField {
            path: path.to_string(),
            value_type: js::type_name(Some(value)),
            preview: preview(value),
            depth,
        });
        let mut children = children(value, path);
        if depth >= RESPONSE_FIELD_DEPTH {
            out.truncated |= children.next().is_some();
            return;
        }
        for (child_path, child) in children {
            if out.fields.len() >= RESPONSE_FIELD_LIMIT {
                out.truncated = true;
                break;
            }
            visit(child, &child_path, depth + 1, out);
        }
    }
    let mut out = ResponseFields {
        fields: Vec::new(),
        truncated: false,
    };
    if let Some(value) = value {
        visit(value, &JsonPath::root(), 0, &mut out);
    }
    out
}

/// Paths suggested for a sample value, depth first, without the root.
///
/// Each level considers its first 50 entries and contributes at most 150 paths;
/// nesting below six levels is not explored.
pub fn sample_paths(value: Option<&Value>) -> Vec<String> {
    fn visit(value: &Value, path: &JsonPath, depth: usize) -> Vec<String> {
        if depth > SAMPLE_DEPTH {
            return vec![];
        }
        let entries = match value {
            Value::Array(items) => items.len(),
            Value::Object(fields) => fields.len(),
            _ => return vec![],
        };
        let mut paths = Vec::new();
        // Reserved keys still count towards the per-level entry limit.
        let considered = entries.min(SAMPLE_ENTRIES_PER_LEVEL);
        let skipped_before = |limit: usize| match value {
            Value::Object(fields) => fields
                .keys()
                .take(limit)
                .filter(|key| !supported_key(key))
                .count(),
            _ => 0,
        };
        let take = considered - skipped_before(considered);
        for (child_path, child) in children(value, path).take(take) {
            paths.push(child_path.to_string());
            paths.extend(visit(child, &child_path, depth + 1));
            if paths.len() >= SAMPLE_PATHS_PER_LEVEL {
                break;
            }
        }
        paths.truncate(SAMPLE_PATHS_PER_LEVEL);
        paths
    }
    value.map_or_else(Vec::new, |value| visit(value, &JsonPath::root(), 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::resolve;
    use serde_json::json;

    fn resolves(value: &Value, path: &str) -> Option<Value> {
        resolve(value, path).map(|found| found.into_owned())
    }

    #[test]
    fn offered_paths_resolve_through_the_runner_lookup() {
        let value = json!({"data": {"items": [{"id": 0, "label": "value"}]}, "empty": null});
        let paths = sample_paths(Some(&value));
        assert_eq!(
            paths,
            [
                "data",
                "data.items",
                "data.items[0]",
                "data.items[0].id",
                "data.items[0].label",
                "empty",
            ]
        );
        for path in &paths {
            assert!(resolves(&value, path).is_some(), "{path}");
        }
        assert!(sample_paths(None).is_empty());
        assert!(sample_paths(Some(&json!(1))).is_empty());
    }

    #[test]
    fn quoted_paths_distinguish_dots_brackets_empty_keys_and_unicode() {
        let value = json!({
            "a.b": {"id": "literal"},
            "a": {"b": "nested"},
            "items[0]": 123,
            "": false,
            "zażółć": 42,
            "constructor": {"id": 1},
        });
        let paths = sample_paths(Some(&value));
        for expected in [
            r#"["a.b"].id"#,
            r#"["items[0]"]"#,
            r#"[""]"#,
            r#"["zażółć"]"#,
        ] {
            assert!(paths.iter().any(|path| path == expected), "{expected}");
        }
        assert_eq!(resolves(&value, "a.b"), Some(json!("nested")));
        assert_eq!(resolves(&value, r#"["a.b"].id"#), Some(json!("literal")));
        assert_eq!(resolves(&value, r#"[""]"#), Some(json!(false)));
        assert_eq!(resolves(&value, r#"["items[0]"]"#), Some(json!(123)));
        for path in &paths {
            assert!(resolves(&value, path).is_some(), "{path}");
        }
        assert!(!paths.iter().any(|path| path.contains("constructor")));
    }

    #[test]
    fn sample_paths_respect_the_per_level_limits() {
        let wide: serde_json::Map<String, Value> = (0..60)
            .map(|index| (format!("k{index}"), json!(index)))
            .collect();
        let paths = sample_paths(Some(&Value::Object(wide)));
        assert_eq!(paths.len(), 50);
        let nested = json!((0..50).map(|_| json!([1, 2, 3, 4])).collect::<Vec<_>>());
        assert_eq!(sample_paths(Some(&nested)).len(), 150);
        let mut deep = json!(1);
        for _ in 0..10 {
            deep = json!({"x": deep});
        }
        assert_eq!(sample_paths(Some(&deep)).len(), 7);
        let reserved = json!({"constructor": 1, "a": 1});
        assert_eq!(sample_paths(Some(&reserved)), ["a"]);
    }

    #[test]
    fn response_fields_keep_null_false_zero_arrays_and_unusual_keys() {
        let sample = json!({
            "a.b": [{"count": 0, "active": false, "empty": null, "other name": "text", "": 42}],
            "constructor": "reserved",
        });
        let fields = collect_response_fields(Some(&sample));
        assert_eq!(fields[0].path, "");
        assert_eq!(fields[0].preview, "Object · 2");
        let field = |path: &str| fields.iter().find(|field| field.path == path).unwrap();
        assert_eq!(field(r#"["a.b"][0].empty"#).value_type, "null");
        assert_eq!(field(r#"["a.b"][0].active"#).preview, "false");
        assert_eq!(field(r#"["a.b"][0].count"#).preview, "0");
        assert_eq!(field(r#"["a.b"]"#).preview, "List · 1");
        assert_eq!(field(r#"["a.b"][0]["other name"]"#).preview, "\"text\"");
        assert_eq!(field(r#"["a.b"][0][""]"#).depth, 3);
        assert!(
            fields
                .iter()
                .any(|field| resolves(&sample, &field.path) == Some(json!(42)))
        );
        assert!(!fields.iter().any(|field| field.path == "constructor"));
        for field in &fields {
            assert!(resolves(&sample, &field.path).is_some(), "{}", field.path);
        }
        assert!(collect_response_fields(None).is_empty());
        assert_eq!(
            collect_response_fields(Some(&Value::Null))[0].preview,
            "null"
        );
        let long = json!(vec![1; 2000]);
        assert_eq!(collect_response_fields(Some(&long)).len(), 1000);
    }

    #[test]
    fn previews_and_depth_are_bounded() {
        let text = json!("ż".repeat(200));
        let preview = &collect_response_fields(Some(&text))[0].preview;
        assert_eq!(js::utf16_len(preview), 160);
        let emoji = json!("😀".repeat(100));
        assert_eq!(
            js::utf16_len(&collect_response_fields(Some(&emoji))[0].preview),
            159
        );
        let mut deep = json!(1);
        for _ in 0..20 {
            deep = json!([deep]);
        }
        let fields = collect_response_fields(Some(&deep));
        assert_eq!(fields.len(), 13);
        assert_eq!(fields.last().unwrap().depth, 12);
    }
    #[test]
    fn bounded_collection_reports_real_truncation_without_false_positives() {
        let exact = Value::Array(vec![Value::Null; RESPONSE_FIELD_LIMIT - 1]);
        let list = collect_response_fields_bounded(Some(&exact));
        assert_eq!(list.fields.len(), RESPONSE_FIELD_LIMIT);
        assert!(!list.truncated);
        let larger = Value::Array(vec![Value::Null; RESPONSE_FIELD_LIMIT]);
        let list = collect_response_fields_bounded(Some(&larger));
        assert_eq!(list.fields.len(), RESPONSE_FIELD_LIMIT);
        assert!(list.truncated);
        assert!(!collect_response_fields_bounded(None).truncated);
    }

    #[test]
    fn bounded_collection_reports_depth_truncation_but_not_reserved_keys() {
        let mut value = Value::Null;
        for _ in 0..RESPONSE_FIELD_DEPTH {
            value = serde_json::json!({"next":value});
        }
        assert!(!collect_response_fields_bounded(Some(&value)).truncated);
        value = serde_json::json!({"next":value});
        assert!(collect_response_fields_bounded(Some(&value)).truncated);
        let reserved = serde_json::json!({"__proto__":{"hidden":1}});
        assert!(!collect_response_fields_bounded(Some(&reserved)).truncated);
    }
}
