//! The board: an interactive view of one graph.
//!
//! The board reads the document and returns [`Command`]s; the application applies
//! them through the editor, which validates every change. Positions being dragged,
//! wires being drawn and the camera are view state only.

mod layout;
mod menu;
mod paint;
mod routing;

pub use layout::{Card, Chip, parse_inline};
pub use routing::WireStyle;

use crate::icons::Icon;
use crate::{dnd::Payload, theme, widgets};
use apiforge_core::{
    binding::data_edge_id,
    block::Block,
    edit::proxy_binding,
    format::{Binding, Graph, Position, Project, ProxySource},
    graph::{ORDER_PORT, WireKind, wires},
    model::{Branch, NodeKind},
    operator::Operator,
    run::StepResult,
};
use eframe::egui::{
    self, Align2, Color32, FontId, Key, Modifiers, PointerButton, Pos2, Rect, Sense, Stroke,
    StrokeKind, Vec2, vec2,
};
use layout::{BoardOverlays, HintPlacement, OverlayMeasures, RowValue};
use menu::ActionMenu;
use std::collections::HashSet;

const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 1.6;
/// Below this zoom, inline fields are drawn as text and edited in the inspector.
const INLINE_EDIT_ZOOM: f32 = 0.8;
const SOCKET_HIT: f32 = 10.0;

/// A change requested on the board.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Select(Vec<String>),
    /// Final positions after a drag.
    Move(Vec<(String, Position)>),
    Connect {
        source: String,
        source_port: String,
        target: String,
        target_input: String,
        /// Slot path inside the target input; empty for the whole input.
        target_path: Vec<String>,
    },
    AddNode {
        block: Block,
        component: Option<(String, u32)>,
        position: Position,
        source: Option<(String, String)>,
    },
    AddOperator {
        operator: Operator,
        position: Position,
    },
    SetLiteral {
        node: String,
        input: String,
        path: Vec<String>,
        value: serde_json::Value,
    },
    Bind {
        node: String,
        input: String,
        path: Vec<String>,
        binding: Binding,
    },
    Delete {
        /// Blocks and proxies, by identifier.
        ids: Vec<String>,
        wires: Vec<String>,
    },
    /// Show a variable, parameter or environment value in another place.
    AddProxy {
        source: ProxySource,
        position: Position,
    },
    EditComponent(String),
    SaveSelectionAsComponent,
    Copy,
    Paste,
    Duplicate,
    /// Lay out the selection, or the whole board when nothing is selected.
    TidyUp(Vec<String>),
    /// Change how wires are drawn. A user setting, not a document edit.
    SetWireStyle(WireStyle),
    SetBridges(bool),
}

/// Everything the board needs to draw one frame.
pub struct Input<'a> {
    pub graph: &'a Graph,
    /// Nodes that bindings may reference (main graph nodes on the cleanup board).
    pub reference_graph: &'a Graph,
    pub project: &'a Project,
    /// Parameters of the scenario, or arguments of the component being edited.
    pub parameters: &'a [String],
    pub selection: &'a [String],
    pub steps: Option<&'a [StepResult]>,
    pub can_paste: bool,
    /// Changing context resets the camera and transient state.
    pub context: &'a str,
}

/// Everything the board draws: the blocks' cards and the proxies' chips.
#[derive(Clone, Copy)]
struct Shapes<'a> {
    cards: &'a [Card],
    chips: &'a [Chip],
}

#[derive(Clone, Copy, Debug)]
struct Camera {
    zoom: f32,
    pan: Vec2,
}

impl Camera {
    fn to_screen(self, world: Pos2) -> Pos2 {
        (world.to_vec2() * self.zoom + self.pan).to_pos2()
    }

    fn to_world(self, screen: Pos2) -> Pos2 {
        ((screen.to_vec2() - self.pan) / self.zoom).to_pos2()
    }

    fn rect(self, world: Rect) -> Rect {
        Rect::from_min_max(self.to_screen(world.min), self.to_screen(world.max))
    }
}

#[derive(Clone, Debug)]
enum Drag {
    /// Moving the selected cards and proxies; `offset` is in world units.
    Nodes { ids: Vec<String>, offset: Vec2 },
    /// Drawing a wire from an output or a proxy.
    Wire { start: WireStart, from: Pos2 },
    /// Rubber-band selection from a screen position.
    Select { from: Pos2, additive: bool },
}

/// What a dragged wire comes from.
#[derive(Clone, Debug)]
enum WireStart {
    /// An output of a block: dropping it on an input connects the two.
    Output { node: String, port: String },
    /// A proxy: dropping it on an input binds the value it stands for, exactly as
    /// dragging the variable out of the panel does.
    Proxy { binding: Binding },
}

#[derive(Clone, Debug)]
struct InlineEdit {
    node: String,
    input: String,
    path: Vec<String>,
    text: String,
    error: Option<String>,
}

pub struct Canvas {
    camera: Camera,
    context: String,
    fit_pending: bool,
    camera_moved_by_user: bool,
    reveal_pending: Option<String>,
    last_size: Vec2,
    last_focus: Option<Rect>,
    last_viewport: Rect,
    drag: Option<Drag>,
    menu: Option<ActionMenu>,
    selected_wires: Vec<String>,
    inline: Option<InlineEdit>,
    error: Option<String>,
    router: routing::Router,
    /// How wires are drawn, from the user's settings.
    pub wire_style: WireStyle,
    /// Whether a crossing is drawn as a bridge, from the user's settings.
    pub bridges: bool,
}

impl Default for Canvas {
    fn default() -> Self {
        Self {
            camera: Camera {
                zoom: 1.0,
                pan: Vec2::ZERO,
            },
            context: String::new(),
            fit_pending: true,
            camera_moved_by_user: false,
            reveal_pending: None,
            last_size: Vec2::ZERO,
            last_focus: None,
            last_viewport: Rect::NOTHING,
            router: routing::Router::default(),
            wire_style: WireStyle::default(),
            bridges: true,
            drag: None,
            menu: None,
            selected_wires: Vec::new(),
            inline: None,
            error: None,
        }
    }
}

impl Canvas {
    #[cfg(test)]
    pub(crate) fn debug_viewport(&self) -> Rect {
        self.last_viewport
    }

    /// Bring an explicitly located block into view without editing its position.
    pub fn reveal(&mut self, node_id: String) {
        self.reveal_pending = Some(node_id);
    }

    /// The camera as `(pan x, pan y, zoom)` once the user has moved it, so a
    /// document's view can be kept between sessions. A fitted camera is not
    /// worth keeping: it is fitted again.
    pub fn camera(&self) -> Option<(f32, f32, f32)> {
        self.camera_moved_by_user.then_some((
            self.camera.pan.x,
            self.camera.pan.y,
            self.camera.zoom,
        ))
    }

    /// Restores a kept camera for a context, instead of fitting the board.
    pub fn restore_camera(&mut self, context: &str, (x, y, zoom): (f32, f32, f32)) {
        self.context = context.to_owned();
        self.camera = Camera {
            zoom: zoom.clamp(MIN_ZOOM, MAX_ZOOM),
            pan: vec2(x, y),
        };
        self.fit_pending = false;
        self.camera_moved_by_user = true;
    }

    /// Shows an error banner on the board, e.g. a rejected connection.
    pub fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
    }

    /// Board position of the visible centre, for blocks added from the library.
    pub fn visible_center(&self) -> Position {
        let local_center = self
            .last_focus
            .map_or((self.last_size / 2.0).to_pos2(), |focus| {
                (focus.center() - self.last_viewport.min).to_pos2()
            });
        let center = self.camera.to_world(local_center);
        Position {
            x: f64::from(center.x - layout::WIDTH / 2.0),
            y: f64::from(center.y - 80.0),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, input: &Input<'_>) -> Vec<Command> {
        let mut commands = Vec::new();
        if self.context != input.context {
            self.context = input.context.to_owned();
            self.fit_pending = true;
            self.camera_moved_by_user = false;
            self.last_focus = None;
            self.drag = None;
            self.menu = None;
            self.inline = None;
            self.selected_wires.clear();
            self.error = None;
        }
        let (viewport, background) = ui.allocate_exact_size(
            ui.available_size().max(vec2(1.0, 1.0)),
            Sense::click_and_drag(),
        );
        let painter = ui.painter().with_clip_rect(viewport);
        painter.rect_filled(viewport, 0.0, theme::BG);

        let measures = self.measure_overlays(ui, input);
        let overlays = layout::board_overlays(viewport, &measures);

        let mut cards: Vec<Card> = input
            .graph
            .nodes
            .iter()
            .map(|node| Card::build(node, input.reference_graph, input.project))
            .collect();
        let mut chips: Vec<Chip> = input
            .graph
            .proxies
            .iter()
            .map(|proxy| Chip::build(proxy, input.graph, input.project, input.parameters))
            .collect();

        let focus_changed = self
            .last_focus
            .is_some_and(|prev| (prev.size() - overlays.focus.size()).length() > 1.0);
        if self.fit_pending || (self.last_size == Vec2::ZERO && !self.camera_moved_by_user) {
            self.fit(viewport, overlays.focus, shapes(&cards, &chips));
        } else if focus_changed {
            if !self.camera_moved_by_user {
                self.fit(viewport, overlays.focus, shapes(&cards, &chips));
            } else if let Some(prev) = self.last_focus {
                let delta = (overlays.focus.center() - viewport.min)
                    - (prev.center() - self.last_viewport.min);
                self.camera.pan += delta;
                if let Some(selected_id) = input.selection.first()
                    && let Some(card) = cards.iter().find(|c| &c.id == selected_id)
                {
                    let screen_card = self.screen_camera(viewport).rect(card.rect());
                    if screen_card.width() <= overlays.focus.width()
                        && screen_card.height() <= overlays.focus.height()
                    {
                        let mut shift = Vec2::ZERO;
                        if screen_card.left() < overlays.focus.left() {
                            shift.x = overlays.focus.left() - screen_card.left();
                        } else if screen_card.right() > overlays.focus.right() {
                            shift.x = overlays.focus.right() - screen_card.right();
                        }
                        if screen_card.top() < overlays.focus.top() {
                            shift.y = overlays.focus.top() - screen_card.top();
                        } else if screen_card.bottom() > overlays.focus.bottom() {
                            shift.y = overlays.focus.bottom() - screen_card.bottom();
                        }
                        self.camera.pan += shift;
                    }
                }
            }
        }
        self.last_size = viewport.size();
        self.last_focus = Some(overlays.focus);
        self.last_viewport = viewport;
        if let Some(id) = self.reveal_pending.take()
            && let Some(card) = cards.iter().find(|card| card.id == id)
        {
            self.camera.zoom = self
                .camera
                .zoom
                .max(0.85)
                .min(overlays.focus.width() / card.rect().width())
                .min(overlays.focus.height() / card.rect().height())
                .clamp(MIN_ZOOM, MAX_ZOOM);
            let local_focus_center = overlays.focus.center() - viewport.min;
            self.camera.pan =
                local_focus_center - card.rect().center().to_vec2() * self.camera.zoom;
            // Navigation is an explicit camera action: later toolbar measurement
            // or drawer resizing must preserve the chosen card, not refit all cards.
            self.camera_moved_by_user = true;
        }
        self.handle_camera(ui, &background, viewport);
        let origin_offset = viewport.min.to_vec2();
        // Cards are laid out in world space; the camera maps them into the viewport.
        let camera = Camera {
            zoom: self.camera.zoom,
            pan: self.camera.pan + origin_offset,
        };
        if let Some(Drag::Nodes { ids, offset }) = &self.drag {
            for card in cards.iter_mut().filter(|card| ids.contains(&card.id)) {
                *card = card.translated(*offset);
            }
            for chip in chips.iter_mut().filter(|chip| ids.contains(&chip.proxy.id)) {
                *chip = chip.translated(*offset);
            }
        }

        let board_painter = painter.with_clip_rect(overlays.board);
        paint::grid(&board_painter, overlays.board, camera);

        // Wires go under the cards. They are routed around the cards and chips, so
        // what is drawn is also what hit testing uses.
        let selection: HashSet<&str> = input.selection.iter().map(String::as_str).collect();
        let board_wires = board_wires(input.graph, &cards, &chips);
        let (related_blocks, related_wires) = highlights(input.graph, &chips, &selection);
        let moving: HashSet<String> = match &self.drag {
            Some(Drag::Nodes { ids, .. }) => ids.iter().cloned().collect(),
            _ => HashSet::new(),
        };
        let selected_wires: HashSet<String> = self.selected_wires.iter().cloned().collect();
        let mut drawn: Vec<(String, Vec<Pos2>)> = Vec::with_capacity(board_wires.len());
        {
            let routed = self.router.routes(&routing::Request {
                cards: &cards,
                chips: &chips,
                wires: &board_wires,
                moving: &moving,
                style: self.wire_style,
                bridges: self.bridges,
            });
            for wire in routed {
                paint::routed_wire(
                    &board_painter,
                    wire,
                    camera,
                    selected_wires.contains(&wire.id) || related_wires.contains(&wire.id),
                );
                drawn.push((wire.id.clone(), wire.points.clone()));
            }
        }

        let pointer = ui.input(|state| state.pointer.interact_pos());
        let modifiers = ui.input(|state| state.modifiers);
        let mut clicked_card = false;

        let mut board_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(overlays.board)
                .layout(*ui.layout()),
        );
        board_ui.set_clip_rect(overlays.board);

        for card in &cards {
            let rect = camera.rect(card.rect());
            if !rect.intersects(overlays.board) {
                continue;
            }
            let clipped_card_rect = rect.intersect(overlays.board);
            let id = board_ui.id().with(("card", &card.id));
            let mut response = board_ui.interact(clipped_card_rect, id, Sense::click_and_drag());
            let title_rect = camera.rect(Rect::from_min_max(
                card.origin + vec2(56.0, 8.0),
                card.origin + vec2(layout::WIDTH - 48.0, 60.0),
            ));
            if layout::CardDetail::at_zoom(camera.zoom) != layout::CardDetail::Full
                || pointer.is_some_and(|pos| title_rect.contains(pos))
            {
                response = response.on_hover_text(format!("{}\n{}", card.name, card.caption));
            }
            let a11y_label = format!("{} ({})", card.name, card.caption);
            response.widget_info(move || {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &a11y_label)
            });
            if response.clicked() {
                clicked_card = true;
                self.menu = None;
                self.selected_wires.clear();
                commands.push(Command::Select(toggle_selection(
                    input.selection,
                    &card.id,
                    modifiers,
                )));
            }
            if response.double_clicked() && card.kind == NodeKind::Component {
                commands.push(Command::EditComponent(card.id.clone()));
            }
            if response.secondary_clicked() {
                let source = menu::default_source(input.graph, input.project, &card.id);
                self.menu = Some(ActionMenu::new(
                    pointer.unwrap_or(rect.center()),
                    camera.to_world(pointer.unwrap_or(rect.center())),
                    source,
                ));
            }
            if response.drag_started_by(PointerButton::Primary) {
                let ids = if selection.contains(card.id.as_str()) {
                    input.selection.to_vec()
                } else {
                    commands.push(Command::Select(vec![card.id.clone()]));
                    vec![card.id.clone()]
                };
                self.drag = Some(Drag::Nodes {
                    ids,
                    offset: Vec2::ZERO,
                });
            }
            if response.dragged_by(PointerButton::Primary)
                && let Some(Drag::Nodes { offset, .. }) = &mut self.drag
            {
                *offset += response.drag_delta() / camera.zoom;
            }
            if response.drag_stopped()
                && let Some(Drag::Nodes { ids, offset }) = self.drag.take()
                && offset.length() > 0.0
            {
                commands.push(Command::Move(moved_positions(input.graph, &ids, offset)));
            }

            let step = input
                .steps
                .and_then(|steps| steps.iter().find(|step| step.node_id == card.id));
            paint::card(
                &board_painter,
                card,
                camera,
                selection.contains(card.id.as_str()),
                related_blocks.contains(&card.id),
                step,
            );
            self.card_controls(&mut board_ui, card, camera, input, &mut commands);
        }

        for chip in &chips {
            let rect = camera.rect(chip.rect());
            if !rect.intersects(overlays.board) {
                continue;
            }
            let id = board_ui.id().with(("chip", chip.id()));
            let response = board_ui
                .interact(rect.intersect(overlays.board), id, Sense::click_and_drag())
                .on_hover_text(format!(
                    "{} · {}{}",
                    chip.label,
                    chip.caption,
                    if chip.known {
                        ""
                    } else {
                        " — nothing of this name is on the board any more"
                    }
                ));
            let a11y_label = format!("{} ({} proxy)", chip.label, chip.caption);
            response.widget_info(move || {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &a11y_label)
            });
            if response.clicked() {
                clicked_card = true;
                self.menu = None;
                self.selected_wires.clear();
                commands.push(Command::Select(toggle_selection(
                    input.selection,
                    chip.id(),
                    modifiers,
                )));
            }
            if response.double_clicked()
                && let Some(node) = chip.source_node()
            {
                self.reveal(node.to_owned());
                commands.push(Command::Select(vec![node.to_owned()]));
            }
            if response.drag_started_by(PointerButton::Primary) {
                let ids = if selection.contains(chip.id()) {
                    input.selection.to_vec()
                } else {
                    commands.push(Command::Select(vec![chip.id().to_owned()]));
                    vec![chip.id().to_owned()]
                };
                self.drag = Some(Drag::Nodes {
                    ids,
                    offset: Vec2::ZERO,
                });
            }
            if response.dragged_by(PointerButton::Primary)
                && let Some(Drag::Nodes { offset, .. }) = &mut self.drag
            {
                *offset += response.drag_delta() / camera.zoom;
            }
            if response.drag_stopped()
                && let Some(Drag::Nodes { ids, offset }) = self.drag.take()
                && offset.length() > 0.0
            {
                commands.push(Command::Move(moved_positions(input.graph, &ids, offset)));
            }
            paint::chip(&board_painter, chip, camera, selection.contains(chip.id()));
            self.chip_controls(&mut board_ui, chip, camera, &mut commands);
        }

        self.handle_wire_drag(&board_ui, &cards, camera, input, &mut commands);
        self.handle_box_selection(
            &board_ui,
            &background,
            shapes(&cards, &chips),
            camera,
            input,
            &mut commands,
        );

        if background.clicked() && !clicked_card {
            self.menu = None;
            self.inline = None;
            let hit = pointer.and_then(|pointer| hit_wire(&drawn, camera, pointer));
            match hit {
                Some(wire) => {
                    if modifiers.command {
                        toggle(&mut self.selected_wires, &wire);
                    } else {
                        self.selected_wires = vec![wire];
                    }
                    commands.push(Command::Select(vec![]));
                }
                None => {
                    self.selected_wires.clear();
                    if !input.selection.is_empty() {
                        commands.push(Command::Select(vec![]));
                    }
                }
            }
        }
        if background.secondary_clicked()
            && let Some(pointer) = pointer
        {
            self.menu = Some(ActionMenu::new(pointer, camera.to_world(pointer), None));
        }

        self.handle_drop(
            &board_ui,
            overlays.board,
            &cards,
            camera,
            input,
            &mut commands,
        );
        self.handle_keys(ui, input, &mut commands);

        paint::overlay_chrome(&painter, &overlays, input.graph);
        ui.interact(
            overlays.banner,
            ui.id().with("execution-mode-hint"),
            Sense::hover(),
        )
        .on_hover_text(paint::mode_line_text(input.graph));
        self.toolbar(ui, viewport, &overlays, input, &mut commands);
        self.zoom_controls(ui, viewport, &overlays, shapes(&cards, &chips));
        if let Some((mm_inner, bounds, scale)) = paint::minimap(
            &painter,
            overlays.minimap,
            overlays.board,
            shapes(&cards, &chips),
            camera,
            &selection,
        ) && let Some(mm_rect) = overlays.minimap
        {
            let mm_response = ui
                .interact(
                    mm_rect,
                    ui.id().with("board-minimap"),
                    Sense::click_and_drag(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Click or drag to pan the board");
            if (mm_response.clicked() || mm_response.dragged())
                && let Some(pos) = mm_response.interact_pointer_pos()
                && scale > 0.0
            {
                let world_target = bounds.min + (pos - mm_inner.min) / scale;
                let local_focus_center = overlays.focus.center() - viewport.min;
                self.camera.pan = local_focus_center - world_target.to_vec2() * self.camera.zoom;
                self.camera_moved_by_user = true;
            }
        }
        if input.graph.nodes.is_empty() && input.graph.proxies.is_empty() {
            self.empty_state(ui, overlays.board, &mut commands);
        }
        self.error_banner(ui, &overlays);
        if let Some(menu) = &mut self.menu {
            match menu.show(ui.ctx(), input.graph, input.project) {
                menu::Outcome::Open => {}
                menu::Outcome::Close => self.menu = None,
                menu::Outcome::Add(command) => {
                    commands.push(command);
                    self.menu = None;
                }
            }
        }
        commands
    }

    fn measure_overlays(&self, ui: &egui::Ui, input: &Input<'_>) -> OverlayMeasures {
        let has_selection = !input.selection.is_empty() || !self.selected_wires.is_empty();
        let toolbar_width = 96.0
            + if input.can_paste { 56.0 } else { 0.0 }
            + if has_selection { 108.0 } else { 0.0 };
        let toolbar_size = ui
            .ctx()
            .data(|data| {
                data.get_temp(ui.id().with((
                    "toolbar-size",
                    input.selection.len(),
                    !self.selected_wires.is_empty(),
                    input.can_paste,
                )))
            })
            .unwrap_or(vec2(toolbar_width, 30.0));
        let banner_height = toolbar_size.y.max(26.0);

        let hint_galley = ui.fonts_mut(|fonts| {
            fonts.layout_no_wrap(
                paint::SHORTCUT_HINT_TEXT.to_owned(),
                FontId::proportional(theme::FONT_META),
                theme::MUTED,
            )
        });
        let error_size = self.error.as_ref().map(|_| vec2(320.0, 32.0));
        OverlayMeasures {
            banner_height,
            toolbar_size,
            zoom_size: vec2(180.0, 36.0),
            hint_width: hint_galley.size().x,
            shortcuts_badge_width: 78.0,
            error_size,
        }
    }

    fn fit(&mut self, viewport: Rect, focus: Rect, shapes: Shapes<'_>) {
        self.fit_pending = false;
        self.camera_moved_by_user = false;
        let local_focus_center = focus.center() - viewport.min;
        let Some(bounds) = shapes.bounds() else {
            self.camera = Camera {
                zoom: 1.0,
                pan: vec2(60.0, 60.0),
            };
            return;
        };
        let zoom = (focus.width().max(60.0) / bounds.width().max(1.0))
            .min(focus.height().max(60.0) / bounds.height().max(1.0))
            .clamp(MIN_ZOOM, 1.0);
        self.camera = Camera {
            zoom,
            pan: local_focus_center - bounds.center().to_vec2() * zoom,
        };
    }

    fn zoom_at(&mut self, viewport: Rect, screen: Pos2, factor: f32) {
        let before = self.camera.zoom;
        self.camera.zoom = (before * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let local = screen - viewport.min;
        self.camera.pan = local - (local - self.camera.pan) * (self.camera.zoom / before);
        self.camera_moved_by_user = true;
    }

    fn handle_camera(&mut self, ui: &egui::Ui, background: &egui::Response, viewport: Rect) {
        if background.contains_pointer() {
            let (zoom, scroll, pointer) = ui.input(|state| {
                (
                    state.zoom_delta(),
                    state.smooth_scroll_delta,
                    state.pointer.hover_pos(),
                )
            });
            if zoom != 1.0
                && let Some(pointer) = pointer
            {
                self.zoom_at(viewport, pointer, zoom);
            } else if scroll != Vec2::ZERO {
                self.camera.pan += scroll;
                self.camera_moved_by_user = true;
            }
        }
        if background.dragged_by(PointerButton::Middle)
            || background.dragged_by(PointerButton::Secondary)
        {
            self.camera.pan += background.drag_delta();
            self.camera_moved_by_user = true;
        }
    }

    fn card_controls(
        &mut self,
        ui: &mut egui::Ui,
        card: &Card,
        camera: Camera,
        input: &Input<'_>,
        commands: &mut Vec<Command>,
    ) {
        let zoom = camera.zoom;
        // Output sockets start wires.
        let mut outputs: Vec<(String, Pos2)> = card
            .outputs
            .iter()
            .filter_map(|row| {
                card.output_anchor(&row.name)
                    .map(|anchor| (row.name.clone(), anchor))
            })
            .collect();
        outputs.extend(card.controls.iter().filter_map(|(port, _)| {
            card.output_anchor(port)
                .map(|anchor| (port.to_string(), anchor))
        }));
        for (port, anchor) in outputs {
            let center = camera.to_screen(anchor);
            let rect =
                Rect::from_center_size(center, Vec2::splat((SOCKET_HIT * zoom).max(3.0) * 2.0));
            let response = ui
                .interact(
                    rect,
                    ui.id().with(("socket", &card.id, &port)),
                    Sense::drag(),
                )
                .on_hover_cursor(egui::CursorIcon::Crosshair);
            if response.drag_started() {
                self.drag = Some(Drag::Wire {
                    start: WireStart::Output {
                        node: card.id.clone(),
                        port: port.clone(),
                    },
                    from: center,
                });
            }
        }

        if layout::CardDetail::at_zoom(zoom) != layout::CardDetail::Full {
            return;
        }

        // Footer buttons.
        let action = camera.rect(card.action_button());
        if ui
            .interact(action, ui.id().with(("action", &card.id)), Sense::click())
            .on_hover_text("Add a connected step")
            .clicked()
        {
            let source = menu::default_source(input.graph, input.project, &card.id);
            self.menu = Some(ActionMenu::new(
                action.left_bottom(),
                camera.to_world(action.left_bottom()),
                source,
            ));
        }
        for (index, (port, label)) in card.controls.iter().enumerate() {
            let rect = camera.rect(card.control_button(index));
            if ui
                .interact(
                    rect,
                    ui.id().with(("control", &card.id, *port)),
                    Sense::click(),
                )
                .on_hover_text(format!(
                    "Add the next step ({})",
                    label.trim_end_matches(" →").to_lowercase()
                ))
                .clicked()
            {
                self.menu = Some(ActionMenu::new(
                    rect.center_bottom(),
                    camera.to_world(rect.center_bottom()),
                    Some((card.id.clone(), (*port).to_owned())),
                ));
            }
        }
        if card.kind == NodeKind::Component {
            let rect = camera.rect(card.open_button());
            if ui
                .interact(rect, ui.id().with(("open", &card.id)), Sense::click())
                .on_hover_text("Open the component's steps")
                .clicked()
            {
                commands.push(Command::EditComponent(card.id.clone()));
            }
        }

        // Inline literal fields.
        if zoom < INLINE_EDIT_ZOOM {
            return;
        }
        for (index, row) in card.inputs.iter().enumerate() {
            let field = camera.rect(card.input_field(index));
            match &row.value {
                RowValue::Inline { text, value_type } => {
                    let editing = self.inline.as_ref().filter(|edit| {
                        edit.node == card.id && edit.input == row.name && edit.path == row.path
                    });
                    let mut buffer = editing.map_or_else(|| text.clone(), |edit| edit.text.clone());
                    let invalid = editing.and_then(|edit| edit.error.clone());
                    let response = ui.put(
                        field.shrink2(vec2(6.0 * zoom, 2.0 * zoom)),
                        egui::TextEdit::singleline(&mut buffer)
                            .id(ui.id().with(("inline", &card.id, &row.name, &row.path)))
                            .font(FontId::proportional(13.0 * zoom))
                            .margin(egui::Margin::ZERO)
                            .frame(egui::Frame::NONE)
                            .hint_text(if row.required { "Required" } else { "Optional" })
                            .text_color(if invalid.is_some() {
                                theme::DANGER
                            } else {
                                theme::TEXT
                            }),
                    );
                    let response = match &invalid {
                        Some(message) => response.on_hover_text(message),
                        None => response.on_hover_text(&row.label),
                    };
                    if response.changed() || response.has_focus() {
                        self.inline = Some(InlineEdit {
                            node: card.id.clone(),
                            input: row.name.clone(),
                            path: row.path.clone(),
                            text: buffer.clone(),
                            error: invalid.clone(),
                        });
                    }
                    if response.lost_focus() {
                        if &buffer != text {
                            match parse_inline(&buffer, *value_type) {
                                Ok(value) => {
                                    commands.push(Command::SetLiteral {
                                        node: card.id.clone(),
                                        input: row.name.clone(),
                                        path: row.path.clone(),
                                        value,
                                    });
                                    self.inline = None;
                                }
                                Err(message) => {
                                    self.inline = Some(InlineEdit {
                                        node: card.id.clone(),
                                        input: row.name.clone(),
                                        path: row.path.clone(),
                                        text: buffer,
                                        error: Some(message),
                                    });
                                }
                            }
                        } else {
                            self.inline = None;
                        }
                    }
                }
                RowValue::TypeChoice(current) => {
                    let id = ui.id().with(("type", &card.id, &row.name, &row.path));
                    let mut selected = current.clone();
                    ui.scope_builder(egui::UiBuilder::new().max_rect(field), |ui| {
                        ui.spacing_mut().button_padding = vec2(6.0 * zoom, 2.0 * zoom);
                        ui.spacing_mut().interact_size.y = field.height() - 4.0 * zoom;
                        egui::ComboBox::from_id_salt(id)
                            .width(field.width() - 8.0 * zoom)
                            .selected_text(
                                egui::RichText::new(if selected.is_empty() {
                                    "Choose a type"
                                } else {
                                    &selected
                                })
                                .size(13.0 * zoom),
                            )
                            .show_ui(ui, |ui| {
                                for name in
                                    ["string", "number", "boolean", "object", "array", "null"]
                                {
                                    ui.selectable_value(&mut selected, name.to_owned(), name);
                                }
                            });
                    });
                    if &selected != current {
                        commands.push(Command::SetLiteral {
                            node: card.id.clone(),
                            input: row.name.clone(),
                            path: row.path.clone(),
                            value: serde_json::Value::String(selected),
                        });
                    }
                }
                _ => {}
            }
        }
    }

    /// The socket a proxy's wires leave from, and the button to its variable.
    fn chip_controls(
        &mut self,
        ui: &mut egui::Ui,
        chip: &Chip,
        camera: Camera,
        commands: &mut Vec<Command>,
    ) {
        if chip.source_node().is_some()
            && layout::CardDetail::at_zoom(camera.zoom) == layout::CardDetail::Full
        {
            let rect = camera.rect(chip.follow_button());
            if ui
                .interact(rect, ui.id().with(("follow", chip.id())), Sense::click())
                .on_hover_text("Go to the variable this stands for")
                .clicked()
                && let Some(node) = chip.source_node()
            {
                self.reveal(node.to_owned());
                commands.push(Command::Select(vec![node.to_owned()]));
            }
        }
        let center = camera.to_screen(chip.output_anchor());
        let rect = Rect::from_center_size(
            center,
            Vec2::splat((SOCKET_HIT * camera.zoom).max(3.0) * 2.0),
        );
        let response = ui
            .interact(
                rect,
                ui.id().with(("chip-socket", chip.id())),
                Sense::drag(),
            )
            .on_hover_cursor(egui::CursorIcon::Crosshair);
        if response.drag_started() {
            self.drag = Some(Drag::Wire {
                start: WireStart::Proxy {
                    binding: proxy_binding(&chip.proxy.source),
                },
                from: center,
            });
        }
    }

    fn handle_wire_drag(
        &mut self,
        ui: &egui::Ui,
        cards: &[Card],
        camera: Camera,
        input: &Input<'_>,
        commands: &mut Vec<Command>,
    ) {
        let Some(Drag::Wire { start, from }) = &self.drag else {
            return;
        };
        let (source, port) = match start {
            WireStart::Output { node, port } => (Some(node.clone()), port.clone()),
            // A proxy carries a value, not an output: it fits any argument.
            WireStart::Proxy { .. } => (None, String::new()),
        };
        let (pointer, released) =
            ui.input(|state| (state.pointer.interact_pos(), state.pointer.any_released()));
        let Some(pointer) = pointer else {
            return;
        };
        let control = port.starts_with("__");
        let target = cards
            .iter()
            .filter(|card| Some(&card.id) != source.as_ref())
            .flat_map(|card| {
                let slots: Vec<(String, Vec<String>, Pos2)> = if control {
                    card.input_anchor(ORDER_PORT, &[])
                        .map(|anchor| (ORDER_PORT.to_owned(), Vec::new(), anchor))
                        .into_iter()
                        .collect()
                } else {
                    card.inputs
                        .iter()
                        .enumerate()
                        .map(|(index, row)| {
                            (row.name.clone(), row.path.clone(), card.row_anchor(index))
                        })
                        .collect()
                };
                slots.into_iter().map(move |(name, path, anchor)| {
                    (card.id.clone(), name, path, camera.to_screen(anchor))
                })
            })
            .min_by(|a, b| a.3.distance(pointer).total_cmp(&b.3.distance(pointer)))
            .filter(|(.., anchor)| anchor.distance(pointer) <= SOCKET_HIT * 2.0);

        let end = target.as_ref().map_or(pointer, |(.., anchor)| *anchor);
        let painter = ui.painter();
        paint::wire(
            painter,
            *from,
            end,
            control,
            branch_of(&port),
            true,
            camera.zoom,
        );

        if released {
            let binding = match start {
                WireStart::Proxy { binding } => Some(binding.clone()),
                WireStart::Output { .. } => None,
            };
            self.drag = None;
            match (target, binding) {
                // A proxy binds the value it stands for, as dragging the variable
                // out of the panel does.
                (Some((target, input_name, path, _)), Some(binding)) => {
                    commands.push(Command::Bind {
                        node: target,
                        input: input_name,
                        path,
                        binding,
                    });
                }
                (Some((target, input_name, path, _)), None) => commands.push(Command::Connect {
                    source: source.unwrap_or_default(),
                    source_port: port,
                    target,
                    target_input: input_name,
                    target_path: path,
                }),
                (None, binding) => {
                    if binding.is_some()
                        || cards
                            .iter()
                            .any(|card| camera.rect(card.rect()).contains(pointer))
                    {
                        self.error = Some(if control {
                            "Drop the wire on the top input of the next step.".into()
                        } else {
                            "Drop the wire on an input port.".into()
                        });
                    } else {
                        self.menu = Some(ActionMenu::new(
                            pointer,
                            camera.to_world(pointer),
                            source.map(|source| (source, port)),
                        ));
                        let _ = input;
                    }
                }
            }
        }
    }

    fn handle_box_selection(
        &mut self,
        ui: &egui::Ui,
        background: &egui::Response,
        shapes: Shapes<'_>,
        camera: Camera,
        input: &Input<'_>,
        commands: &mut Vec<Command>,
    ) {
        if background.drag_started_by(PointerButton::Primary)
            && let Some(from) = background.interact_pointer_pos()
        {
            self.drag = Some(Drag::Select {
                from,
                additive: ui.input(|state| state.modifiers.command),
            });
        }
        let Some(Drag::Select { from, additive }) = &self.drag else {
            return;
        };
        let Some(pointer) = ui.input(|state| state.pointer.interact_pos()) else {
            return;
        };
        let area = Rect::from_two_pos(*from, pointer);
        ui.painter().rect(
            area,
            2.0,
            theme::ACCENT.gamma_multiply(0.08),
            Stroke::new(1.0, theme::ACCENT),
            StrokeKind::Inside,
        );
        if background.drag_stopped() {
            let mut selected: Vec<String> = if *additive {
                input.selection.to_vec()
            } else {
                vec![]
            };
            for card in shapes.cards {
                if camera.rect(card.rect()).intersects(area) && !selected.contains(&card.id) {
                    selected.push(card.id.clone());
                }
            }
            for chip in shapes.chips {
                let id = chip.id().to_owned();
                if camera.rect(chip.rect()).intersects(area) && !selected.contains(&id) {
                    selected.push(id);
                }
            }
            self.drag = None;
            self.selected_wires.clear();
            commands.push(Command::Select(selected));
        }
    }

    fn handle_drop(
        &mut self,
        ui: &egui::Ui,
        viewport: Rect,
        cards: &[Card],
        camera: Camera,
        input: &Input<'_>,
        commands: &mut Vec<Command>,
    ) {
        let (pointer, released) =
            ui.input(|state| (state.pointer.interact_pos(), state.pointer.any_released()));
        let Some(pointer) = pointer.filter(|pointer| viewport.contains(*pointer)) else {
            return;
        };
        let Some(payload) = egui::DragAndDrop::payload::<Payload>(ui.ctx()) else {
            return;
        };
        // Highlight the input a variable would bind to.
        let row_under = |cards: &[Card]| {
            cards.iter().find_map(|card| {
                (0..card.inputs.len())
                    .find(|index| camera.rect(card.input_row(*index)).contains(pointer))
                    .map(|index| {
                        (
                            card.id.clone(),
                            card.inputs[index].name.clone(),
                            card.inputs[index].path.clone(),
                            camera.rect(card.input_row(index)),
                        )
                    })
            })
        };
        if let Payload::Variable { .. } = payload.as_ref()
            && let Some((.., rect)) = row_under(cards)
        {
            ui.painter().rect_stroke(
                rect.shrink(2.0),
                4.0,
                Stroke::new(1.5, theme::ACCENT),
                StrokeKind::Inside,
            );
        }
        if !released {
            return;
        }
        let Some(payload) = egui::DragAndDrop::take_payload::<Payload>(ui.ctx()) else {
            return;
        };
        let world = camera.to_world(pointer);
        let position = Position {
            x: f64::from(world.x).round(),
            y: f64::from(world.y).round(),
        };
        match payload.as_ref() {
            Payload::Node { block, component } => commands.push(Command::AddNode {
                block: *block,
                component: component.clone(),
                position,
                source: None,
            }),
            Payload::Operator(operator) => commands.push(Command::AddOperator {
                operator: *operator,
                position,
            }),
            Payload::Variable { binding, .. } => match row_under(cards) {
                Some((node, target_input, path, _)) => commands.push(Command::Bind {
                    node,
                    input: target_input,
                    path,
                    binding: binding.clone(),
                }),
                // Dropped on a card but beside its arguments, it was aimed at one
                // of them.
                None if cards
                    .iter()
                    .any(|card| camera.rect(card.rect()).contains(pointer)) =>
                {
                    self.error = Some("Drop a variable on an argument of a step.".into());
                }
                // Dropped beside the steps, the value is placed there instead, so
                // that its consumers can take it from nearby.
                None => match placeable(binding, input.graph) {
                    Some(source) => commands.push(Command::AddProxy { source, position }),
                    None => {
                        self.error =
                            Some("Only a whole variable, parameter or environment value can be placed on the board. Drop this on an argument of a step.".into());
                    }
                },
            },
        }
    }

    fn handle_keys(&mut self, ui: &egui::Ui, input: &Input<'_>, commands: &mut Vec<Command>) {
        // Keys belong to text fields while one has focus.
        if ui.ctx().egui_wants_keyboard_input()
            || ui.ctx().memory(|memory| memory.top_modal_layer().is_some())
        {
            return;
        }
        if ui.input(|state| state.key_pressed(Key::Escape)) {
            self.menu = None;
            self.drag = None;
        }
        if ui.input_mut(|state| {
            state.consume_shortcut(&egui::KeyboardShortcut::new(
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::L,
            ))
        }) {
            commands.push(Command::TidyUp(input.selection.to_vec()));
        }
        let delete =
            ui.input(|state| state.key_pressed(Key::Delete) || state.key_pressed(Key::Backspace));
        if delete && (!input.selection.is_empty() || !self.selected_wires.is_empty()) {
            commands.push(Command::Delete {
                ids: input.selection.to_vec(),
                wires: std::mem::take(&mut self.selected_wires),
            });
        }
    }

    fn toolbar(
        &mut self,
        ui: &mut egui::Ui,
        viewport: Rect,
        overlays: &BoardOverlays,
        input: &Input<'_>,
        commands: &mut Vec<Command>,
    ) {
        let has_selection = !input.selection.is_empty();
        let measured = egui::Area::new(ui.id().with("board-toolbar"))
            .enabled(ui.is_enabled())
            .constrain_to(viewport)
            .fixed_pos(overlays.toolbar.right_top())
            .pivot(Align2::RIGHT_TOP)
            .fade_in(false)
            .show(ui.ctx(), |ui| {
                ui.spacing_mut().interact_size.y = 26.0;
                ui.spacing_mut().button_padding.y = 4.0;
                egui::Frame::new()
                    .fill(theme::WHITE)
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .corner_radius(8)
                    .inner_margin(egui::Margin::symmetric(6, 3))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .add(widgets::action(Icon::Plus, "Add step").compact().quiet())
                                .on_hover_text("Add a step to the board")
                                .clicked()
                            {
                                let at = ui.min_rect().left_bottom() + vec2(-6.0, 12.0);
                                let camera = self.screen_camera(viewport);
                                self.menu = Some(ActionMenu::new(at, camera.to_world(at), None));
                            }
                            if input.can_paste
                                && ui
                                    .add(
                                        widgets::icon_button(
                                            Icon::Clipboard,
                                            "Paste steps (Ctrl+V)",
                                        )
                                        .compact(),
                                    )
                                    .clicked()
                            {
                                commands.push(Command::Paste);
                            }
                            if ui
                                .add(widgets::icon_button(Icon::Layout, "Tidy up").compact())
                                .on_hover_text(if has_selection {
                                    "Lay out the selected steps (Ctrl+Shift+L)"
                                } else {
                                    "Lay out the whole board (Ctrl+Shift+L)"
                                })
                                .clicked()
                            {
                                commands.push(Command::TidyUp(input.selection.to_vec()));
                            }
                            widgets::action_menu(
                                ui,
                                widgets::icon_button(Icon::Wire, "Wire style").compact(),
                                |ui| {
                                    for (style, label) in [
                                        (WireStyle::Orthogonal, "Right angles"),
                                        (WireStyle::Spline, "Curves"),
                                    ] {
                                        if ui
                                            .add(
                                                widgets::action(
                                                    if self.wire_style == style {
                                                        Icon::Check
                                                    } else {
                                                        Icon::Wire
                                                    },
                                                    label,
                                                )
                                                .quiet(),
                                            )
                                            .clicked()
                                        {
                                            commands.push(Command::SetWireStyle(style));
                                            ui.close();
                                        }
                                    }
                                    ui.separator();
                                    if ui
                                        .add(
                                            widgets::action(
                                                if self.bridges {
                                                    Icon::Check
                                                } else {
                                                    Icon::Wire
                                                },
                                                "Bridges at crossings",
                                            )
                                            .quiet(),
                                        )
                                        .clicked()
                                    {
                                        commands.push(Command::SetBridges(!self.bridges));
                                        ui.close();
                                    }
                                },
                            );
                            // Keep infrequent selection actions in one menu rather
                            // than covering the board with disabled commands.
                            if has_selection || !self.selected_wires.is_empty() {
                                widgets::action_menu(
                                    ui,
                                    widgets::icon_button(Icon::Layers, "Selection actions")
                                        .compact(),
                                    |ui| {
                                        let items = [
                                            (Icon::Copy, "Copy", "Ctrl+C", Command::Copy),
                                            (Icon::Copy, "Duplicate", "Ctrl+D", Command::Duplicate),
                                            (
                                                Icon::Save,
                                                "Save as component",
                                                "",
                                                Command::SaveSelectionAsComponent,
                                            ),
                                        ];
                                        for (icon, label, shortcut, command) in items {
                                            if ui
                                                .add_enabled(
                                                    has_selection,
                                                    widgets::action(icon, label).quiet(),
                                                )
                                                .on_hover_text(shortcut)
                                                .clicked()
                                            {
                                                commands.push(command);
                                                ui.close();
                                            }
                                        }
                                        ui.separator();
                                        if ui
                                            .add(
                                                widgets::action(Icon::Trash, "Delete selection")
                                                    .danger(),
                                            )
                                            .clicked()
                                        {
                                            commands.push(Command::Delete {
                                                ids: input.selection.to_vec(),
                                                wires: std::mem::take(&mut self.selected_wires),
                                            });
                                            ui.close();
                                        }
                                    },
                                );
                            }
                        });
                    });
            });
        let key = ui.id().with((
            "toolbar-size",
            input.selection.len(),
            !self.selected_wires.is_empty(),
            input.can_paste,
        ));
        let size = measured.response.rect.size();
        if ui.ctx().data(|data| data.get_temp::<Vec2>(key)) != Some(size) {
            ui.ctx().data_mut(|data| data.insert_temp(key, size));
            ui.ctx().request_repaint();
        }
    }

    fn screen_camera(&self, viewport: Rect) -> Camera {
        Camera {
            zoom: self.camera.zoom,
            pan: self.camera.pan + viewport.min.to_vec2(),
        }
    }

    fn zoom_controls(
        &mut self,
        ui: &mut egui::Ui,
        viewport: Rect,
        overlays: &BoardOverlays,
        shapes: Shapes<'_>,
    ) {
        egui::Area::new(ui.id().with("board-zoom"))
            .fixed_pos(overlays.zoom_controls.left_center())
            .pivot(Align2::LEFT_CENTER)
            .fade_in(false)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(theme::WHITE)
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(4, 2))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            ui.spacing_mut().button_padding = vec2(6.0, 3.0);
                            ui.spacing_mut().interact_size = vec2(24.0, 26.0);
                            let center = overlays.focus.center();
                            if ui
                                .add(widgets::icon_button(Icon::Minus, "Zoom out").compact())
                                .clicked()
                            {
                                self.zoom_at(viewport, center, 1.0 / 1.2);
                            }
                            ui.add_sized(
                                [44.0, 26.0],
                                egui::Label::new(
                                    egui::RichText::new(format!(
                                        "{}%",
                                        (self.camera.zoom * 100.0).round() as u32
                                    ))
                                    .size(11.0)
                                    .color(theme::TEXT),
                                ),
                            );
                            if ui
                                .add(widgets::icon_button(Icon::Plus, "Zoom in").compact())
                                .clicked()
                            {
                                self.zoom_at(viewport, center, 1.2);
                            }
                            ui.separator();
                            if ui
                                .add(widgets::icon_button(Icon::Fit, "Fit the view").compact())
                                .clicked()
                            {
                                self.fit(viewport, overlays.focus, shapes);
                            }
                        });
                    });
            });
        if let HintPlacement::Tooltip(badge_rect) = overlays.hint {
            egui::Area::new(ui.id().with("board-shortcuts-hint"))
                .fixed_pos(badge_rect.right_center())
                .pivot(Align2::RIGHT_CENTER)
                .fade_in(false)
                .show(ui.ctx(), |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new("Shortcuts")
                                .size(11.0)
                                .color(theme::MUTED),
                        )
                        .sense(Sense::hover()),
                    )
                    .on_hover_text(paint::SHORTCUT_HINT_TEXT);
                });
        }
    }

    fn empty_state(&mut self, ui: &mut egui::Ui, viewport: Rect, commands: &mut Vec<Command>) {
        let rect = Rect::from_center_size(viewport.center() - vec2(0.0, 40.0), vec2(360.0, 150.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("From the first request to a complete test")
                        .size(17.0)
                        .family(theme::bold_family()),
                );
                ui.label(
                    egui::RichText::new("Drag a block from the library or add the first step.")
                        .color(theme::MUTED),
                );
                ui.add_space(8.0);
                if ui
                    .add(widgets::action(Icon::Plus, "Add an HTTP request"))
                    .clicked()
                {
                    commands.push(Command::AddNode {
                        block: Block::Request,
                        component: None,
                        position: Position { x: 100.0, y: 100.0 },
                        source: None,
                    });
                }
            });
        });
    }

    fn error_banner(&mut self, ui: &mut egui::Ui, overlays: &BoardOverlays) {
        let Some(message) = self.error.clone() else {
            return;
        };
        let pos = overlays
            .error_banner
            .map_or(overlays.board.center_top() + vec2(0.0, 24.0), |r| {
                r.center_top()
            });
        egui::Area::new(ui.id().with("board-error"))
            .fixed_pos(pos)
            .pivot(Align2::CENTER_TOP)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgb(253, 236, 232))
                    .stroke(Stroke::new(1.0, theme::DANGER))
                    .corner_radius(8)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.colored_label(theme::DANGER, &message);
                            if ui
                                .add(widgets::icon_button(Icon::Close, "Dismiss").compact())
                                .clicked()
                            {
                                self.error = None;
                            }
                        });
                    });
            });
    }
}

impl Shapes<'_> {
    /// Everything the board draws, as one rectangle.
    fn bounds(self) -> Option<Rect> {
        self.cards
            .iter()
            .map(Card::rect)
            .chain(self.chips.iter().map(Chip::rect))
            .reduce(Rect::union)
    }
}

fn shapes<'a>(cards: &'a [Card], chips: &'a [Chip]) -> Shapes<'a> {
    Shapes { cards, chips }
}

/// The positions a drag leaves behind, for the blocks and proxies it moved.
fn moved_positions(graph: &Graph, ids: &[String], offset: Vec2) -> Vec<(String, Position)> {
    let moved = |position: &Position| Position {
        x: (position.x + f64::from(offset.x)).round(),
        y: (position.y + f64::from(offset.y)).round(),
    };
    graph
        .nodes
        .iter()
        .filter(|node| ids.contains(&node.id))
        .map(|node| (node.id.clone(), moved(&node.position)))
        .chain(
            graph
                .proxies
                .iter()
                .filter(|proxy| ids.contains(&proxy.id))
                .map(|proxy| (proxy.id.clone(), moved(&proxy.position))),
        )
        .collect()
}

/// What a dragged value can be placed as, or `None` for something no proxy stands
/// for: a field of a response, or a value that is not a variable.
fn placeable(binding: &Binding, graph: &Graph) -> Option<ProxySource> {
    let source = apiforge_core::edit::proxy_source(binding)?;
    match &source {
        ProxySource::Variable { node_id } => graph
            .nodes
            .iter()
            .any(|node| &node.id == node_id && node.is_variable())
            .then_some(source),
        ProxySource::Parameter { .. } | ProxySource::Environment { .. } => Some(source),
    }
}

/// Whether a binding reads what a proxy stands for, a field of it included.
fn reads_source(source: &ProxySource, leaf: &Binding) -> bool {
    match (source, leaf) {
        (ProxySource::Variable { node_id }, Binding::Reference { node_id: read, .. }) => {
            read == node_id
        }
        (ProxySource::Parameter { name }, Binding::Parameter { name: read, .. })
        | (ProxySource::Environment { name }, Binding::Environment { name: read }) => read == name,
        _ => false,
    }
}

/// The nearest proxy `wanted` accepts, with its distance to `end`.
fn nearest_chip(chips: &[Chip], wanted: impl Fn(&Chip) -> bool, end: Pos2) -> Option<(usize, f32)> {
    chips
        .iter()
        .enumerate()
        .filter(|(_, chip)| wanted(chip))
        .map(|(index, chip)| (index, chip.output_anchor().distance(end)))
        .min_by(|left, right| left.1.total_cmp(&right.1))
}

/// Where every wire of the board starts and ends.
///
/// A consumer takes its value from the nearest place that value is shown: a proxy
/// of it, or the block itself when the block is nearer. Each reference draws
/// exactly one wire, however many proxies stand for it. A parameter or an
/// environment value has no block, so it is drawn only where it has been placed —
/// without a proxy such a binding is text on the card, as before.
fn board_wires(graph: &Graph, cards: &[Card], chips: &[Chip]) -> Vec<routing::BoardWire> {
    let card_of = |id: &str| cards.iter().position(|card| card.id == id);
    let mut result = Vec::new();
    for wire in wires(graph) {
        let (Some(source), Some(target)) = (card_of(&wire.source), card_of(&wire.target)) else {
            continue;
        };
        let (output, input) = wire_ports(&wire);
        let from_block = routing::Start::Output {
            card: source,
            port: output.clone(),
        };
        let start = match (
            wire.kind,
            cards[target].input_anchor(&input, &wire.operand_path),
        ) {
            (WireKind::Data, Some(end)) => {
                let direct = cards[source]
                    .output_anchor(&output)
                    .map_or(f32::INFINITY, |anchor| anchor.distance(end));
                match nearest_chip(
                    chips,
                    |chip| chip.source_node() == Some(wire.source.as_str()),
                    end,
                ) {
                    Some((index, distance)) if distance < direct => routing::Start::Chip(index),
                    _ => from_block,
                }
            }
            _ => from_block,
        };
        result.push(routing::BoardWire {
            id: wire.id,
            start,
            target,
            input,
            operand_path: wire.operand_path,
            control: wire.kind == WireKind::Order,
            branch: wire.branch,
        });
    }
    for node in &graph.nodes {
        let Some(target) = card_of(&node.id) else {
            continue;
        };
        for (input, binding) in &node.inputs {
            binding.for_each_leaf(&mut |leaf, path| {
                // References are wires of the document already; a parameter or an
                // environment value becomes one only where it is placed.
                if matches!(leaf, Binding::Reference { .. }) {
                    return;
                }
                let Some(end) = cards[target].input_anchor(input, path) else {
                    return;
                };
                let Some((index, _)) =
                    nearest_chip(chips, |chip| reads_source(&chip.proxy.source, leaf), end)
                else {
                    return;
                };
                result.push(routing::BoardWire {
                    id: data_edge_id(&node.id, input, path),
                    start: routing::Start::Chip(index),
                    target,
                    input: input.clone(),
                    operand_path: path.to_vec(),
                    control: false,
                    branch: None,
                });
            });
        }
    }
    result
}

/// What the selected proxies stand for and everything that reads it: the blocks to
/// ring and the wires to pick out.
fn highlights(
    graph: &Graph,
    chips: &[Chip],
    selection: &HashSet<&str>,
) -> (HashSet<String>, HashSet<String>) {
    let mut blocks = HashSet::new();
    let mut wire_ids = HashSet::new();
    for chip in chips.iter().filter(|chip| selection.contains(chip.id())) {
        if let Some(node) = chip.source_node() {
            blocks.insert(node.to_owned());
        }
        for node in &graph.nodes {
            for (input, binding) in &node.inputs {
                binding.for_each_leaf(&mut |leaf, path| {
                    if reads_source(&chip.proxy.source, leaf) {
                        blocks.insert(node.id.clone());
                        wire_ids.insert(data_edge_id(&node.id, input, path));
                    }
                });
            }
        }
    }
    (blocks, wire_ids)
}

fn toggle(list: &mut Vec<String>, id: &str) {
    if let Some(index) = list.iter().position(|item| item == id) {
        list.remove(index);
    } else {
        list.push(id.to_owned());
    }
}

fn toggle_selection(selection: &[String], id: &str, modifiers: Modifiers) -> Vec<String> {
    if modifiers.command || modifiers.shift {
        let mut next = selection.to_vec();
        toggle(&mut next, id);
        next
    } else {
        vec![id.to_owned()]
    }
}

fn branch_of(port: &str) -> Option<Branch> {
    match port {
        apiforge_core::graph::TRUE_PORT => Some(Branch::True),
        apiforge_core::graph::FALSE_PORT => Some(Branch::False),
        _ => None,
    }
}

/// The output and input names a wire connects on its cards.
fn wire_ports(wire: &apiforge_core::graph::Wire) -> (String, String) {
    match wire.kind {
        WireKind::Order => (
            match wire.branch {
                Some(Branch::True) => apiforge_core::graph::TRUE_PORT,
                Some(Branch::False) => apiforge_core::graph::FALSE_PORT,
                None => ORDER_PORT,
            }
            .to_owned(),
            ORDER_PORT.to_owned(),
        ),
        WireKind::Data => (
            wire.source_port.clone().unwrap_or_default(),
            wire.target_input.clone().unwrap_or_default(),
        ),
    }
}

/// The wire under the pointer, within a few pixels of the route that was drawn.
fn hit_wire(drawn: &[(String, Vec<Pos2>)], camera: Camera, pointer: Pos2) -> Option<String> {
    drawn
        .iter()
        .filter_map(|(id, points)| {
            let distance = points
                .windows(2)
                .map(|pair| {
                    routing::distance_to_segment(
                        pointer,
                        camera.to_screen(pair[0]),
                        camera.to_screen(pair[1]),
                    )
                })
                .fold(f32::INFINITY, f32::min);
            (distance <= 6.0).then(|| (distance, id.clone()))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_core::{
        format::{Node, Proxy},
        graph::create_node,
    };

    fn board(variable_x: f64, consumer_x: f64, binding: Binding) -> (Graph, Project) {
        let mut variable = create_node(
            NodeKind::Value,
            Position {
                x: variable_x,
                y: 0.0,
            },
            None,
        );
        variable.id = "token".into();
        variable.handle = "token".into();
        variable.config.variable = Some(true);
        let mut consumer = create_node(
            NodeKind::Assert,
            Position {
                x: consumer_x,
                y: 0.0,
            },
            None,
        );
        consumer.id = "check".into();
        consumer.handle = "check".into();
        consumer.inputs.insert("actual".into(), binding);
        let project = apiforge_core::demos::shop_project();
        (
            Graph {
                nodes: vec![variable, consumer],
                edges: vec![],
                proxies: Vec::new(),
            },
            project,
        )
    }

    fn placed(graph: &Graph, source: ProxySource, x: f64) -> Graph {
        let mut next = graph.clone();
        next.proxies.push(Proxy {
            id: format!("proxy-{}", next.proxies.len()),
            source,
            position: Position { x, y: 40.0 },
        });
        next
    }

    fn drawn(graph: &Graph, project: &Project) -> Vec<routing::BoardWire> {
        let cards: Vec<Card> = graph
            .nodes
            .iter()
            .map(|node: &Node| Card::build(node, graph, project))
            .collect();
        let chips: Vec<Chip> = graph
            .proxies
            .iter()
            .map(|proxy| Chip::build(proxy, graph, project, &["quantity".to_owned()]))
            .collect();
        board_wires(graph, &cards, &chips)
    }

    #[test]
    fn a_consumer_takes_its_value_from_the_nearest_place_it_is_shown() {
        let (graph, project) = board(0.0, 3000.0, Binding::reference("token", "value", ""));
        // Without a proxy the wire crosses the board from the block itself.
        let wires = drawn(&graph, &project);
        assert_eq!(wires.len(), 1);
        assert!(matches!(
            wires[0].start,
            routing::Start::Output { card: 0, .. }
        ));

        // A proxy beside the consumer is nearer, so the wire starts there instead —
        // and it is still one wire, not two.
        let near = placed(
            &graph,
            ProxySource::Variable {
                node_id: "token".into(),
            },
            2700.0,
        );
        let wires = drawn(&near, &project);
        assert_eq!(wires.len(), 1);
        assert_eq!(wires[0].start, routing::Start::Chip(0));
        assert_eq!(wires[0].target, 1);

        // A proxy that sits by the block changes nothing about where the wire
        // starts, so placing one never makes the board longer.
        let far = placed(
            &graph,
            ProxySource::Variable {
                node_id: "token".into(),
            },
            -200.0,
        );
        let wires = drawn(&far, &project);
        assert_eq!(wires.len(), 1);
        assert!(matches!(
            wires[0].start,
            routing::Start::Output { card: 0, .. }
        ));

        // Of several places, the nearest wins.
        let both = placed(
            &far,
            ProxySource::Variable {
                node_id: "token".into(),
            },
            2700.0,
        );
        let wires = drawn(&both, &project);
        assert_eq!(wires.len(), 1);
        assert_eq!(wires[0].start, routing::Start::Chip(1));
    }

    #[test]
    fn a_parameter_is_drawn_only_where_it_has_been_placed() {
        let (graph, project) = board(0.0, 600.0, Binding::parameter_field("quantity", "unit"));
        assert!(drawn(&graph, &project).is_empty(), "no proxy, no wire");
        let with_proxy = placed(
            &graph,
            ProxySource::Parameter {
                name: "quantity".into(),
            },
            400.0,
        );
        let wires = drawn(&with_proxy, &project);
        assert_eq!(wires.len(), 1);
        assert_eq!(wires[0].start, routing::Start::Chip(0));
        assert_eq!(wires[0].input, "actual");
        assert!(!wires[0].control);

        // The proxy stands for the whole parameter, so a field of it is a use too.
        let selection = HashSet::from(["proxy-0"]);
        let chips = vec![Chip::build(
            &with_proxy.proxies[0],
            &with_proxy,
            &project,
            &["quantity".to_owned()],
        )];
        let (blocks, highlighted) = highlights(&with_proxy, &chips, &selection);
        assert_eq!(blocks, HashSet::from(["check".to_owned()]));
        assert_eq!(highlighted, HashSet::from([wires[0].id.clone()]));
    }

    #[test]
    fn selecting_a_proxy_points_at_its_variable_and_everything_reading_it() {
        let (graph, project) = board(0.0, 600.0, Binding::reference("token", "value", "id"));
        let graph = placed(
            &graph,
            ProxySource::Variable {
                node_id: "token".into(),
            },
            400.0,
        );
        let chips = vec![Chip::build(&graph.proxies[0], &graph, &project, &[])];
        assert!(chips[0].known);
        assert_eq!(chips[0].source_node(), Some("token"));
        let (blocks, wires) = highlights(&graph, &chips, &HashSet::from(["proxy-0"]));
        assert_eq!(
            blocks,
            HashSet::from(["token".to_owned(), "check".to_owned()])
        );
        assert_eq!(wires.len(), 1);
        // Nothing is highlighted while the proxy is not selected.
        assert_eq!(
            highlights(&graph, &chips, &HashSet::new()),
            (HashSet::new(), HashSet::new())
        );
    }

    #[test]
    fn a_response_field_is_not_something_the_board_can_place() {
        let (graph, _) = board(0.0, 600.0, Binding::literal(serde_json::json!(1)));
        // What the Variables panel offers can be placed.
        assert_eq!(
            placeable(&Binding::reference("token", "value", ""), &graph),
            Some(ProxySource::Variable {
                node_id: "token".into()
            })
        );
        assert_eq!(
            placeable(&Binding::environment("baseUrl"), &graph),
            Some(ProxySource::Environment {
                name: "baseUrl".into()
            })
        );
        // A field of a response, a plain Value and a literal cannot: they are not
        // listed with the variables, so there would be nothing to show.
        assert_eq!(
            placeable(&Binding::reference("token", "value", "id"), &graph),
            None
        );
        assert_eq!(
            placeable(&Binding::reference("check", "value", ""), &graph),
            None
        );
        assert_eq!(
            placeable(&Binding::literal(serde_json::json!("x")), &graph),
            None
        );
    }

    #[test]
    fn a_proxy_of_something_the_board_no_longer_has_says_so() {
        let (graph, project) = board(0.0, 600.0, Binding::parameter("gone"));
        let graph = placed(
            &graph,
            ProxySource::Parameter {
                name: "gone".into(),
            },
            400.0,
        );
        let chip = Chip::build(&graph.proxies[0], &graph, &project, &[]);
        assert!(!chip.known);
        assert_eq!(chip.label, "gone");
        assert_eq!(chip.source_node(), None);
    }

    #[test]
    fn adding_a_block_uses_the_focus_center_with_viewport_offsets() {
        let canvas = Canvas {
            camera: Camera {
                zoom: 0.7,
                pan: vec2(12.0, -35.0),
            },
            last_viewport: Rect::from_min_size(egui::pos2(235.0, 220.0), vec2(800.0, 500.0)),
            last_focus: Some(Rect::from_min_size(
                egui::pos2(259.0, 290.0),
                vec2(600.0, 340.0),
            )),
            ..Default::default()
        };
        let position = canvas.visible_center();
        let world_center = egui::pos2(
            position.x as f32 + layout::WIDTH / 2.0,
            position.y as f32 + 80.0,
        );
        let actual = canvas
            .screen_camera(canvas.last_viewport)
            .to_screen(world_center);
        assert!(actual.distance(canvas.last_focus.unwrap().center()) < 0.01);
    }
    #[test]
    fn navigator_reveal_survives_toolbar_and_drawer_layout_settling() {
        let context = egui::Context::default();
        crate::theme::apply(&context);
        let project = apiforge_core::demos::shop_project();
        let graph = &project.scenarios[0].graph;
        let target = &graph.nodes[0];
        let mut canvas = Canvas::default();
        let mut selection = Vec::new();
        for frame in 0..8 {
            if frame == 3 {
                selection = vec![target.id.clone()];
                canvas.reveal(target.id.clone());
            }
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        Pos2::ZERO,
                        vec2(if frame < 3 { 1200.0 } else { 850.0 }, 600.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        canvas.show(
                            ui,
                            &Input {
                                graph,
                                reference_graph: graph,
                                project: &project,
                                parameters: &[],
                                selection: &selection,
                                steps: None,
                                can_paste: false,
                                context: "test",
                            },
                        );
                    });
                },
            );
            output.textures_delta.clear();
        }
        let card = Card::build(target, graph, &project);
        let actual = canvas.screen_camera(canvas.last_viewport).rect(card.rect());
        let focus = canvas.last_focus.unwrap();
        assert!(
            actual.center().distance(focus.center()) < 1.0,
            "{actual:?} outside center {focus:?}"
        );
        assert!(focus.expand(0.1).contains_rect(actual));
    }
}
