//! Traversal and rewriting of argument bindings, including nested operator trees,
//! objects, arrays and templates.
//!
//! Every reference leaf, however deeply nested, is a graph dependency. Code that
//! visits, rewrites or removes references must therefore go through these helpers
//! instead of looking only at the outer binding kind.

use crate::{
    format::{Binding, TemplatePart},
    operator::Operator,
};
use indexmap::IndexMap;
use serde_json::Value;

/// A reference leaf: the producing node, its output port and the field path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reference<'a> {
    pub node_id: &'a str,
    pub port: &'a str,
    pub path: &'a str,
}

impl Binding {
    pub fn literal(value: Value) -> Self {
        Self::Literal { value }
    }

    pub fn reference(
        node_id: impl Into<String>,
        port: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        Self::Reference {
            node_id: node_id.into(),
            port: port.into(),
            path: path.into(),
        }
    }

    pub fn parameter(name: impl Into<String>) -> Self {
        Self::Parameter { name: name.into() }
    }

    pub fn environment(name: impl Into<String>) -> Self {
        Self::Environment { name: name.into() }
    }

    /// An expression whose operand slots hold the operator's catalogue defaults.
    pub fn expression(operator: Operator) -> Self {
        let inputs = operator
            .spec()
            .operands
            .iter()
            .map(|operand| {
                let value = operand
                    .default
                    .map_or(Value::Null, |default| default.to_value());
                (operand.name.to_owned(), Self::literal(value))
            })
            .collect();
        Self::Expression { operator, inputs }
    }

    /// The same expression with another operator, keeping operand slots (and their
    /// nested sources) whose names the new operator also declares.
    pub fn with_operator(&self, operator: Operator) -> Self {
        let mut next = Self::expression(operator);
        if let (Self::Expression { inputs: old, .. }, Self::Expression { inputs: new, .. }) =
            (self, &mut next)
        {
            for (name, slot) in new.iter_mut() {
                if let Some(previous) = old.get(name) {
                    *slot = previous.clone();
                }
            }
        }
        next
    }

    pub fn is_expression(&self) -> bool {
        matches!(self, Self::Expression { .. })
    }

    /// Whether the binding contains other bindings: an expression, object, array or
    /// template. Every other kind is a leaf.
    pub fn is_composite(&self) -> bool {
        matches!(
            self,
            Self::Expression { .. }
                | Self::Object { .. }
                | Self::Array { .. }
                | Self::Template { .. }
        )
    }

    /// Direct children of a composite binding with the path segment leading to each:
    /// the operand name, field name, item index or template part index.
    pub fn children(&self) -> Vec<(String, &Binding)> {
        match self {
            Self::Expression { inputs: map, .. } | Self::Object { fields: map } => map
                .iter()
                .map(|(name, child)| (name.clone(), child))
                .collect(),
            Self::Array { items } => items
                .iter()
                .enumerate()
                .map(|(index, child)| (index.to_string(), child))
                .collect(),
            Self::Template { parts } => parts
                .iter()
                .enumerate()
                .filter_map(|(index, part)| match part {
                    TemplatePart::Value { binding } => Some((index.to_string(), binding)),
                    TemplatePart::Text { .. } => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The direct child at a path segment.
    fn child(&self, segment: &str) -> Option<&Self> {
        match self {
            Self::Expression { inputs: map, .. } | Self::Object { fields: map } => map.get(segment),
            Self::Array { items } => items.get(segment.parse::<usize>().ok()?),
            Self::Template { parts } => match parts.get(segment.parse::<usize>().ok()?)? {
                TemplatePart::Value { binding } => Some(binding),
                TemplatePart::Text { .. } => None,
            },
            _ => None,
        }
    }

    /// Visits every leaf in document order, with the path (e.g. `["left", "right"]`)
    /// leading to it from the root through operands, fields, items and template parts.
    pub fn for_each_leaf<'a>(&'a self, visit: &mut impl FnMut(&'a Binding, &[String])) {
        fn walk<'a>(
            binding: &'a Binding,
            path: &mut Vec<String>,
            visit: &mut impl FnMut(&'a Binding, &[String]),
        ) {
            if binding.is_composite() {
                for (segment, child) in binding.children() {
                    path.push(segment);
                    walk(child, path, visit);
                    path.pop();
                }
            } else {
                visit(binding, path);
            }
        }
        walk(self, &mut Vec::new(), visit);
    }

    /// Every leaf with an owned path.
    pub fn leaves(&self) -> Vec<(Vec<String>, &Binding)> {
        let mut result = Vec::new();
        self.for_each_leaf(&mut |leaf, path| result.push((path.to_vec(), leaf)));
        result
    }

    /// Every reference leaf, in document order.
    pub fn references(&self) -> Vec<Reference<'_>> {
        let mut result = Vec::new();
        self.for_each_leaf(&mut |leaf, _| {
            if let Self::Reference {
                node_id,
                port,
                path,
            } = leaf
            {
                result.push(Reference {
                    node_id,
                    port,
                    path,
                });
            }
        });
        result
    }

    pub fn references_node(&self, node_id: &str) -> bool {
        self.references()
            .iter()
            .any(|reference| reference.node_id == node_id)
    }

    /// Rewrites leaves; returning `None` removes the leaf's slot (operand, field, item
    /// or template part) while the surrounding composite stays. Returns `None` when
    /// the root leaf is removed.
    pub fn map_leaves(
        &self,
        visit: &mut impl FnMut(&Binding, &[String]) -> Option<Binding>,
    ) -> Option<Self> {
        self.map_leaves_at(&mut Vec::new(), visit)
    }

    fn map_leaves_at(
        &self,
        path: &mut Vec<String>,
        visit: &mut impl FnMut(&Binding, &[String]) -> Option<Binding>,
    ) -> Option<Self> {
        let mut map_child =
            |segment: String, child: &Binding, path: &mut Vec<String>| -> Option<Binding> {
                path.push(segment);
                let next = child.map_leaves_at(path, visit);
                path.pop();
                next
            };
        match self {
            Self::Expression { operator, inputs } => {
                let mut mapped = IndexMap::with_capacity(inputs.len());
                for (name, child) in inputs {
                    if let Some(next) = map_child(name.clone(), child, path) {
                        mapped.insert(name.clone(), next);
                    }
                }
                Some(Self::Expression {
                    operator: *operator,
                    inputs: mapped,
                })
            }
            Self::Object { fields } => {
                let mut mapped = IndexMap::with_capacity(fields.len());
                for (name, child) in fields {
                    if let Some(next) = map_child(name.clone(), child, path) {
                        mapped.insert(name.clone(), next);
                    }
                }
                Some(Self::Object { fields: mapped })
            }
            Self::Array { items } => Some(Self::Array {
                items: items
                    .iter()
                    .enumerate()
                    .filter_map(|(index, child)| map_child(index.to_string(), child, path))
                    .collect(),
            }),
            Self::Template { parts } => Some(Self::Template {
                parts: parts
                    .iter()
                    .enumerate()
                    .filter_map(|(index, part)| match part {
                        TemplatePart::Text { .. } => Some(part.clone()),
                        TemplatePart::Value { binding } => {
                            map_child(index.to_string(), binding, path)
                                .map(|binding| TemplatePart::Value { binding })
                        }
                    })
                    .collect(),
            }),
            leaf => visit(leaf, path),
        }
    }

    /// Replaces the binding at a path; an empty path replaces the root. A missing
    /// operand or object field is created; array items and template parts must exist.
    ///
    /// Returns `None` when the path does not lead through composites.
    pub fn replace_at(&self, path: &[String], value: Self) -> Option<Self> {
        let Some((first, rest)) = path.split_first() else {
            return Some(value);
        };
        let replace = |child: Option<&Self>| {
            child
                .cloned()
                .unwrap_or_else(|| Self::literal(Value::Null))
                .replace_at(rest, value.clone())
        };
        let mut next = self.clone();
        match &mut next {
            Self::Expression { inputs: map, .. } | Self::Object { fields: map } => {
                let child = replace(map.get(first))?;
                map.insert(first.clone(), child);
            }
            Self::Array { items } => {
                let index = first.parse::<usize>().ok()?;
                let child = replace(Some(items.get(index)?))?;
                items[index] = child;
            }
            Self::Template { parts } => {
                let index = first.parse::<usize>().ok()?;
                let TemplatePart::Value { binding } = parts.get_mut(index)? else {
                    return None;
                };
                *binding = replace(Some(binding))?;
            }
            _ => return None,
        }
        Some(next)
    }

    /// The binding at a path, if the path exists.
    pub fn at(&self, path: &[String]) -> Option<&Self> {
        path.iter()
            .try_fold(self, |binding, segment| binding.child(segment))
    }
}

/// Stable identifier of the data wire created by a reference leaf.
///
/// Matches the reference implementation, e.g. `data:node-1:actual:left/right`.
pub fn data_edge_id(node_id: &str, input: &str, path: &[String]) -> String {
    if path.is_empty() {
        format!("data:{node_id}:{input}")
    } else {
        format!("data:{node_id}:{input}:{}", path.join("/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn nested() -> Binding {
        let mut inner = Binding::expression(Operator::Length);
        inner = inner
            .replace_at(&["left".into()], Binding::reference("a", "body", "name"))
            .unwrap();
        let mut outer = Binding::expression(Operator::Greater);
        outer = outer.replace_at(&["left".into()], inner).unwrap();
        outer
            .replace_at(&["right".into()], Binding::parameter("min"))
            .unwrap()
    }

    #[test]
    fn expression_defaults_follow_the_catalogue() {
        let Binding::Expression { inputs, .. } = Binding::expression(Operator::Round) else {
            panic!("expression expected");
        };
        assert_eq!(inputs["left"], Binding::literal(json!(0)));
        assert_eq!(inputs["right"], Binding::literal(json!(0)));
        let Binding::Expression { inputs, .. } = Binding::expression(Operator::Exists) else {
            panic!("expression expected");
        };
        assert_eq!(inputs["left"], Binding::literal(Value::Null));
    }

    #[test]
    fn leaves_and_references_include_nested_operands() {
        let binding = nested();
        let leaves = binding.leaves();
        assert_eq!(leaves.len(), 2);
        assert_eq!(leaves[0].0, vec!["left".to_string(), "left".to_string()]);
        assert_eq!(
            binding.references(),
            vec![Reference {
                node_id: "a",
                port: "body",
                path: "name"
            }]
        );
        assert!(binding.references_node("a"));
        let mut paths = Vec::new();
        binding.for_each_leaf(&mut |_, path| paths.push(path.join("/")));
        assert_eq!(paths, ["left/left", "right"]);
    }

    #[test]
    fn map_leaves_removes_slots_without_removing_the_operator() {
        let mapped = nested()
            .map_leaves(&mut |leaf, _| {
                (!matches!(leaf, Binding::Reference { .. })).then(|| leaf.clone())
            })
            .unwrap();
        let Binding::Expression { inputs, .. } = &mapped else {
            panic!("expression expected");
        };
        assert!(matches!(&inputs["left"], Binding::Expression { inputs, .. } if inputs.is_empty()));
        assert_eq!(inputs["right"], Binding::parameter("min"));
        assert_eq!(
            Binding::literal(json!(1)).map_leaves(&mut |_, _| None),
            None
        );
    }

    #[test]
    fn replace_and_read_at_operand_paths() {
        let binding = nested();
        let path = ["left".to_string(), "left".to_string()];
        assert_eq!(
            binding.at(&path),
            Some(&Binding::reference("a", "body", "name"))
        );
        let replaced = binding
            .replace_at(&path, Binding::literal(json!("x")))
            .unwrap();
        assert_eq!(replaced.at(&path), Some(&Binding::literal(json!("x"))));
        assert_eq!(
            Binding::literal(json!(1)).replace_at(&path, Binding::literal(json!(2))),
            None
        );
    }

    #[test]
    fn changing_the_operator_keeps_matching_slots() {
        let changed = nested().with_operator(Operator::Not);
        let Binding::Expression { operator, inputs } = changed else {
            panic!("expression expected");
        };
        assert_eq!(operator, Operator::Not);
        assert_eq!(inputs.len(), 1);
        assert!(inputs["left"].is_expression());
    }

    /// `{ id: a.body.id, tags: [b.body, "x"], text: "#{{c.status}}" }`
    fn composite() -> Binding {
        Binding::Object {
            fields: IndexMap::from([
                ("id".to_owned(), Binding::reference("a", "body", "id")),
                (
                    "tags".to_owned(),
                    Binding::Array {
                        items: vec![
                            Binding::reference("b", "body", ""),
                            Binding::literal(json!("x")),
                        ],
                    },
                ),
                (
                    "text".to_owned(),
                    Binding::Template {
                        parts: vec![
                            TemplatePart::Text { text: "#".into() },
                            TemplatePart::Value {
                                binding: Binding::reference("c", "status", ""),
                            },
                        ],
                    },
                ),
            ]),
        }
    }

    #[test]
    fn composites_expose_leaves_through_fields_items_and_parts() {
        let binding = composite();
        assert!(binding.is_composite());
        let paths: Vec<_> = binding
            .leaves()
            .into_iter()
            .map(|(path, _)| path.join("/"))
            .collect();
        assert_eq!(paths, ["id", "tags/0", "tags/1", "text/1"]);
        let sources: Vec<_> = binding
            .references()
            .iter()
            .map(|reference| reference.node_id)
            .collect();
        assert_eq!(sources, ["a", "b", "c"]);
        assert_eq!(
            binding.at(&["text".into(), "1".into()]),
            Some(&Binding::reference("c", "status", ""))
        );
        assert_eq!(binding.at(&["text".into(), "0".into()]), None, "text part");
        assert_eq!(binding.at(&["tags".into(), "9".into()]), None);
    }

    #[test]
    fn composites_rewrite_and_remove_slots() {
        let replaced = composite()
            .replace_at(&["tags".into(), "1".into()], Binding::parameter("p"))
            .unwrap();
        assert_eq!(
            replaced.at(&["tags".into(), "1".into()]),
            Some(&Binding::parameter("p"))
        );
        assert_eq!(
            composite().replace_at(&["tags".into(), "5".into()], Binding::parameter("p")),
            None,
            "array items must exist"
        );
        let added = composite()
            .replace_at(&["extra".into()], Binding::parameter("p"))
            .unwrap();
        assert_eq!(added.at(&["extra".into()]), Some(&Binding::parameter("p")));

        let without_references = composite()
            .map_leaves(&mut |leaf, _| {
                (!matches!(leaf, Binding::Reference { .. })).then(|| leaf.clone())
            })
            .unwrap();
        assert_eq!(
            without_references,
            Binding::Object {
                fields: IndexMap::from([
                    (
                        "tags".to_owned(),
                        Binding::Array {
                            items: vec![Binding::literal(json!("x"))]
                        }
                    ),
                    (
                        "text".to_owned(),
                        Binding::Template {
                            parts: vec![TemplatePart::Text { text: "#".into() }]
                        }
                    ),
                ])
            }
        );
    }

    #[test]
    fn data_edge_ids_match_the_reference_format() {
        assert_eq!(data_edge_id("n", "actual", &[]), "data:n:actual");
        assert_eq!(
            data_edge_id("n", "actual", &["left".into(), "right".into()]),
            "data:n:actual:left/right"
        );
    }
}
