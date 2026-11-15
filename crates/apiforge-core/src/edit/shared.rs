//! Values saved in variables (LAY-25): which values of a scenario many
//! steps read from far away, the variable each would be saved as, the edit
//! that saves one or many of them and its inverse.
//!
//! A value is a field of a request's or a component call's output, read
//! exactly (`cart.cart.id`). Saving it adds `var cartId = cart.cart.id`
//! after its step and makes every step of Main and Cleanup that read the
//! value read the variable; nothing else changes. *Read directly again*
//! takes a variable that holds a plain reference back.
use super::{EditError, unique_id};
use crate::{
    format::{Binding, Graph, Node, Position, Project},
    graph::{VALUE_INPUT, create_node, delete_nodes, has_cycle, output_ports, sequence},
    handle::{is_valid_handle, unique_handle},
    model::NodeKind,
};
use std::collections::{BTreeSet, HashMap, HashSet};

/// A value steps read: a field of a step's output.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ValueRef {
    pub node_id: String,
    pub port: String,
    pub path: String,
}

impl ValueRef {
    pub fn new(
        node_id: impl Into<String>,
        port: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            port: port.into(),
            path: path.into(),
        }
    }

    /// The value a binding reads, when it is a reference.
    pub fn of(binding: &Binding) -> Option<Self> {
        match binding {
            Binding::Reference {
                node_id,
                port,
                path,
            } => Some(Self::new(node_id, port, path)),
            _ => None,
        }
    }

    fn binding(&self) -> Binding {
        Binding::reference(&self.node_id, &self.port, &self.path)
    }

    fn is(&self, binding: &Binding) -> bool {
        matches!(binding, Binding::Reference { node_id, port, path }
            if *node_id == self.node_id && *port == self.port && *path == self.path)
    }
}

/// A value and the variable it would be saved as.
#[derive(Clone, Debug, PartialEq)]
pub struct SharedValue {
    pub value: ValueRef,
    /// The value as code writes it, `cart.cart.id`.
    pub text: String,
    /// The variable's name: the one that already holds the value, or the
    /// one a new variable would get.
    pub name: String,
    /// The variable that already holds exactly the value.
    pub held_by: Option<String>,
    /// The steps of Main and Cleanup that read the value directly.
    pub readers: usize,
}

impl SharedValue {
    /// The line a new variable writes, `var cartId = cart.cart.id`.
    pub fn line(&self) -> String {
        format!("var {} = {}", self.name, self.text)
    }
}

/// A scenario after values were saved or a variable was read directly
/// again.
#[derive(Clone, Debug, PartialEq)]
pub struct Saved {
    pub main: Graph,
    pub cleanup: Option<Graph>,
    /// The variables the edit added, in the order of the values.
    pub created: Vec<String>,
    /// The variable each value is read through now, new or not.
    pub variables: Vec<String>,
}

/// A value read by at least this many steps is proposed (LAY-23's count).
pub const SHARED_READERS: usize = 3;

/// A field alone that says nothing: the variable gets the step's name too.
const VAGUE: [&str; 4] = ["id", "value", "status", "body"];

fn all<'a>(main: &'a Graph, cleanup: Option<&'a Graph>) -> impl Iterator<Item = &'a Node> + Clone {
    main.nodes
        .iter()
        .chain(cleanup.into_iter().flat_map(|graph| graph.nodes.iter()))
}

fn find<'a>(main: &'a Graph, cleanup: Option<&'a Graph>, id: &str) -> Option<&'a Node> {
    all(main, cleanup).find(|node| node.id == id)
}

/// Whether `node` is a variable that holds exactly `value`.
fn holds(node: &Node, value: &ValueRef) -> bool {
    node.is_variable()
        && node
            .inputs
            .get(VALUE_INPUT)
            .is_some_and(|binding| value.is(binding))
}

/// The value as code writes it: the step's handle, its output unless it is
/// the step's only one, then the path.
pub fn value_text(
    main: &Graph,
    cleanup: Option<&Graph>,
    project: &Project,
    value: &ValueRef,
) -> String {
    let Some(source) = find(main, cleanup, &value.node_id) else {
        return value.node_id.clone();
    };
    let mut text = source.handle.clone();
    if output_ports(source, project).len() != 1 {
        text.push('.');
        text.push_str(&value.port);
    }
    if !value.path.is_empty() {
        if !value.path.starts_with('[') {
            text.push('.');
        }
        text.push_str(&value.path);
    }
    text
}

/// The last field of a value: of its path without indexes, else its output.
fn last_field(value: &ValueRef) -> String {
    value
        .path
        .rsplit('.')
        .next()
        .map(|field| field.split('[').next().unwrap_or_default())
        .filter(|field| !field.is_empty())
        .unwrap_or(&value.port)
        .to_owned()
}

/// The name a new variable of `value` gets: the last field; the step and the
/// field (`cartId`) where the field alone is `id`, `value`, `status` or
/// `body`, or is taken; then a number.
fn proposed_name(step: &str, value: &ValueRef, taken: impl Fn(&str) -> bool) -> String {
    let field = last_field(value);
    let name = if VAGUE.contains(&field.as_str()) || taken(&field) || !is_valid_handle(&field) {
        let mut letters = field.chars();
        let mut name = step.to_owned();
        if let Some(first) = letters.next() {
            name.extend(first.to_uppercase());
            name.push_str(letters.as_str());
        }
        name
    } else {
        field
    };
    unique_handle(&name, taken)
}

/// Whether a step reads `value` directly: a reference to it anywhere in its
/// inputs, unless the step is a variable that holds it.
fn reads(node: &Node, value: &ValueRef) -> bool {
    !holds(node, value)
        && node.inputs.values().any(|binding| {
            binding.references().iter().any(|reference| {
                reference.node_id == value.node_id
                    && reference.port == value.port
                    && reference.path == value.path
            })
        })
}

/// The variable a value would be read through, for one value: the one that
/// already holds it, or the name a new one would get, with the number of
/// steps that read it directly.
pub fn propose(
    main: &Graph,
    cleanup: Option<&Graph>,
    project: &Project,
    value: &ValueRef,
) -> SharedValue {
    let held = all(main, cleanup).find(|node| holds(node, value));
    let step = find(main, cleanup, &value.node_id).map_or("", |node| node.handle.as_str());
    let name = held.map_or_else(
        || {
            proposed_name(step, value, |name| {
                all(main, cleanup).any(|node| node.handle == name)
            })
        },
        |node| node.handle.clone(),
    );
    SharedValue {
        value: value.clone(),
        text: value_text(main, cleanup, project, value),
        name,
        held_by: held.map(|node| node.id.clone()),
        readers: all(main, cleanup).filter(|node| reads(node, value)).count(),
    }
}

/// The step of each node in the engine's order (LAY-26): a request, a
/// component call or a branch is a step of its own; a check or a value
/// belongs to the last such step before it. Cleanup counts on after Main.
fn steps(main: &Graph, cleanup: Option<&Graph>) -> HashMap<String, usize> {
    let mut step = 0usize;
    let mut steps = HashMap::new();
    for graph in std::iter::once(main).chain(cleanup) {
        for id in sequence(graph) {
            let kind = graph
                .nodes
                .iter()
                .find(|node| node.id == id)
                .map(|node| node.kind);
            if matches!(
                kind,
                Some(NodeKind::Request | NodeKind::Component | NodeKind::If)
            ) {
                step += 1;
            }
            steps.insert(id.to_owned(), step);
        }
    }
    steps
}

/// The distance rule of LAY-25 and LAY-26, read off the document.
struct Distance<'a> {
    steps: HashMap<String, usize>,
    in_main: HashSet<&'a str>,
}

impl<'a> Distance<'a> {
    fn new(main: &'a Graph, cleanup: Option<&Graph>) -> Self {
        Self {
            steps: steps(main, cleanup),
            in_main: main.nodes.iter().map(|node| node.id.as_str()).collect(),
        }
    }

    /// Whether `reader` reads a value of `source` from far away: its step
    /// is two or more steps after the source's, or it is in Cleanup and the
    /// source in Main.
    fn far(&self, source: &str, reader: &str) -> bool {
        let step = |id: &str| self.steps.get(id).copied().unwrap_or_default();
        (self.in_main.contains(source) && !self.in_main.contains(reader))
            || step(reader) >= step(source) + 2
    }
}

/// Every direct read of a value of `main` and `cleanup`, by reader: each
/// field of a request's or a component call's output that a step reads,
/// except a variable's read of the value it holds, in document order of
/// the reader and then of its inputs. A step that reads a value twice
/// counts once.
fn direct_reads<'a>(main: &'a Graph, cleanup: Option<&'a Graph>) -> Vec<(&'a Node, ValueRef)> {
    let mut reads: Vec<(&Node, ValueRef)> = Vec::new();
    let mut seen: HashSet<(&str, ValueRef)> = HashSet::new();
    for reader in all(main, cleanup) {
        for binding in reader.inputs.values() {
            for reference in binding.references() {
                let Some(source) = find(main, cleanup, reference.node_id) else {
                    continue;
                };
                if !matches!(source.kind, NodeKind::Request | NodeKind::Component) {
                    continue;
                }
                let value = ValueRef::new(reference.node_id, reference.port, reference.path);
                if holds(reader, &value) || !seen.insert((reader.id.as_str(), value.clone())) {
                    continue;
                }
                reads.push((reader, value));
            }
        }
    }
    reads
}

/// A step that reads a value directly from far away (LAY-26).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FarRead {
    /// The step that reads the value.
    pub reader: String,
    pub value: ValueRef,
}

/// Every far read of the scenario: a step of Main or Cleanup that reads a
/// field of a request's or a component call's output directly, two or more
/// steps after the source's step in the engine's order, or from Cleanup a
/// value of Main. A step's own checks and values, the next step and what
/// hangs on it are near. A variable's read of the value it holds is not a
/// read; a read of a variable, a parameter or an environment value never is
/// far. In document order of the reader. The rule of LAY-25's
/// [`shared_values`], for every read.
pub fn far_reads(main: &Graph, cleanup: Option<&Graph>) -> Vec<FarRead> {
    let distance = Distance::new(main, cleanup);
    direct_reads(main, cleanup)
        .into_iter()
        .filter(|(reader, value)| distance.far(&value.node_id, &reader.id))
        .map(|(reader, value)| FarRead {
            reader: reader.id.clone(),
            value,
        })
        .collect()
}

/// The values worth a variable: each field of a request's or a component
/// call's output that [`SHARED_READERS`] or more steps of Main and Cleanup
/// read directly, one of them far ([`far_reads`]), in document order. A
/// value a variable already holds is listed while steps still read it
/// directly; it is read through that variable. New names do not clash with
/// one another.
pub fn shared_values(main: &Graph, cleanup: Option<&Graph>, project: &Project) -> Vec<SharedValue> {
    let distance = Distance::new(main, cleanup);
    let order: HashMap<&str, usize> = all(main, cleanup)
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect();
    // Each value with its readers and whether one is far, by first read.
    let mut found: Vec<(ValueRef, BTreeSet<&str>, bool)> = Vec::new();
    for (reader, value) in direct_reads(main, cleanup) {
        let far = distance.far(&value.node_id, &reader.id);
        match found.iter_mut().find(|(known, ..)| *known == value) {
            Some((_, readers, any_far)) => {
                readers.insert(reader.id.as_str());
                *any_far |= far;
            }
            None => found.push((value, BTreeSet::from([reader.id.as_str()]), far)),
        }
    }
    found.retain(|(_, readers, far)| *far && readers.len() >= SHARED_READERS);
    // Document order of the source, then of the first read.
    found.sort_by_key(|(value, ..)| order.get(value.node_id.as_str()).copied());
    let mut named: HashSet<String> = HashSet::new();
    found
        .into_iter()
        .map(|(value, readers, _)| {
            let held = all(main, cleanup).find(|node| holds(node, &value));
            let name = match held {
                Some(node) => node.handle.clone(),
                None => {
                    let step =
                        find(main, cleanup, &value.node_id).map_or("", |node| node.handle.as_str());
                    let name = proposed_name(step, &value, |name| {
                        named.contains(name) || all(main, cleanup).any(|node| node.handle == name)
                    });
                    named.insert(name.clone());
                    name
                }
            };
            SharedValue {
                text: value_text(main, cleanup, project, &value),
                name,
                held_by: held.map(|node| node.id.clone()),
                readers: readers.len(),
                value,
            }
        })
        .collect()
}

/// Rewrites every reference to `from` in `graph`, except in `skip`, with
/// `to`.
fn redirect(graph: &mut Graph, from: &ValueRef, to: &Binding, skip: &str) {
    for node in graph.nodes.iter_mut().filter(|node| node.id != skip) {
        for binding in node.inputs.values_mut() {
            if binding.references().iter().any(|reference| {
                reference.node_id == from.node_id
                    && reference.port == from.port
                    && reference.path == from.path
            }) && let Some(next) = binding.map_leaves(&mut |leaf, _| {
                Some(if from.is(leaf) {
                    to.clone()
                } else {
                    leaf.clone()
                })
            }) {
                *binding = next;
            }
        }
    }
}

/// Saves `values` in variables, in one edit: each gets the variable that
/// already holds it, or a new one right after its step (the name of
/// [`propose`], clear of the names given before it in this edit), and
/// every step of Main and Cleanup that read the value reads the variable.
/// A new variable stands under its step; the caller may move it clear of
/// the cards.
pub fn save_values(
    main: &Graph,
    cleanup: Option<&Graph>,
    project: &Project,
    values: &[ValueRef],
) -> Result<Saved, EditError> {
    let mut graphs = [main.clone(), cleanup.cloned().unwrap_or_default()];
    let mut created: Vec<String> = Vec::new();
    let mut variables: Vec<String> = Vec::new();
    for value in values {
        let view = (&graphs[0], cleanup.map(|_| &graphs[1]));
        let source = find(view.0, view.1, &value.node_id).ok_or(EditError::NodeNotFound)?;
        if !output_ports(source, project)
            .iter()
            .any(|port| port.name == value.port)
        {
            return Err(EditError::ResponseSourceUnavailable);
        }
        let proposal = propose(view.0, view.1, project, value);
        let variable_id = match proposal.held_by {
            Some(id) => id,
            None => {
                let index =
                    usize::from(!graphs[0].nodes.iter().any(|node| node.id == value.node_id));
                let graph = &mut graphs[index];
                let at = graph
                    .nodes
                    .iter()
                    .position(|node| node.id == value.node_id)
                    .ok_or(EditError::NodeNotFound)?;
                // After the step and the variables this edit gave it.
                let mut after = at + 1;
                while graph
                    .nodes
                    .get(after)
                    .is_some_and(|node| created.contains(&node.id))
                {
                    after += 1;
                }
                let below = (after - at) as f64;
                let origin = graph.nodes[at].position.clone();
                let mut node = create_node(
                    NodeKind::Value,
                    Position {
                        x: origin.x,
                        y: origin.y + 160.0 * below,
                    },
                    None,
                );
                node.id = unique_id("node", |id| {
                    graphs
                        .iter()
                        .any(|graph| graph.nodes.iter().any(|node| node.id == id))
                });
                node.handle = proposal.name.clone();
                node.label = None;
                node.config.variable = Some(true);
                node.auto_placed = true;
                node.inputs.clear();
                node.inputs.insert(VALUE_INPUT.into(), value.binding());
                let id = node.id.clone();
                graphs[index].nodes.insert(after, node);
                created.push(id.clone());
                id
            }
        };
        let through = Binding::reference(&variable_id, VALUE_INPUT, "");
        for graph in &mut graphs {
            redirect(graph, value, &through, &variable_id);
        }
        variables.push(variable_id);
    }
    if graphs.iter().any(has_cycle) {
        return Err(EditError::AssignmentCycle);
    }
    let [main, cleanup_graph] = graphs;
    Ok(Saved {
        main,
        cleanup: cleanup.map(|_| cleanup_graph),
        created,
        variables,
    })
}

/// The value a variable holds when it is a plain reference: what *Read
/// directly again* puts back in its readers.
pub fn plain_value(node: &Node) -> Option<ValueRef> {
    if !node.is_variable() {
        return None;
    }
    node.inputs.get(VALUE_INPUT).and_then(ValueRef::of)
}

/// *Read directly again*: every step that reads the variable `variable_id`
/// reads the value it holds, and the variable is removed, in one edit.
pub fn read_directly(
    main: &Graph,
    cleanup: Option<&Graph>,
    variable_id: &str,
) -> Result<Saved, EditError> {
    let variable = find(main, cleanup, variable_id).ok_or(EditError::VariableNotFound)?;
    let value = plain_value(variable).ok_or(EditError::VariableNotFound)?;
    let mut graphs = [main.clone(), cleanup.cloned().unwrap_or_default()];
    for graph in &mut graphs {
        for node in graph.nodes.iter_mut().filter(|node| node.id != variable_id) {
            for binding in node.inputs.values_mut() {
                if !binding.references_node(variable_id) {
                    continue;
                }
                let next = binding.map_leaves(&mut |leaf, _| {
                    Some(match leaf {
                        Binding::Reference { node_id, path, .. } if node_id == variable_id => {
                            let joined = match (value.path.is_empty(), path.is_empty()) {
                                (_, true) => value.path.clone(),
                                (true, false) => path.clone(),
                                (false, false) if path.starts_with('[') => {
                                    format!("{}{path}", value.path)
                                }
                                (false, false) => format!("{}.{path}", value.path),
                            };
                            Binding::reference(&value.node_id, &value.port, joined)
                        }
                        other => other.clone(),
                    })
                });
                if let Some(next) = next {
                    *binding = next;
                }
            }
        }
    }
    let gone = HashSet::from([variable_id]);
    let [main, cleanup_graph] = graphs.map(|graph| delete_nodes(&graph, &gone));
    Ok(Saved {
        main,
        cleanup: cleanup.map(|_| cleanup_graph),
        created: Vec::new(),
        variables: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        edit::test_support::{empty_project, node},
        format::{OrderEdge, TemplatePart},
    };

    fn request(handle: &str) -> Node {
        let mut request = node(NodeKind::Request);
        request.id = handle.to_owned();
        request.handle = handle.to_owned();
        request
    }

    /// `login`, then four requests in a chain, each reading `login.body.token`
    /// and `login.body.id` in its URL; a request in Cleanup reads the token.
    fn chain() -> (Graph, Graph) {
        let mut nodes = vec![request("login")];
        let mut edges = Vec::new();
        for (index, handle) in ["a", "b", "c", "d"].into_iter().enumerate() {
            let mut reader = request(handle);
            reader.inputs.insert(
                "url".into(),
                Binding::Template {
                    parts: vec![
                        TemplatePart::Value {
                            binding: Binding::reference("login", "body", "token"),
                        },
                        TemplatePart::Value {
                            binding: Binding::reference("login", "body", "id"),
                        },
                    ],
                },
            );
            edges.push(OrderEdge {
                id: format!("order-{index}"),
                source: nodes[index].id.clone(),
                target: handle.to_owned(),
                branch: None,
            });
            nodes.push(reader);
        }
        let mut removed = request("removed");
        removed
            .inputs
            .insert("body".into(), Binding::reference("login", "body", "token"));
        (
            Graph {
                nodes,
                edges,
                proxies: Vec::new(),
            },
            Graph {
                nodes: vec![removed],
                edges: Vec::new(),
                proxies: Vec::new(),
            },
        )
    }

    #[test]
    fn names_take_the_last_field_then_the_step_then_a_number() {
        let free = |_: &str| false;
        let named = |step: &str, port: &str, path: &str| {
            proposed_name(step, &ValueRef::new(step, port, path), free)
        };
        assert_eq!(named("cart", "cart", "cart.id"), "cartId");
        assert_eq!(named("login", "body", "token"), "token");
        assert_eq!(named("login", "status", ""), "loginStatus");
        assert_eq!(named("list", "body", "items[0].id"), "listId");
        assert_eq!(named("list", "body", "items[0]"), "items");
        let taken = |name: &str| ["token", "loginToken"].contains(&name);
        assert_eq!(
            proposed_name("login", &ValueRef::new("login", "body", "token"), taken),
            "loginToken_2"
        );
    }

    #[test]
    fn a_value_three_steps_read_far_is_shared_in_main_and_cleanup() {
        let (main, cleanup) = chain();
        let project = empty_project();
        let shared = shared_values(&main, Some(&cleanup), &project);
        let found: Vec<(&str, &str, usize)> = shared
            .iter()
            .map(|value| (value.name.as_str(), value.text.as_str(), value.readers))
            .collect();
        assert_eq!(
            found,
            [
                ("token", "login.body.token", 5),
                ("loginId", "login.body.id", 4)
            ]
        );
        assert_eq!(shared[0].line(), "var token = login.body.token");
    }

    #[test]
    fn far_reads_are_two_steps_on_or_in_cleanup() {
        let (main, cleanup) = chain();
        let far: Vec<(String, String)> = far_reads(&main, Some(&cleanup))
            .into_iter()
            .map(|read| (read.reader, read.value.path))
            .collect();
        // `a` is the step after `login`; Cleanup's `removed` is far.
        assert_eq!(
            far,
            [
                ("b", "token"),
                ("b", "id"),
                ("c", "token"),
                ("c", "id"),
                ("d", "token"),
                ("d", "id"),
                ("removed", "token"),
            ]
            .map(|(reader, path)| (reader.to_owned(), path.to_owned()))
        );
    }

    #[test]
    fn saving_moves_every_reader_and_reading_directly_takes_it_back() {
        let (main, cleanup) = chain();
        let project = empty_project();
        let values: Vec<ValueRef> = shared_values(&main, Some(&cleanup), &project)
            .into_iter()
            .map(|value| value.value)
            .collect();
        let saved = save_values(&main, Some(&cleanup), &project, &values).unwrap();
        assert_eq!(saved.created.len(), 2);
        let handles: Vec<&str> = saved
            .main
            .nodes
            .iter()
            .map(|node| node.handle.as_str())
            .collect();
        assert_eq!(handles, ["login", "token", "loginId", "a", "b", "c", "d"]);
        let cleanup_after = saved.cleanup.clone().unwrap();
        assert_eq!(
            cleanup_after.nodes[0].inputs["body"],
            Binding::reference(&saved.created[0], VALUE_INPUT, "")
        );
        assert!(shared_values(&saved.main, Some(&cleanup_after), &project).is_empty());
        // Saving again reuses the variables.
        let again = save_values(&saved.main, Some(&cleanup_after), &project, &values).unwrap();
        assert!(again.created.is_empty());
        assert_eq!(again.variables, saved.created);

        let mut back = (saved.main.clone(), cleanup_after);
        for variable in &saved.created {
            let read = read_directly(&back.0, Some(&back.1), variable).unwrap();
            back = (read.main, read.cleanup.unwrap());
        }
        assert_eq!(back.0, main);
        assert_eq!(back.1, cleanup);
    }

    #[test]
    fn reading_directly_joins_the_paths_and_needs_a_plain_reference() {
        let mut variable = node(NodeKind::Value);
        variable.id = "first".into();
        variable.handle = "first".into();
        variable.config.variable = Some(true);
        variable.inputs.insert(
            VALUE_INPUT.into(),
            Binding::reference("list", "body", "items[0]"),
        );
        let mut reader = request("reader");
        reader.inputs.insert(
            "body".into(),
            Binding::reference("first", VALUE_INPUT, "id"),
        );
        reader.inputs.insert(
            "query".into(),
            Binding::reference("first", VALUE_INPUT, "[1]"),
        );
        let graph = Graph {
            nodes: vec![request("list"), variable.clone(), reader],
            edges: Vec::new(),
            proxies: Vec::new(),
        };
        let read = read_directly(&graph, None, "first").unwrap();
        assert_eq!(read.main.nodes.len(), 2);
        assert_eq!(
            read.main.nodes[1].inputs["body"],
            Binding::reference("list", "body", "items[0].id")
        );
        assert_eq!(
            read.main.nodes[1].inputs["query"],
            Binding::reference("list", "body", "items[0][1]")
        );
        let mut calculated = variable;
        calculated
            .inputs
            .insert(VALUE_INPUT.into(), Binding::literal(serde_json::json!(2)));
        assert!(plain_value(&calculated).is_none());
        assert!(matches!(
            read_directly(&graph, None, "reader"),
            Err(EditError::VariableNotFound)
        ));
    }
}
