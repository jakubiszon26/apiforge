//! The braces a statement stands in and where an `if` ends, read from the
//! ordering edges of one graph. Lowering, printing and the chain of the
//! written language ([`crate::graph::chain`]) share this reading, so a source
//! lowers to exactly the edges the window prints back.
//!
//! A statement stands in the braces of branch `b` of `x` when it follows
//! `x.b` and everything else it follows stands in the same braces, at any
//! depth. The end of a branch is its last steps, those no other step of the
//! same braces follows, or the branch itself when its braces hold no step; an
//! `if` among the last steps ends where its own branches end. An edge from an
//! `if` without a branch, which only lowering produces before it replaces it,
//! reads as following the whole `if`.
use crate::{
    format::Graph,
    model::{Branch, NodeKind},
};
use std::collections::{HashMap, VecDeque};

/// What a statement follows: a statement, or a branch of an `if`.
pub type Source<'g> = (&'g str, Option<Branch>);

pub struct Branches<'g> {
    parents: HashMap<&'g str, (&'g str, Branch)>,
    ends: HashMap<&'g str, Vec<Source<'g>>>,
}

impl<'g> Branches<'g> {
    pub fn new(graph: &'g Graph) -> Self {
        let kinds: HashMap<&str, NodeKind> = graph
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node.kind))
            .collect();
        let is_if = |id: &str| kinds.get(id) == Some(&NodeKind::If);
        let mut incoming: HashMap<&str, Vec<Source<'g>>> = HashMap::new();
        for edge in &graph.edges {
            incoming
                .entry(edge.target.as_str())
                .or_default()
                .push((edge.source.as_str(), edge.branch));
        }
        let order = topological(graph);

        // Sources come first, so their braces are known when a target asks.
        let mut parents: HashMap<&str, (&str, Branch)> = HashMap::new();
        let within = |parents: &HashMap<&str, (&str, Branch)>, id: &str, braces| {
            let mut current = id;
            for _ in 0..=graph.nodes.len() {
                match parents.get(current) {
                    Some(&parent) if parent == braces => return true,
                    Some(&(source, _)) => current = source,
                    None => return false,
                }
            }
            false
        };
        for &id in &order {
            let Some(edges) = incoming.get(id) else {
                continue;
            };
            let parent = edges
                .iter()
                .filter_map(|&(source, branch)| Some((source, branch?)))
                .filter(|&(source, _)| is_if(source))
                .find(|&(braces, branch)| {
                    edges.iter().all(|&(source, other)| {
                        (source, other) == (braces, Some(branch))
                            || within(&parents, source, (braces, branch))
                    })
                });
            if let Some(parent) = parent {
                parents.insert(id, parent);
            }
        }

        // Keyed by the `if` and whether the branch is `then`.
        let mut members: HashMap<(&str, bool), Vec<&str>> = HashMap::new();
        for node in &graph.nodes {
            if let Some(&(x, branch)) = parents.get(node.id.as_str()) {
                members
                    .entry((x, branch == Branch::True))
                    .or_default()
                    .push(node.id.as_str());
            }
        }
        // Inner `if`s come later in the order, so their ends are known first.
        let mut ends: HashMap<&str, Vec<Source<'g>>> = HashMap::new();
        for &id in order.iter().rev().filter(|&&id| is_if(id)) {
            let mut result: Vec<Source<'g>> = vec![];
            for branch in [Branch::True, Branch::False] {
                let steps: Vec<&str> = members
                    .get(&(id, branch == Branch::True))
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|step| is_step(kinds[step]))
                    .collect();
                let follows = |step: &str, other: &str| {
                    let sources = &incoming[step];
                    sources.contains(&(other, None))
                        || ends
                            .get(other)
                            .is_some_and(|ends| ends.iter().all(|end| sources.contains(end)))
                };
                let last: Vec<&str> = steps
                    .iter()
                    .copied()
                    .filter(|&step| {
                        !steps
                            .iter()
                            .any(|&other| other != step && follows(other, step))
                    })
                    .collect();
                if last.is_empty() {
                    result.push((id, Some(branch)));
                }
                for step in last {
                    match ends.get(step) {
                        Some(inner) => result.extend(inner.iter().copied()),
                        None => result.push((step, None)),
                    }
                }
            }
            let mut seen = Vec::with_capacity(result.len());
            result.retain(|end| {
                let new = !seen.contains(end);
                seen.push(*end);
                new
            });
            ends.insert(id, result);
        }
        Self { parents, ends }
    }

    /// The braces a statement stands in.
    pub fn parent(&self, id: &str) -> Option<(&'g str, Branch)> {
        self.parents.get(id).copied()
    }

    /// Where the `if` ends: what `@after(id)` follows.
    pub fn ends(&self, id: &str) -> Option<&[Source<'g>]> {
        self.ends.get(id).map(Vec::as_slice)
    }

    /// `sources` with the end of every `if` they contain whole written as the
    /// `if`, `(x, None)`, at the place of its first end. The larger end goes
    /// first, so an `if` ending in another is written as the outer one.
    pub fn shorten(&self, mut sources: Vec<Source<'g>>) -> Vec<Source<'g>> {
        let mut whole: Vec<(&str, &[Source<'g>])> = self
            .ends
            .iter()
            .map(|(&id, ends)| (id, ends.as_slice()))
            .filter(|(_, ends)| ends.iter().all(|end| sources.contains(end)))
            .collect();
        whole.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));
        for (id, ends) in whole {
            if !ends.iter().all(|end| sources.contains(end)) {
                continue;
            }
            let at = sources
                .iter()
                .take_while(|source| !ends.contains(source))
                .count();
            sources.retain(|source| !ends.contains(source));
            sources.insert(at, (id, None));
        }
        sources
    }
}

/// A statement that does something or routes the flow.
fn is_step(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Request | NodeKind::Component | NodeKind::If)
}

/// Nodes with every ordering source before its target, ties in document
/// order. Nodes of a cycle are left out.
fn topological(graph: &Graph) -> Vec<&str> {
    let mut waiting: HashMap<&str, usize> = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), 0))
        .collect();
    let mut targets: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in &graph.edges {
        if waiting.contains_key(edge.source.as_str())
            && let Some(count) = waiting.get_mut(edge.target.as_str())
        {
            *count += 1;
            targets
                .entry(edge.source.as_str())
                .or_default()
                .push(edge.target.as_str());
        }
    }
    let mut ready: VecDeque<&str> = graph
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .filter(|id| waiting[id] == 0)
        .collect();
    let mut order = Vec::with_capacity(graph.nodes.len());
    while let Some(id) = ready.pop_front() {
        order.push(id);
        for &target in targets.get(id).into_iter().flatten() {
            let count = waiting.get_mut(target).expect("a known target");
            *count -= 1;
            if *count == 0 {
                ready.push_back(target);
            }
        }
    }
    order
}
