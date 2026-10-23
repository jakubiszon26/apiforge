//! Handles: the names blocks have in scenario code ([ADR 0004], [ADR 0005]).
//!
//! A handle is an identifier `[A-Za-z_][A-Za-z0-9_]*` of at most 64 characters that
//! is not a reserved word of the scenario language. Handles are unique within a
//! scenario's main and cleanup graphs together, or within a component graph.
//!
//! [ADR 0004]: ../../../docs/decisions/0004-document-format-v2.md
//! [ADR 0005]: ../../../docs/decisions/0005-scenario-language.md

use crate::format::Graph;
use std::collections::HashSet;

pub const MAX_HANDLE_LENGTH: usize = 64;

/// Words the scenario language reserves, including HTTP methods, which start a
/// request statement.
pub const RESERVED_WORDS: &[&str] = &[
    "DELETE",
    "GET",
    "HEAD",
    "OPTIONS",
    "PATCH",
    "POST",
    "PUT",
    "all",
    "any",
    "assert",
    "cleanup",
    "component",
    "condition",
    "contains",
    "else",
    "env",
    "exists",
    "export",
    "false",
    "if",
    "input",
    "is",
    "nonempty",
    "null",
    "param",
    "scenario",
    "start",
    "true",
    "var",
    "with",
    "within",
];

/// Handle used when a label folds to nothing.
const FALLBACK: &str = "step";

pub fn is_reserved(word: &str) -> bool {
    RESERVED_WORDS.contains(&word)
}

pub fn is_valid_handle(text: &str) -> bool {
    let mut characters = text.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    text.len() <= MAX_HANDLE_LENGTH
        && (first.is_ascii_alphabetic() || first == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
        && !is_reserved(text)
}

/// A valid handle derived from a label, before de-duplication: lower snake case
/// with diacritics folded, e.g. `Cena po promocji` → `cena_po_promocji`.
///
/// A reserved word gets a `_step` suffix and a leading digit a `_` prefix.
pub fn handle_base(label: &str) -> String {
    let mut folded = String::new();
    for character in label.chars() {
        // Letters without a canonical decomposition, frequent in European names.
        let replacement = match character {
            'ł' => Some("l"),
            'Ł' => Some("L"),
            'ß' => Some("ss"),
            'ø' => Some("o"),
            'Ø' => Some("O"),
            'đ' => Some("d"),
            'Đ' => Some("D"),
            'æ' => Some("ae"),
            'Æ' => Some("AE"),
            'œ' => Some("oe"),
            'Œ' => Some("OE"),
            _ => None,
        };
        match replacement {
            Some(text) => folded.push_str(text),
            None => crate::edit::push_identifier_part(character, &mut folded),
        }
    }
    let mut handle = String::with_capacity(folded.len());
    for character in folded.chars() {
        let character = character.to_ascii_lowercase();
        // Collapse separators; folded text is ASCII.
        if character == '_' && (handle.is_empty() || handle.ends_with('_')) {
            continue;
        }
        handle.push(character);
    }
    while handle.ends_with('_') {
        handle.pop();
    }
    if handle.is_empty() {
        handle.push_str(FALLBACK);
    }
    if handle.starts_with(|first: char| first.is_ascii_digit()) {
        handle.insert(0, '_');
    }
    handle.truncate(MAX_HANDLE_LENGTH);
    if is_reserved(&handle) {
        handle.push_str("_step");
    }
    handle
}

/// `base`, or `base_2`, `base_3`, … – the first one `taken` does not report. The
/// base is shortened when the suffix would exceed the length limit.
pub fn unique_handle(base: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) && is_valid_handle(base) {
        return base.to_owned();
    }
    (2..)
        .map(|number| {
            let suffix = format!("_{number}");
            let mut stem = base.to_owned();
            stem.truncate(MAX_HANDLE_LENGTH - suffix.len());
            stem + &suffix
        })
        .find(|candidate| !taken(candidate) && is_valid_handle(candidate))
        .expect("an unused numbered handle exists")
}

/// Handles of every node in `graph`.
pub fn graph_handles(graph: &Graph) -> HashSet<String> {
    graph.nodes.iter().map(|node| node.handle.clone()).collect()
}

/// Renames nodes whose handle is invalid, in `reserved` or used by an earlier node,
/// so that every handle is valid and unique. Earlier nodes keep their handles.
///
/// The editor applies this after each edit with the handles of the sibling graph
/// (main or cleanup) as `reserved`, so new blocks never collide with them.
pub fn dedupe_handles(graph: &Graph, reserved: &HashSet<String>) -> Graph {
    let mut next = graph.clone();
    let mut taken: HashSet<String> = HashSet::new();
    for node in &mut next.nodes {
        let conflict = |handle: &str| reserved.contains(handle) || taken.contains(handle);
        if !is_valid_handle(&node.handle) || conflict(&node.handle) {
            let base = if is_valid_handle(&node.handle) {
                node.handle.clone()
            } else {
                handle_base(&node.handle)
            };
            node.handle = unique_handle(&base, conflict);
        }
        taken.insert(node.handle.clone());
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_are_identifiers_outside_the_reserved_words() {
        for valid in ["a", "_", "get_product", "Product2", &"x".repeat(64)] {
            assert!(is_valid_handle(valid), "{valid}");
        }
        for invalid in ["", "2x", "a-b", "zażółć", "GET", "start", &"x".repeat(65)] {
            assert!(!is_valid_handle(invalid), "{invalid}");
        }
        let mut sorted = RESERVED_WORDS.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, RESERVED_WORDS, "keep the list sorted");
    }

    #[test]
    fn labels_fold_to_lower_snake_case() {
        for (label, handle) in [
            ("Cena po promocji", "cena_po_promocji"),
            ("Zażółć gęślą jaźń", "zazolc_gesla_jazn"),
            ("  Get / product (by id) ", "get_product_by_id"),
            ("HTTP request", "http_request"),
            ("2nd step", "_2nd_step"),
            ("Start", "start_step"),
            ("GET", "get"),
            ("🦀", "step"),
            ("", "step"),
        ] {
            assert_eq!(handle_base(label), handle, "{label}");
            assert!(is_valid_handle(&handle_base(label)));
        }
        assert_eq!(handle_base(&"a".repeat(100)).len(), MAX_HANDLE_LENGTH);
    }

    #[test]
    fn unique_handles_number_collisions_within_the_length_limit() {
        let taken = ["item", "item_2"];
        assert_eq!(unique_handle("item", |h| taken.contains(&h)), "item_3");
        assert_eq!(unique_handle("fresh", |h| taken.contains(&h)), "fresh");
        let long = "x".repeat(64);
        let numbered = unique_handle(&long, |h| h == long);
        assert_eq!(numbered.len(), 64);
        assert!(numbered.ends_with("_2"));
    }

    #[test]
    fn deduplication_keeps_earlier_handles_and_respects_the_sibling_graph() {
        let node = |handle: &str| {
            let mut node = crate::graph::create_node(
                crate::model::NodeKind::Request,
                crate::format::Position { x: 0.0, y: 0.0 },
                None,
            );
            node.handle = handle.to_owned();
            node
        };
        let graph = Graph {
            nodes: vec![
                node("a"),
                node("a"),
                node("b"),
                node("not valid"),
                node("if"),
            ],
            edges: vec![],
        };
        let reserved = HashSet::from(["b".to_owned()]);
        let handles: Vec<_> = dedupe_handles(&graph, &reserved)
            .nodes
            .into_iter()
            .map(|node| node.handle)
            .collect();
        assert_eq!(handles, ["a", "a_2", "b_2", "not_valid", "if_step"]);
    }
}
