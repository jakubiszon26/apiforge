//! The board: an interactive view of one graph.
//!
//! The board reads the document and returns [`Command`]s; the application applies
//! them through the editor, which validates every change. Positions being dragged,
//! wires being drawn and the camera are view state only.

mod layout;
mod menu;
mod paint;

pub use layout::{Card, parse_inline};

use crate::{dnd::Payload, theme};
use apiforge_core::{
    format::{Binding, Graph, Position, Project},
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
    },
    AddNode {
        kind: NodeKind,
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
        value: serde_json::Value,
    },
    Bind {
        node: String,
        input: String,
        binding: Binding,
    },
    Delete {
        nodes: Vec<String>,
        wires: Vec<String>,
    },
    EditComponent(String),
    SaveSelectionAsComponent,
    Copy,
    Paste,
    Duplicate,
}

/// Everything the board needs to draw one frame.
pub struct Input<'a> {
    pub graph: &'a Graph,
    /// Nodes that bindings may reference (main graph nodes on the cleanup board).
    pub reference_graph: &'a Graph,
    pub project: &'a Project,
    pub selection: &'a [String],
    pub steps: Option<&'a [StepResult]>,
    pub can_paste: bool,
    /// Changing context resets the camera and transient state.
    pub context: &'a str,
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
    /// Moving the selected cards; `offset` is in world units.
    Nodes { ids: Vec<String>, offset: Vec2 },
    /// Drawing a wire from an output.
    Wire {
        source: String,
        port: String,
        from: Pos2,
    },
    /// Rubber-band selection from a screen position.
    Select { from: Pos2, additive: bool },
}

#[derive(Clone, Debug)]
struct InlineEdit {
    node: String,
    input: String,
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

        let focus_changed = self
            .last_focus
            .is_some_and(|prev| (prev.size() - overlays.focus.size()).length() > 1.0);
        if self.fit_pending || self.last_size == Vec2::ZERO {
            self.fit(viewport, overlays.focus, &cards);
        } else if focus_changed {
            if !self.camera_moved_by_user {
                self.fit(viewport, overlays.focus, &cards);
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
        }

        let board_painter = painter.with_clip_rect(overlays.board);
        paint::grid(&board_painter, overlays.board, camera);

        // Wires go under the cards.
        let graph_wires = wires(input.graph);
        for wire in &graph_wires {
            let (Some(source), Some(target)) = (
                cards.iter().find(|card| card.id == wire.source),
                cards.iter().find(|card| card.id == wire.target),
            ) else {
                continue;
            };
            let (output, inp) = wire_ports(wire);
            let (Some(start), Some(end)) =
                (source.output_anchor(&output), target.input_anchor(&inp))
            else {
                continue;
            };
            paint::wire(
                &board_painter,
                camera.to_screen(start),
                camera.to_screen(end),
                wire.kind == WireKind::Order,
                wire.branch,
                self.selected_wires.contains(&wire.id),
                camera.zoom,
            );
        }

        let selection: HashSet<&str> = input.selection.iter().map(String::as_str).collect();
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
                let moved = input
                    .graph
                    .nodes
                    .iter()
                    .filter(|node| ids.contains(&node.id))
                    .map(|node| {
                        (
                            node.id.clone(),
                            Position {
                                x: (node.position.x + f64::from(offset.x)).round(),
                                y: (node.position.y + f64::from(offset.y)).round(),
                            },
                        )
                    })
                    .collect();
                commands.push(Command::Move(moved));
            }

            let step = input
                .steps
                .and_then(|steps| steps.iter().find(|step| step.node_id == card.id));
            paint::card(
                &board_painter,
                card,
                camera,
                selection.contains(card.id.as_str()),
                step,
            );
            self.card_controls(&mut board_ui, card, camera, input, &mut commands);
        }

        self.handle_wire_drag(&board_ui, &cards, camera, input, &mut commands);
        self.handle_box_selection(&board_ui, &background, &cards, camera, input, &mut commands);

        if background.clicked() && !clicked_card {
            self.menu = None;
            self.inline = None;
            let hit = pointer.and_then(|pointer| hit_wire(&graph_wires, &cards, camera, pointer));
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

        self.handle_drop(&board_ui, overlays.board, &cards, camera, &mut commands);
        self.handle_keys(ui, input, &mut commands);

        paint::overlay_chrome(&painter, &overlays, input.graph);
        ui.interact(
            overlays.banner,
            ui.id().with("execution-mode-hint"),
            Sense::hover(),
        )
        .on_hover_text(paint::mode_line_text(input.graph));
        self.toolbar(ui, viewport, &overlays, input, &mut commands);
        self.zoom_controls(ui, viewport, &overlays, &cards);
        if let Some((mm_inner, bounds, scale)) = paint::minimap(
            &painter,
            overlays.minimap,
            overlays.board,
            &cards,
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
        if input.graph.nodes.is_empty() {
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

    fn fit(&mut self, viewport: Rect, focus: Rect, cards: &[Card]) {
        self.fit_pending = false;
        self.camera_moved_by_user = false;
        let local_focus_center = focus.center() - viewport.min;
        let Some(bounds) = cards.iter().map(Card::rect).reduce(Rect::union) else {
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
                    source: card.id.clone(),
                    port: port.clone(),
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
                    let editing = self
                        .inline
                        .as_ref()
                        .filter(|edit| edit.node == card.id && edit.input == row.name);
                    let mut buffer = editing.map_or_else(|| text.clone(), |edit| edit.text.clone());
                    let invalid = editing.and_then(|edit| edit.error.clone());
                    let response = ui.put(
                        field.shrink2(vec2(6.0 * zoom, 2.0 * zoom)),
                        egui::TextEdit::singleline(&mut buffer)
                            .id(ui.id().with(("inline", &card.id, &row.name)))
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
                                        value,
                                    });
                                    self.inline = None;
                                }
                                Err(message) => {
                                    self.inline = Some(InlineEdit {
                                        node: card.id.clone(),
                                        input: row.name.clone(),
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
                    let id = ui.id().with(("type", &card.id, &row.name));
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
                            value: serde_json::Value::String(selected),
                        });
                    }
                }
                _ => {}
            }
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
        let Some(Drag::Wire { source, port, from }) = &self.drag else {
            return;
        };
        let (pointer, released) =
            ui.input(|state| (state.pointer.interact_pos(), state.pointer.any_released()));
        let Some(pointer) = pointer else {
            return;
        };
        let control = port.starts_with("__");
        let target = cards
            .iter()
            .filter(|card| &card.id != source)
            .flat_map(|card| {
                let names: Vec<String> = if control {
                    vec![ORDER_PORT.to_owned()]
                } else {
                    card.inputs.iter().map(|row| row.name.clone()).collect()
                };
                names.into_iter().filter_map(move |name| {
                    let anchor = camera.to_screen(card.input_anchor(&name)?);
                    Some((card.id.clone(), name, anchor))
                })
            })
            .min_by(|a, b| a.2.distance(pointer).total_cmp(&b.2.distance(pointer)))
            .filter(|(_, _, anchor)| anchor.distance(pointer) <= SOCKET_HIT * 2.0);

        let end = target.as_ref().map_or(pointer, |(_, _, anchor)| *anchor);
        let painter = ui.painter();
        paint::wire(
            painter,
            *from,
            end,
            control,
            branch_of(port),
            true,
            camera.zoom,
        );

        if released {
            let (source, port) = (source.clone(), port.clone());
            self.drag = None;
            match target {
                Some((target, input_name, _)) => commands.push(Command::Connect {
                    source,
                    source_port: port,
                    target,
                    target_input: input_name,
                }),
                None if cards
                    .iter()
                    .any(|card| camera.rect(card.rect()).contains(pointer)) =>
                {
                    self.error = Some(if control {
                        "Drop the wire on the top input of the next step.".into()
                    } else {
                        "Drop the wire on an input port.".into()
                    });
                }
                None => {
                    self.menu = Some(ActionMenu::new(
                        pointer,
                        camera.to_world(pointer),
                        Some((source, port)),
                    ));
                    let _ = input;
                }
            }
        }
    }

    fn handle_box_selection(
        &mut self,
        ui: &egui::Ui,
        background: &egui::Response,
        cards: &[Card],
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
            for card in cards {
                if camera.rect(card.rect()).intersects(area) && !selected.contains(&card.id) {
                    selected.push(card.id.clone());
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
                            camera.rect(card.input_row(index)),
                        )
                    })
            })
        };
        if let Payload::Variable { .. } = payload.as_ref()
            && let Some((_, _, rect)) = row_under(cards)
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
            Payload::Node { kind, component } => commands.push(Command::AddNode {
                kind: *kind,
                component: component.clone(),
                position,
                source: None,
            }),
            Payload::Operator(operator) => commands.push(Command::AddOperator {
                operator: *operator,
                position,
            }),
            Payload::Variable { binding, .. } => match row_under(cards) {
                Some((node, input, _)) => commands.push(Command::Bind {
                    node,
                    input,
                    binding: binding.clone(),
                }),
                None => self.error = Some("Drop a variable on an argument of a step.".into()),
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
        let delete =
            ui.input(|state| state.key_pressed(Key::Delete) || state.key_pressed(Key::Backspace));
        if delete && (!input.selection.is_empty() || !self.selected_wires.is_empty()) {
            commands.push(Command::Delete {
                nodes: input.selection.to_vec(),
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
                                .button("+ Add step")
                                .on_hover_text("Add a step to the board")
                                .clicked()
                            {
                                let at = ui.min_rect().left_bottom() + vec2(-6.0, 12.0);
                                let camera = self.screen_camera(viewport);
                                self.menu = Some(ActionMenu::new(at, camera.to_world(at), None));
                            }
                            if input.can_paste
                                && ui
                                    .button("Paste")
                                    .on_hover_text("Paste copied steps (Ctrl+V)")
                                    .clicked()
                            {
                                commands.push(Command::Paste);
                            }
                            // Keep infrequent selection actions in one menu rather
                            // than covering the board with disabled commands.
                            if has_selection || !self.selected_wires.is_empty() {
                                ui.menu_button(
                                    format!("Selection ({})", input.selection.len()),
                                    |ui| {
                                        let items = [
                                            ("Copy", "Ctrl+C", Command::Copy),
                                            ("Duplicate", "Ctrl+D", Command::Duplicate),
                                            (
                                                "Save as component",
                                                "",
                                                Command::SaveSelectionAsComponent,
                                            ),
                                        ];
                                        for (label, shortcut, command) in items {
                                            if ui
                                                .add_enabled(
                                                    has_selection,
                                                    egui::Button::new(label)
                                                        .shortcut_text(shortcut),
                                                )
                                                .clicked()
                                            {
                                                commands.push(command);
                                                ui.close();
                                            }
                                        }
                                        ui.separator();
                                        if ui
                                            .button(
                                                egui::RichText::new("Delete selection")
                                                    .color(theme::DANGER),
                                            )
                                            .clicked()
                                        {
                                            commands.push(Command::Delete {
                                                nodes: input.selection.to_vec(),
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
        cards: &[Card],
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
                                .add_sized([28.0, 28.0], egui::Button::new("−").frame(false))
                                .on_hover_text("Zoom out")
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
                                .add_sized([28.0, 28.0], egui::Button::new("+").frame(false))
                                .on_hover_text("Zoom in")
                                .clicked()
                            {
                                self.zoom_at(viewport, center, 1.2);
                            }
                            ui.separator();
                            if ui
                                .add(
                                    egui::Button::new(egui::RichText::new("Fit").size(11.0))
                                        .frame(false),
                                )
                                .on_hover_text("Fit the view")
                                .clicked()
                            {
                                self.fit(viewport, overlays.focus, cards);
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
                if ui.button("+ Add an HTTP request").clicked() {
                    commands.push(Command::AddNode {
                        kind: NodeKind::Request,
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
                            if ui.small_button("✕").on_hover_text("Dismiss").clicked() {
                                self.error = None;
                            }
                        });
                    });
            });
    }
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

/// The wire under the pointer, within a few pixels of its curve.
fn hit_wire(
    graph_wires: &[apiforge_core::graph::Wire],
    cards: &[Card],
    camera: Camera,
    pointer: Pos2,
) -> Option<String> {
    graph_wires
        .iter()
        .filter_map(|wire| {
            let source = cards.iter().find(|card| card.id == wire.source)?;
            let target = cards.iter().find(|card| card.id == wire.target)?;
            let (output, input) = wire_ports(wire);
            let start = camera.to_screen(source.output_anchor(&output)?);
            let end = camera.to_screen(target.input_anchor(&input)?);
            let points = paint::wire_points(start, end, wire.kind == WireKind::Order, camera.zoom);
            let distance = (0..=24)
                .map(|step| paint::bezier(points, step as f32 / 24.0).distance(pointer))
                .fold(f32::INFINITY, f32::min);
            (distance <= 6.0).then(|| (distance, wire.id.clone()))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

#[cfg(test)]
mod tests {
    use super::*;

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
