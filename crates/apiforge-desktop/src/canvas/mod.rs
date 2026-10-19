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
use layout::RowValue;
use menu::ActionMenu;
use std::collections::HashSet;

const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 1.6;
/// Below this zoom, inline fields are drawn as text and edited in the inspector.
const INLINE_EDIT_ZOOM: f32 = 0.6;
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
    last_size: Vec2,
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
            last_size: Vec2::ZERO,
            drag: None,
            menu: None,
            selected_wires: Vec::new(),
            inline: None,
            error: None,
        }
    }
}

impl Canvas {
    /// Shows an error banner on the board, e.g. a rejected connection.
    pub fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
    }

    /// Board position of the visible centre, for blocks added from the library.
    pub fn visible_center(&self) -> Position {
        let center = self.camera.to_world((self.last_size / 2.0).to_pos2());
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

        let mut cards: Vec<Card> = input
            .graph
            .nodes
            .iter()
            .map(|node| Card::build(node, input.reference_graph, input.project))
            .collect();
        if self.fit_pending
            || (viewport.size() - self.last_size).length() > 1.0 && self.last_size == Vec2::ZERO
        {
            self.fit(viewport, &cards);
        }
        self.last_size = viewport.size();
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

        self.handle_camera(ui, &background, viewport);
        paint::grid(&painter, viewport, camera);

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
                &painter,
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

        for card in &cards {
            let rect = camera.rect(card.rect());
            if !rect.intersects(viewport) {
                continue;
            }
            let id = ui.id().with(("card", &card.id));
            let response = ui.interact(rect, id, Sense::click_and_drag());
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
                &painter,
                card,
                camera,
                selection.contains(card.id.as_str()),
                step,
            );
            self.card_controls(ui, card, camera, input, &mut commands);
        }

        self.handle_wire_drag(ui, &cards, camera, input, &mut commands);
        self.handle_box_selection(ui, &background, &cards, camera, input, &mut commands);

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

        self.handle_drop(ui, viewport, &cards, camera, &mut commands);
        self.handle_keys(ui, &background, input, &mut commands);

        paint::overlay_labels(&painter, viewport, input.graph);
        self.toolbar(ui, viewport, input, &mut commands);
        self.zoom_controls(ui, viewport, &cards);
        paint::minimap(&painter, viewport, &cards, camera, &selection);
        if input.graph.nodes.is_empty() {
            self.empty_state(ui, viewport, &mut commands);
        }
        self.error_banner(ui, viewport);
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

    fn fit(&mut self, viewport: Rect, cards: &[Card]) {
        self.fit_pending = false;
        let Some(bounds) = cards.iter().map(Card::rect).reduce(Rect::union) else {
            self.camera = Camera {
                zoom: 1.0,
                pan: vec2(60.0, 60.0),
            };
            return;
        };
        let space = viewport.shrink2(vec2(50.0, 60.0));
        let zoom = (space.width() / bounds.width())
            .min(space.height() / bounds.height())
            .clamp(MIN_ZOOM, 1.0);
        let center = (viewport.size() / 2.0).to_pos2();
        self.camera = Camera {
            zoom,
            pan: center.to_vec2() - bounds.center().to_vec2() * zoom,
        };
    }

    fn zoom_at(&mut self, viewport: Rect, screen: Pos2, factor: f32) {
        let before = self.camera.zoom;
        self.camera.zoom = (before * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let local = screen - viewport.min;
        self.camera.pan = local - (local - self.camera.pan) * (self.camera.zoom / before);
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
            } else {
                self.camera.pan += scroll;
            }
        }
        if background.dragged_by(PointerButton::Middle)
            || background.dragged_by(PointerButton::Secondary)
        {
            self.camera.pan += background.drag_delta();
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
            let rect = Rect::from_center_size(center, Vec2::splat(SOCKET_HIT * 2.0));
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
                        field.shrink2(vec2(2.0, 1.0)),
                        egui::TextEdit::singleline(&mut buffer)
                            .id(ui.id().with(("inline", &card.id, &row.name)))
                            .font(FontId::proportional(11.0 * zoom))
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
                        egui::ComboBox::from_id_salt(id)
                            .width(field.width() - 8.0)
                            .selected_text(if selected.is_empty() {
                                "Choose a type"
                            } else {
                                &selected
                            })
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

    fn handle_keys(
        &mut self,
        ui: &egui::Ui,
        background: &egui::Response,
        input: &Input<'_>,
        commands: &mut Vec<Command>,
    ) {
        // Keys belong to text fields while one has focus.
        if ui.ctx().egui_wants_keyboard_input()
            || !(background.contains_pointer() || background.has_focus())
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
        input: &Input<'_>,
        commands: &mut Vec<Command>,
    ) {
        let has_selection = !input.selection.is_empty();
        let anchor = viewport.right_top() + vec2(-14.0, 14.0);
        egui::Area::new(ui.id().with("board-toolbar"))
            .fixed_pos(anchor)
            .pivot(Align2::RIGHT_TOP)
            .show(ui.ctx(), |ui| {
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
                            ui.separator();
                            let tool = |ui: &mut egui::Ui, enabled: bool, text: &str, tip: &str| {
                                ui.add_enabled(enabled, egui::Button::new(text).frame(false))
                                    .on_hover_text(tip)
                                    .clicked()
                            };
                            if tool(
                                ui,
                                has_selection,
                                "Component",
                                "Save the selected steps as a component",
                            ) {
                                commands.push(Command::SaveSelectionAsComponent);
                            }
                            if tool(ui, has_selection, "Copy", "Copy the selection (Ctrl+C)") {
                                commands.push(Command::Copy);
                            }
                            if tool(ui, input.can_paste, "Paste", "Paste copied steps (Ctrl+V)") {
                                commands.push(Command::Paste);
                            }
                            if tool(
                                ui,
                                has_selection,
                                "Duplicate",
                                "Duplicate the selection (Ctrl+D)",
                            ) {
                                commands.push(Command::Duplicate);
                            }
                            if tool(
                                ui,
                                has_selection || !self.selected_wires.is_empty(),
                                "Delete",
                                "Delete the selected steps and connections (Delete)",
                            ) {
                                commands.push(Command::Delete {
                                    nodes: input.selection.to_vec(),
                                    wires: std::mem::take(&mut self.selected_wires),
                                });
                            }
                        });
                    });
            });
    }

    fn screen_camera(&self, viewport: Rect) -> Camera {
        Camera {
            zoom: self.camera.zoom,
            pan: self.camera.pan + viewport.min.to_vec2(),
        }
    }

    fn zoom_controls(&mut self, ui: &mut egui::Ui, viewport: Rect, cards: &[Card]) {
        egui::Area::new(ui.id().with("board-zoom"))
            .fixed_pos(viewport.left_bottom() + vec2(14.0, -14.0))
            .pivot(Align2::LEFT_BOTTOM)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(theme::WHITE)
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .corner_radius(6)
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            let center = viewport.center();
                            if ui
                                .add(egui::Button::new("+").frame(false))
                                .on_hover_text("Zoom in")
                                .clicked()
                            {
                                self.zoom_at(viewport, center, 1.2);
                            }
                            if ui
                                .add(egui::Button::new("−").frame(false))
                                .on_hover_text("Zoom out")
                                .clicked()
                            {
                                self.zoom_at(viewport, center, 1.0 / 1.2);
                            }
                            if ui
                                .add(egui::Button::new("⛶").frame(false))
                                .on_hover_text("Fit the view")
                                .clicked()
                            {
                                self.fit(viewport, cards);
                            }
                        });
                    });
            });
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

    fn error_banner(&mut self, ui: &mut egui::Ui, viewport: Rect) {
        let Some(message) = self.error.clone() else {
            return;
        };
        egui::Area::new(ui.id().with("board-error"))
            .fixed_pos(viewport.center_top() + vec2(0.0, 60.0))
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
