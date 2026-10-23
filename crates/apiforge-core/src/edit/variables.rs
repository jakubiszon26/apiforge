//! Variables dropped into text fields as `{{alias}}` template tokens.
//!
//! A dropped variable becomes a named node input (an alias declared in
//! `templateInputs`), so the dependency stays explicit in the graph and in exports.
use super::fold::push_identifier_part;
use crate::{
    format::{Binding, Node, Port},
    model::ValueType,
    template::token_keys,
};
use std::collections::HashSet;

/// A variable that can be dropped into a text field.
#[derive(Clone, Debug, PartialEq)]
pub struct VariableBlock {
    /// Name shown to the user, also the basis of the alias.
    pub label: String,
    /// A reference, parameter or environment binding. Never a value.
    pub binding: Binding,
}

/// Text of a field with the selection the token replaces.
///
/// `start` and `end` are character (Unicode scalar) offsets, as text editors report
/// cursors. Offsets past the end are clamped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextSelection<'a> {
    pub value: &'a str,
    pub start: usize,
    pub end: usize,
}

/// Input names that built-in ports of some block kind use.
const RESERVED_ALIASES: [&str; 6] = ["body", "value", "actual", "expected", "left", "right"];
const ALIAS_MAX_LENGTH: usize = 60;
const ALIAS_FALLBACK: &str = "variable";

/// The text with the selection replaced by `{{name}}`.
pub fn insert_variable_token(selection: &TextSelection<'_>, name: &str) -> String {
    let byte_offset = |offset: usize| {
        selection
            .value
            .char_indices()
            .nth(offset)
            .map_or(selection.value.len(), |(index, _)| index)
    };
    let (start, end) = (byte_offset(selection.start), byte_offset(selection.end));
    format!(
        "{}{{{{{name}}}}}{}",
        &selection.value[..start],
        &selection.value[end..]
    )
}

/// Inserts `variable` into a text field of `node` as a template token.
///
/// An input with an identical binding is reused; otherwise a new alias input is
/// declared, named after the label: diacritics are dropped, other characters
/// outside `[A-Za-z0-9_]` become `_`, the name is at most 60 characters, starts
/// with a letter or `_` (else it gets a `v_` prefix) and is made unique with `_2`,
/// `_3`, ... `write` stores the new text in the field being edited.
pub fn bind_variable_text(
    node: &Node,
    variable: &VariableBlock,
    selection: &TextSelection<'_>,
    write: impl FnOnce(Node, String) -> Node,
) -> Node {
    let existing = node
        .inputs
        .iter()
        .find(|(_, binding)| **binding == variable.binding)
        .map(|(name, _)| name.clone());
    let mut next = node.clone();
    let name = match existing {
        Some(name) => name,
        None => {
            let name = unique_alias(node, &alias_base(&variable.label));
            next.inputs.insert(name.clone(), variable.binding.clone());
            let mut port = Port::new(name.clone(), ValueType::Any)
                .labelled(variable.label.clone())
                .required(true);
            port.exposed = Some(false);
            next.config
                .template_inputs
                .get_or_insert_with(Vec::new)
                .push(port);
            name
        }
    };
    let text = insert_variable_token(selection, &name);
    write(next, text)
}

/// The alias derived from a label, before de-duplication.
fn alias_base(label: &str) -> String {
    let mut folded = String::new();
    for character in label.chars() {
        push_identifier_part(character, &mut folded);
        if folded.len() >= ALIAS_MAX_LENGTH {
            break;
        }
    }
    // Folded text is ASCII, so byte and character lengths agree.
    folded.truncate(ALIAS_MAX_LENGTH);
    if folded.is_empty() {
        folded.push_str(ALIAS_FALLBACK);
    }
    if folded.starts_with(|first: char| first.is_ascii_alphabetic() || first == '_') {
        folded
    } else {
        format!("v_{folded}")
    }
}

fn unique_alias(node: &Node, initial: &str) -> String {
    let config = &node.config;
    let declared: HashSet<&str> = RESERVED_ALIASES
        .into_iter()
        .chain(node.inputs.keys().map(String::as_str))
        .chain(config.input_ports.iter().flatten().map(|p| p.name.as_str()))
        .chain(
            config
                .template_inputs
                .iter()
                .flatten()
                .map(|p| p.name.as_str()),
        )
        .collect();
    let mut name = initial.to_owned();
    let mut suffix = 2;
    while declared.contains(name.as_str()) {
        name = format!("{initial}_{suffix}");
        suffix += 1;
    }
    name
}

/// Removes alias inputs that no template token uses any more.
///
/// Tokens are looked up in the URL, the selection path, header and query values
/// and the paths of condition predicates. Ordinary inputs are never removed.
pub fn prune_template_inputs(node: &Node) -> Node {
    let config = &node.config;
    let Some(declared) = config
        .template_inputs
        .as_ref()
        .filter(|ports| !ports.is_empty())
    else {
        return node.clone();
    };
    let texts = config
        .url
        .iter()
        .chain(&config.path)
        .chain(config.headers.iter().flat_map(|headers| headers.values()))
        .chain(config.query.iter().flat_map(|query| query.values()))
        .chain(
            config
                .predicates
                .iter()
                .flatten()
                .filter_map(|predicate| predicate.path.as_ref()),
        );
    let used: HashSet<&str> = texts.flat_map(|text| token_keys(text)).collect();
    let obsolete: HashSet<&str> = declared
        .iter()
        .map(|port| port.name.as_str())
        .filter(|name| !used.contains(name))
        .collect();
    if obsolete.is_empty() {
        return node.clone();
    }
    let mut next = node.clone();
    next.inputs
        .retain(|name, _| !obsolete.contains(name.as_str()));
    next.config.template_inputs = Some(
        declared
            .iter()
            .filter(|port| !obsolete.contains(port.name.as_str()))
            .cloned()
            .collect(),
    );
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        edit::test_support::{empty_project, node},
        format::{ConditionPredicate, Graph},
        graph::{dependencies, input_ports},
        model::{NodeKind, Predicate},
    };
    use indexmap::IndexMap;
    use serde_json::json;

    fn end(value: &str) -> TextSelection<'_> {
        let length = value.chars().count();
        TextSelection {
            value,
            start: length,
            end: length,
        }
    }

    fn write_url(mut node: Node, url: String) -> Node {
        node.config.url = Some(url);
        node
    }

    #[test]
    fn dropped_variables_reuse_bindings_avoid_collisions_and_are_pruned() {
        let mut source = node(NodeKind::Variable);
        source.label = Some("code".into());
        source
            .inputs
            .insert("value".into(), Binding::literal(json!(123)));
        let mut request = node(NodeKind::Request);
        request
            .inputs
            .insert("code".into(), Binding::literal(json!("unrelated")));
        let variable = VariableBlock {
            label: "code".into(),
            binding: Binding::reference(&source.id, "value", ""),
        };
        request = bind_variable_text(&request, &variable, &end("/items/"), write_url);
        assert_eq!(request.config.url.as_deref(), Some("/items/{{code_2}}"));
        assert_eq!(request.inputs["code_2"], variable.binding);
        let aliases = request.config.template_inputs.as_ref().unwrap();
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].label.as_deref(), Some("code"));
        assert_eq!(aliases[0].exposed, Some(false));
        assert!(
            input_ports(&request, &empty_project())
                .iter()
                .any(|port| port.name == "code_2")
        );

        let selection = TextSelection {
            value: "prefix REPLACE suffix",
            start: 7,
            end: 14,
        };
        request = bind_variable_text(&request, &variable, &selection, |mut node, text| {
            node.config.headers = Some(IndexMap::from([("X-Code".into(), text)]));
            node
        });
        assert_eq!(
            request.config.headers.as_ref().unwrap()["X-Code"],
            "prefix {{code_2}} suffix"
        );
        assert_eq!(request.config.template_inputs.as_ref().unwrap().len(), 1);

        request.config.url = Some("/items".into());
        assert!(
            prune_template_inputs(&request)
                .inputs
                .contains_key("code_2")
        );
        request.config.headers = Some(IndexMap::new());
        let cleaned = prune_template_inputs(&request);
        assert!(!cleaned.inputs.contains_key("code_2"));
        assert_eq!(cleaned.inputs["code"], Binding::literal(json!("unrelated")));
        assert_eq!(cleaned.config.template_inputs, Some(vec![]));
        let graph = Graph {
            nodes: vec![source.clone(), request],
            edges: vec![],
        };
        assert_eq!(
            dependencies(&graph, &graph.nodes[1].id),
            [source.id.as_str()]
        );
    }

    #[test]
    fn query_and_predicate_paths_keep_their_aliases() {
        let region = VariableBlock {
            label: "region".into(),
            binding: Binding::parameter("region"),
        };
        let request = bind_variable_text(
            &node(NodeKind::Request),
            &region,
            &end(""),
            |mut node, text| {
                node.config.query = Some(IndexMap::from([("region".into(), text)]));
                node
            },
        );
        assert_eq!(
            request.config.query.as_ref().unwrap()["region"],
            "{{region}}"
        );
        assert_eq!(prune_template_inputs(&request), request);

        let mut condition = node(NodeKind::Condition);
        condition = bind_variable_text(&condition, &region, &end("items."), |mut node, text| {
            node.config.predicates = Some(vec![ConditionPredicate {
                input: "actual".into(),
                expected: "expected".into(),
                predicate: Predicate::Equals,
                path: Some(text),
            }]);
            node
        });
        assert_eq!(prune_template_inputs(&condition), condition);
        condition.config.predicates = Some(vec![]);
        assert!(
            !prune_template_inputs(&condition)
                .inputs
                .contains_key("region")
        );
    }

    #[test]
    fn aliases_are_identifiers_derived_from_labels() {
        assert_eq!(alias_base("Cena brutto (zł)"), "Cena_brutto__z__");
        assert_eq!(alias_base("Zażółć"), "Zazo_c");
        assert_eq!(alias_base("1st value"), "v_1st_value");
        assert_eq!(alias_base(""), "variable");
        assert_eq!(alias_base("_id"), "_id");
        assert_eq!(alias_base(&"ą".repeat(100)), "a".repeat(60));
        assert_eq!(alias_base(&format!("{}😀", "x".repeat(59))).len(), 60);
        let mut target = node(NodeKind::Formula);
        target.config.template_inputs = Some(vec![Port::new("value_2", ValueType::Any)]);
        let block = VariableBlock {
            label: "value".into(),
            binding: Binding::environment("value"),
        };
        let bound = bind_variable_text(&target, &block, &end(""), write_url);
        assert!(bound.inputs.contains_key("value_3"));
    }

    #[test]
    fn tokens_replace_the_selection_by_character_offsets() {
        let selection = TextSelection {
            value: "żółw/x",
            start: 4,
            end: 5,
        };
        assert_eq!(insert_variable_token(&selection, "id"), "żółw{{id}}x");
        let past_end = TextSelection {
            value: "ab",
            start: 9,
            end: 9,
        };
        assert_eq!(insert_variable_token(&past_end, "id"), "ab{{id}}");
    }
}
