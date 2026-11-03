//! Saving selected blocks as a component that is called in their place
//! (UX-30, COMP-00 decision 5).
//!
//! [`propose_extraction`] proposes the draft the dialog edits: the inputs of
//! [`initialize_inputs`], typed from their sources and optional when a value
//! becomes their default, and the outputs of [`initialize_outputs`], ticked
//! when a step after the selection reads them. [`extraction_component`] turns
//! the draft into the definition, and [`replace_with_call`] replaces the
//! selected blocks by one call to its published revision, wired from the same
//! sources and read by the same steps.
use super::{
    ComponentDraft, DraftError, EditError, initialize_inputs, initialize_outputs,
    materialize_component, unique_id,
};
use crate::{
    format::{Binding, Component, Graph, Node, OrderEdge, Position, Project, ProxySource},
    graph::{component_definition, create_node, output_ports},
    handle::unique_handle,
    js,
    model::{NodeKind, ValueType},
};
use std::collections::HashSet;

/// The board being extracted from and what it is read by.
#[derive(Clone, Copy)]
pub struct Extraction<'a> {
    /// The board holding the selection.
    pub graph: &'a Graph,
    /// Nodes the board's bindings may read besides its own: the scenario's
    /// main graph for its cleanup board, else the board itself.
    pub reference: &'a Graph,
    /// Boards that run after this one and may read its outputs: the cleanup
    /// for a scenario's main board.
    pub later: &'a [&'a Graph],
    pub selected: &'a HashSet<&'a str>,
}

impl Extraction<'_> {
    fn readers(&self) -> impl Iterator<Item = &Node> {
        self.graph
            .nodes
            .iter()
            .filter(|node| !self.selected.contains(node.id.as_str()))
            .chain(self.later.iter().flat_map(|graph| graph.nodes.iter()))
    }

    /// The outputs of selected blocks that a step outside the selection
    /// reads, as `(node, port)` in reading order.
    pub fn read_after(&self) -> Vec<(String, String)> {
        let mut read: Vec<(String, String)> = Vec::new();
        for node in self.readers() {
            for binding in node.inputs.values() {
                for reference in binding.references() {
                    let key = (reference.node_id.to_owned(), reference.port.to_owned());
                    if self.selected.contains(reference.node_id) && !read.contains(&key) {
                        read.push(key);
                    }
                }
            }
        }
        read
    }

    fn source(&self, node_id: &str) -> Option<&Node> {
        self.graph
            .nodes
            .iter()
            .chain(&self.reference.nodes)
            .find(|node| node.id == node_id)
    }
}

/// The draft the *Save as component* dialog starts from.
///
/// An input read from another step takes that output's type and contract
/// when its own slot accepts any value; a value that becomes an input's
/// default leaves it optional. The outputs a later step reads are ticked,
/// or the first output of the last block when nothing is read. A selected
/// call whose component has an action offers the new one after the same
/// kind of output.
pub fn propose_extraction(
    extraction: &Extraction<'_>,
    project: &Project,
    name: String,
) -> ComponentDraft {
    let Extraction {
        graph, selected, ..
    } = *extraction;
    let mut inputs = initialize_inputs(graph, selected, project, None);
    for row in &mut inputs {
        if let Some(Binding::Reference {
            node_id,
            port,
            path,
        }) = &row.original
            && path.is_empty()
            && row.value_type == ValueType::Any
            && let Some(source) = extraction.source(node_id)
            && let Some(output) = output_ports(source, project)
                .into_iter()
                .find(|output| &output.name == port)
        {
            row.value_type = output.value_type;
            if row.contract_id.is_none() {
                row.contract_id = output.contract_id;
            }
        }
        if row.default_value.is_some() {
            row.required = false;
        }
    }
    let mut outputs = initialize_outputs(graph, selected, project, None);
    let read = extraction.read_after();
    if outputs
        .iter()
        .any(|row| read.contains(&(row.node_id.clone(), row.port.clone())))
    {
        for row in &mut outputs {
            row.enabled = read.contains(&(row.node_id.clone(), row.port.clone()));
        }
    }
    let action = graph
        .nodes
        .iter()
        .filter(|node| selected.contains(node.id.as_str()))
        .find_map(|node| {
            let action = component_definition(project, node)?.action.as_ref()?;
            let row = inputs
                .iter()
                .find(|row| row.node_id == node.id && row.input == action.input && row.enabled)?;
            Some((
                row.key.clone(),
                action.value_type,
                action.contract_id.clone(),
            ))
        });
    ComponentDraft {
        name,
        description: String::new(),
        inputs,
        outputs,
        action_type: action.as_ref().map_or(ValueType::Any, |action| action.1),
        action_contract: action.as_ref().and_then(|action| action.2.clone()),
        action: action.map(|action| action.0),
    }
}

/// The outputs a step after the selection reads that the draft does not
/// keep, by the name of the block and port, e.g. `Add to cart → cart`.
pub fn unkept_outputs(extraction: &Extraction<'_>, draft: &ComponentDraft) -> Vec<String> {
    extraction
        .read_after()
        .into_iter()
        .filter(|(node, port)| {
            !draft
                .outputs
                .iter()
                .any(|row| row.enabled && &row.node_id == node && &row.port == port)
        })
        .map(|(node, port)| {
            let name = extraction
                .source(&node)
                .map_or(node.as_str(), Node::display_name);
            format!("{name} → {port}")
        })
        .collect()
}

/// The definition a valid draft describes, its inputs written as a folder
/// project writes them: `required` and `exposed` only where they differ
/// from what the input's default implies (KTLO-32).
pub fn extraction_component(
    extraction: &Extraction<'_>,
    draft: &ComponentDraft,
) -> Result<Component, EditError> {
    let mut component = materialize_component(extraction.graph, extraction.selected, draft, None)?;
    for port in &mut component.inputs {
        let required = port.required.unwrap_or(true);
        let has_default = port.default().is_some();
        port.required = (required == has_default).then_some(required);
        port.exposed = (port.exposed == Some(false)).then_some(false);
    }
    Ok(component)
}

/// The board, and the boards after it, with the selection replaced by a call.
#[derive(Clone, Debug, PartialEq)]
pub struct Replaced {
    pub graph: Graph,
    pub later: Vec<Graph>,
    /// The call's node.
    pub call: String,
}

/// Replaces the selected blocks by one call to `component`, the published
/// revision of the draft's definition.
///
/// The call takes each input from the binding it replaces; a value that
/// became the input's default is left to it. Every step after the selection
/// that read a kept output reads the call's output of the same name, and
/// ordering connections that crossed the selection start or end at the call.
pub fn replace_with_call(
    extraction: &Extraction<'_>,
    draft: &ComponentDraft,
    component: &Component,
) -> Result<Replaced, EditError> {
    let Extraction {
        graph, selected, ..
    } = *extraction;
    let unkept = unkept_outputs(extraction, draft);
    if let Some(output) = unkept.into_iter().next() {
        return Err(EditError::OutputNotKept(output));
    }
    if graph.edges.iter().any(|edge| {
        edge.branch.is_some()
            && selected.contains(edge.source.as_str())
            && !selected.contains(edge.target.as_str())
    }) {
        return Err(EditError::BranchLeavesSelection);
    }
    let chosen: Vec<&Node> = graph
        .nodes
        .iter()
        .filter(|node| selected.contains(node.id.as_str()))
        .collect();
    let Some(first) = chosen.first() else {
        return Err(EditError::InvalidDraft(vec![DraftError::NoBlocks]));
    };
    let position = Position {
        x: chosen
            .iter()
            .map(|node| node.position.x)
            .fold(f64::INFINITY, f64::min),
        y: chosen
            .iter()
            .map(|node| node.position.y)
            .fold(f64::INFINITY, f64::min),
    };

    let mut call = create_node(NodeKind::Component, position, Some(component));
    call.inputs.clear();
    call.label = Some(component.name.clone());
    let kept: Vec<&Node> = graph
        .nodes
        .iter()
        .chain(extraction.later.iter().flat_map(|graph| graph.nodes.iter()))
        .filter(|node| !selected.contains(node.id.as_str()))
        .collect();
    call.id = unique_id("node", |id| {
        kept.iter().chain(&chosen).any(|node| node.id == id)
    });
    call.handle = unique_handle(&component.handle, |handle| {
        kept.iter().any(|node| node.handle == handle)
    });
    for row in draft.inputs.iter().filter(|row| row.enabled) {
        let Some(original) = &row.original else {
            continue;
        };
        if matches!(original, Binding::Literal { value } if Some(value) == row.default_value.as_ref())
        {
            continue;
        }
        call.inputs
            .insert(js::trim(&row.name).to_owned(), original.clone());
    }

    // A reader of a kept output reads the call's output of that name.
    let outputs: Vec<(&str, &str, &str, &str)> = draft
        .outputs
        .iter()
        .filter(|row| row.enabled)
        .map(|row| {
            (
                row.node_id.as_str(),
                row.port.as_str(),
                row.path.as_str(),
                js::trim(&row.name),
            )
        })
        .collect();
    let call_id = call.id.clone();
    let redirect = |binding: &Binding| -> Binding {
        binding
            .map_leaves(&mut |leaf, _| {
                Some(match leaf {
                    Binding::Reference {
                        node_id,
                        port,
                        path,
                    } if selected.contains(node_id.as_str()) => outputs
                        .iter()
                        .find_map(|(node, output, prefix, name)| {
                            if node != node_id || output != port {
                                return None;
                            }
                            let rest = if prefix.is_empty() {
                                path.as_str()
                            } else if path == prefix {
                                ""
                            } else {
                                path.strip_prefix(prefix)?.strip_prefix('.')?
                            };
                            Some(Binding::reference(&call_id, *name, rest))
                        })
                        .unwrap_or_else(|| leaf.clone()),
                    other => other.clone(),
                })
            })
            .unwrap_or_else(|| binding.clone())
    };
    let redirect_node = |node: &Node| -> Node {
        let mut node = node.clone();
        for binding in node.inputs.values_mut() {
            *binding = redirect(binding);
        }
        node
    };

    let mut nodes = Vec::with_capacity(graph.nodes.len());
    for node in &graph.nodes {
        if node.id == first.id {
            nodes.push(call.clone());
        }
        if !selected.contains(node.id.as_str()) {
            nodes.push(redirect_node(node));
        }
    }
    let mut edges: Vec<OrderEdge> = Vec::new();
    for edge in &graph.edges {
        let inside = (
            selected.contains(edge.source.as_str()),
            selected.contains(edge.target.as_str()),
        );
        if inside == (true, true) {
            continue;
        }
        let mut edge = edge.clone();
        if inside.0 {
            edge.source = call_id.clone();
        }
        if inside.1 {
            edge.target = call_id.clone();
        }
        let duplicate = edges.iter().any(|other| {
            other.source == edge.source
                && other.target == edge.target
                && other.branch == edge.branch
        });
        if !duplicate {
            edges.push(edge);
        }
    }
    let proxies = graph
        .proxies
        .iter()
        .filter(|proxy| {
            !matches!(&proxy.source, ProxySource::Variable { node_id } if selected.contains(node_id.as_str()))
        })
        .cloned()
        .collect();
    let later = extraction
        .later
        .iter()
        .map(|later| Graph {
            nodes: later.nodes.iter().map(redirect_node).collect(),
            edges: later.edges.clone(),
            proxies: later.proxies.clone(),
        })
        .collect();
    Ok(Replaced {
        graph: Graph {
            nodes,
            edges,
            proxies,
        },
        later,
        call: call_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demos;
    use crate::format::Scenario;

    fn shop() -> (Project, Scenario) {
        let project = demos::shop_project();
        let scenario = project.scenarios[0].clone();
        (project, scenario)
    }

    #[test]
    fn the_demo_selection_proposes_the_mock_ups_rows() {
        let (project, scenario) = shop();
        let selected: HashSet<&str> = ["cart", "price"].into();
        let later = [&scenario.cleanup];
        let extraction = Extraction {
            graph: &scenario.graph,
            reference: &scenario.graph,
            later: &later,
            selected: &selected,
        };
        let draft = propose_extraction(&extraction, &project, "Check cart price".into());
        let inputs: Vec<_> = draft
            .inputs
            .iter()
            .map(|row| {
                (
                    row.name.as_str(),
                    row.value_type,
                    row.contract_id.as_deref(),
                    row.required,
                    row.enabled,
                )
            })
            .collect();
        assert_eq!(
            inputs,
            vec![
                ("product", ValueType::Object, Some("Product"), true, true),
                ("quantity", ValueType::Number, None, false, true),
                ("expected", ValueType::Number, None, true, true),
            ]
        );
        let outputs: Vec<_> = draft
            .outputs
            .iter()
            .map(|row| (row.name.as_str(), row.enabled))
            .collect();
        assert_eq!(
            outputs,
            vec![("cart", true), ("value", false), ("passed", false)]
        );
        assert_eq!(draft.action.as_deref(), Some("cart:product:"));
        assert_eq!(draft.action_type, ValueType::Object);
        assert_eq!(draft.action_contract.as_deref(), Some("Product"));
        assert!(unkept_outputs(&extraction, &draft).is_empty());
    }

    #[test]
    fn the_call_replaces_the_steps_and_feeds_their_readers() {
        let (project, scenario) = shop();
        let selected: HashSet<&str> = ["cart", "price"].into();
        let later = [&scenario.cleanup];
        let extraction = Extraction {
            graph: &scenario.graph,
            reference: &scenario.graph,
            later: &later,
            selected: &selected,
        };
        let draft = propose_extraction(&extraction, &project, "Check cart price".into());
        let component = extraction_component(&extraction, &draft).unwrap();
        assert_eq!(component.handle, "check_cart_price");
        let quantity = &component.inputs[1];
        assert_eq!((quantity.required, quantity.exposed), (None, None));
        let replaced = replace_with_call(&extraction, &draft, &component).unwrap();
        let handles: Vec<&str> = replaced
            .graph
            .nodes
            .iter()
            .map(|node| node.handle.as_str())
            .collect();
        assert_eq!(
            handles,
            [
                "product",
                "active_promotion",
                "expected_price",
                "check_cart_price",
                "field_inna_nazwa"
            ]
        );
        let call = &replaced.graph.nodes[3];
        assert_eq!(call.id, replaced.call);
        let arguments: Vec<_> = call.inputs.iter().collect();
        assert_eq!(
            arguments,
            vec![
                (
                    &"product".to_owned(),
                    &Binding::reference("active", "product", "")
                ),
                (
                    &"expected".to_owned(),
                    &Binding::reference("expected", "value", "")
                ),
            ]
        );
        let delete = &replaced.later[0].nodes[0];
        assert!(
            delete
                .inputs
                .values()
                .any(|binding| binding
                    .references()
                    .iter()
                    .any(|reference| reference.node_id == call.id
                        && reference.port == "cart"
                        && reference.path == "id"))
        );
    }

    #[test]
    fn an_unticked_output_that_is_read_is_refused() {
        let (project, scenario) = shop();
        let selected: HashSet<&str> = ["cart", "price"].into();
        let later = [&scenario.cleanup];
        let extraction = Extraction {
            graph: &scenario.graph,
            reference: &scenario.graph,
            later: &later,
            selected: &selected,
        };
        let mut draft = propose_extraction(&extraction, &project, "Check".into());
        draft.outputs[0].enabled = false;
        draft.outputs[1].enabled = true;
        assert_eq!(
            unkept_outputs(&extraction, &draft),
            vec!["Add to cart → cart".to_owned()]
        );
        let component = extraction_component(&extraction, &draft).unwrap();
        assert!(matches!(
            replace_with_call(&extraction, &draft, &component),
            Err(EditError::OutputNotKept(_))
        ));
    }
}
