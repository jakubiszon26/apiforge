//! Deleting statements on the board (DIAG-06e).
//!
//! One function, [`delete_preview`], answers what a delete would do: which
//! statements go and which statements that stay would be left reading
//! something that went. The refusal, the confirmation and the delete itself
//! all use its answer, so what the person is told is what happens.
//!
//! What goes is the selection; for an `if`, every statement in its braces at
//! any depth; and a check, value or variable that reads at least one
//! statement that goes and no statement that stays, until nothing more is
//! added. A statement that reads no statement at all — a literal, a check of
//! constants, a value of a parameter or of `env` only — belongs to nothing
//! and stays.
//!
//! A delete with a statement left reading is refused, and a delete that is
//! done changes no input of a statement that stays. The legacy window keeps
//! [`crate::graph::delete_nodes`], which drops such inputs instead.
use super::EditError;
use crate::{
    branches::Branches,
    chain::is_step,
    format::{Graph, Node, ProxySource},
    model::NodeKind,
};
use std::collections::{HashMap, HashSet};

/// What deleting a selection would do.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Deletion {
    /// The node identifiers of every statement that goes, in the order of
    /// the graph.
    pub gone: Vec<String>,
    /// Statements that stay and read something that goes: of the graph
    /// first, then of the graphs beside it, each in its order.
    pub readers: Vec<Reader>,
    /// *Start* would go while a step of its flow stays.
    pub start: bool,
    /// Every selected `if` with statements in its braces, outermost only.
    pub braces: Vec<Braced>,
}

/// A statement that stays and reads something that goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reader {
    pub id: String,
    pub handle: String,
    /// What it reads that goes, in the order of the graph.
    pub reads: Vec<Read>,
}

/// A statement that goes, as a reader reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Read {
    pub handle: String,
    /// The selected statements it goes with, when it is not selected itself:
    /// the `if` whose braces hold it, or the statements a check, value or
    /// variable reads.
    pub with: Vec<String>,
}

/// A selected `if` with statements in its braces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Braced {
    pub handle: String,
    /// The steps in its braces at any depth.
    pub steps: usize,
    /// Every statement in its braces at any depth.
    pub statements: usize,
}

impl Deletion {
    /// Why the delete is refused, as the board says it; `None` when it may
    /// be done.
    pub fn refusal(&self) -> Option<String> {
        if self.start {
            return Some("A scenario starts at Start. Delete its steps first.".into());
        }
        if self.readers.is_empty() {
            return None;
        }
        let readers: Vec<&str> = self.readers.iter().map(|r| r.handle.as_str()).collect();
        let mut read: Vec<&Read> = vec![];
        for reader in &self.readers {
            for item in &reader.reads {
                if !read.iter().any(|known| known.handle == item.handle) {
                    read.push(item);
                }
            }
        }
        let names: Vec<&str> = read.iter().map(|item| item.handle.as_str()).collect();
        let (verb, them) = if readers.len() == 1 {
            ("reads", "it")
        } else {
            ("read", "them")
        };
        let mut message = format!(
            "{} {verb} {}. Change or delete {them} first.",
            listed(&readers),
            listed(&names)
        );
        // What goes with the selection without being selected, grouped by
        // what it goes with.
        let mut groups: Vec<(&[String], Vec<&str>)> = vec![];
        for item in read.iter().filter(|item| !item.with.is_empty()) {
            match groups
                .iter_mut()
                .find(|(with, _)| *with == item.with.as_slice())
            {
                Some((_, handles)) => handles.push(&item.handle),
                None => groups.push((&item.with, vec![&item.handle])),
            }
        }
        for (with, handles) in groups {
            let with: Vec<&str> = with.iter().map(String::as_str).collect();
            message.push_str(&format!(
                " {} would go with {}.",
                listed(&handles),
                listed(&with)
            ));
        }
        Some(message)
    }

    /// The one question a delete asks before it is done: when a selected
    /// `if` has statements in its braces. `None` when there is nothing to
    /// ask.
    pub fn question(&self) -> Option<String> {
        if self.braces.is_empty() {
            return None;
        }
        let ifs: Vec<&str> = self.braces.iter().map(|b| b.handle.as_str()).collect();
        let steps: usize = self.braces.iter().map(|b| b.steps).sum();
        let statements: usize = self.braces.iter().map(|b| b.statements).sum();
        let inside = match (steps, statements) {
            (1, _) => "the step".to_owned(),
            (0, 1) => "the statement".to_owned(),
            (0, count) => format!("the {count} statements"),
            (count, _) => format!("the {count} steps"),
        };
        let it = if ifs.len() == 1 { "it" } else { "them" };
        Some(format!("Delete {} and {inside} inside {it}?", listed(&ifs)))
    }
}

/// `a`, `` `a` and `b` ``, `` `a`, `b` and `c` ``.
fn listed(handles: &[&str]) -> String {
    let quoted: Vec<String> = handles.iter().map(|handle| format!("`{handle}`")).collect();
    match quoted.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The statements a node reads, by identifier, each once.
pub(super) fn reads(node: &Node) -> Vec<&str> {
    let mut result: Vec<&str> = vec![];
    for binding in node.inputs.values() {
        for reference in binding.references() {
            if !result.contains(&reference.node_id) {
                result.push(reference.node_id);
            }
        }
    }
    result
}

/// A check, a value or a variable: what goes with what it reads.
pub(super) fn follows_what_it_reads(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Assert | NodeKind::Condition | NodeKind::Value
    )
}

/// Adds to `with` what goes with its statements, until nothing more is
/// added: a statement `may` accepts that reads at least one statement of
/// `with` and, besides them, only statements of `neutral`. Each added
/// statement is recorded with what it goes with: the statements of `with`
/// that went without going with another, through the ones it reads. The
/// same notion takes the checks along with a deleted step (DIAG-06e) and
/// with a moved one (DIAG-06a).
pub(super) fn taken_along<'g>(
    graph: &'g Graph,
    with: &mut HashMap<&'g str, Vec<&'g str>>,
    may: impl Fn(&Node) -> bool,
    neutral: &HashSet<&str>,
) {
    loop {
        let mut added = false;
        for node in &graph.nodes {
            let id = node.id.as_str();
            if with.contains_key(id) || !may(node) {
                continue;
            }
            let read = reads(node);
            if read.iter().any(|source| with.contains_key(source))
                && read
                    .iter()
                    .all(|source| with.contains_key(source) || neutral.contains(source))
            {
                let mut owners: Vec<&str> = vec![];
                for source in read.into_iter().filter(|source| with.contains_key(source)) {
                    let own = &with[source];
                    let sources = if own.is_empty() {
                        std::slice::from_ref(&source)
                    } else {
                        own.as_slice()
                    };
                    for owner in sources {
                        if !owners.contains(owner) {
                            owners.push(owner);
                        }
                    }
                }
                with.insert(id, owners);
                added = true;
            }
        }
        if !added {
            break;
        }
    }
}

/// What deleting `selection` from `graph` would do. `beside` are the graphs
/// whose statements may read `graph`'s (a scenario's cleanup, which sees the
/// main flow); their statements stay and are never taken with the delete.
/// `needs_start` is true for the main flow of a scenario, which starts at
/// *Start* while it has a step. Identifiers of the selection that name no
/// statement of `graph` (a proxy, a wire) are left out.
pub fn delete_preview(
    graph: &Graph,
    beside: &[&Graph],
    selection: &HashSet<&str>,
    needs_start: bool,
) -> Deletion {
    let branches = Branches::new(graph);
    let selected: HashSet<&str> = graph
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .filter(|id| selection.contains(id))
        .collect();
    let handles: HashMap<&str, &str> = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node.handle.as_str()))
        .collect();

    // The outermost selected `if` whose braces hold a statement.
    let enclosing = |id: &str| -> Option<&str> {
        let mut found = None;
        let mut current = id;
        for _ in 0..=graph.nodes.len() {
            match branches.parent(current) {
                Some((x, _)) => {
                    if selected.contains(x) {
                        found = Some(x);
                    }
                    current = x;
                }
                None => break,
            }
        }
        found
    };
    // What each statement that goes goes with: nothing for a selected one.
    let mut with: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut braces: Vec<Braced> = vec![];
    let mut counted: HashMap<&str, (usize, usize)> = HashMap::new();
    for node in &graph.nodes {
        let id = node.id.as_str();
        if selected.contains(id) && enclosing(id).is_none() {
            with.insert(id, vec![]);
        } else if let Some(x) = enclosing(id) {
            with.insert(id, vec![x]);
            let count = counted.entry(x).or_default();
            count.1 += 1;
            if is_step(node.kind) {
                count.0 += 1;
            }
        }
    }
    for node in &graph.nodes {
        if let Some(&(steps, statements)) = counted.get(node.id.as_str()) {
            braces.push(Braced {
                handle: node.handle.clone(),
                steps,
                statements,
            });
        }
    }
    // A check, value or variable that reads only what goes goes with it.
    taken_along(
        graph,
        &mut with,
        |node| follows_what_it_reads(node.kind),
        &HashSet::new(),
    );

    let gone: Vec<String> = graph
        .nodes
        .iter()
        .filter(|node| with.contains_key(node.id.as_str()))
        .map(|node| node.id.clone())
        .collect();
    let order: HashMap<&str, usize> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect();
    let readers = std::iter::once(graph)
        .chain(beside.iter().copied())
        .flat_map(|graph| &graph.nodes)
        .filter(|node| !with.contains_key(node.id.as_str()))
        .filter_map(|node| {
            let mut read: Vec<&str> = reads(node)
                .into_iter()
                .filter(|source| with.contains_key(source))
                .collect();
            if read.is_empty() {
                return None;
            }
            read.sort_by_key(|source| order[source]);
            Some(Reader {
                id: node.id.clone(),
                handle: node.handle.clone(),
                reads: read
                    .into_iter()
                    .map(|source| Read {
                        handle: handles[source].to_owned(),
                        with: with[source]
                            .iter()
                            .map(|owner| handles[owner].to_owned())
                            .collect(),
                    })
                    .collect(),
            })
        })
        .collect();
    let start = needs_start
        && graph
            .nodes
            .iter()
            .any(|node| node.kind == NodeKind::Start && with.contains_key(node.id.as_str()))
        && graph.nodes.iter().any(|node| {
            is_step(node.kind)
                && node.kind != NodeKind::Start
                && !with.contains_key(node.id.as_str())
        });
    Deletion {
        gone,
        readers,
        start,
        braces,
    }
}

/// `graph` without what `deletion` takes: its statements, the ordering
/// edges that touch them and the proxies of its variables. No input of a
/// statement that stays is changed; the chain of the written language joins
/// the neighbours when the model is written ([`crate::chain::edited`]).
/// Call it only for a deletion without a [`Deletion::refusal`].
pub fn apply_deletion(graph: &Graph, deletion: &Deletion) -> Graph {
    let gone: HashSet<&str> = deletion.gone.iter().map(String::as_str).collect();
    Graph {
        nodes: graph
            .nodes
            .iter()
            .filter(|node| !gone.contains(node.id.as_str()))
            .cloned()
            .collect(),
        edges: graph
            .edges
            .iter()
            .filter(|edge| {
                !gone.contains(edge.source.as_str()) && !gone.contains(edge.target.as_str())
            })
            .cloned()
            .collect(),
        proxies: graph
            .proxies
            .iter()
            .filter(|proxy| match &proxy.source {
                ProxySource::Variable { node_id } => !gone.contains(node_id.as_str()),
                ProxySource::Environment { .. } | ProxySource::Parameter { .. } => true,
            })
            .cloned()
            .collect(),
    }
}

/// Deletes `selection` from `graph` by [`delete_preview`]'s answer, or
/// refuses with the board's sentence. The confirmation an `if` asks for is
/// the caller's.
pub fn delete_statements(
    graph: &Graph,
    beside: &[&Graph],
    selection: &HashSet<&str>,
    needs_start: bool,
) -> Result<Graph, EditError> {
    let deletion = delete_preview(graph, beside, selection, needs_start);
    match deletion.refusal() {
        Some(refusal) => Err(EditError::DeleteRefused(refusal)),
        None => Ok(apply_deletion(graph, &deletion)),
    }
}

#[cfg(test)]
mod tests {
    //! DIAG-06e, criteria 1–5 on the preview. The scenarios are written in
    //! the language and lowered by `apiforge-script`, which links another
    //! build of this crate: the documents cross between the two builds as
    //! JSON. Every edit ends by printing the edited scenario, reading it back
    //! without an error and as the same document.
    use super::*;
    use crate::{
        chain::{edited, is_step, with_start},
        format::{Project, Scenario},
    };
    use serde::{Serialize, de::DeserializeOwned};

    /// A value of one build of this crate as the same type of the other.
    fn crossed<A: Serialize, B: DeserializeOwned>(value: &A) -> B {
        serde_json::from_value(serde_json::to_value(value).unwrap()).unwrap()
    }

    fn lowered(project: &Project, text: &str) -> Scenario {
        let theirs = crossed(project);
        let lowered = apiforge_script::lower(
            &apiforge_script::parse(text),
            &apiforge_script::LowerContext::folder(&theirs),
        );
        assert!(!lowered.has_errors(), "{:#?}", lowered.diagnostics);
        match lowered.document {
            Some(apiforge_script::Document::Scenario(scenario)) => crossed(&scenario),
            other => panic!("not a scenario: {other:?}"),
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
        let scenario = lowered(&project, &fixture("shop-30.afs"));
        (project, scenario)
    }

    fn small(text: &str) -> (Project, Scenario) {
        let project = super::super::test_support::empty_project();
        let scenario = lowered(&project, text);
        (project, scenario)
    }

    fn id(scenario: &Scenario, handle: &str) -> String {
        scenario
            .graph
            .nodes
            .iter()
            .chain(&scenario.cleanup.nodes)
            .find(|node| node.handle == handle)
            .unwrap_or_else(|| panic!("a statement `{handle}`"))
            .id
            .clone()
    }

    fn handles(scenario: &Scenario, ids: &[String]) -> Vec<String> {
        ids.iter()
            .map(|id| {
                scenario
                    .graph
                    .nodes
                    .iter()
                    .find(|node| &node.id == id)
                    .unwrap()
                    .handle
                    .clone()
            })
            .collect()
    }

    /// The preview of deleting `selected` from the main flow.
    fn preview(scenario: &Scenario, selected: &[&str]) -> Deletion {
        let ids: Vec<String> = selected.iter().map(|handle| id(scenario, handle)).collect();
        delete_preview(
            &scenario.graph,
            &[&scenario.cleanup],
            &ids.iter().map(String::as_str).collect(),
            true,
        )
    }

    fn reader_handles(deletion: &Deletion) -> Vec<&str> {
        deletion
            .readers
            .iter()
            .map(|reader| reader.handle.as_str())
            .collect()
    }

    /// Deletes `selected` from the main flow, as the session writes it, and
    /// checks the text: it passes the check and reads back as the document.
    fn deleted(project: &Project, scenario: &Scenario, selected: &[&str]) -> Scenario {
        let ids: Vec<String> = selected.iter().map(|handle| id(scenario, handle)).collect();
        let graph = delete_statements(
            &scenario.graph,
            &[&scenario.cleanup],
            &ids.iter().map(String::as_str).collect(),
            true,
        )
        .expect("the delete is done");
        let mut after = scenario.clone();
        after.graph = with_start(&edited(&scenario.graph, &graph), &[&after.cleanup]);
        let theirs = crossed(project);
        let document = apiforge_script::Document::Scenario(crossed(&after));
        let text = apiforge_script::print_folder_document(&theirs, &document)
            .expect("the edited scenario prints");
        let read = lowered(project, &text);
        assert_eq!(
            apiforge_script::semantic(
                &apiforge_script::Document::Scenario(crossed(&read)),
                &theirs
            ),
            apiforge_script::semantic(&document, &theirs),
            "the text reads back as the document:\n{text}"
        );
        after
    }

    fn steps(scenario: &Scenario) -> Vec<String> {
        crate::graph::sequence(&scenario.graph)
            .into_iter()
            .filter_map(|id| scenario.graph.nodes.iter().find(|node| node.id == id))
            .filter(|node| is_step(node.kind))
            .map(|node| node.handle.clone())
            .collect()
    }

    const PAY: &str = r#"scenario "Pay" {
  begin = start
  cart = POST "/cart" {
    sample 201 { id: "c-1", total: 10 }
  }
  pay = POST "/pay/{{cart.body.id}}" {
    sample 200 { ok: true }
  }
  receipt = GET "/receipts" {
    query { cart: cart.body.id }
    sample 200 { total: 10 }
  }
  done = GET "/done" {
    sample 200 {}
  }
}
"#;

    // Criterion 1 -------------------------------------------------------------

    #[test]
    fn delete_preview_refuses_a_step_that_is_read_and_names_every_reader() {
        let (_, scenario) = small(PAY);
        let deletion = preview(&scenario, &["cart"]);
        assert_eq!(reader_handles(&deletion), ["pay", "receipt"]);
        assert_eq!(
            deletion.refusal().as_deref(),
            Some("`pay` and `receipt` read `cart`. Change or delete them first.")
        );
        let ids = [id(&scenario, "cart")];
        let refused = delete_statements(
            &scenario.graph,
            &[&scenario.cleanup],
            &ids.iter().map(String::as_str).collect(),
            true,
        );
        assert!(matches!(refused, Err(EditError::DeleteRefused(_))));
    }

    #[test]
    fn delete_preview_with_the_readers_selected_deletes_all_and_joins_the_neighbours() {
        let (project, scenario) = small(PAY);
        let deletion = preview(&scenario, &["cart", "pay", "receipt"]);
        assert_eq!(deletion.refusal(), None);
        let after = deleted(&project, &scenario, &["cart", "pay", "receipt"]);
        assert_eq!(steps(&after), ["begin", "done"]);
    }

    #[test]
    fn delete_preview_refuses_a_reader_in_the_cleanup() {
        let (_, scenario) = shop();
        let deletion = preview(&scenario, &["cart"]);
        assert_eq!(
            reader_handles(&deletion),
            [
                "cart_total",
                "cart_view",
                "address",
                "order",
                "receipt_total",
                "removed"
            ],
            "`cart_created` reads only `cart` and goes with it"
        );
        assert!(
            deletion.gone.contains(&id(&scenario, "cart_created")),
            "{:?}",
            handles(&scenario, &deletion.gone)
        );
    }

    #[test]
    fn delete_preview_refuses_a_reader_whose_input_is_optional() {
        let (_, scenario) = shop();
        // `checkout`'s body reads `expected_total`: refused, nothing dropped.
        let deletion = preview(&scenario, &["expected_total"]);
        assert!(reader_handles(&deletion).contains(&"checkout"));
        let ids = [id(&scenario, "expected_total")];
        assert!(
            delete_statements(
                &scenario.graph,
                &[&scenario.cleanup],
                &ids.iter().map(String::as_str).collect(),
                true,
            )
            .is_err()
        );
    }

    // Criterion 2 -------------------------------------------------------------

    #[test]
    fn delete_preview_refuses_a_reader_of_something_else_that_stays() {
        let (_, scenario) = small(
            r#"scenario "Both" {
  begin = start
  a = GET "/a" {
    sample 200 {}
  }
  b = GET "/b" {
    sample 200 {}
  }
  both = assert a.status == b.status
}
"#,
        );
        let deletion = preview(&scenario, &["a"]);
        assert_eq!(reader_handles(&deletion), ["both"]);
        assert_eq!(
            deletion.refusal().as_deref(),
            Some("`both` reads `a`. Change or delete it first.")
        );
    }

    // Criterion 3 -------------------------------------------------------------

    const EXPANDS: &str = r#"scenario "Expands" {
  param limit = 5
  begin = start
  a = GET "/a" {
    sample 200 { n: 1 }
  }
  v = a.body.n
  timeout = 30
  ok = assert 1 == 1
  limited = param.limit
  b = POST "/b" {
    body { n: v }
    sample 200 {}
  }
  c = POST "/c" {
    body { wait: timeout }
    sample 200 {}
  }
}
"#;

    #[test]
    fn delete_preview_expands() {
        let (project, scenario) = small(EXPANDS);
        let deletion = preview(&scenario, &["a"]);
        assert_eq!(handles(&scenario, &deletion.gone), ["a", "v"]);
        assert_eq!(
            reader_handles(&deletion),
            ["b"],
            "`b` reads `a` through `v`"
        );
        assert_eq!(
            deletion.refusal().as_deref(),
            Some("`b` reads `v`. Change or delete it first. `v` would go with `a`.")
        );
        let deletion = preview(&scenario, &["a", "b"]);
        assert_eq!(handles(&scenario, &deletion.gone), ["a", "v", "b"]);
        assert_eq!(deletion.refusal(), None);
        let after = deleted(&project, &scenario, &["a", "b"]);
        assert_eq!(steps(&after), ["begin", "c"]);
    }

    #[test]
    fn delete_preview_leaves_what_reads_no_statement() {
        let (project, scenario) = small(EXPANDS);
        let deletion = preview(&scenario, &["a", "b"]);
        for kept in ["timeout", "ok", "limited", "c"] {
            assert!(
                !deletion.gone.contains(&id(&scenario, kept)),
                "`{kept}` reads no statement that goes and stays"
            );
        }
        assert!(
            deletion.readers.is_empty(),
            "`c` reads `timeout`, which stays"
        );
        let after = deleted(&project, &scenario, &["a", "b"]);
        for kept in ["timeout", "ok", "limited", "c"] {
            assert!(after.graph.nodes.iter().any(|node| node.handle == kept));
        }
    }

    // Criterion 4 -------------------------------------------------------------

    #[test]
    fn delete_preview_surviving_inputs_are_untouched() {
        let (project, scenario) = shop();
        let after = deleted(&project, &scenario, &["refund", "refund_amount"]);
        for node in after.graph.nodes.iter().chain(&after.cleanup.nodes) {
            let before = scenario
                .graph
                .nodes
                .iter()
                .chain(&scenario.cleanup.nodes)
                .find(|old| old.id == node.id)
                .expect("a statement that stayed");
            assert_eq!(before.inputs.len(), node.inputs.len(), "`{}`", node.handle);
            for (name, binding) in &before.inputs {
                assert_eq!(
                    node.inputs.get(name),
                    Some(binding),
                    "`{}`.{name} is unchanged",
                    node.handle
                );
            }
        }
    }

    // Criterion 5 -------------------------------------------------------------

    #[test]
    fn delete_preview_an_if_takes_its_braces_and_asks_once() {
        let (project, scenario) = shop();
        let deletion = preview(&scenario, &["paid"]);
        assert_eq!(deletion.refusal(), None);
        assert_eq!(
            handles(&scenario, &deletion.gone),
            [
                "paid",
                "payment",
                "receipt",
                "receipt_total",
                "refund",
                "refund_amount",
                "logout",
                "not_created"
            ]
        );
        assert_eq!(
            deletion.question().as_deref(),
            Some("Delete `paid` and the 4 steps inside it?")
        );
        let after = deleted(&project, &scenario, &["paid"]);
        assert_eq!(steps(&after).last().map(String::as_str), Some("order"));
    }

    #[test]
    fn delete_preview_an_if_read_from_outside_is_refused_before_any_question() {
        let (_, scenario) = small(
            r#"scenario "Outside" {
  begin = start
  a = GET "/a" {
    sample 200 {}
  }
  x = if a.status == 200 {
    inner = GET "/inner" {
      sample 200 { id: 1 }
    }
  }
  after = GET "/after/{{inner.body.id}}" {
    sample 200 {}
  }
}
"#,
        );
        let deletion = preview(&scenario, &["x"]);
        assert_eq!(
            deletion.refusal().as_deref(),
            Some("`after` reads `inner`. Change or delete it first. `inner` would go with `x`.")
        );
    }

    #[test]
    fn delete_preview_an_if_with_empty_braces_asks_nothing() {
        let (project, scenario) = small(
            r#"scenario "Empty" {
  begin = start
  a = GET "/a" {
    sample 200 {}
  }
  x = if a.status == 200 {
  }
  b = GET "/b" {
    sample 200 {}
  }
}
"#,
        );
        let deletion = preview(&scenario, &["x"]);
        assert_eq!((deletion.refusal(), deletion.question()), (None, None));
        let after = deleted(&project, &scenario, &["x"]);
        assert_eq!(steps(&after), ["begin", "a", "b"]);
    }

    // Criterion 6 -------------------------------------------------------------

    #[test]
    fn delete_preview_start_with_steps_is_refused_alone_it_goes() {
        let (_, scenario) = small(PAY);
        assert_eq!(
            preview(&scenario, &["begin"]).refusal().as_deref(),
            Some("A scenario starts at Start. Delete its steps first.")
        );
        let (project, scenario) = small("scenario \"Alone\" {\n  begin = start\n}\n");
        assert_eq!(preview(&scenario, &["begin"]).refusal(), None);
        let after = deleted(&project, &scenario, &["begin"]);
        assert!(after.graph.nodes.is_empty());
    }
}
