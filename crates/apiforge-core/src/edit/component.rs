//! Extracting selected blocks into a reusable component definition.
//!
//! The builder proposes a public argument for every input leaf fed from outside the
//! selection and one output per port of the selection; the user edits the
//! [`ComponentDraft`] and [`materialize_component`] turns it into a definition whose
//! outputs are Export blocks. The scenario itself is never changed.
use super::{EditError, unique_id};
use crate::{
    format::{Binding, Component, ComponentAction, Graph, Node, OptionalJson, Port, Project},
    graph::{VALUE_INPUT, create_node, export_position, input_ports, output_ports},
    handle::{handle_base, is_valid_handle, unique_handle},
    js,
    model::{NodeKind, ValueType},
};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fmt,
};
use thiserror::Error;

/// A node input, or an operand leaf inside it, that a public argument feeds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputTarget {
    pub node_id: String,
    pub input: String,
    /// Operand path inside the input's expression; empty for the whole input.
    pub path: Vec<String>,
}

/// A proposed public argument of the component.
#[derive(Clone, Debug, PartialEq)]
pub struct InputRow {
    pub key: String,
    /// Node of the first target; empty for an argument without targets.
    pub node_id: String,
    pub input: String,
    /// Description of the first target, e.g. `Check → Actual value → A`.
    pub label: String,
    pub enabled: bool,
    /// The binding comes from outside the selection, so the row must stay enabled.
    pub mandatory: bool,
    pub name: String,
    pub value_type: ValueType,
    pub contract_id: Option<String>,
    pub exposed: bool,
    pub required: bool,
    /// The binding the argument replaces; instances pass it to keep behaviour.
    pub original: Option<Binding>,
    pub targets: Vec<InputTarget>,
    /// `None` is no default; `Some(Value::Null)` is a null default.
    pub default_value: Option<Value>,
    pub public_label: Option<String>,
}

/// A proposed output of the component.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputRow {
    pub key: String,
    /// Identity of an Export already present on the edited board.
    pub export_id: Option<String>,
    pub enabled: bool,
    pub name: String,
    pub value_type: ValueType,
    pub contract_id: Option<String>,
    pub node_id: String,
    pub port: String,
    pub path: String,
    /// Binding of the revised definition's output; non-reference bindings are kept.
    pub original_binding: Option<Binding>,
    pub label: Option<String>,
}

/// Everything the component builder edits before saving.
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentDraft {
    pub name: String,
    pub description: String,
    pub inputs: Vec<InputRow>,
    pub outputs: Vec<OutputRow>,
    /// Key of the input row that receives the value a component is dropped onto.
    pub action: Option<String>,
    pub action_type: ValueType,
    pub action_contract: Option<String>,
}

/// Which list of a draft a naming problem is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DraftSection {
    Arguments,
    Outputs,
}

impl fmt::Display for DraftSection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Arguments => "Arguments",
            Self::Outputs => "Outputs",
        })
    }
}

/// A problem that prevents saving a draft. The messages are user-facing.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum DraftError {
    #[error("Select at least one block.")]
    NoBlocks,
    #[error("Enter a component name.")]
    MissingName,
    #[error("Choose at least one output.")]
    NoOutputs,
    #[error("Bindings from outside the component must be public arguments.")]
    BoundaryNotPublic,
    #[error("An output refers to a removed step. Choose a new source.")]
    RemovedOutputSource,
    #[error("{0}: names cannot be empty.")]
    EmptyName(DraftSection),
    #[error("{0}: names must be unique.")]
    DuplicateName(DraftSection),
    #[error(
        "{0}: use letters without diacritics, digits and _, start with a letter or _, and avoid reserved words."
    )]
    InvalidName(DraftSection),
    #[error("Choose an available action argument.")]
    UnknownAction,
}

fn selected_nodes<'a>(
    graph: &'a Graph,
    selected: &'a HashSet<&str>,
) -> impl Iterator<Item = &'a crate::format::Node> {
    graph
        .nodes
        .iter()
        .filter(|node| selected.contains(node.id.as_str()))
}

fn operand_port(operand: &crate::operator::OperandSpec) -> Port {
    Port::new(operand.name, operand.value_type)
        .labelled(operand.label)
        .required(operand.required)
}

/// Proposes public arguments for the selected blocks.
///
/// Every input leaf that is not fed from inside the selection gets a row; leaves
/// reading the same parameter share one row. External references and parameters
/// are mandatory. When revising `initial`, its public arguments keep their
/// metadata and arguments without targets are kept as well.
pub fn initialize_inputs(
    graph: &Graph,
    selected: &HashSet<&str>,
    project: &Project,
    initial: Option<&Component>,
) -> Vec<InputRow> {
    let mut used: HashSet<String> = HashSet::new();
    let mut rows: Vec<InputRow> = Vec::new();
    for node in selected_nodes(graph, selected) {
        let declared = input_ports(node, project);
        let undeclared = node
            .inputs
            .keys()
            .filter(|name| !declared.iter().any(|port| &port.name == *name))
            .map(|name| Port::new(name.clone(), ValueType::Any));
        let ports: Vec<Port> = declared.iter().cloned().chain(undeclared).collect();
        for port in &ports {
            let root = node.inputs.get(&port.name);
            let leaves: Vec<(Vec<String>, Option<&Binding>)> = match root {
                Some(root) if root.is_composite() => root
                    .leaves()
                    .into_iter()
                    .map(|(path, leaf)| (path, Some(leaf)))
                    .collect(),
                other => vec![(vec![], other)],
            };
            for (path, original) in leaves {
                let target = InputTarget {
                    node_id: node.id.clone(),
                    input: port.name.clone(),
                    path: path.clone(),
                };
                // A leaf's contract comes from its own operator slot, not the outer result.
                let mut leaf_port = port.clone();
                let mut cursor = root;
                for name in &path {
                    let Some(Binding::Expression { operator, inputs }) = cursor else {
                        break;
                    };
                    if let Some(operand) = operator.spec().operand(name) {
                        leaf_port = operand_port(operand);
                    }
                    cursor = inputs.get(name);
                }
                match original {
                    Some(Binding::Reference { node_id, .. })
                        if selected.contains(node_id.as_str()) =>
                    {
                        continue;
                    }
                    Some(Binding::Environment { .. }) => continue,
                    _ => {}
                }
                let parameter = match original {
                    Some(Binding::Parameter { name, .. }) => Some(name.as_str()),
                    _ => None,
                };
                if let Some(parameter) = parameter
                    && let Some(existing) = rows.iter_mut().find(|row| {
                        matches!(&row.original, Some(Binding::Parameter { name, .. }) if name == parameter)
                    })
                {
                    existing.targets.push(target);
                    continue;
                }
                let public = parameter.and_then(|parameter| {
                    initial?.inputs.iter().find(|input| input.name == parameter)
                });
                let mut proposed = match parameter {
                    Some(parameter) => parameter.to_owned(),
                    None => std::iter::once(port.name.as_str())
                        .chain(path.iter().map(String::as_str))
                        .collect::<Vec<_>>()
                        .join("_"),
                };
                let mut suffix = 2;
                while used.contains(&proposed) {
                    proposed = format!("{}_{suffix}", port.name);
                    suffix += 1;
                }
                used.insert(proposed.clone());
                let mandatory = match original {
                    Some(Binding::Reference { .. } | Binding::Parameter { .. }) => true,
                    None => port.is_required(),
                    _ => false,
                };
                let mut label = format!("{} → {}", node.display_name(), port.display_name());
                for name in &path {
                    label.push_str(" → ");
                    label.push_str(match name.as_str() {
                        "left" => "A",
                        "right" => "B",
                        other => other,
                    });
                }
                let default_value = match (public.and_then(Port::default), original) {
                    (Some(default), _) => Some(default.clone()),
                    (None, Some(Binding::Literal { value })) => Some(value.clone()),
                    _ => None,
                };
                rows.push(InputRow {
                    key: format!("{}:{}:{}", node.id, port.name, path.join("/")),
                    node_id: node.id.clone(),
                    input: port.name.clone(),
                    label,
                    enabled: mandatory
                        || (node.kind != NodeKind::Export
                            && path.is_empty()
                            && port.exposed.unwrap_or(false)),
                    mandatory,
                    name: proposed,
                    value_type: public.map_or(leaf_port.value_type, |public| public.value_type),
                    contract_id: public
                        .and_then(|public| public.contract_id.clone())
                        .or_else(|| leaf_port.contract_id.clone()),
                    public_label: public.and_then(|public| public.label.clone()),
                    exposed: public
                        .and_then(|public| public.exposed)
                        .or(port.exposed)
                        .unwrap_or(true),
                    required: public
                        .and_then(|public| public.required)
                        .or(leaf_port.required)
                        .unwrap_or(true),
                    // An argument takes the whole parameter; each use keeps its field.
                    original: match original {
                        Some(Binding::Parameter { name, .. }) => Some(Binding::parameter(name)),
                        other => other.cloned(),
                    },
                    targets: vec![target],
                    default_value,
                });
            }
        }
    }
    // An argument may be returned directly, or kept for compatibility, without
    // feeding any block.
    for port in initial
        .map(|initial| initial.inputs.as_slice())
        .unwrap_or_default()
    {
        let covered = rows.iter().any(
            |row| matches!(&row.original, Some(Binding::Parameter { name, .. }) if *name == port.name),
        );
        if covered {
            continue;
        }
        rows.push(InputRow {
            key: format!("public:{}", port.name),
            node_id: String::new(),
            input: port.name.clone(),
            label: format!("Public argument → {}", port.display_name()),
            enabled: true,
            mandatory: true,
            name: port.name.clone(),
            value_type: port.value_type,
            contract_id: port.contract_id.clone(),
            exposed: port.exposed.unwrap_or(true),
            required: port.required.unwrap_or(false),
            original: Some(Binding::parameter(port.name.clone())),
            targets: vec![],
            default_value: port.default().cloned(),
            public_label: port.label.clone(),
        });
    }
    rows
}

/// Proposes outputs: the selected Export blocks of the current board, or every output
/// port of the selection with only the first port of the last selected block
/// enabled.
pub fn initialize_outputs(
    graph: &Graph,
    selected: &HashSet<&str>,
    project: &Project,
    initial: Option<&Component>,
) -> Vec<OutputRow> {
    if initial.is_some()
        || selected_nodes(graph, selected).any(|node| node.kind == NodeKind::Export)
    {
        return selected_nodes(graph, selected)
            .filter(|node| node.kind == NodeKind::Export)
            .map(|export| {
                let value = export.inputs.get(VALUE_INPUT);
                let (node_id, port, path) = match value {
                    Some(Binding::Reference {
                        node_id,
                        port,
                        path,
                    }) => (node_id.clone(), port.clone(), path.clone()),
                    _ => Default::default(),
                };
                OutputRow {
                    key: export.id.clone(),
                    export_id: Some(export.id.clone()),
                    enabled: true,
                    name: export.handle.clone(),
                    label: export.label.clone(),
                    value_type: export.config.output_type.unwrap_or(ValueType::Any),
                    contract_id: export.config.contract_id.clone(),
                    node_id,
                    port,
                    path,
                    original_binding: value.cloned(),
                }
            })
            .collect();
    }
    let nodes: Vec<_> = selected_nodes(graph, selected)
        .filter(|node| node.kind != NodeKind::Export)
        .collect();
    let last = nodes.len().saturating_sub(1);
    nodes
        .iter()
        .enumerate()
        .flat_map(|(index, node)| {
            output_ports(node, project)
                .into_iter()
                .enumerate()
                .map(move |(position, port)| OutputRow {
                    key: format!("{}:{}", node.id, port.name),
                    export_id: None,
                    enabled: index == last && position == 0,
                    name: port.name.clone(),
                    label: port.label.clone(),
                    value_type: port.value_type,
                    contract_id: port.contract_id.clone(),
                    node_id: node.id.clone(),
                    port: port.name,
                    path: String::new(),
                    original_binding: None,
                })
        })
        .collect()
}

/// Public names are written in scenario code (`param.name`, `cart.name`), so they
/// follow the rules of handles.
fn valid_public_name(name: &str) -> bool {
    is_valid_handle(name)
}

fn check_names<'a>(
    section: DraftSection,
    names: impl ExactSizeIterator<Item = &'a str> + Clone,
    errors: &mut Vec<DraftError>,
) {
    if names.clone().any(|name| js::trim(name).is_empty()) {
        errors.push(DraftError::EmptyName(section));
    }
    let count = names.len();
    let unique: HashSet<&str> = names.clone().map(js::trim).collect();
    if unique.len() != count {
        errors.push(DraftError::DuplicateName(section));
    }
    // Validated untrimmed: surrounding spaces are rejected, not silently accepted.
    if names.clone().any(|name| !valid_public_name(name)) {
        errors.push(DraftError::InvalidName(section));
    }
}

/// Every reason the draft cannot be saved, in display order.
pub fn validate_draft(
    graph: &Graph,
    selected: &HashSet<&str>,
    draft: &ComponentDraft,
) -> Vec<DraftError> {
    let nodes: Vec<_> = selected_nodes(graph, selected).collect();
    let inputs: Vec<&InputRow> = draft.inputs.iter().filter(|row| row.enabled).collect();
    let outputs: Vec<&OutputRow> = draft.outputs.iter().filter(|row| row.enabled).collect();
    let mut errors = Vec::new();
    if nodes.is_empty() {
        errors.push(DraftError::NoBlocks);
    }
    if js::trim(&draft.name).is_empty() {
        errors.push(DraftError::MissingName);
    }
    if outputs.is_empty() {
        errors.push(DraftError::NoOutputs);
    }
    if draft.inputs.iter().any(|row| row.mandatory && !row.enabled) {
        errors.push(DraftError::BoundaryNotPublic);
    }
    let removed_source = outputs.iter().any(|row| {
        matches!(row.original_binding, None | Some(Binding::Reference { .. }))
            && !nodes.iter().any(|node| node.id == row.node_id)
    });
    if removed_source {
        errors.push(DraftError::RemovedOutputSource);
    }
    check_names(
        DraftSection::Arguments,
        inputs.iter().map(|row| row.name.as_str()),
        &mut errors,
    );
    check_names(
        DraftSection::Outputs,
        outputs.iter().map(|row| row.name.as_str()),
        &mut errors,
    );
    if let Some(action) = &draft.action
        && !inputs.iter().any(|row| &row.key == action)
    {
        errors.push(DraftError::UnknownAction);
    }
    errors
}

fn non_empty(text: &Option<String>) -> Option<String> {
    text.clone().filter(|text| !text.is_empty())
}

/// Builds the component definition described by a valid draft.
///
/// Public argument targets of the copied blocks are replaced by parameters, and
/// outputs that read a renamed parameter follow the rename. Revising `initial`
/// keeps its identifier and revision; [`add_component`] assigns the next revision
/// when the result is saved. The graph is copied, never modified.
pub fn materialize_component(
    graph: &Graph,
    selected: &HashSet<&str>,
    draft: &ComponentDraft,
    initial: Option<&Component>,
) -> Result<Component, EditError> {
    let errors = validate_draft(graph, selected, draft);
    if !errors.is_empty() {
        return Err(EditError::InvalidDraft(errors));
    }
    let inputs: Vec<&InputRow> = draft.inputs.iter().filter(|row| row.enabled).collect();
    let outputs = draft.outputs.iter().filter(|row| row.enabled);

    let mut prepared = Vec::new();
    for node in selected_nodes(graph, selected) {
        let mut node = node.clone();
        for row in &inputs {
            let name = js::trim(&row.name);
            for target in row
                .targets
                .iter()
                .filter(|target| target.node_id == node.id)
            {
                // A parameter use keeps the field it selects.
                let parameter = match node
                    .inputs
                    .get(&target.input)
                    .and_then(|binding| binding.at(&target.path))
                {
                    Some(Binding::Parameter { path, .. }) => Binding::parameter_field(name, path),
                    _ => Binding::parameter(name),
                };
                let binding = if target.path.is_empty() {
                    parameter.clone()
                } else {
                    node.inputs
                        .get(&target.input)
                        .and_then(|binding| binding.replace_at(&target.path, parameter.clone()))
                        .ok_or(EditError::InvalidOperandPath)?
                };
                node.inputs.insert(target.input.clone(), binding);
            }
        }
        prepared.push(node);
    }
    let existing_exports: HashMap<&str, &Node> = prepared
        .iter()
        .filter(|node| node.kind == NodeKind::Export)
        .map(|node| (node.id.as_str(), node))
        .collect();
    let mut nodes: Vec<Node> = prepared
        .iter()
        .filter(|node| node.kind != NodeKind::Export)
        .cloned()
        .collect();

    let renamed: HashMap<&str, &str> = inputs
        .iter()
        .filter_map(|row| match &row.original {
            Some(Binding::Parameter { name, .. }) => Some((name.as_str(), js::trim(&row.name))),
            _ => None,
        })
        .collect();
    // Inner blocks give way to output names, which are the Exports' handles.
    let output_names: HashSet<String> = outputs
        .clone()
        .map(|row| js::trim(&row.name).to_owned())
        .collect();
    let mut handles: HashSet<String> = output_names.clone();
    for node in &mut nodes {
        if handles.contains(&node.handle) {
            node.handle = unique_handle(&node.handle, |handle| handles.contains(handle));
        }
        handles.insert(node.handle.clone());
    }
    let mut used_ids: HashSet<String> = prepared.iter().map(|node| node.id.clone()).collect();
    let exports: Vec<Node> = outputs
        .enumerate()
        .map(|(index, row)| {
            let existing = row
                .export_id
                .as_deref()
                .and_then(|id| existing_exports.get(id))
                .copied();
            let source_unchanged = match &row.original_binding {
                Some(Binding::Reference {
                    node_id,
                    port,
                    path,
                }) => node_id == &row.node_id && port == &row.port && path == &row.path,
                Some(_) => true,
                None => false,
            };
            let binding = match &row.original_binding {
                Some(binding) if !matches!(binding, Binding::Reference { .. }) => binding.clone(),
                _ => Binding::reference(&row.node_id, &row.port, &row.path),
            };
            // Exposing an Export's input as a public argument applies to that
            // Export too. An explicit source change in the builder takes priority.
            let binding = if source_unchanged {
                existing
                    .and_then(|export| {
                        let before = graph.nodes.iter().find(|node| node.id == export.id)?;
                        (before.inputs.get(VALUE_INPUT) != export.inputs.get(VALUE_INPUT))
                            .then(|| export.inputs.get(VALUE_INPUT).cloned())
                            .flatten()
                    })
                    .unwrap_or(binding)
            } else {
                binding
            };
            let binding = binding
                .map_leaves(&mut |leaf, _| {
                    Some(match leaf {
                        Binding::Parameter { name, path } => {
                            renamed.get(name.as_str()).map_or_else(
                                || leaf.clone(),
                                |name| Binding::parameter_field(*name, path),
                            )
                        }
                        other => other.clone(),
                    })
                })
                .unwrap_or(binding);
            let mut export = existing.cloned().unwrap_or_else(|| {
                let mut export =
                    create_node(NodeKind::Export, export_position(&nodes, index), None);
                export.id = unique_id("node", |id| used_ids.contains(id));
                used_ids.insert(export.id.clone());
                export
            });
            export.handle = js::trim(&row.name).to_owned();
            export.label = row.label.clone();
            if export.config.output_type.unwrap_or(ValueType::Any) != row.value_type {
                export.config.output_type =
                    (row.value_type != ValueType::Any).then_some(row.value_type);
            }
            export.config.contract_id = non_empty(&row.contract_id);
            export.inputs.insert(VALUE_INPUT.to_owned(), binding);
            export
        })
        .collect();
    nodes.extend(exports);
    // Existing Export positions in graph order determine the public port order.
    // Newly generated outputs follow them, without changing existing identities.
    let order: HashMap<&str, usize> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.id.as_str(), i))
        .collect();
    nodes.sort_by_key(|node| order.get(node.id.as_str()).copied().unwrap_or(usize::MAX));
    let kept_ids: HashSet<String> = nodes.iter().map(|node| node.id.clone()).collect();

    let action = draft.action.as_ref().and_then(|action| {
        inputs
            .iter()
            .find(|row| &row.key == action)
            .map(|row| ComponentAction {
                input: js::trim(&row.name).to_owned(),
                value_type: draft.action_type,
                contract_id: non_empty(&draft.action_contract),
            })
    });
    Ok(Component {
        id: initial.map_or_else(
            || crate::ids::new_id("component"),
            |initial| initial.id.clone(),
        ),
        revision: initial.map_or(1, |initial| initial.revision),
        handle: initial.map_or_else(
            || handle_base(js::trim(&draft.name)),
            |initial| initial.handle.clone(),
        ),
        name: js::trim(&draft.name).to_owned(),
        description: js::trim(&draft.description).to_owned(),
        inputs: inputs
            .iter()
            .map(|row| Port {
                name: js::trim(&row.name).to_owned(),
                value_type: row.value_type,
                label: row.public_label.clone(),
                contract_id: non_empty(&row.contract_id),
                required: Some(row.required),
                default_value: row
                    .default_value
                    .clone()
                    .map_or(OptionalJson::Missing, OptionalJson::Present),
                exposed: Some(row.exposed),
            })
            .collect(),
        graph: Graph {
            nodes,
            edges: graph
                .edges
                .iter()
                .filter(|edge| {
                    kept_ids.contains(edge.source.as_str())
                        && kept_ids.contains(edge.target.as_str())
                })
                .cloned()
                .collect(),
        },
        action,
    })
}

/// Adds a component definition to the project after validating the result.
///
/// With `revision_of`, the definition becomes the next revision of that component
/// (its identifier and the highest existing revision plus one), so existing
/// instances keep the revision they are pinned to. Otherwise the definition is
/// added as is; a clashing identifier and revision fails validation.
pub fn add_component(
    project: &Project,
    component: &Component,
    revision_of: Option<&str>,
) -> Result<Project, EditError> {
    let mut saved = component.clone();
    // A new component takes a handle no other component uses.
    if revision_of.is_none() {
        saved.handle = unique_handle(&saved.handle, |handle| {
            project
                .components
                .iter()
                .any(|other| other.id != saved.id && other.handle == handle)
        });
    }
    if let Some(id) = revision_of {
        let latest = project
            .components
            .iter()
            .filter(|existing| existing.id == id)
            .map(|existing| existing.revision)
            .max()
            .ok_or(EditError::ComponentNotFound)?;
        saved.id = id.to_owned();
        saved.revision = latest.saturating_add(1);
    }
    let mut next = project.clone();
    next.components.push(saved);
    crate::validate_project(&next)?;
    Ok(next)
}

#[cfg(test)]
mod tests;
