//! User-facing descriptions of document values.

use apiforge_core::{
    format::{Binding, Graph},
    model::HttpMethod,
};
use serde_json::Value;

pub fn method(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Head => "HEAD",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
        HttpMethod::Options => "OPTIONS",
    }
}

pub const METHODS: [HttpMethod; 7] = [
    HttpMethod::Get,
    HttpMethod::Post,
    HttpMethod::Put,
    HttpMethod::Patch,
    HttpMethod::Delete,
    HttpMethod::Head,
    HttpMethod::Options,
];

/// A short description of where a binding takes its value from.
pub fn binding(binding: &Binding, graph: &Graph) -> String {
    match binding {
        Binding::Expression { operator, .. } => format!("{} · block", operator.label()),
        Binding::Reference {
            node_id,
            port,
            path,
        } => {
            let name = graph
                .nodes
                .iter()
                .find(|node| &node.id == node_id)
                .map_or("Missing step", |node| node.name.as_str());
            if path.is_empty() {
                format!("{name} → {port}")
            } else {
                format!("{name} → {port}.{path}")
            }
        }
        Binding::Parameter { name } => format!("Argument → {name}"),
        Binding::Environment { name } => format!("Environment → {name}"),
        Binding::Literal { value } => literal_summary(value),
    }
}

/// A one-line preview of a JSON literal.
pub fn literal_summary(value: &Value) -> String {
    match value {
        Value::Object(fields) => format!("{{ {} fields }}", fields.len()),
        Value::Array(items) => format!("[ {} items ]", items.len()),
        Value::String(text) => truncate(text, 80),
        other => apiforge_core::js::json_stringify(other),
    }
}

pub fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        text.to_owned()
    } else {
        let mut short: String = text.chars().take(limit).collect();
        short.push('…');
        short
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn summaries_are_short() {
        assert_eq!(literal_summary(&json!({"a": 1})), "{ 1 fields }");
        assert_eq!(literal_summary(&json!(null)), "null");
        assert_eq!(truncate("abcdef", 3), "abc…");
    }
}
