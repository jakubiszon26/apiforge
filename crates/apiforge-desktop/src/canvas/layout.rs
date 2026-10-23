//! Card geometry in board (world) coordinates.
//!
//! Geometry is derived from the document every frame and never stored in it.

use crate::labels;
use apiforge_core::{
    format::{Binding, Graph, Node, Port, Project},
    graph::{
        FALSE_PORT, ORDER_PORT, TRUE_PORT, formula_operator, input_ports, output_ports, predicate,
    },
    model::{NodeKind, Predicate, ValueType},
};
use eframe::egui::{Pos2, Rect, Vec2, pos2, vec2};

pub const WIDTH: f32 = 260.0;
const HEADER: f32 = 62.0;
const SUBTITLE: f32 = 24.0;
const INPUT_ROW: f32 = 46.0;
const OUTPUT_ROW: f32 = 22.0;
const FOOTER: f32 = 36.0;
const CONTROL: f32 = 28.0;

/// How an input row shows its value.
#[derive(Clone, Debug, PartialEq)]
pub enum RowValue {
    /// A literal edited directly on the card.
    Inline {
        text: String,
        value_type: ValueType,
    },
    /// The expected JSON type of a "has type" comparison.
    TypeChoice(String),
    /// A non-literal source, described in words.
    Source(String),
    /// A literal shown but edited in the inspector.
    Summary(String),
    Missing,
    Optional,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InputRow {
    pub name: String,
    pub label: String,
    pub required: bool,
    pub value: RowValue,
    pub port: Port,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OutputRow {
    pub name: String,
    pub label: String,
    pub type_label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub id: String,
    pub kind: NodeKind,
    pub name: String,
    pub caption: String,
    pub subtitle: Option<String>,
    pub inputs: Vec<InputRow>,
    pub outputs: Vec<OutputRow>,
    /// Control outputs: pseudo-port and label.
    pub controls: Vec<(&'static str, &'static str)>,
    pub origin: Pos2,
}

impl Card {
    pub fn build(node: &Node, reference_graph: &Graph, project: &Project) -> Self {
        let ports = input_ports(node, project);
        let inline_allowed = node.kind != NodeKind::Component && ports.len() <= 3;
        let inputs = ports
            .iter()
            .filter(|port| {
                // Unary comparisons have no second operand. Keep the document
                // binding intact when hidden, so changing the predicate is safe.
                port.name != "expected"
                    || !matches!(node.kind, NodeKind::Assert | NodeKind::If)
                    || predicate(node).takes_expected()
            })
            .map(|port| {
                let binding = node.inputs.get(&port.name);
                let value = row_value(node, port, binding, inline_allowed, reference_graph);
                InputRow {
                    name: port.name.clone(),
                    label: port.display_name().to_owned(),
                    required: port.is_required(),
                    value,
                    port: port.clone(),
                }
            })
            .collect();
        let outputs = output_ports(node, project)
            .into_iter()
            .map(|port| OutputRow {
                type_label: port
                    .contract_id
                    .as_ref()
                    .and_then(|id| project.contracts.iter().find(|contract| &contract.id == id))
                    .map_or_else(
                        || port.value_type.as_str().to_owned(),
                        |contract| contract.name.clone(),
                    ),
                label: port.display_name().to_owned(),
                name: port.name,
            })
            .collect();
        let caption = match (node.kind, node.config.revision) {
            (NodeKind::Component, Some(revision)) => format!("{} · r{revision}", node.kind.label()),
            _ => node.kind.label().to_owned(),
        };
        let subtitle = match node.kind {
            NodeKind::Request => Some(format!(
                "{}  {}",
                labels::method(
                    node.config
                        .method
                        .unwrap_or(apiforge_core::model::HttpMethod::Get)
                ),
                node.config
                    .url
                    .as_deref()
                    .filter(|url| !url.is_empty())
                    .unwrap_or("/endpoint")
            )),
            NodeKind::Select => Some(
                node.config
                    .path
                    .as_deref()
                    .filter(|path| !path.is_empty())
                    .unwrap_or("Whole value")
                    .to_owned(),
            ),
            NodeKind::Formula => Some(formula_operator(node).label().to_owned()),
            NodeKind::Assert | NodeKind::If => Some(predicate(node).label().to_owned()),
            _ => None,
        };
        let controls = match node.kind {
            NodeKind::If => vec![(TRUE_PORT, "Yes →"), (FALSE_PORT, "No →")],
            _ => vec![(ORDER_PORT, "Next →")],
        };
        Self {
            id: node.id.clone(),
            kind: node.kind,
            name: node.name.clone(),
            caption,
            subtitle,
            inputs,
            outputs,
            controls,
            origin: pos2(node.position.x as f32, node.position.y as f32),
        }
    }

    fn header_height(&self) -> f32 {
        HEADER
            + if self.subtitle.is_some() {
                SUBTITLE
            } else {
                0.0
            }
    }

    fn outputs_top(&self) -> f32 {
        self.header_height() + self.inputs.len() as f32 * INPUT_ROW
    }

    fn footer_top(&self) -> f32 {
        self.outputs_top() + self.outputs.len() as f32 * OUTPUT_ROW + 4.0
    }

    pub fn height(&self) -> f32 {
        self.footer_top() + FOOTER + CONTROL
    }

    pub fn rect(&self) -> Rect {
        Rect::from_min_size(self.origin, vec2(WIDTH, self.height()))
    }

    pub fn subtitle_center(&self) -> Pos2 {
        self.origin + vec2(14.0, HEADER + SUBTITLE / 2.0 - 4.0)
    }

    pub fn input_row_top(&self, index: usize) -> f32 {
        self.header_height() + index as f32 * INPUT_ROW
    }

    /// The editable field of an input row.
    pub fn input_field(&self, index: usize) -> Rect {
        let top = self.input_row_top(index);
        Rect::from_min_max(
            self.origin + vec2(14.0, top + 19.0),
            self.origin + vec2(WIDTH - 14.0, top + 41.0),
        )
    }

    pub fn input_row(&self, index: usize) -> Rect {
        let top = self.input_row_top(index);
        Rect::from_min_size(self.origin + vec2(0.0, top), vec2(WIDTH, INPUT_ROW))
    }

    pub fn output_y(&self, index: usize) -> f32 {
        self.outputs_top() + index as f32 * OUTPUT_ROW + OUTPUT_ROW / 2.0 + 2.0
    }

    pub fn outputs_divider(&self) -> f32 {
        self.outputs_top()
    }

    pub fn footer(&self) -> Rect {
        let top = self.footer_top();
        Rect::from_min_size(self.origin + vec2(0.0, top), vec2(WIDTH, FOOTER))
    }

    /// The "+ Action" button in the footer.
    pub fn action_button(&self) -> Rect {
        let footer = self.footer();
        Rect::from_min_size(footer.right_top() + vec2(-82.0, 7.0), vec2(70.0, 22.0))
    }

    /// The button opening a component's inner graph.
    pub fn open_button(&self) -> Rect {
        Rect::from_min_size(self.origin + vec2(WIDTH - 40.0, 14.0), vec2(28.0, 28.0))
    }

    pub fn control_button(&self, index: usize) -> Rect {
        let count = self.controls.len() as f32;
        let width = WIDTH / count;
        let top = self.footer_top() + FOOTER;
        Rect::from_min_size(
            self.origin + vec2(index as f32 * width, top),
            vec2(width, CONTROL),
        )
    }

    /// Anchor of a data input, or of the ordering input for [`ORDER_PORT`].
    pub fn input_anchor(&self, name: &str) -> Option<Pos2> {
        if name == ORDER_PORT {
            return (self.kind != NodeKind::Trigger).then(|| self.origin + vec2(WIDTH / 2.0, 0.0));
        }
        let index = self.inputs.iter().position(|row| row.name == name)?;
        Some(self.origin + vec2(0.0, self.input_row_top(index) + 30.0))
    }

    /// Anchor of a data output or a control pseudo-port.
    pub fn output_anchor(&self, name: &str) -> Option<Pos2> {
        if let Some(index) = self.controls.iter().position(|(port, _)| *port == name) {
            let button = self.control_button(index);
            return Some(pos2(button.center().x, self.rect().bottom()));
        }
        let index = self.outputs.iter().position(|row| row.name == name)?;
        Some(self.origin + vec2(WIDTH, self.output_y(index)))
    }

    pub fn translated(&self, offset: Vec2) -> Self {
        let mut card = self.clone();
        card.origin += offset;
        card
    }
}

fn row_value(
    node: &Node,
    port: &Port,
    binding: Option<&Binding>,
    inline_allowed: bool,
    reference_graph: &Graph,
) -> RowValue {
    match binding {
        None if port.is_required() => RowValue::Missing,
        None => RowValue::Optional,
        Some(Binding::Literal { value }) => {
            if node.kind.is_check() && predicate(node) == Predicate::Type && port.name == "expected"
            {
                return RowValue::TypeChoice(value.as_str().unwrap_or_default().to_owned());
            }
            let text = match value {
                serde_json::Value::String(text) => text.clone(),
                other => apiforge_core::js::json_stringify(other),
            };
            let inline = port.exposed.unwrap_or(false) || (inline_allowed && port.name != "body");
            if inline {
                RowValue::Inline {
                    text,
                    value_type: port.value_type,
                }
            } else {
                RowValue::Summary(labels::literal_summary(value))
            }
        }
        Some(other) => RowValue::Source(labels::binding(other, reference_graph)),
    }
}

/// Converts text typed into an inline field to a literal of the port's type.
///
/// Text in an `any` port is parsed as JSON when possible and kept as a string
/// otherwise; typed ports reject text that is not a value of their type.
pub fn parse_inline(text: &str, value_type: ValueType) -> Result<serde_json::Value, String> {
    use serde_json::Value;
    let parsed = || serde_json::from_str::<Value>(text);
    let invalid = || format!("Enter a valid {} value", value_type.as_str());
    match value_type {
        ValueType::Any => Ok(parsed().unwrap_or_else(|_| Value::String(text.to_owned()))),
        ValueType::String => Ok(Value::String(text.to_owned())),
        ValueType::Number => text
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite() && !text.trim().is_empty())
            .and_then(apiforge_core::js::number_value)
            .ok_or_else(invalid),
        ValueType::Boolean => match parsed() {
            Ok(value @ Value::Bool(_)) => Ok(value),
            _ => Err(invalid()),
        },
        ValueType::Object => match parsed() {
            Ok(value @ Value::Object(_)) => Ok(value),
            _ => Err(invalid()),
        },
        ValueType::Array => match parsed() {
            Ok(value @ Value::Array(_)) => Ok(value),
            _ => Err(invalid()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_core::{ImportLimits, import_document};
    use serde_json::json;

    #[test]
    fn every_fixture_card_has_anchors_for_its_wires() {
        let workspace = import_document(
            include_bytes!("../../../../fixtures/compatibility/workspace-v1.json"),
            &ImportLimits::default(),
        )
        .unwrap()
        .into_workspace();
        for project in &workspace.projects {
            for scenario in &project.scenarios {
                for graph in [&scenario.graph, &scenario.cleanup] {
                    for wire in apiforge_core::graph::wires(graph) {
                        let Some(source) = graph.nodes.iter().find(|node| node.id == wire.source)
                        else {
                            continue; // cleanup references into the main graph
                        };
                        let target = graph
                            .nodes
                            .iter()
                            .find(|node| node.id == wire.target)
                            .unwrap();
                        let source = Card::build(source, graph, project);
                        let target = Card::build(target, graph, project);
                        let (output, input) = match wire.kind {
                            apiforge_core::graph::WireKind::Order => (
                                match wire.branch {
                                    Some(apiforge_core::model::Branch::True) => TRUE_PORT,
                                    Some(apiforge_core::model::Branch::False) => FALSE_PORT,
                                    None => ORDER_PORT,
                                },
                                ORDER_PORT,
                            ),
                            apiforge_core::graph::WireKind::Data => (
                                wire.source_port.as_deref().unwrap(),
                                wire.target_input.as_deref().unwrap(),
                            ),
                        };
                        assert!(
                            source.output_anchor(output).is_some(),
                            "{} {output}",
                            source.name
                        );
                        assert!(
                            target.input_anchor(input).is_some(),
                            "{} {input}",
                            target.name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn unary_comparison_shows_its_predicate_and_only_one_operand() {
        let project = apiforge_core::demos::empty_project();
        let graph = &project.scenarios[0].graph;
        let mut node = apiforge_core::graph::create_node(
            NodeKind::Assert,
            apiforge_core::format::Position { x: 0.0, y: 0.0 },
            None,
        );
        node.config.predicate = Some(Predicate::NotEmpty);
        let card = Card::build(&node, graph, &project);
        assert_eq!(card.subtitle.as_deref(), Some("is not empty"));
        assert!(card.inputs.iter().any(|row| row.name == "actual"));
        assert!(!card.inputs.iter().any(|row| row.name == "expected"));
        node.config.predicate = Some(Predicate::Equals);
        let card = Card::build(&node, graph, &project);
        assert!(card.inputs.iter().any(|row| row.name == "expected"));
    }

    #[test]
    fn inline_values_are_parsed_by_port_type() {
        assert_eq!(parse_inline("12", ValueType::Number), Ok(json!(12)));
        assert!(parse_inline("12a", ValueType::Number).is_err());
        assert!(parse_inline(" ", ValueType::Number).is_err());
        assert_eq!(parse_inline("[1]", ValueType::Any), Ok(json!([1])));
        assert_eq!(parse_inline("hello", ValueType::Any), Ok(json!("hello")));
        assert_eq!(parse_inline("123", ValueType::String), Ok(json!("123")));
        assert!(parse_inline("1", ValueType::Boolean).is_err());
        assert_eq!(parse_inline("{}", ValueType::Object), Ok(json!({})));
    }
}
