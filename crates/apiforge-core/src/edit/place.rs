//! Putting a step at a place on the board (DIAG-06a): the one operation the
//! folder window moves a step with, and that later edits add cases to.
//!
//! A place is directly after a step `a` — after `a`, the statements of its
//! braces and the checks and values written under it, in the braces `a`
//! stands in — or first in the braces of a branch of an `if`. The step goes
//! there with the checks, values and variables written directly under it
//! that read it and no other step, and an `if` with every statement of its
//! braces at any depth (DIAG-06d); its old neighbours join. A place in
//! other braces puts the step in those braces: into a branch, out of one,
//! from *Yes* to *No*. A check, value, variable or export is put into a
//! branch only from *Yes* or *No*, and moved out of its branch to after the
//! `if`. The result is a graph whose nodes are in the new written order and
//! whose ordering edges are one chain per braces ([`crate::chain::chain`]).
//!
//! A move is refused, naming the reader, what it reads and the step it stays
//! below, exactly when the result has a cycle through ordering and values.
//! Where a check or a value is written is no reason: data statements stand
//! anywhere in their block. An `if` does not go into its own braces.
use super::{
    EditError,
    delete::{follows_what_it_reads, reads, taken_along},
    shared::{ValueRef, value_text},
};
use crate::{
    chain::{Braces, braces, chain, is_step},
    format::{Graph, Node, Project},
    graph::{dependency_index, has_cycle, sequence},
    model::{Branch, NodeKind},
};
use std::collections::{HashMap, HashSet};

/// Where a step is put.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place<'a> {
    /// Directly after the step `a`, its braces and the checks and values
    /// written under it, in the braces `a` stands in: what a wire from
    /// `a`'s *Next* says. For a check, value, variable or export, `a` is the
    /// `if` whose braces hold it: *Move out of the branch*.
    After(&'a str),
    /// First in the braces of the branch of an `if`: what a wire from its
    /// *Yes* or *No* says.
    First(&'a str, Branch),
}

/// `graph` with the statement `step` put at `at`, with the statements that
/// move with it, or the refusal the board shows. The node identifiers of
/// what moved, the statement first, are returned beside the graph, so that
/// the text moves those statements' lines and no others.
pub fn place(
    graph: &Graph,
    project: &Project,
    step: &str,
    at: Place<'_>,
) -> Result<(Graph, Vec<String>), EditError> {
    let find = |id: &str| graph.nodes.iter().find(|node| node.id == id);
    let node = find(step).ok_or(EditError::NodeNotFound)?;
    if node.kind == NodeKind::Start {
        return Err(EditError::StartHasPredecessor);
    }
    let anchor = match at {
        Place::After(anchor) | Place::First(anchor, _) => anchor,
    };
    let anchor_node = find(anchor).ok_or(EditError::NodeNotFound)?;
    if anchor == step {
        return Err(EditError::SelfConnection);
    }
    if matches!(at, Place::First(..)) && anchor_node.kind != NodeKind::If {
        return Err(EditError::BranchRequired);
    }
    let written = braces(graph);
    let own = written.get(step).copied();
    // A check, value, variable or export is not ordered: it goes into a
    // branch from *Yes* or *No*, or out of its branch to after the `if`.
    if !is_step(node.kind) {
        let out_of_its_branch =
            matches!(at, Place::After(x) if own.is_some_and(|(parent, _)| parent == x));
        if !out_of_its_branch && !matches!(at, Place::First(..)) {
            return Err(EditError::OnlyStepsOrdered);
        }
    }
    if inside(&written, anchor, step) {
        return Err(EditError::IntoOwnBraces);
    }
    let target = match at {
        Place::After(anchor) => written.get(anchor).copied(),
        Place::First(x, branch) => Some((x, branch)),
    };

    // What goes with it: an `if`'s braces at any depth, and the checks,
    // values and variables written directly under it that read it and no
    // other step.
    let mut with: HashMap<&str, Vec<&str>> = graph
        .nodes
        .iter()
        .filter(|node| node.id == step || inside(&written, &node.id, step))
        .map(|node| (node.id.as_str(), vec![]))
        .collect();
    if is_step(node.kind) {
        let end = graph
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| with.contains_key(node.id.as_str()))
            .map(|(index, _)| index)
            .max()
            .expect("the step is in the graph");
        let under: HashSet<&str> = graph.nodes[end + 1..]
            .iter()
            .take_while(|node| node.kind.is_data() && written.get(node.id.as_str()).copied() == own)
            .filter(|node| follows_what_it_reads(node.kind))
            .map(|node| node.id.as_str())
            .collect();
        taken_along(
            graph,
            &mut with,
            |node| under.contains(node.id.as_str()),
            &constants(graph),
        );
    }
    let moving: Vec<&Node> = graph
        .nodes
        .iter()
        .filter(|node| with.contains_key(node.id.as_str()))
        .collect();
    let rest: Vec<&Node> = graph
        .nodes
        .iter()
        .filter(|node| !with.contains_key(node.id.as_str()))
        .collect();
    // The braces after the move: the statement and what is written under
    // it stand in the place's braces; what stands in an `if`'s braces stays
    // in them.
    let mut placed = written.clone();
    for node in &moving {
        let id = node.id.as_str();
        if inside(&written, id, step) {
            continue;
        }
        match target {
            Some(braces) => {
                placed.insert(id, braces);
            }
            None => {
                placed.remove(id);
            }
        }
    }

    // Where it goes among the rest.
    let at_index = match at {
        Place::After(anchor) => {
            let position = rest
                .iter()
                .position(|node| node.id == anchor)
                .expect("the anchor stays");
            let last_inside = rest
                .iter()
                .enumerate()
                .filter(|(_, node)| inside(&written, &node.id, anchor))
                .map(|(index, _)| index)
                .max()
                .unwrap_or(position)
                .max(position);
            let mut end = last_inside + 1;
            while end < rest.len()
                && rest[end].kind.is_data()
                && written.get(rest[end].id.as_str()).copied() == target
            {
                end += 1;
            }
            end
        }
        Place::First(x, branch) => match rest
            .iter()
            .position(|node| written.get(node.id.as_str()).copied() == Some((x, branch)))
        {
            Some(first) => first,
            None => {
                let position = rest
                    .iter()
                    .position(|node| node.id == x)
                    .expect("the if stays");
                if branch == Branch::True {
                    position + 1
                } else {
                    rest.iter()
                        .enumerate()
                        .filter(|(_, node)| inside(&written, &node.id, x))
                        .map(|(index, _)| index)
                        .max()
                        .unwrap_or(position)
                        .max(position)
                        + 1
                }
            }
        },
    };
    let mut ordered = graph.clone();
    ordered.nodes = rest[..at_index]
        .iter()
        .chain(&moving)
        .chain(&rest[at_index..])
        .map(|node| (*node).clone())
        .collect();
    let result = chain(&ordered, &placed);
    if has_cycle(&result) {
        return Err(EditError::PlaceRefused(cycle(&result, project)));
    }
    let moved = moving.iter().map(|node| node.id.clone()).collect();
    Ok((result, moved))
}

/// Whether the statement `id` stands in the braces of the `if` `x`, at any
/// depth.
fn inside(braces: &Braces<'_>, id: &str, x: &str) -> bool {
    let mut current = id;
    for _ in 0..=braces.len() {
        match braces.get(current) {
            Some(&(parent, _)) if parent == x => return true,
            Some(&(parent, _)) => current = parent,
            None => return false,
        }
    }
    false
}

/// The data statements that read no step, directly or through others: a
/// literal, a check of constants, a value of a parameter or of `env`. A
/// check that reads a step and one of them reads no other step.
fn constants(graph: &Graph) -> HashSet<&str> {
    let nodes: HashMap<&str, &Node> = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let mut known: HashMap<&str, bool> = HashMap::new();
    fn constant<'g>(
        id: &'g str,
        nodes: &HashMap<&'g str, &'g Node>,
        known: &mut HashMap<&'g str, bool>,
        depth: usize,
    ) -> bool {
        if let Some(&answer) = known.get(id) {
            return answer;
        }
        let answer = depth <= nodes.len()
            && nodes.get(id).is_some_and(|node| {
                node.kind.is_data()
                    && reads(node)
                        .into_iter()
                        .all(|source| constant(source, nodes, known, depth + 1))
            });
        known.insert(id, answer);
        answer
    }
    graph
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .filter(|id| constant(id, &nodes, &mut known, 0))
        .collect()
}

/// The board's sentence for a move whose result has a cycle: the first
/// step, in written order, that reads something which depends on it, what
/// it reads, and the step that something comes from. A check or a value is
/// named only when no step is: it may stand anywhere in its block, so its
/// place is never the reason. What an `if` reads cannot stand in its own
/// braces (DIAG-06d).
fn cycle(graph: &Graph, project: &Project) -> String {
    let index = dependency_index(graph);
    let written = braces(graph);
    let handles: HashMap<&str, &Node> = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    // Whether `from` depends on `on`, through ordering and values.
    let depends = |from: &str, on: &str| -> bool {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut stack = vec![from];
        while let Some(id) = stack.pop() {
            for &source in index.get(id).into_iter().flatten() {
                if source == on {
                    return true;
                }
                if seen.insert(source) {
                    stack.push(source);
                }
            }
        }
        false
    };
    let readers = graph
        .nodes
        .iter()
        .filter(|node| is_step(node.kind))
        .chain(graph.nodes.iter().filter(|node| !is_step(node.kind)));
    for reader in readers {
        for binding in reader.inputs.values() {
            for reference in binding.references() {
                let Some(read) = handles.get(reference.node_id) else {
                    continue;
                };
                if !depends(&read.id, &reader.id) {
                    continue;
                }
                // The step what it reads comes from: itself, or the first
                // step it is computed from that depends on the reader.
                let mut below = None;
                let mut seen: HashSet<&str> = HashSet::new();
                let mut stack = vec![read.id.as_str()];
                while let Some(id) = stack.pop() {
                    let Some(node) = handles.get(id) else {
                        continue;
                    };
                    if is_step(node.kind) && (id == read.id || depends(id, &reader.id)) {
                        below = Some(node.handle.as_str());
                        break;
                    }
                    for &source in index.get(id).into_iter().flatten().rev() {
                        if seen.insert(source) {
                            stack.push(source);
                        }
                    }
                }
                let value = value_text(
                    graph,
                    None,
                    project,
                    &ValueRef::new(reference.node_id, reference.port, reference.path),
                );
                if inside(&written, &read.id, &reader.id) {
                    return format!(
                        "`{}` reads `{value}`; it stays outside the braces of `{}`.",
                        reader.handle, reader.handle
                    );
                }
                if let Some(below) = below {
                    return format!(
                        "`{}` reads `{value}`; it stays below `{below}`.",
                        reader.handle
                    );
                }
            }
        }
    }
    EditError::ConnectionCycle.to_string()
}

/// The step *Add step* on the board's bar puts a new step after: with a
/// step selected, that step; with an `if`, the whole `if`; with a check or
/// value, the step it is written under; with several, the one that runs
/// last. `None` with nothing selected, or when no selected statement is
/// written under a step: the new step goes last in the main flow.
pub fn add_step_after<'g>(graph: &'g Graph, selection: &[&str]) -> Option<&'g str> {
    let written = braces(graph);
    let runs: HashMap<&str, usize> = sequence(graph)
        .into_iter()
        .enumerate()
        .map(|(rank, id)| (id, rank))
        .collect();
    let under = |id: &str| -> Option<&'g str> {
        let index = graph.nodes.iter().position(|node| node.id == id)?;
        let node = &graph.nodes[index];
        if is_step(node.kind) {
            return Some(node.id.as_str());
        }
        let own = written.get(node.id.as_str()).copied();
        for above in graph.nodes[..index].iter().rev() {
            if own.is_some_and(|(x, _)| x == above.id) {
                return None;
            }
            if written.get(above.id.as_str()).copied() != own {
                continue;
            }
            if is_step(above.kind) {
                return Some(above.id.as_str());
            }
        }
        None
    };
    selection
        .iter()
        .filter_map(|id| under(id))
        .max_by_key(|id| runs.get(id).copied().unwrap_or(0))
}

#[cfg(test)]
mod tests {
    //! DIAG-06a, criteria 1, 3, 4 and 8 on the operation. The documents are
    //! written in the language and lowered by `apiforge-script`, which links
    //! another build of this crate: they cross between the two builds as
    //! JSON. Every edit ends by printing the edited document, reading it back
    //! without an error and as the same document.
    use super::*;
    use crate::{
        chain::is_step,
        format::{Component, Node, Scenario},
        graph::sequence,
    };
    use serde::{Serialize, de::DeserializeOwned};

    /// A value of one build of this crate as the same type of the other.
    fn crossed<A: Serialize, B: DeserializeOwned>(value: &A) -> B {
        serde_json::from_value(serde_json::to_value(value).unwrap()).unwrap()
    }

    fn lowered(project: &Project, text: &str) -> apiforge_script::Document {
        let theirs = crossed(project);
        let lowered = apiforge_script::lower(
            &apiforge_script::parse(text),
            &apiforge_script::LowerContext::folder(&theirs),
        );
        assert!(!lowered.has_errors(), "{:#?}", lowered.diagnostics);
        lowered.document.expect("a document")
    }

    fn scenario(project: &Project, text: &str) -> Scenario {
        match lowered(project, text) {
            apiforge_script::Document::Scenario(scenario) => crossed(&scenario),
            other => panic!("not a scenario: {other:?}"),
        }
    }

    fn component(project: &Project, text: &str) -> Component {
        match lowered(project, text) {
            apiforge_script::Document::Component(component) => crossed(&component),
            other => panic!("not a component: {other:?}"),
        }
    }

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/scenarios/lay")
                .join(name),
        )
        .unwrap()
    }

    fn shop() -> (Project, Scenario) {
        let project = crate::demos::shop_project();
        let scenario = scenario(&project, &fixture("shop-30.afs"));
        (project, scenario)
    }

    fn small(text: &str) -> (Project, Scenario) {
        let project = super::super::test_support::empty_project();
        let scenario = scenario(&project, text);
        (project, scenario)
    }

    fn node<'g>(graph: &'g Graph, handle: &str) -> &'g Node {
        graph
            .nodes
            .iter()
            .find(|node| node.handle == handle)
            .unwrap_or_else(|| panic!("a statement `{handle}`"))
    }

    fn id(graph: &Graph, handle: &str) -> String {
        node(graph, handle).id.clone()
    }

    fn handles(graph: &Graph, ids: &[String]) -> Vec<String> {
        ids.iter()
            .map(|id| {
                graph
                    .nodes
                    .iter()
                    .find(|node| &node.id == id)
                    .unwrap()
                    .handle
                    .clone()
            })
            .collect()
    }

    /// The steps in the order they run.
    fn steps(graph: &Graph) -> Vec<String> {
        sequence(graph)
            .into_iter()
            .filter_map(|id| graph.nodes.iter().find(|node| node.id == id))
            .filter(|node| is_step(node.kind))
            .map(|node| node.handle.clone())
            .collect()
    }

    /// Every statement in written order.
    fn written(graph: &Graph) -> Vec<&str> {
        graph
            .nodes
            .iter()
            .map(|node| node.handle.as_str())
            .collect()
    }

    /// The document printed reads back without an error and as itself.
    fn reads_back(project: &Project, document: apiforge_script::Document) {
        let theirs = crossed(project);
        let text = apiforge_script::print_folder_document(&theirs, &document)
            .expect("the edited document prints");
        let read = lowered(project, &text);
        assert_eq!(
            apiforge_script::semantic(&read, &theirs),
            apiforge_script::semantic(&document, &theirs),
            "the text reads back as the document:\n{text}"
        );
    }

    fn after(graph: &Graph, handle: &str) -> Place<'static> {
        Place::After(Box::leak(id(graph, handle).into_boxed_str()))
    }

    /// `step` of the main flow put at `at`, checked by printing.
    fn moved(project: &Project, scenario: &Scenario, step: &str, at: Place<'_>) -> Scenario {
        let (graph, _) = place(&scenario.graph, project, &id(&scenario.graph, step), at)
            .unwrap_or_else(|error| panic!("`{step}` moves: {error}"));
        let mut result = scenario.clone();
        result.graph = graph;
        reads_back(
            project,
            apiforge_script::Document::Scenario(crossed(&result)),
        );
        result
    }

    fn refused(graph: &Graph, project: &Project, step: &str, at: Place<'_>) -> String {
        match place(graph, project, &id(graph, step), at) {
            Ok(_) => panic!("moving `{step}` is refused"),
            Err(error) => error.to_string(),
        }
    }

    // Criterion 1 -------------------------------------------------------------

    #[test]
    fn place_moves_a_step() {
        let (project, shop) = shop();
        // Forwards in the main flow: `address` runs directly after `cart`
        // and its checks; `cart_view`, which followed `cart`, follows it.
        let forwards = moved(&project, &shop, "address", after(&shop.graph, "cart"));
        let order = steps(&forwards.graph);
        let at = |step: &str| order.iter().position(|s| s == step).unwrap();
        assert_eq!(at("address"), at("cart") + 1, "{order:?}");
        assert_eq!(at("cart_view"), at("address") + 1, "{order:?}");
        assert_eq!(
            at("checkout"),
            at("cart_view") + 1,
            "the old neighbours join"
        );
        // Backwards: `cart_view` runs after `address`.
        let backwards = moved(&project, &shop, "cart_view", after(&shop.graph, "address"));
        let order = steps(&backwards.graph);
        let at = |step: &str| order.iter().position(|s| s == step).unwrap();
        assert_eq!(at("address"), at("cart") + 1, "the old neighbours join");
        assert_eq!(at("cart_view"), at("address") + 1, "{order:?}");
        assert_eq!(at("checkout"), at("cart_view") + 1, "{order:?}");
        // In a branch, both ways.
        let branch = moved(&project, &shop, "refund", after(&shop.graph, "payment"));
        assert_eq!(
            steps(&branch.graph)[10..],
            ["paid", "payment", "refund", "receipt", "logout"]
        );
        let branch = moved(&project, &shop, "refund", after(&shop.graph, "logout"));
        assert_eq!(
            steps(&branch.graph)[10..],
            ["paid", "payment", "receipt", "logout", "refund"]
        );
        // First in the braces of `paid`.
        let paid = id(&shop.graph, "paid");
        let first = moved(&project, &shop, "refund", Place::First(&paid, Branch::True));
        assert_eq!(
            steps(&first.graph)[10..],
            ["paid", "refund", "payment", "receipt", "logout"]
        );
    }

    const CLEANUP: &str = r#"scenario "Cleanup" {
  begin = start
  made = POST "/made" {
    sample 201 { id: 1 }
  }
  cleanup {
    a = DELETE "/a/{{made.body.id}}" {
      sample 204 {}
    }
    b = DELETE "/b" {
      sample 204 {}
    }
    b_gone = assert b.status == 204
    c = DELETE "/c" {
      sample 204 {}
    }
  }
}
"#;

    #[test]
    fn place_moves_a_step_in_the_cleanup() {
        let (project, scenario) = small(CLEANUP);
        let cleanup = &scenario.cleanup;
        for (step, anchor, expected) in [("c", "a", ["a", "c", "b"]), ("a", "b", ["b", "a", "c"])] {
            let (graph, _) = place(
                cleanup,
                &project,
                &id(cleanup, step),
                after(cleanup, anchor),
            )
            .unwrap_or_else(|error| panic!("`{step}` moves: {error}"));
            assert_eq!(steps(&graph), expected);
            let mut result = scenario.clone();
            result.cleanup = graph;
            reads_back(
                &project,
                apiforge_script::Document::Scenario(crossed(&result)),
            );
        }
    }

    const TRIO: &str = r#"component trio "Trio" {
  a = GET "/a" {
    sample 200 { id: 1 }
  }
  b = GET "/b" {
    sample 200 { id: 2 }
  }
  c = GET "/c" {
    sample 200 { id: 3 }
  }
  export id: number = a.body.id
}
"#;

    #[test]
    fn place_moves_a_step_in_a_component() {
        let project = super::super::test_support::empty_project();
        let trio = component(&project, TRIO);
        for (step, anchor, expected) in [("c", "a", ["a", "c", "b"]), ("a", "c", ["b", "c", "a"])] {
            let (graph, moved) = place(
                &trio.graph,
                &project,
                &id(&trio.graph, step),
                after(&trio.graph, anchor),
            )
            .unwrap_or_else(|error| panic!("`{step}` moves: {error}"));
            assert_eq!(steps(&graph), expected);
            assert_eq!(handles(&graph, &moved), [step]);
            let mut result = trio.clone();
            result.graph = graph;
            reads_back(
                &project,
                apiforge_script::Document::Component(crossed(&result)),
            );
        }
    }

    // Criterion 3 -------------------------------------------------------------

    #[test]
    fn a_step_moves_with_its_checks() {
        let (project, shop) = shop();
        // `cart_created` reads only `cart`; `cart_total` also reads
        // `expected_total`, a value of `promoted`, and stays where it is.
        let (graph, moved) = place(
            &shop.graph,
            &project,
            &id(&shop.graph, "cart"),
            after(&shop.graph, "catalogue"),
        )
        .expect("`cart` moves");
        assert_eq!(handles(&graph, &moved), ["cart", "cart_created"]);
        let order = written(&graph);
        let at = |handle: &str| order.iter().position(|h| *h == handle).unwrap();
        assert_eq!(
            at("cart"),
            at("second") + 1,
            "after `catalogue`'s statements"
        );
        assert_eq!(at("cart_created"), at("cart") + 1);
        assert_eq!(at("product"), at("cart_created") + 1);
        assert!(at("cart_total") > at("expected_total"), "{order:?}");
        let mut result = shop.clone();
        result.graph = graph;
        reads_back(
            &project,
            apiforge_script::Document::Scenario(crossed(&result)),
        );
    }

    #[test]
    fn a_step_moves_with_a_check_of_it_and_a_constant() {
        let (project, scenario) = small(
            r#"scenario "Constant" {
  begin = start
  limit = 30
  a = GET "/a" {
    sample 200 { n: 1 }
  }
  b = GET "/b" {
    sample 200 { n: 2 }
  }
  b_ok = assert b.status == 200
  b_fast = assert b.body.n < limit
  b_v = b.body.n
  b_both = assert b.body.n == a.body.n
}
"#,
        );
        let (graph, moved) = place(
            &scenario.graph,
            &project,
            &id(&scenario.graph, "b"),
            after(&scenario.graph, "begin"),
        )
        .expect("`b` moves");
        assert_eq!(handles(&graph, &moved), ["b", "b_ok", "b_fast", "b_v"]);
        // After *Start* and the value written under it.
        assert_eq!(
            written(&graph),
            [
                "begin", "limit", "b", "b_ok", "b_fast", "b_v", "a", "b_both"
            ]
        );
    }

    // Criterion 4 -------------------------------------------------------------

    const PAY: &str = r#"scenario "Pay" {
  begin = start
  cart = POST "/cart" {
    sample 201 { id: "c-1", total: 10 }
  }
  pay = POST "/pay/{{cart.body.id}}" {
    sample 200 { ok: true }
  }
  done = GET "/done" {
    sample 200 {}
  }
}
"#;

    #[test]
    fn place_refuses_the_step_above_what_it_reads() {
        let (project, scenario) = small(PAY);
        let graph = &scenario.graph;
        assert_eq!(
            refused(graph, &project, "pay", after(graph, "begin")),
            "`pay` reads `cart.body.id`; it stays below `cart`."
        );
    }

    #[test]
    fn place_refuses_a_step_below_a_step_that_reads_it() {
        let (project, scenario) = small(PAY);
        let graph = &scenario.graph;
        assert_eq!(
            refused(graph, &project, "cart", after(graph, "done")),
            "`pay` reads `cart.body.id`; it stays below `cart`."
        );
    }

    #[test]
    fn place_refuses_through_a_value_between_them() {
        let (project, scenario) = small(
            r#"scenario "Value" {
  begin = start
  cart = POST "/cart" {
    sample 201 { id: "c-1" }
  }
  pay = POST "/pay/{{cart_id}}" {
    sample 200 { ok: true }
  }
  cart_id = cart.body.id
}
"#,
        );
        let graph = &scenario.graph;
        assert_eq!(
            refused(graph, &project, "pay", after(graph, "begin")),
            "`pay` reads `cart_id`; it stays below `cart`."
        );
    }

    #[test]
    fn place_refuses_through_a_variable_between_them() {
        let (project, scenario) = small(
            r#"scenario "Variable" {
  begin = start
  cart = POST "/cart" {
    sample 201 { id: "c-1" }
  }
  done = GET "/done" {
    sample 200 {}
  }
  var cart_id = cart.body.id
  pay = POST "/pay/{{cart_id}}" {
    sample 200 { ok: true }
  }
}
"#,
        );
        let graph = &scenario.graph;
        assert_eq!(
            refused(graph, &project, "cart", after(graph, "pay")),
            "`pay` reads `cart_id`; it stays below `cart`."
        );
    }

    #[test]
    fn place_refuses_through_a_statement_that_travels() {
        let (project, scenario) = small(
            r#"scenario "Travels" {
  begin = start
  cart = POST "/cart" {
    sample 201 { id: "c-1" }
  }
  cart_id = cart.body.id
  done = GET "/done" {
    sample 200 {}
  }
  pay = POST "/pay/{{cart_id}}" {
    sample 200 { ok: true }
  }
}
"#,
        );
        let graph = &scenario.graph;
        // `cart_id` goes with `cart` below `pay`, which reads it.
        assert_eq!(
            refused(graph, &project, "cart", after(graph, "pay")),
            "`pay` reads `cart_id`; it stays below `cart`."
        );
        // Below `done`, which reads nothing, it moves.
        let (moved, _) = place(graph, &project, &id(graph, "cart"), after(graph, "done"))
            .expect("`cart` moves below `done`");
        assert_eq!(written(&moved), ["begin", "done", "cart", "cart_id", "pay"]);
    }

    #[test]
    fn place_allows_a_data_statement_written_above() {
        let (project, scenario) = small(
            r#"scenario "Above" {
  begin = start
  ok = assert b.status == 200
  v = a.body.id
  a = GET "/a" {
    sample 200 { id: 1 }
  }
  c = GET "/c" {
    sample 200 {}
  }
  b = POST "/b/{{v}}" {
    sample 200 {}
  }
}
"#,
        );
        let after_a = moved(&project, &scenario, "b", after(&scenario.graph, "a"));
        assert_eq!(steps(&after_a.graph), ["begin", "a", "b", "c"]);
        let first = moved(&project, &scenario, "c", after(&scenario.graph, "begin"));
        assert_eq!(steps(&first.graph), ["begin", "c", "a", "b"]);
    }

    #[test]
    fn place_moves_an_if_with_empty_braces() {
        let (project, scenario) = small(
            r#"scenario "Empty" {
  begin = start
  a = GET "/a" {
    sample 200 {}
  }
  b = GET "/b" {
    sample 200 {}
  }
  x = if 1 == 1 {
  }
}
"#,
        );
        let moved = moved(&project, &scenario, "x", after(&scenario.graph, "a"));
        assert_eq!(steps(&moved.graph), ["begin", "a", "x", "b"]);
    }

    // Criterion 5 -------------------------------------------------------------

    #[test]
    fn add_step_after_the_selection() {
        let (_, shop) = shop();
        let graph = &shop.graph;
        let pick = |selected: &[&str]| -> Option<String> {
            let ids: Vec<String> = selected.iter().map(|handle| id(graph, handle)).collect();
            let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
            add_step_after(graph, &ids).map(|id| {
                graph
                    .nodes
                    .iter()
                    .find(|node| node.id == id)
                    .unwrap()
                    .handle
                    .clone()
            })
        };
        assert_eq!(pick(&[]), None);
        assert_eq!(pick(&["cart"]).as_deref(), Some("cart"));
        assert_eq!(pick(&["paid"]).as_deref(), Some("paid"));
        assert_eq!(pick(&["cart_total"]).as_deref(), Some("cart"));
        assert_eq!(pick(&["receipt_total"]).as_deref(), Some("receipt"));
        assert_eq!(
            pick(&["checkout", "login", "cart"]).as_deref(),
            Some("checkout")
        );
        assert_eq!(pick(&["refund", "order"]).as_deref(), Some("refund"));
    }

    // DIAG-06d ------------------------------------------------------------------

    /// `a` reads nothing; `x` holds a step, a nested `if` and an `else`;
    /// `b` and `c` follow; `x_inner_ok` reads only `inner`.
    const BRACES: &str = r#"scenario "Braces" {
  begin = start
  a = GET "/a" {
    sample 200 { id: 1 }
  }
  x = if a.status == 200 {
    inner = GET "/inner" {
      sample 200 { id: 2 }
    }
    y = if inner.status == 200 {
      deep = GET "/deep" {
        sample 200 {}
      }
    }
  } else {
    other = GET "/other" {
      sample 200 {}
    }
  }
  b = GET "/b" {
    sample 200 {}
  }
  c = GET "/c" {
    sample 200 {}
  }
}
"#;

    /// The braces each statement stands in, by handle: the `if` and whether
    /// it is *Yes*.
    fn braced(graph: &Graph, handle: &str) -> Option<(String, bool)> {
        let written = braces(graph);
        written.get(id(graph, handle).as_str()).map(|(x, branch)| {
            let x = graph.nodes.iter().find(|node| node.id == *x).unwrap();
            (x.handle.clone(), *branch == Branch::True)
        })
    }

    fn yes(x: &str) -> Option<(String, bool)> {
        Some((x.to_owned(), true))
    }

    fn no(x: &str) -> Option<(String, bool)> {
        Some((x.to_owned(), false))
    }

    // Criterion 1: an `if` moves with its braces at any depth.
    #[test]
    fn place_moves_an_if_with_its_braces() {
        let (project, scenario) = small(BRACES);
        let (graph, moved_ids) = place(
            &scenario.graph,
            &project,
            &id(&scenario.graph, "x"),
            after(&scenario.graph, "b"),
        )
        .expect("`x` moves with its braces");
        assert_eq!(
            handles(&graph, &moved_ids),
            ["x", "inner", "y", "deep", "other"]
        );
        assert_eq!(
            steps(&graph),
            ["begin", "a", "b", "x", "inner", "y", "deep", "other", "c"]
        );
        assert_eq!(braced(&graph, "x"), None);
        assert_eq!(braced(&graph, "inner"), yes("x"));
        assert_eq!(braced(&graph, "deep"), yes("y"));
        assert_eq!(braced(&graph, "other"), no("x"));
        let mut result = scenario.clone();
        result.graph = graph;
        reads_back(
            &project,
            apiforge_script::Document::Scenario(crossed(&result)),
        );
    }

    // Criterion 2 ---------------------------------------------------------------

    /// `step` put at `at`, with the braces `expected` it stands in after.
    fn across(
        step: &str,
        at: impl Fn(&Graph) -> Place<'static>,
        expected: Option<(String, bool)>,
    ) -> Graph {
        let (project, scenario) = small(BRACES);
        let moved = moved(&project, &scenario, step, at(&scenario.graph));
        assert_eq!(braced(&moved.graph, step), expected, "`{step}`");
        moved.graph
    }

    fn first(graph: &Graph, x: &str, branch: Branch) -> Place<'static> {
        Place::First(Box::leak(id(graph, x).into_boxed_str()), branch)
    }

    #[test]
    fn place_across_braces_into_a_branch() {
        let graph = across("c", |graph| after(graph, "inner"), yes("x"));
        assert_eq!(
            steps(&graph),
            ["begin", "a", "x", "inner", "c", "y", "deep", "other", "b"]
        );
        let graph = across("b", |graph| first(graph, "x", Branch::False), no("x"));
        assert_eq!(
            steps(&graph),
            ["begin", "a", "x", "inner", "y", "deep", "b", "other", "c"]
        );
    }

    #[test]
    fn place_across_braces_out_to_the_main_flow() {
        let graph = across("other", |graph| after(graph, "b"), None);
        assert_eq!(
            steps(&graph),
            ["begin", "a", "x", "inner", "y", "deep", "b", "other", "c"]
        );
        let graph = across("deep", |graph| after(graph, "a"), None);
        assert_eq!(steps(&graph)[..3], ["begin", "a", "deep"]);
    }

    #[test]
    fn place_across_braces_from_yes_to_no_and_into_a_nested_branch() {
        across("deep", |graph| first(graph, "x", Branch::False), no("x"));
        across("other", |graph| after(graph, "inner"), yes("x"));
        let graph = across("c", |graph| first(graph, "y", Branch::True), yes("y"));
        assert_eq!(
            steps(&graph),
            ["begin", "a", "x", "inner", "y", "c", "deep", "other", "b"]
        );
    }

    #[test]
    fn place_across_braces_refuses_an_if_into_its_own_braces() {
        let (project, scenario) = small(BRACES);
        let graph = &scenario.graph;
        for at in [
            after(graph, "inner"),
            after(graph, "deep"),
            first(graph, "y", Branch::False),
        ] {
            assert_eq!(
                refused(graph, &project, "x", at),
                "A condition cannot go into its own braces."
            );
        }
    }

    #[test]
    fn place_across_braces_does_not_join_the_cleanup_and_the_main_flow() {
        let (project, scenario) = small(CLEANUP);
        // The main flow's place has no statement of the cleanup.
        let made = id(&scenario.graph, "made");
        let a = id(&scenario.cleanup, "a");
        assert!(matches!(
            place(&scenario.graph, &project, &a, Place::After(&made)),
            Err(EditError::NodeNotFound)
        ));
        assert!(matches!(
            place(&scenario.cleanup, &project, &a, Place::After(&made)),
            Err(EditError::NodeNotFound)
        ));
    }

    // Criterion 3 ---------------------------------------------------------------

    #[test]
    fn place_refuses_across_braces_a_reader_above_its_producers_if() {
        let (project, scenario) = small(
            r#"scenario "Reader" {
  begin = start
  a = GET "/a" {
    sample 200 {}
  }
  x = if a.status == 200 {
    p = GET "/p" {
      sample 200 { id: 1 }
    }
  }
  r = GET "/r/{{p.body.id}}" {
    sample 200 {}
  }
}
"#,
        );
        let graph = &scenario.graph;
        assert_eq!(
            refused(graph, &project, "r", after(graph, "begin")),
            "`r` reads `p.body.id`; it stays below `p`."
        );
    }

    const PRODUCER: &str = r#"scenario "Producer" {
  begin = start
  p = GET "/p" {
    sample 200 { id: 1 }
  }
  v = p.body.id
  q = GET "/q/{{p.body.id}}" {
    sample 200 {}
  }
  w = GET "/w/{{v}}" {
    sample 200 {}
  }
  ok = assert p.status == 200
  x = if 1 == 1 {
    inside = GET "/inside" {
      sample 200 {}
    }
  }
}
"#;

    #[test]
    fn place_refuses_across_braces_a_producer_into_a_branch_below_its_reader() {
        let (project, scenario) = small(PRODUCER);
        let graph = &scenario.graph;
        assert_eq!(
            refused(graph, &project, "p", after(graph, "inside")),
            "`q` reads `p.body.id`; it stays below `p`."
        );
    }

    #[test]
    fn place_refuses_across_braces_through_a_value() {
        let (project, scenario) = small(&PRODUCER.replace(
            "  q = GET \"/q/{{p.body.id}}\" {\n    sample 200 {}\n  }\n",
            "",
        ));
        let graph = &scenario.graph;
        // `v`, written directly under `p`, goes with it into the branch.
        assert_eq!(
            refused(graph, &project, "p", first(graph, "x", Branch::True)),
            "`w` reads `v`; it stays below `p`."
        );
    }

    #[test]
    fn place_allows_a_check_written_above_after_a_move_across_braces() {
        let (project, scenario) = small(
            r#"scenario "Above" {
  begin = start
  ok = assert p.status == 200
  x = if 1 == 1 {
    inside = GET "/inside" {
      sample 200 {}
    }
  }
  p = GET "/p" {
    sample 200 {}
  }
}
"#,
        );
        let moved = moved(&project, &scenario, "p", after(&scenario.graph, "inside"));
        assert_eq!(braced(&moved.graph, "p"), yes("x"));
        assert_eq!(
            braced(&moved.graph, "ok"),
            None,
            "the check stays where it is written"
        );
    }

    // Criterion 5 ---------------------------------------------------------------

    const DATA: &str = r#"scenario "Data" {
  begin = start
  a = GET "/a" {
    sample 200 { n: 1 }
  }
  a_ok = assert a.status == 200
  limit = 3
  x = if a.body.n < limit {
    inside = GET "/inside" {
      sample 200 {}
    }
    in_ok = assert inside.status == 200
  } else {
    other = GET "/other" {
      sample 200 {}
    }
  }
  b = GET "/b" {
    sample 200 {}
  }
}
"#;

    #[test]
    fn place_puts_a_check_into_a_branch_from_yes_or_no_only() {
        let (project, scenario) = small(DATA);
        let graph = &scenario.graph;
        let into = moved(
            &project,
            &scenario,
            "a_ok",
            first(graph, "x", Branch::False),
        );
        assert_eq!(braced(&into.graph, "a_ok"), no("x"));
        let into = moved(&project, &scenario, "a_ok", first(graph, "x", Branch::True));
        assert_eq!(braced(&into.graph, "a_ok"), yes("x"));
        for anchor in ["a", "b", "inside"] {
            assert_eq!(
                refused(graph, &project, "a_ok", after(graph, anchor)),
                "Only steps are ordered. A check runs when what it reads exists."
            );
        }
    }

    #[test]
    fn place_moves_a_check_out_of_its_branch() {
        let (project, scenario) = small(DATA);
        let out = moved(&project, &scenario, "in_ok", after(&scenario.graph, "x"));
        assert_eq!(braced(&out.graph, "in_ok"), None);
        let order = written(&out.graph);
        let at = |handle: &str| order.iter().position(|h| *h == handle).unwrap();
        assert_eq!(
            at("in_ok"),
            at("other") + 1,
            "after the `if`'s braces: {order:?}"
        );
        assert!(at("in_ok") < at("b"));
    }

    #[test]
    fn place_refuses_a_value_into_a_branch_whose_condition_reads_it() {
        let (project, scenario) = small(DATA);
        let graph = &scenario.graph;
        assert_eq!(
            refused(graph, &project, "limit", first(graph, "x", Branch::True)),
            "`x` reads `limit`; it stays outside the braces of `x`."
        );
    }

    #[test]
    fn place_allows_a_check_into_a_branch_above_what_it_reads() {
        let (project, scenario) = small(
            r#"scenario "Check" {
  begin = start
  x = if 1 == 1 {
    inside = GET "/inside" {
      sample 200 {}
    }
  }
  b = GET "/b" {
    sample 200 {}
  }
  b_ok = assert b.status == 200
}
"#,
        );
        let into = moved(
            &project,
            &scenario,
            "b_ok",
            first(&scenario.graph, "x", Branch::True),
        );
        assert_eq!(braced(&into.graph, "b_ok"), yes("x"));
    }
}
