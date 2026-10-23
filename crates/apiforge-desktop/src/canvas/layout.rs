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

pub const WIDTH: f32 = 280.0;
pub const CARD_PADDING: f32 = 16.0;
const HEADER: f32 = 64.0;
const SUBTITLE: f32 = 24.0;
const INPUT_ROW: f32 = 52.0;
const OUTPUT_ROW: f32 = 28.0;
const FOOTER: f32 = 36.0;
const CONTROL: f32 = 28.0;

pub const OVERLAY_PADDING_LEFT: f32 = 18.0;
pub const OVERLAY_PADDING_RIGHT: f32 = 14.0;
pub const OVERLAY_PADDING_Y: f32 = 4.0;
pub const OVERLAY_GAP: f32 = 12.0;
pub const MIN_BANNER_SIDE_BY_SIDE: f32 = 140.0;
pub const FOOTER_BAND_HEIGHT: f32 = 44.0;
pub const MINIMAP_MIN_BOARD_WIDTH: f32 = 560.0;
pub const MINIMAP_MIN_BOARD_HEIGHT: f32 = 300.0;
pub const MINIMAP_SIZE: Vec2 = vec2(136.0, 88.0);
pub const FOCUS_MARGIN: f32 = 24.0;

/// Keep the same graph geometry while reducing visual detail at distant zooms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardDetail {
    Overview,
    Compact,
    Full,
}

impl CardDetail {
    pub fn at_zoom(zoom: f32) -> Self {
        if zoom < 0.45 {
            Self::Overview
        } else if zoom < 0.75 {
            Self::Compact
        } else {
            Self::Full
        }
    }
}

/// How the keyboard/mouse shortcut hint is shown in the footer band.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HintPlacement {
    /// Full one-line shortcut text rendered inline inside this rect.
    Inline(Rect),
    /// Compact "Shortcuts" affordance with the full text in a hover tooltip.
    Tooltip(Rect),
}

/// Measured widget/text sizes used to compute the board overlay geometry.
#[derive(Clone, Copy, Debug)]
pub struct OverlayMeasures {
    pub banner_height: f32,
    pub toolbar_size: Vec2,
    pub zoom_size: Vec2,
    pub hint_width: f32,
    pub shortcuts_badge_width: f32,
    pub error_size: Option<Vec2>,
}

/// Disjoint regions of the canvas viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoardOverlays {
    pub header: Rect,
    pub banner: Rect,
    pub banner_wrap_width: f32,
    pub toolbar: Rect,
    pub footer: Rect,
    pub zoom_controls: Rect,
    pub hint: HintPlacement,
    pub board: Rect,
    pub minimap: Option<Rect>,
    pub error_banner: Option<Rect>,
    pub focus: Rect,
}

/// Computes the wrap width available for the top-left mode banner.
pub fn banner_wrap_width(viewport_width: f32, toolbar_width: f32) -> f32 {
    let side_by_side =
        viewport_width - OVERLAY_PADDING_LEFT - OVERLAY_PADDING_RIGHT - toolbar_width - OVERLAY_GAP;
    if side_by_side >= MIN_BANNER_SIDE_BY_SIDE {
        side_by_side
    } else {
        (viewport_width - OVERLAY_PADDING_LEFT - OVERLAY_PADDING_RIGHT).max(80.0)
    }
}

/// Pure layout function placing header, footer, board, minimap and camera focus rects.
pub fn board_overlays(viewport: Rect, measures: &OverlayMeasures) -> BoardOverlays {
    let side_by_side = viewport.width()
        - OVERLAY_PADDING_LEFT
        - OVERLAY_PADDING_RIGHT
        - measures.toolbar_size.x
        - OVERLAY_GAP;
    let wrap_width = banner_wrap_width(viewport.width(), measures.toolbar_size.x);
    let banner_h = measures.banner_height.max(26.0);
    let banner = Rect::from_min_size(
        pos2(
            viewport.left() + OVERLAY_PADDING_LEFT,
            viewport.top() + OVERLAY_PADDING_Y,
        ),
        vec2(wrap_width, banner_h),
    );
    let toolbar = if side_by_side >= MIN_BANNER_SIDE_BY_SIDE {
        Rect::from_min_size(
            pos2(
                viewport.right() - OVERLAY_PADDING_RIGHT - measures.toolbar_size.x,
                viewport.top() + OVERLAY_PADDING_Y,
            ),
            measures.toolbar_size,
        )
    } else {
        let x = (viewport.right() - OVERLAY_PADDING_RIGHT - measures.toolbar_size.x)
            .max(viewport.left() + OVERLAY_PADDING_LEFT);
        Rect::from_min_size(pos2(x, banner.bottom() + 6.0), measures.toolbar_size)
    };
    let header_bottom =
        (banner.bottom().max(toolbar.bottom()) + OVERLAY_PADDING_Y).min(viewport.bottom());
    let header = Rect::from_min_max(viewport.min, pos2(viewport.right(), header_bottom));

    let footer_top = (viewport.bottom() - FOOTER_BAND_HEIGHT).max(header.bottom());
    let footer = Rect::from_min_max(pos2(viewport.left(), footer_top), viewport.max);
    let zoom_y = (footer.center().y - measures.zoom_size.y / 2.0).max(footer.top() + 2.0);
    let zoom_controls = Rect::from_min_size(
        pos2(footer.left() + OVERLAY_PADDING_RIGHT, zoom_y),
        measures.zoom_size,
    );
    let avail_for_hint =
        footer.right() - OVERLAY_PADDING_RIGHT - (zoom_controls.right() + OVERLAY_GAP);
    let hint = if measures.hint_width <= avail_for_hint {
        HintPlacement::Inline(Rect::from_min_size(
            pos2(zoom_controls.right() + OVERLAY_GAP, footer.top() + 4.0),
            vec2(measures.hint_width, (footer.height() - 8.0).max(16.0)),
        ))
    } else {
        let badge_w = measures.shortcuts_badge_width.max(76.0);
        let badge_x =
            (footer.right() - OVERLAY_PADDING_RIGHT - badge_w).max(zoom_controls.right() + 6.0);
        HintPlacement::Tooltip(Rect::from_min_size(
            pos2(badge_x, (footer.center().y - 11.0).max(footer.top() + 2.0)),
            vec2(badge_w, 22.0),
        ))
    };

    let board = Rect::from_min_max(
        pos2(viewport.left(), header.bottom()),
        pos2(viewport.right(), footer.top()),
    );
    let minimap =
        if board.width() >= MINIMAP_MIN_BOARD_WIDTH && board.height() >= MINIMAP_MIN_BOARD_HEIGHT {
            Some(Rect::from_min_size(
                board.right_bottom() - MINIMAP_SIZE - vec2(14.0, 14.0),
                MINIMAP_SIZE,
            ))
        } else {
            None
        };
    let error_banner = measures.error_size.map(|size| {
        Rect::from_center_size(
            pos2(board.center().x, board.top() + 10.0 + size.y / 2.0),
            size,
        )
    });
    let focus_right = minimap.map_or(board.right(), |mm| {
        (mm.left() - OVERLAY_GAP).max(board.left() + 80.0)
    });
    let focus_raw = Rect::from_min_max(board.min, pos2(focus_right, board.bottom()));
    let shrink_x = FOCUS_MARGIN.min(focus_raw.width() * 0.15);
    let shrink_y = FOCUS_MARGIN.min(focus_raw.height() * 0.15);
    let focus = focus_raw.shrink2(vec2(shrink_x, shrink_y));

    BoardOverlays {
        header,
        banner,
        banner_wrap_width: wrap_width,
        toolbar,
        footer,
        zoom_controls,
        hint,
        board,
        minimap,
        error_banner,
        focus,
    }
}

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

    pub fn input_row_top(&self, index: usize) -> f32 {
        self.header_height() + index as f32 * INPUT_ROW
    }

    /// The editable field of an input row.
    pub fn input_field(&self, index: usize) -> Rect {
        let top = self.input_row_top(index);
        Rect::from_min_max(
            self.origin + vec2(CARD_PADDING, top + 22.0),
            self.origin + vec2(WIDTH - CARD_PADDING, top + 46.0),
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

    #[test]
    fn board_overlays_are_disjoint_and_respect_thresholds_across_viewport_sizes() {
        let sizes = [
            vec2(525.0, 220.0),
            vec2(525.0, 400.0),
            vec2(705.0, 300.0),
            vec2(865.0, 420.0),
            vec2(1345.0, 600.0),
        ];
        for size in sizes {
            let viewport = Rect::from_min_size(pos2(235.0, 160.0), size);
            let measures = OverlayMeasures {
                banner_height: 36.0,
                toolbar_size: vec2(204.0, 30.0),
                zoom_size: vec2(88.0, 26.0),
                hint_width: 440.0,
                shortcuts_badge_width: 78.0,
                error_size: Some(vec2(300.0, 30.0)),
            };
            let out = board_overlays(viewport, &measures);
            assert!(viewport.contains_rect(out.header));
            assert!(viewport.contains_rect(out.footer));
            assert!(viewport.contains_rect(out.board));
            assert!(!out.banner.intersects(out.toolbar));
            assert!(!out.header.intersects(out.board.shrink(0.5)));
            assert!(!out.footer.intersects(out.board.shrink(0.5)));
            let hint_rect = match out.hint {
                HintPlacement::Inline(r) | HintPlacement::Tooltip(r) => r,
            };
            assert!(!out.zoom_controls.intersects(hint_rect));
            if out.board.width() < MINIMAP_MIN_BOARD_WIDTH
                || out.board.height() < MINIMAP_MIN_BOARD_HEIGHT
            {
                assert!(out.minimap.is_none());
            } else {
                let mm = out.minimap.expect("minimap shown on large board");
                assert!(!mm.intersects(out.focus));
            }
        }
    }
}
