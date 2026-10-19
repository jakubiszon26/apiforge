//! Traversal and rewriting of argument bindings, including nested operator trees.
//!
//! Every reference leaf, however deeply nested, is a graph dependency. Code that
//! visits, rewrites or removes references must therefore go through these helpers
//! instead of looking only at the outer binding kind.

use crate::{format::Binding, operator::Operator};
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

    /// Visits every non-expression leaf in operand order, with the operand path
    /// (e.g. `["left", "right"]`) leading to it from the root.
    pub fn for_each_leaf<'a>(&'a self, visit: &mut impl FnMut(&'a Binding, &[String])) {
        fn walk<'a>(
            binding: &'a Binding,
            path: &mut Vec<String>,
            visit: &mut impl FnMut(&'a Binding, &[String]),
        ) {
            match binding {
                Binding::Expression { inputs, .. } => {
                    for (name, child) in inputs {
                        path.push(name.clone());
                        walk(child, path, visit);
                        path.pop();
                    }
                }
                leaf => visit(leaf, path),
            }
        }
        walk(self, &mut Vec::new(), visit);
    }

    /// Every non-expression leaf with an owned operand path.
    pub fn leaves(&self) -> Vec<(Vec<String>, &Binding)> {
        let mut result = Vec::new();
        self.for_each_leaf(&mut |leaf, path| result.push((path.to_vec(), leaf)));
        result
    }

    /// Every reference leaf, in operand order.
    pub fn references(&self) -> Vec<Reference<'_>> {
        let mut result = Vec::new();
        self.collect_references(&mut result);
        result
    }

    fn collect_references<'a>(&'a self, result: &mut Vec<Reference<'a>>) {
        match self {
            Self::Reference {
                node_id,
                port,
                path,
            } => result.push(Reference {
                node_id,
                port,
                path,
            }),
            Self::Expression { inputs, .. } => {
                for child in inputs.values() {
                    child.collect_references(result);
                }
            }
            _ => {}
        }
    }

    pub fn references_node(&self, node_id: &str) -> bool {
        self.references()
            .iter()
            .any(|reference| reference.node_id == node_id)
    }

    /// Rewrites leaves; returning `None` removes the leaf's operand slot while the
    /// surrounding operator stays. Returns `None` when the root leaf is removed.
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
        match self {
            Self::Expression { operator, inputs } => {
                let mut mapped = IndexMap::with_capacity(inputs.len());
                for (name, child) in inputs {
                    path.push(name.clone());
                    if let Some(next) = child.map_leaves_at(path, visit) {
                        mapped.insert(name.clone(), next);
                    }
                    path.pop();
                }
                Some(Self::Expression {
                    operator: *operator,
                    inputs: mapped,
                })
            }
            leaf => visit(leaf, path),
        }
    }

    /// Replaces the binding at an operand path; an empty path replaces the root.
    ///
    /// Returns `None` when the path does not lead through expressions.
    pub fn replace_at(&self, path: &[String], value: Self) -> Option<Self> {
        let Some((first, rest)) = path.split_first() else {
            return Some(value);
        };
        let Self::Expression { operator, inputs } = self else {
            return None;
        };
        let mut inputs = inputs.clone();
        let child = inputs
            .get(first)
            .cloned()
            .unwrap_or_else(|| Self::literal(Value::Null));
        inputs.insert(first.clone(), child.replace_at(rest, value)?);
        Some(Self::Expression {
            operator: *operator,
            inputs,
        })
    }

    /// The binding at an operand path, if the path exists.
    pub fn at(&self, path: &[String]) -> Option<&Self> {
        path.iter().try_fold(self, |binding, name| match binding {
            Self::Expression { inputs, .. } => inputs.get(name),
            _ => None,
        })
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

    #[test]
    fn data_edge_ids_match_the_reference_format() {
        assert_eq!(data_edge_id("n", "actual", &[]), "data:n:actual");
        assert_eq!(
            data_edge_id("n", "actual", &["left".into(), "right".into()]),
            "data:n:actual:left/right"
        );
    }
}
