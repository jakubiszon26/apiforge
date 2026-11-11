//! Card geometry in board (world) coordinates.
//!
//! Geometry is derived from the document every frame and never stored in it.

use crate::labels;
use apiforge_core::{
    block::Block,
    format::{Binding, Graph, Node, Port, Project, Proxy, ProxySource},
    graph::{
        FALSE_PORT, ORDER_PORT, TRUE_PORT, URL_INPUT, VALUE_INPUT, condition_inputs,
        condition_predicates, input_slots, output_ports, predicate,
    },
    model::{NodeKind, Predicate, ValueType},
    template_text::{Scope, print_template},
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
/// A proxy chip is a pill, not a card: one line, and as wide as its name needs.
pub const CHIP_HEIGHT: f32 = 36.0;
const CHIP_MIN_WIDTH: f32 = 112.0;
const CHIP_MAX_WIDTH: f32 = 240.0;

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

/// How much a request card's box holds (LAY-16): the rows it shows
/// ([`Compact`]), or every port it could have. A board-level setting, never
/// the zoom, so zooming moves no card.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CardDensity {
    #[default]
    Compact,
    Full,
}

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
    /// A header the caller draws at the left of the header row (KTLO-12):
    /// its size, or zero when the row has none.
    pub leading_size: Vec2,
    /// The height of a strip under the header row across the whole board, a
    /// component's inputs (UX-31), or zero for none.
    pub strip_height: f32,
}

/// Disjoint regions of the canvas viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoardOverlays {
    pub header: Rect,
    /// The strip under the header row, when the board has one (UX-31).
    pub strip: Option<Rect>,
    /// Where the caller's own header sits, at the left of the header row.
    pub leading: Rect,
    /// The steps summary's room: after the leading header, before the toolbar.
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

/// Pure layout function placing header, footer, board, minimap and camera focus rects.
///
/// The header row holds, left to right, the caller's leading header, the
/// banner and the toolbar. When they do not fit side by side, with at least
/// [`MIN_BANNER_SIDE_BY_SIDE`] for the leading header and the banner together,
/// the toolbar moves below them.
pub fn board_overlays(viewport: Rect, measures: &OverlayMeasures) -> BoardOverlays {
    let leading_width = measures.leading_size.x.max(0.0);
    let after_leading = if leading_width > 0.0 {
        leading_width + OVERLAY_GAP
    } else {
        0.0
    };
    let row = viewport.width() - OVERLAY_PADDING_LEFT - OVERLAY_PADDING_RIGHT - after_leading;
    let room = row - measures.toolbar_size.x - OVERLAY_GAP;
    let side_by_side = room >= (MIN_BANNER_SIDE_BY_SIDE - after_leading).max(0.0);
    let wrap_width = if side_by_side {
        room
    } else if leading_width > 0.0 {
        row.max(0.0)
    } else {
        row.max(80.0)
    };
    let banner_h = measures
        .banner_height
        .max(measures.leading_size.y)
        .max(26.0);
    let row_top = viewport.top() + OVERLAY_PADDING_Y;
    let leading = Rect::from_min_size(
        pos2(viewport.left() + OVERLAY_PADDING_LEFT, row_top),
        vec2(leading_width, banner_h),
    );
    let banner = Rect::from_min_size(
        pos2(
            viewport.left() + OVERLAY_PADDING_LEFT + after_leading,
            row_top,
        ),
        vec2(wrap_width, banner_h),
    );
    let toolbar = if side_by_side {
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
    let strip_bottom = (header.bottom() + measures.strip_height.max(0.0)).min(viewport.bottom());
    let strip = (measures.strip_height > 0.0).then(|| {
        Rect::from_min_max(
            pos2(viewport.left(), header.bottom()),
            pos2(viewport.right(), strip_bottom),
        )
    });

    let footer_top = (viewport.bottom() - FOOTER_BAND_HEIGHT).max(strip_bottom);
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
        pos2(viewport.left(), strip_bottom),
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
        strip,
        leading,
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

/// One slot of the card: a whole input, or an operand, field or item of a Value.
#[derive(Clone, Debug, PartialEq)]
pub struct InputRow {
    /// The input the slot belongs to.
    pub name: String,
    /// Path of the slot inside the input's binding; empty for the whole input.
    pub path: Vec<String>,
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
    pub block: Block,
    pub name: String,
    pub caption: String,
    pub subtitle: Option<String>,
    pub inputs: Vec<InputRow>,
    pub outputs: Vec<OutputRow>,
    /// Control outputs: pseudo-port and label.
    pub controls: Vec<(&'static str, &'static str)>,
    pub origin: Pos2,
    /// The compact box of a request at the compact density (LAY-16).
    pub compact: Option<Compact>,
}

/// What one row of a compact card stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompactRole {
    /// The method and address, carrying the URL input's socket.
    Subtitle,
    /// What a component call calls and its revision (`Component · r1`).
    Caption,
    /// An input that is configured, wired, or required and missing.
    Input(usize),
    /// An output some step reads.
    Output(usize),
    /// The one line of a value, a check, a *Data condition* or an `if`: its
    /// source or expression, or its predicate in words (LAY-21).
    Line,
    /// The last run's result.
    Result,
    /// How many checks read the step.
    Checks(usize),
    /// The *Yes* and *No* of an `if`, over their sockets.
    Branches,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompactRow {
    pub role: CompactRole,
    /// Top of the row, from the card's origin.
    pub top: f32,
    pub height: f32,
    /// The line below Full: an input's summary (`Body · 2 fields`).
    pub text: String,
    /// The line at Full: an input's values (`Query · limit: 20`).
    pub full_text: String,
    /// The inputs whose sockets are on the row's left edge, each with its
    /// socket's height from the card's origin.
    pub inputs: Vec<(usize, f32)>,
    /// The outputs whose sockets are on the row's right edge, likewise.
    pub outputs: Vec<(usize, f32)>,
    /// The line names a required input that has no value.
    pub missing: bool,
}

/// A card's box at the compact density: the header, then one row per thing
/// it shows, the box ending 6 px under the last.
#[derive(Clone, Debug, PartialEq)]
pub struct Compact {
    pub rows: Vec<CompactRow>,
    pub height: f32,
    /// What the card's one line stands for in full, for its tooltip.
    pub hover: Option<String>,
}

pub const COMPACT_HEADER: f32 = 40.0;
pub const COMPACT_ROW: f32 = 24.0;
const COMPACT_SUBTITLE: f32 = 22.0;
const COMPACT_RESULT: f32 = 26.0;
const COMPACT_END: f32 = 6.0;
/// The tray of the ports a compact card hides: its narrowest width, which
/// fits the 96 px column gap Tidy up leaves, its widest, and its gap from the
/// card.
pub const TRAY_WIDTH: f32 = 88.0;
pub const TRAY_MAX_WIDTH: f32 = 160.0;
pub const TRAY_GAP: f32 = 4.0;

impl Compact {
    /// The row an input's socket is on, and the socket's height.
    fn input(&self, index: usize) -> Option<(&CompactRow, f32)> {
        self.rows.iter().find_map(|row| {
            row.inputs
                .iter()
                .find(|(input, _)| *input == index)
                .map(|(_, y)| (row, *y))
        })
    }

    /// The row an output's socket is on, and the socket's height.
    fn output(&self, index: usize) -> Option<(&CompactRow, f32)> {
        self.rows.iter().find_map(|row| {
            row.outputs
                .iter()
                .find(|(output, _)| *output == index)
                .map(|(_, y)| (row, *y))
        })
    }
}

/// Whether a card of this kind has a compact box at the compact density:
/// requests (LAY-16) and every kind LAY-21 added. `Start` and a component's
/// outputs keep the full card.
fn compacts(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Request
            | NodeKind::Component
            | NodeKind::Value
            | NodeKind::Assert
            | NodeKind::Condition
            | NodeKind::If
    )
}

impl Card {
    pub fn build(
        node: &Node,
        reference_graph: &Graph,
        project: &Project,
        density: CardDensity,
    ) -> Self {
        let mut card = Self::build_full(node, reference_graph, project);
        if density == CardDensity::Compact && compacts(node.kind) {
            card.compact = Some(compact(&card, node, reference_graph, project));
        }
        card
    }

    fn build_full(node: &Node, reference_graph: &Graph, project: &Project) -> Self {
        let slots = input_slots(node, project);
        let inline_allowed = node.kind != NodeKind::Component && slots.len() <= 3;
        let inputs = slots
            .into_iter()
            .filter(|slot| {
                // Unary comparisons have no second operand. Keep the document
                // binding intact when hidden, so changing the predicate is safe.
                slot.input != "expected"
                    || !matches!(node.kind, NodeKind::Assert | NodeKind::If)
                    || predicate(node).takes_expected()
            })
            .map(|slot| {
                let binding = node
                    .inputs
                    .get(&slot.input)
                    .and_then(|binding| binding.at(&slot.path));
                let value = row_value(node, &slot.port, binding, inline_allowed, reference_graph);
                InputRow {
                    name: slot.input,
                    path: slot.path,
                    label: slot.port.display_name().to_owned(),
                    required: slot.port.is_required(),
                    value,
                    port: slot.port,
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
                node.inputs
                    .get(URL_INPUT)
                    .and_then(|url| print_template(
                        url,
                        Scope {
                            nodes: &reference_graph.nodes,
                            project,
                            request: Some(node),
                        }
                    ))
                    .filter(|url| !url.is_empty())
                    .unwrap_or_else(|| "…".to_owned())
            )),
            NodeKind::Condition => Some(condition_subtitle(node)),
            NodeKind::Value => value_subtitle(node),
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
            block: Block::of(node),
            name: node.display_name().to_owned(),
            caption,
            subtitle,
            inputs,
            outputs,
            controls,
            origin: pos2(node.position.x as f32, node.position.y as f32),
            compact: None,
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
        if let Some(compact) = &self.compact {
            return compact.height;
        }
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

    /// The row an input is shown on, which is where a variable drops on it;
    /// [`Rect::NOTHING`] for an input a compact card does not show.
    pub fn input_row(&self, index: usize) -> Rect {
        if let Some(compact) = &self.compact {
            return compact
                .input(index)
                .map_or(Rect::NOTHING, |(row, _)| self.row_rect(row));
        }
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
        Rect::from_min_size(footer.right_top() + vec2(-40.0, 5.0), vec2(26.0, 26.0))
    }

    /// The button opening a component's inner graph: in the header, at its
    /// right end.
    pub fn open_button(&self) -> Rect {
        if self.compact.is_some() {
            return Rect::from_min_size(self.origin + vec2(WIDTH - 36.0, 8.0), vec2(26.0, 26.0));
        }
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

    /// Anchor of the slot showing a reference at `path` inside input `name` (the
    /// deepest row containing it), or of the ordering input for [`ORDER_PORT`].
    pub fn input_anchor(&self, name: &str, path: &[String]) -> Option<Pos2> {
        if name == ORDER_PORT {
            return (self.kind != NodeKind::Start).then(|| self.origin + vec2(WIDTH / 2.0, 0.0));
        }
        let index = self
            .inputs
            .iter()
            .enumerate()
            .filter(|(_, row)| row.name == name && path.starts_with(&row.path))
            .max_by_key(|(_, row)| row.path.len())?
            .0;
        self.shown_row_anchor(index)
    }

    /// Anchor of the input row at `index`, or `None` for an input a compact
    /// card does not show.
    pub fn shown_row_anchor(&self, index: usize) -> Option<Pos2> {
        match &self.compact {
            Some(compact) => {
                let (_, y) = compact.input(index)?;
                Some(self.origin + vec2(0.0, y))
            }
            None => Some(self.row_anchor(index)),
        }
    }

    /// Anchor of the input row at `index` of a full card.
    pub fn row_anchor(&self, index: usize) -> Pos2 {
        self.origin + vec2(0.0, self.input_row_top(index) + 30.0)
    }

    /// Anchor of a data output or a control pseudo-port; `None` for an output
    /// a compact card does not show.
    pub fn output_anchor(&self, name: &str) -> Option<Pos2> {
        if let Some(compact) = &self.compact {
            if let Some(index) = self.controls.iter().position(|(port, _)| *port == name) {
                return Some(self.origin + vec2(self.control_x(index), compact.height));
            }
            let index = self.outputs.iter().position(|row| row.name == name)?;
            let (_, y) = compact.output(index)?;
            return Some(self.origin + vec2(WIDTH, y));
        }
        if let Some(index) = self.controls.iter().position(|(port, _)| *port == name) {
            let button = self.control_button(index);
            return Some(pos2(button.center().x, self.rect().bottom()));
        }
        let index = self.outputs.iter().position(|row| row.name == name)?;
        Some(self.origin + vec2(WIDTH, self.output_y(index)))
    }

    /// Where a control socket sits across the card's bottom edge: the middle
    /// of its button on a full card, so *Yes* and *No* keep their places.
    pub fn control_x(&self, index: usize) -> f32 {
        let width = WIDTH / self.controls.len().max(1) as f32;
        (index as f32 + 0.5) * width
    }

    pub fn translated(&self, offset: Vec2) -> Self {
        let mut card = self.clone();
        card.origin += offset;
        card
    }

    fn row_rect(&self, row: &CompactRow) -> Rect {
        Rect::from_min_size(self.origin + vec2(0.0, row.top), vec2(WIDTH, row.height))
    }

    /// The inputs a compact card does not show; the URL has the subtitle's
    /// socket. Empty for a full card.
    pub fn hidden_inputs(&self) -> Vec<usize> {
        let Some(compact) = &self.compact else {
            return Vec::new();
        };
        (0..self.inputs.len())
            .filter(|index| compact.input(*index).is_none())
            .collect()
    }

    /// The outputs a compact card does not show. Empty for a full card.
    pub fn hidden_outputs(&self) -> Vec<usize> {
        let Some(compact) = &self.compact else {
            return Vec::new();
        };
        (0..self.outputs.len())
            .filter(|index| compact.output(*index).is_none())
            .collect()
    }

    fn tray(&self, x: f32, width: f32, rows: usize) -> Rect {
        Rect::from_min_size(
            self.origin + vec2(x, COMPACT_HEADER),
            vec2(width, rows as f32 * COMPACT_ROW + 8.0),
        )
    }

    /// A tray as wide as its longest label needs, between [`TRAY_WIDTH`] and
    /// [`TRAY_MAX_WIDTH`]. Like a chip's, the width is estimated from the
    /// text, so hit testing and painting agree without a font.
    fn tray_width<'a>(labels: impl Iterator<Item = &'a str>) -> f32 {
        let longest = labels.map(|label| label.chars().count()).max().unwrap_or(0);
        (6.4 * longest as f32 + 30.0).clamp(TRAY_WIDTH, TRAY_MAX_WIDTH)
    }

    /// The tray of hidden inputs, outside the card's left edge (decision 2 =
    /// D): view state, which neither the box nor the routing sees.
    pub fn input_tray(&self) -> Option<Rect> {
        let hidden = self.hidden_inputs();
        let width = Self::tray_width(hidden.iter().map(|i| self.inputs[*i].label.as_str()));
        (!hidden.is_empty()).then(|| self.tray(-TRAY_GAP - width, width, hidden.len()))
    }

    /// The tray of hidden outputs, outside the card's right edge.
    pub fn output_tray(&self) -> Option<Rect> {
        let hidden = self.hidden_outputs();
        let width = Self::tray_width(hidden.iter().map(|i| self.outputs[*i].label.as_str()));
        (!hidden.is_empty()).then(|| self.tray(WIDTH + TRAY_GAP, width, hidden.len()))
    }

    /// The `k`-th row of a tray.
    pub fn tray_row(tray: Rect, k: usize) -> Rect {
        Rect::from_min_size(
            tray.min + vec2(0.0, 4.0 + k as f32 * COMPACT_ROW),
            vec2(tray.width(), COMPACT_ROW),
        )
    }

    /// The tray row of a hidden input, and its socket on the tray's left edge.
    pub fn tray_input(&self, index: usize) -> Option<(Rect, Pos2)> {
        let k = self.hidden_inputs().iter().position(|i| *i == index)?;
        let row = Self::tray_row(self.input_tray()?, k);
        Some((row, pos2(row.left(), row.center().y)))
    }

    /// The tray row of a hidden output, and its socket on the tray's right edge.
    pub fn tray_output(&self, index: usize) -> Option<(Rect, Pos2)> {
        let k = self.hidden_outputs().iter().position(|i| *i == index)?;
        let row = Self::tray_row(self.output_tray()?, k);
        Some((row, pos2(row.right(), row.center().y)))
    }
}

/// The compact rows of a card. A request (LAY-16, Q13 = A) and a component
/// call (LAY-21, by the same rules): the subtitle with the URL's socket or the
/// call's caption; an input when it is configured, wired, or required and
/// missing; an output when a step reads it; the result; the checks when one
/// reads the step. A value, a check, a *Data condition* and an `if` (LAY-21):
/// one line, then the result, or the *Yes* and *No* of an `if`.
fn compact(card: &Card, node: &Node, reference_graph: &Graph, project: &Project) -> Compact {
    let mut read_outputs: Vec<&str> = Vec::new();
    let mut checks = 0;
    for other in reference_graph
        .nodes
        .iter()
        .filter(|other| other.id != node.id)
    {
        let mut reads = false;
        for binding in other.inputs.values() {
            binding.for_each_leaf(&mut |leaf, _| {
                if let Binding::Reference { node_id, port, .. } = leaf
                    && node_id == &node.id
                {
                    reads = true;
                    read_outputs.push(port);
                }
            });
        }
        if reads && other.kind.is_check() {
            checks += 1;
        }
    }
    let scope = Scope {
        nodes: &reference_graph.nodes,
        project,
        request: Some(node),
    };
    let read: Vec<usize> = (0..card.outputs.len())
        .filter(|index| read_outputs.contains(&card.outputs[*index].name.as_str()))
        .collect();
    // Whether a wire feeds the input: a value read from a step, a parameter,
    // the environment or an argument somewhere in it, not only literals.
    let wired = |index: usize| {
        let row = &card.inputs[index];
        let mut wired = false;
        if let Some(binding) = node.inputs.get(&row.name).and_then(|b| b.at(&row.path)) {
            binding.for_each_leaf(&mut |leaf, _| {
                wired |= !matches!(leaf, Binding::Literal { .. });
            });
        }
        wired
    };
    let mut rows: Vec<Pending> = Vec::new();
    let mut hover = None;
    match node.kind {
        NodeKind::Request | NodeKind::Component => {
            if let Some(subtitle) = &card.subtitle {
                let url = card.inputs.iter().position(|row| row.name == URL_INPUT);
                let mut row = Pending::new(CompactRole::Subtitle, COMPACT_SUBTITLE, subtitle);
                row.inputs.extend(url);
                rows.push(row);
            }
            if node.kind == NodeKind::Component {
                rows.push(Pending::new(
                    CompactRole::Caption,
                    COMPACT_SUBTITLE,
                    &card.caption,
                ));
            }
            for (index, row) in card.inputs.iter().enumerate() {
                if row.name == URL_INPUT || !row.path.is_empty() {
                    continue;
                }
                let binding = node.inputs.get(&row.name);
                let missing = row.required && binding.is_none();
                if binding.is_none() && !missing {
                    continue;
                }
                let (text, full_text) =
                    input_lines(&row.name, &row.label, binding, scope, reference_graph);
                let mut line = Pending::new(CompactRole::Input(index), COMPACT_ROW, &text);
                line.full_text = full_text;
                line.inputs.push(index);
                line.missing = missing;
                rows.push(line);
            }
            for index in &read {
                let text = format!("{} →", card.outputs[*index].label);
                let mut row = Pending::new(CompactRole::Output(*index), COMPACT_ROW, &text);
                row.outputs.push(*index);
                rows.push(row);
            }
            rows.push(Pending::new(CompactRole::Result, COMPACT_RESULT, ""));
            if checks > 0 {
                let text = format!("{checks} {}", if checks == 1 { "check" } else { "checks" });
                rows.push(Pending::new(
                    CompactRole::Checks(checks),
                    COMPACT_ROW,
                    &text,
                ));
            }
        }
        _ => {
            // A satellite (LAY-21): one line that names its source and says
            // every value, with the sockets of the inputs a wire feeds and of
            // the outputs a step reads. An input with a literal value, or
            // none, takes a wire through the tray.
            let (text, full, missing) = satellite_line(card, node, scope, reference_graph);
            let mut line = Pending::new(CompactRole::Line, COMPACT_ROW, &text);
            line.inputs = (0..card.inputs.len())
                .filter(|index| wired(*index))
                .collect();
            line.outputs = read;
            line.missing = missing;
            rows.push(line);
            hover = Some(full);
            match node.kind {
                NodeKind::If => rows.push(Pending::new(CompactRole::Branches, COMPACT_ROW, "")),
                NodeKind::Value => {}
                _ => rows.push(Pending::new(CompactRole::Result, COMPACT_RESULT, "")),
            }
        }
    }
    let mut top = COMPACT_HEADER;
    let rows = rows
        .into_iter()
        .map(|pending| {
            let middle = top + pending.height / 2.0;
            // The wired inputs of a row share its height evenly, so their
            // wires end apart; an input with a literal value takes the middle.
            let spread = |items: Vec<usize>, sockets: Vec<usize>| -> Vec<(usize, f32)> {
                items
                    .into_iter()
                    .map(|item| {
                        let y = sockets.iter().position(|s| *s == item).map_or(middle, |k| {
                            top + pending.height * (k + 1) as f32 / (sockets.len() + 1) as f32
                        });
                        (item, y)
                    })
                    .collect()
            };
            let wired_inputs: Vec<usize> = pending
                .inputs
                .iter()
                .copied()
                .filter(|index| wired(*index))
                .collect();
            let inputs = spread(pending.inputs, wired_inputs);
            let outputs = spread(pending.outputs.clone(), pending.outputs);
            let row = CompactRow {
                role: pending.role,
                top,
                height: pending.height,
                text: pending.text,
                full_text: pending.full_text,
                inputs,
                outputs,
                missing: pending.missing,
            };
            top += pending.height;
            row
        })
        .collect();
    Compact {
        rows,
        height: top + COMPACT_END,
        hover,
    }
}

/// A compact row before its place on the card is known.
struct Pending {
    role: CompactRole,
    height: f32,
    text: String,
    full_text: String,
    inputs: Vec<usize>,
    outputs: Vec<usize>,
    missing: bool,
}

impl Pending {
    fn new(role: CompactRole, height: f32, text: &str) -> Self {
        Self {
            role,
            height,
            text: text.to_owned(),
            full_text: text.to_owned(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            missing: false,
        }
    }
}

/// The one line of a satellite card, the same at every zoom; what it stands
/// for in full, for the tooltip; and whether it names a missing input.
fn satellite_line(
    card: &Card,
    node: &Node,
    scope: Scope<'_>,
    graph: &Graph,
) -> (String, String, bool) {
    let label = |name: &str| {
        card.inputs
            .iter()
            .find(|row| row.name == name)
            .map_or_else(|| name.to_owned(), |row| row.label.clone())
    };
    match node.kind {
        NodeKind::Value => {
            let text = node.inputs.get(VALUE_INPUT).map_or_else(
                || "No value yet".to_owned(),
                |binding| source_text(binding, scope, graph, 0),
            );
            (text.clone(), text, false)
        }
        NodeKind::Condition => {
            let predicates = condition_predicates(node);
            let sentences: Vec<(String, bool)> = predicates
                .iter()
                .enumerate()
                .map(|(index, predicate)| {
                    let (actual, expected) = condition_inputs(index + 1);
                    sentence(node, *predicate, &actual, &expected, &label, scope, graph)
                })
                .collect();
            let missing = sentences.iter().any(|(_, missing)| *missing);
            let joined = sentences
                .iter()
                .map(|(sentence, _)| sentence.as_str())
                .collect::<Vec<_>>()
                .join(match node.config.match_mode {
                    Some(apiforge_core::model::Match::Any) => " or ",
                    _ => " and ",
                });
            match sentences.as_slice() {
                [(single, _)] => (single.clone(), joined, missing),
                _ => (condition_subtitle(node), joined, missing),
            }
        }
        _ => {
            let (text, missing) = sentence(
                node,
                predicate(node),
                "actual",
                "expected",
                &label,
                scope,
                graph,
            );
            (text.clone(), text, missing)
        }
    }
}

/// A comparison in words, as the subject reads it: `cart.status is equal to
/// 201`. A missing operand is named by its input, and the line says so.
fn sentence(
    node: &Node,
    predicate: Predicate,
    actual: &str,
    expected: &str,
    label: &dyn Fn(&str) -> String,
    scope: Scope<'_>,
    graph: &Graph,
) -> (String, bool) {
    let mut missing = false;
    let mut operand = |name: &str| {
        node.inputs.get(name).map_or_else(
            || {
                missing = true;
                label(name)
            },
            |binding| source_text(binding, scope, graph, 0),
        )
    };
    let mut text = format!("{} {}", operand(actual), predicate.label());
    if predicate.takes_expected() {
        text.push(' ');
        text.push_str(&operand(expected));
    }
    if predicate == Predicate::Approximately
        && let Some(tolerance) = node.config.tolerance
    {
        text.push_str(&format!(" within {tolerance}"));
    }
    (text, missing)
}

/// A binding as a line of a card reads it: references as in scenario code
/// (`catalogue.body.items[0]`), literals as JSON, calculations with their
/// signs (`promoted.promotion.price × 2 + second.price`). `outer` is the
/// binding strength of the operator around it, for brackets.
fn source_text(binding: &Binding, scope: Scope<'_>, graph: &Graph, outer: u8) -> String {
    use apiforge_core::operator::Operator as O;
    match binding {
        Binding::Reference { .. }
        | Binding::Parameter { .. }
        | Binding::Environment { .. }
        | Binding::Argument { .. } => apiforge_core::template_text::print_reference(binding, scope)
            .unwrap_or_else(|| labels::binding(binding, graph)),
        Binding::Literal { value } => apiforge_core::js::json_stringify(value),
        Binding::Template { .. } => print_template(binding, scope).map_or_else(
            || labels::binding(binding, graph),
            |text| format!("\"{text}\""),
        ),
        Binding::Object { fields } => format!(
            "{{ {} }}",
            fields
                .iter()
                .map(|(key, field)| format!("{key}: {}", source_text(field, scope, graph, 0)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Binding::Array { items } => format!(
            "[{}]",
            items
                .iter()
                .map(|item| source_text(item, scope, graph, 0))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Binding::Expression { operator, inputs } => {
            let binary = match operator {
                O::Or => Some(("or", 1)),
                O::And => Some(("and", 2)),
                O::Equals => Some(("=", 3)),
                O::NotEquals => Some(("≠", 3)),
                O::Greater => Some((">", 3)),
                O::Less => Some(("<", 3)),
                O::AtLeast => Some(("≥", 3)),
                O::AtMost => Some(("≤", 3)),
                O::Add => Some(("+", 4)),
                O::Subtract => Some(("−", 4)),
                O::Multiply => Some(("×", 5)),
                O::Divide => Some(("÷", 5)),
                O::Modulo => Some(("mod", 5)),
                _ => None,
            };
            match (binary, inputs.get("left"), inputs.get("right")) {
                (Some((sign, strength)), Some(left), Some(right)) if inputs.len() == 2 => {
                    let text = format!(
                        "{} {sign} {}",
                        source_text(left, scope, graph, strength),
                        source_text(right, scope, graph, strength + 1)
                    );
                    if strength < outer {
                        format!("({text})")
                    } else {
                        text
                    }
                }
                _ => {
                    let name = serde_json::to_value(operator)
                        .ok()
                        .and_then(|name| name.as_str().map(str::to_owned))
                        .unwrap_or_else(|| operator.label().to_owned());
                    let arguments: Vec<String> = inputs
                        .values()
                        .map(|input| source_text(input, scope, graph, 0))
                        .collect();
                    format!("{name}({})", arguments.join(", "))
                }
            }
        }
    }
}

/// An input's line below Full (`Body · 2 fields`) and at Full (`Query ·
/// limit: 20`).
fn input_lines(
    name: &str,
    label: &str,
    binding: Option<&Binding>,
    scope: Scope<'_>,
    graph: &Graph,
) -> (String, String) {
    let count = |n: usize| {
        let (one, many) = match name {
            "query" => ("parameter", "parameters"),
            "headers" => ("header", "headers"),
            _ => ("field", "fields"),
        };
        format!("{label} · {n} {}", if n == 1 { one } else { many })
    };
    let value = |binding: &Binding| {
        print_template(binding, scope).unwrap_or_else(|| match binding {
            Binding::Literal { value } => apiforge_core::js::json_stringify(value),
            other => labels::binding(other, graph),
        })
    };
    let pairs = |pairs: Vec<(String, String)>| {
        let pairs: Vec<String> = pairs
            .into_iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect();
        format!("{label} · {}", pairs.join(", "))
    };
    match binding {
        None => {
            let text = format!("{label} · Missing input");
            (text.clone(), text)
        }
        Some(Binding::Object { fields }) => (
            count(fields.len()),
            pairs(
                fields
                    .iter()
                    .map(|(key, field)| (key.clone(), value(field)))
                    .collect(),
            ),
        ),
        Some(Binding::Literal {
            value: serde_json::Value::Object(fields),
        }) => (
            count(fields.len()),
            pairs(
                fields
                    .iter()
                    .map(|(key, field)| (key.clone(), apiforge_core::js::json_stringify(field)))
                    .collect(),
            ),
        ),
        // One form on every card: the input's name, then its source as the
        // value and check lines write it (`Body · cart.body`, `id · "p-1"`).
        Some(other) => {
            let text = format!("{label} · {}", source_text(other, scope, graph, 0));
            (text.clone(), text)
        }
    }
}

/// A proxy as drawn on the board: a small chip with one output.
///
/// Geometry, like a card's, is derived from the document every frame. A chip's
/// width follows its label so that routing and hit testing agree without needing
/// a font.
#[derive(Clone, Debug, PartialEq)]
pub struct Chip {
    /// The document's proxy: its identity, what it stands for and where it sits.
    pub proxy: Proxy,
    /// The variable's name, or the name of the parameter or environment value.
    pub label: String,
    pub caption: &'static str,
    /// False when nothing on this board or in the project answers to the name.
    pub known: bool,
    pub origin: Pos2,
}

impl Chip {
    pub fn build(proxy: &Proxy, graph: &Graph, project: &Project, parameters: &[String]) -> Self {
        let (label, caption, known) = match &proxy.source {
            ProxySource::Variable { node_id } => {
                let node = graph
                    .nodes
                    .iter()
                    .find(|node| &node.id == node_id && node.is_variable());
                (
                    node.map_or(node_id.clone(), |node| node.display_name().to_owned()),
                    "variable",
                    node.is_some(),
                )
            }
            ProxySource::Parameter { name } => (
                name.clone(),
                "parameter",
                parameters.iter().any(|parameter| parameter == name),
            ),
            ProxySource::Environment { name } => {
                let environment = &project.environment;
                (
                    name.clone(),
                    "environment",
                    name == "baseUrl"
                        || environment.variables.contains_key(name)
                        || environment.secret_names.contains(name),
                )
            }
        };
        Self {
            proxy: proxy.clone(),
            label,
            caption,
            known,
            origin: pos2(proxy.position.x as f32, proxy.position.y as f32),
        }
    }

    pub fn id(&self) -> &str {
        &self.proxy.id
    }

    /// The variable this proxy stands for, when it stands for a block.
    pub fn source_node(&self) -> Option<&str> {
        match &self.proxy.source {
            ProxySource::Variable { node_id } => Some(node_id),
            ProxySource::Parameter { .. } | ProxySource::Environment { .. } => None,
        }
    }

    pub fn width(&self) -> f32 {
        let text = 7.2 * self.label.chars().count() as f32 + 64.0;
        text.clamp(CHIP_MIN_WIDTH, CHIP_MAX_WIDTH)
    }

    pub fn rect(&self) -> Rect {
        Rect::from_min_size(self.origin, vec2(self.width(), CHIP_HEIGHT))
    }

    /// Where a wire taken from this proxy starts.
    pub fn output_anchor(&self) -> Pos2 {
        self.origin + vec2(self.width(), CHIP_HEIGHT / 2.0)
    }

    /// The button that goes to the variable this proxy stands for.
    pub fn follow_button(&self) -> Rect {
        Rect::from_center_size(
            self.origin + vec2(self.width() - 19.0, CHIP_HEIGHT / 2.0),
            Vec2::splat(20.0),
        )
    }

    pub fn translated(&self, offset: Vec2) -> Self {
        let mut chip = self.clone();
        chip.origin += offset;
        chip
    }
}

/// The predicate of a single check, or how many checks must hold.
fn condition_subtitle(node: &Node) -> String {
    match condition_predicates(node).as_slice() {
        [single] => single.label().to_owned(),
        checks => match node.config.match_mode {
            Some(apiforge_core::model::Match::Any) => format!("any of {} checks", checks.len()),
            _ => format!("all {} checks", checks.len()),
        },
    }
}

/// What a Value computes, in a few words: its operator, the field it selects or
/// the shape of its data.
fn value_subtitle(node: &Node) -> Option<String> {
    let text = match node.inputs.get(VALUE_INPUT)? {
        Binding::Expression { operator, .. } => operator.label().to_owned(),
        Binding::Reference { path, .. } | Binding::Parameter { path, .. } if !path.is_empty() => {
            path.clone()
        }
        Binding::Object { .. } => "Object".to_owned(),
        Binding::Array { .. } => "List".to_owned(),
        Binding::Template { .. } => "Text with values".to_owned(),
        _ => return node.is_variable().then(|| "Variable".to_owned()),
    };
    Some(text)
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
            include_bytes!("../../../../fixtures/workspace.json"),
            &ImportLimits::default(),
        )
        .unwrap()
        .into_workspace();
        for project in &workspace.projects {
            for scenario in &project.scenarios {
                for (graph, density) in [&scenario.graph, &scenario.cleanup]
                    .into_iter()
                    .flat_map(|graph| [CardDensity::Compact, CardDensity::Full].map(|d| (graph, d)))
                {
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
                        let source = Card::build(source, graph, project, density);
                        let target = Card::build(target, graph, project, density);
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
                            target.input_anchor(input, &wire.operand_path).is_some(),
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
        let card = Card::build(&node, graph, &project, CardDensity::Full);
        assert_eq!(card.subtitle.as_deref(), Some("is not empty"));
        assert!(card.inputs.iter().any(|row| row.name == "actual"));
        assert!(!card.inputs.iter().any(|row| row.name == "expected"));
        node.config.predicate = Some(Predicate::Equals);
        let card = Card::build(&node, graph, &project, CardDensity::Full);
        assert!(card.inputs.iter().any(|row| row.name == "expected"));
    }

    /// LAY-16 criterion 1: at the compact density `login` and `cart` are at
    /// least 40 % shorter than today's cards, which *Compact cards* off keeps.
    #[test]
    fn lay16_criterion_1_compact_cards_are_shorter() {
        use apiforge_script::{Document, LowerContext, lower, parse};
        let project = apiforge_core::demos::shop_project();
        let text = include_str!("../../../../fixtures/scenarios/lay/shop-30.afs");
        let lowered = lower(&parse(text), &LowerContext::folder(&project));
        let Some(Document::Scenario(scenario)) = lowered.document else {
            panic!("shop-30 lowers: {:#?}", lowered.diagnostics);
        };
        let graph = &scenario.graph;
        for (handle, today) in [("login", 372.0), ("cart", 424.0)] {
            let node = graph
                .nodes
                .iter()
                .find(|node| node.handle == handle)
                .unwrap();
            let full = Card::build(node, graph, &project, CardDensity::Full).height();
            let compact = Card::build(node, graph, &project, CardDensity::Compact).height();
            assert_eq!(full, today, "{handle} at full density is today's card");
            assert!(
                compact <= today * 0.6,
                "{handle}: {compact} px is not 40 % shorter than {today} px"
            );
        }
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
        // No leading header (the legacy window), Split's Main / Cleanup, and
        // Diagram's file name, kind and Main / Cleanup (KTLO-12).
        let leading_sizes = [Vec2::ZERO, vec2(140.0, 32.0), vec2(400.0, 32.0)];
        // A scenario's board, and a component's with its inputs (UX-31).
        let strip_heights = [0.0, 40.0];
        for (size, leading_size, strip_height) in sizes.into_iter().flat_map(|size| {
            leading_sizes
                .into_iter()
                .flat_map(move |leading| strip_heights.map(|strip| (size, leading, strip)))
        }) {
            let viewport = Rect::from_min_size(pos2(235.0, 160.0), size);
            let measures = OverlayMeasures {
                banner_height: 36.0,
                toolbar_size: vec2(204.0, 30.0),
                zoom_size: vec2(88.0, 26.0),
                hint_width: 440.0,
                shortcuts_badge_width: 78.0,
                error_size: Some(vec2(300.0, 30.0)),
                leading_size,
                strip_height,
            };
            let out = board_overlays(viewport, &measures);
            assert!(viewport.contains_rect(out.header));
            assert!(viewport.contains_rect(out.footer));
            assert!(viewport.contains_rect(out.board));
            assert!(!out.banner.intersects(out.toolbar));
            assert_eq!(out.leading.size().x, leading_size.x);
            assert!(out.header.contains_rect(out.leading));
            assert!(out.leading.right() <= out.banner.left());
            assert!(!out.leading.intersects(out.toolbar));
            let room = size.x - OVERLAY_PADDING_LEFT - OVERLAY_PADDING_RIGHT;
            if leading_size.x + OVERLAY_GAP * 2.0 + measures.toolbar_size.x <= room
                && leading_size.x > 0.0
            {
                // Side by side: one row, the leading header left of the toolbar.
                assert!(out.leading.right() < out.toolbar.left());
                assert!((out.leading.center().y - out.toolbar.center().y).abs() <= 3.0);
            } else if leading_size.x > 0.0 {
                // Stacked: the toolbar moves below the leading header.
                assert!(out.toolbar.top() >= out.leading.bottom());
            }
            assert!(!out.header.intersects(out.board.shrink(0.5)));
            assert!(!out.footer.intersects(out.board.shrink(0.5)));
            match out.strip {
                Some(strip) => {
                    // Under the header row, across the board, above it.
                    assert_eq!(strip.top(), out.header.bottom());
                    assert_eq!(strip.width(), viewport.width());
                    assert_eq!(strip.height(), strip_height);
                    assert_eq!(out.board.top(), strip.bottom());
                    assert!(!out.footer.intersects(strip.shrink(0.5)));
                }
                None => assert_eq!(strip_height, 0.0),
            }
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
