//! The "add step" menu: opened from the toolbar, by right-clicking, from a card's
//! action buttons or by dropping a wire on empty space.

use super::Command;
use crate::theme;
use apiforge_core::{
    format::{Graph, Port, Position, Project},
    graph::{ORDER_PORT, TRUE_PORT, compatible, latest_components, output_ports},
    model::{NodeKind, ValueType},
};
use eframe::egui::{self, Key, Pos2, RichText, Stroke};

pub enum Outcome {
    Open,
    Close,
    Add(Command),
}

pub struct ActionMenu {
    screen: Pos2,
    world: Pos2,
    /// Node and output the new step connects from.
    source: Option<(String, String)>,
    search: String,
    focus_search: bool,
}

/// The output a card's menu starts from: control flow for Start and IF, otherwise
/// the first data output.
pub fn default_source(graph: &Graph, project: &Project, node_id: &str) -> Option<(String, String)> {
    let node = graph.nodes.iter().find(|node| node.id == node_id)?;
    let port = match node.kind {
        NodeKind::Trigger => ORDER_PORT.to_owned(),
        NodeKind::If => TRUE_PORT.to_owned(),
        _ => output_ports(node, project)
            .first()
            .map_or_else(|| "value".to_owned(), |port| port.name.clone()),
    };
    Some((node.id.clone(), port))
}

struct Action {
    kind: NodeKind,
    name: String,
    description: String,
    component: Option<(String, u32)>,
}

impl ActionMenu {
    pub fn new(screen: Pos2, world: Pos2, source: Option<(String, String)>) -> Self {
        Self {
            screen,
            world,
            source,
            search: String::new(),
            focus_search: true,
        }
    }

    fn actions(&self, graph: &Graph, project: &Project) -> Vec<Action> {
        let source_port: Option<Port> = self.source.as_ref().and_then(|(node, port)| {
            let node = graph.nodes.iter().find(|candidate| &candidate.id == node)?;
            output_ports(node, project)
                .into_iter()
                .find(|candidate| &candidate.name == port)
        });
        let kinds: &[NodeKind] = if self.source.is_some() {
            &[
                NodeKind::Request,
                NodeKind::If,
                NodeKind::Select,
                NodeKind::Assert,
                NodeKind::Condition,
                NodeKind::Formula,
                NodeKind::Build,
            ]
        } else {
            &[
                NodeKind::Trigger,
                NodeKind::If,
                NodeKind::Request,
                NodeKind::Select,
                NodeKind::Build,
                NodeKind::Formula,
                NodeKind::Assert,
                NodeKind::Condition,
                NodeKind::Variable,
            ]
        };
        let numeric = Port::new("left", ValueType::Number);
        let mut actions: Vec<Action> = kinds
            .iter()
            .filter(|kind| {
                **kind != NodeKind::Formula
                    || source_port
                        .as_ref()
                        .is_none_or(|port| compatible(port, &numeric))
            })
            .map(|kind| Action {
                kind: *kind,
                name: kind.label().to_owned(),
                description: kind.description().to_owned(),
                component: None,
            })
            .collect();
        let data_source = self
            .source
            .as_ref()
            .is_some_and(|(_, port)| !port.starts_with("__"));
        for component in latest_components(project) {
            // From a data output, offer only components declaring a compatible action.
            if data_source {
                let Some(action) = &component.action else {
                    continue;
                };
                let mut target = Port::new(action.input.clone(), action.value_type);
                target.contract_id = action.contract_id.clone();
                if !source_port
                    .as_ref()
                    .is_some_and(|port| compatible(port, &target))
                {
                    continue;
                }
            }
            actions.push(Action {
                kind: NodeKind::Component,
                name: component.name.clone(),
                description: if component.description.is_empty() {
                    format!("Component · revision {}", component.revision)
                } else {
                    component.description.clone()
                },
                component: Some((component.id.clone(), component.revision)),
            });
        }
        actions
    }

    pub fn show(&mut self, ctx: &egui::Context, graph: &Graph, project: &Project) -> Outcome {
        let mut outcome = Outcome::Open;
        let source_node = self
            .source
            .as_ref()
            .and_then(|(id, _)| graph.nodes.iter().find(|node| &node.id == id));
        let actions = self.actions(graph, project);
        let area = egui::Area::new(egui::Id::new("board-action-menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(self.screen)
            .constrain(true)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(theme::WHITE)
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .corner_radius(10)
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.set_width(300.0);
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(match source_node {
                                        Some(node) => {
                                            format!("AFTER: {}", node.name.to_uppercase())
                                        }
                                        None => "STEP LIBRARY".into(),
                                    })
                                    .size(10.0)
                                    .color(theme::MUTED),
                                );
                                ui.label(
                                    RichText::new(if source_node.is_some() {
                                        "What should happen next?"
                                    } else {
                                        "Add a step to the scenario"
                                    })
                                    .family(theme::bold_family())
                                    .size(14.0),
                                );
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                                if ui.small_button("✕").on_hover_text("Close (Esc)").clicked() {
                                    outcome = Outcome::Close;
                                }
                            });
                        });
                        if let (Some(node), Some((_, port))) = (source_node, &mut self.source)
                            && !port.starts_with("__")
                        {
                            let outputs = output_ports(node, project);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Data from").color(theme::MUTED));
                                egui::ComboBox::from_id_salt("menu-source-port")
                                    .selected_text(port.as_str())
                                    .show_ui(ui, |ui| {
                                        for output in &outputs {
                                            ui.selectable_value(
                                                port,
                                                output.name.clone(),
                                                format!(
                                                    "{} · {}",
                                                    output.display_name(),
                                                    output.value_type
                                                ),
                                            );
                                        }
                                    });
                            });
                        }
                        let search = ui.add(
                            egui::TextEdit::singleline(&mut self.search)
                                .hint_text("Find an action…")
                                .desired_width(f32::INFINITY),
                        );
                        if self.focus_search {
                            search.request_focus();
                            self.focus_search = false;
                        }
                        let query = self.search.to_lowercase();
                        egui::ScrollArea::vertical()
                            .max_height(320.0)
                            .show(ui, |ui| {
                                for action in actions.iter().filter(|action| {
                                    format!("{} {}", action.name, action.description)
                                        .to_lowercase()
                                        .contains(&query)
                                }) {
                                    if action_row(ui, action).clicked() {
                                        let position = source_node.map_or(
                                            Position {
                                                x: f64::from(self.world.x).round(),
                                                y: f64::from(self.world.y).round(),
                                            },
                                            |node| Position {
                                                x: node.position.x + 340.0,
                                                y: node.position.y + 20.0,
                                            },
                                        );
                                        outcome = Outcome::Add(Command::AddNode {
                                            kind: action.kind,
                                            component: action.component.clone(),
                                            position,
                                            source: self.source.clone(),
                                        });
                                    }
                                }
                            });
                    });
            });
        if ctx.input(|input| input.key_pressed(Key::Escape)) {
            outcome = Outcome::Close;
        }
        // Clicking elsewhere closes the menu.
        if area.response.clicked_elsewhere()
            && matches!(outcome, Outcome::Open)
            && !self.focus_search
        {
            let pressed_inside = ctx
                .input(|input| input.pointer.press_origin())
                .is_some_and(|origin| area.response.rect.contains(origin));
            if !pressed_inside && ctx.input(|input| input.pointer.primary_clicked()) {
                outcome = Outcome::Close;
            }
        }
        outcome
    }
}

fn action_row(ui: &mut egui::Ui, action: &Action) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 44.0), egui::Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 6.0, theme::BG);
    }
    let icon = egui::Rect::from_min_size(rect.min + egui::vec2(6.0, 8.0), egui::vec2(28.0, 28.0));
    let color = theme::kind_color(action.kind);
    ui.painter()
        .rect_filled(icon, 6.0, color.gamma_multiply(0.12));
    theme::kind_icon(ui.painter(), icon, action.kind, color);
    ui.painter().text(
        rect.min + egui::vec2(44.0, 14.0),
        egui::Align2::LEFT_CENTER,
        &action.name,
        egui::FontId::new(12.5, theme::bold_family()),
        theme::TEXT,
    );
    ui.painter().text(
        rect.min + egui::vec2(44.0, 30.0),
        egui::Align2::LEFT_CENTER,
        crate::labels::truncate(&action.description, 44),
        egui::FontId::proportional(10.5),
        theme::MUTED,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
