//! Extracting selected blocks into a reusable component definition.
//!
//! The builder proposes a public argument for every input leaf fed from outside the
//! selection and one output per port of the selection; the user edits the
//! [`ComponentDraft`] and [`materialize_component`] turns it into a definition. The
//! scenario itself is never changed.
use super::EditError;
use crate::{
    format::{Binding, Component, ComponentAction, Graph, OptionalJson, OutputPort, Port, Project},
    graph::{input_ports, output_ports},
    js,
    model::ValueType,
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
    #[error("{0}: use letters without diacritics, digits, _ or -, and start with a letter or _.")]
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
                    Some(Binding::Parameter { name }) => Some(name.as_str()),
                    _ => None,
                };
                if let Some(parameter) = parameter
                    && let Some(existing) = rows.iter_mut().find(|row| {
                        matches!(&row.original, Some(Binding::Parameter { name }) if name == parameter)
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
                    label.push_str(if name == "left" { " → A" } else { " → B" });
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
                    enabled: mandatory || (path.is_empty() && port.exposed.unwrap_or(false)),
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
                    original: original.cloned(),
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
            |row| matches!(&row.original, Some(Binding::Parameter { name }) if *name == port.name),
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

/// Proposes outputs: those of the revised definition, or every output port of the
/// selection with only the first port of the last selected block enabled.
pub fn initialize_outputs(
    graph: &Graph,
    selected: &HashSet<&str>,
    project: &Project,
    initial: Option<&Component>,
) -> Vec<OutputRow> {
    if let Some(initial) = initial {
        return initial
            .outputs
            .iter()
            .enumerate()
            .map(|(index, output)| {
                let (node_id, port, path) = match &output.binding {
                    Binding::Reference {
                        node_id,
                        port,
                        path,
                    } => (node_id.clone(), port.clone(), path.clone()),
                    _ => Default::default(),
                };
                OutputRow {
                    key: format!("output-{index}"),
                    enabled: true,
                    name: output.name.clone(),
                    label: output.label.clone(),
                    value_type: output.value_type,
                    contract_id: output.contract_id.clone(),
                    node_id,
                    port,
                    path,
                    original_binding: Some(output.binding.clone()),
                }
            })
            .collect();
    }
    let nodes: Vec<_> = selected_nodes(graph, selected).collect();
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

/// Public names: an ASCII letter or `_`, then ASCII letters, digits, `_` or `-`.
fn valid_public_name(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && characters.all(|next| next.is_ascii_alphanumeric() || matches!(next, '_' | '-'))
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

    let mut nodes = Vec::new();
    for node in selected_nodes(graph, selected) {
        let mut node = node.clone();
        for row in &inputs {
            let parameter = Binding::parameter(js::trim(&row.name));
            for target in row
                .targets
                .iter()
                .filter(|target| target.node_id == node.id)
            {
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
        nodes.push(node);
    }

    let renamed: HashMap<&str, &str> = inputs
        .iter()
        .filter_map(|row| match &row.original {
            Some(Binding::Parameter { name }) => Some((name.as_str(), js::trim(&row.name))),
            _ => None,
        })
        .collect();
    let outputs = outputs
        .map(|row| {
            let binding = match &row.original_binding {
                Some(binding) if !matches!(binding, Binding::Reference { .. }) => binding.clone(),
                _ => Binding::reference(&row.node_id, &row.port, &row.path),
            };
            let binding = binding
                .map_leaves(&mut |leaf, _| {
                    Some(match leaf {
                        Binding::Parameter { name } => renamed
                            .get(name.as_str())
                            .map_or_else(|| leaf.clone(), |name| Binding::parameter(*name)),
                        other => other.clone(),
                    })
                })
                .unwrap_or(binding);
            OutputPort {
                name: js::trim(&row.name).to_owned(),
                value_type: row.value_type,
                label: row.label.clone(),
                contract_id: non_empty(&row.contract_id),
                required: None,
                default_value: OptionalJson::Missing,
                exposed: None,
                binding,
            }
        })
        .collect();

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
        outputs,
        graph: Graph {
            nodes,
            edges: graph
                .edges
                .iter()
                .filter(|edge| {
                    selected.contains(edge.source.as_str())
                        && selected.contains(edge.target.as_str())
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
