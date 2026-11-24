//! The "add step" menu: opened from the toolbar, by right-clicking, from a card's
//! action buttons or by dropping a wire on empty space.

use super::Command;
use crate::icons::Icon;
use crate::{library, theme};
use apiforge_core::{
    block::Block,
    format::{Graph, Port, Position, Project},
    graph::{ORDER_PORT, TRUE_PORT, compatible, output_ports},
    model::NodeKind,
    operator::OperatorGroup,
};
use eframe::egui::{self, Key, Pos2, RichText, Stroke};

pub enum Outcome {
    Open,
    Close,
    Add(Command),
    /// A Variables or Operators entry, added as the library panel adds it.
    Library(library::Action),
    /// *Save shared values as variables* (LAY-25).
    SaveShared,
}

/// The entry *Save shared values as variables (N)* at the bottom of the
/// menu opened on the empty board (LAY-25).
#[derive(Clone, Copy, Debug)]
pub struct Shared {
    /// The values it saves.
    pub values: usize,
    pub editable: bool,
}

/// The entry's name, which says how many values it takes.
pub fn shared_entry(values: usize) -> String {
    format!("Save shared values as variables ({values})")
}

pub struct ActionMenu {
    screen: Pos2,
    world: Pos2,
    /// Node and output the new step connects from.
    source: Option<(String, String)>,
    search: String,
    focus_search: bool,
    /// Operator groups open in the popup's Operators section.
    open_groups: Vec<OperatorGroup>,
    /// Opened by *Add step* on the board's bar: a step it adds goes after
    /// the selection (DIAG-06a).
    bar: bool,
    /// Opened on a check, value, variable or export in a branch: the menu
    /// offers *Move out of the branch* for it (DIAG-06d).
    pub out_of_branch: Option<String>,
    /// Opened on a step of the folder window's board, from its card or its
    /// foot: a step it adds goes directly after that step, unbound (DIAG-22).
    pub after_step: bool,
    /// Opened by the `+` on a line of a chain: a step it adds goes on that
    /// line, after its source by its port (DIAG-06b).
    pub on_line: bool,
    /// Opened on a step of the folder window's board: the menu offers *Move
    /// earlier* and *Move later* for it (DIAG-06b).
    pub moves: Option<Moves>,
}

/// *Move earlier* and *Move later* for a step, and why each is refused,
/// worked out once when its menu opens (DIAG-06b): the board checks the
/// move again when it is taken.
pub struct Moves {
    id: String,
    handle: String,
    earlier: Option<String>,
    later: Option<String>,
}

impl Moves {
    pub fn of(graph: &Graph, project: &Project, id: &str) -> Self {
        let refused = |later| {
            apiforge_core::edit::move_by_one(graph, project, id, later)
                .err()
                .map(|error| error.to_string())
        };
        Self {
            id: id.to_owned(),
            handle: graph
                .nodes
                .iter()
                .find(|node| node.id == id)
                .map_or_else(String::new, |node| node.handle.clone()),
            earlier: refused(false),
            later: refused(true),
        }
    }
}

/// The entry that writes a check of a branch after its `if` (DIAG-06d).
pub const MOVE_OUT: &str = "Move out of the branch";
/// The entries that move a step by one in its braces (DIAG-06b).
pub const MOVE_EARLIER: &str = "Move earlier";
pub const MOVE_LATER: &str = "Move later";

/// Whether `id` is a check, value, variable or export that stands in the
/// braces of an `if`.
pub fn in_a_branch(graph: &Graph, id: &str) -> bool {
    graph
        .nodes
        .iter()
        .any(|node| node.id == id && node.kind.is_data())
        && apiforge_core::chain::braces(graph).contains_key(id)
}

/// The output a card's menu starts from: control flow for Start and IF, otherwise
/// the first data output.
pub fn default_source(graph: &Graph, project: &Project, node_id: &str) -> Option<(String, String)> {
    let node = graph.nodes.iter().find(|node| node.id == node_id)?;
    let port = match node.kind {
        NodeKind::Start => ORDER_PORT.to_owned(),
        NodeKind::If => TRUE_PORT.to_owned(),
        _ => output_ports(node, project)
            .first()
            .map_or_else(|| ORDER_PORT.to_owned(), |port| port.name.clone()),
    };
    Some((node.id.clone(), port))
}

/// Whether a step taken from the menu of a card of this kind goes directly
/// after it (DIAG-22). The menus of Start and of an `if` start from a control
/// port, and a check or a value is no step.
pub fn steps_go_after(kind: NodeKind) -> bool {
    apiforge_core::chain::is_step(kind) && !matches!(kind, NodeKind::Start | NodeKind::If)
}

/// Whether `block` adds a step of a chain, Start excluded.
fn adds_step(block: Block) -> bool {
    apiforge_core::chain::is_step(block.kind()) && block.kind() != NodeKind::Start
}

struct Action {
    block: Block,
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
            open_groups: library::default_open_groups(),
            bar: false,
            out_of_branch: None,
            after_step: false,
            on_line: false,
            moves: None,
        }
    }

    /// The menu of *Add step* on the board's bar: what it adds goes after
    /// the selection, not last (DIAG-06a).
    pub fn bar(screen: Pos2, world: Pos2) -> Self {
        Self {
            bar: true,
            ..Self::new(screen, world, None)
        }
    }

    /// The steps offered on a scenario's board, or on a component's with
    /// `component`, its ID: there *Export* is offered, and *Start* and *Data
    /// condition*, which only a scenario runs, are not (UX-21, decision 12),
    /// nor the component itself (UX-22).
    fn actions(&self, graph: &Graph, project: &Project, component: Option<&str>) -> Vec<Action> {
        let source_port: Option<Port> = self.source.as_ref().and_then(|(node, port)| {
            let node = graph.nodes.iter().find(|candidate| &candidate.id == node)?;
            output_ports(node, project)
                .into_iter()
                .find(|candidate| &candidate.name == port)
        });
        let blocks: &[Block] = if self.source.is_some() {
            &[
                Block::Request,
                Block::Branch,
                Block::Value,
                Block::Comparison,
                Block::Condition,
                Block::Object,
                Block::Variable,
            ]
        } else {
            &[
                Block::Start,
                Block::Branch,
                Block::Request,
                Block::Value,
                Block::Object,
                Block::Comparison,
                Block::Condition,
                Block::Variable,
            ]
        };
        // On a line, only a step: the line runs between steps (DIAG-06b).
        let on_line = |block: &Block| !self.on_line || adds_step(*block);
        let mut actions: Vec<Action> = blocks
            .iter()
            .copied()
            .chain([Block::Export])
            .filter(|block| block.offered(component.is_some()))
            .filter(on_line)
            .map(|block| Action {
                block,
                name: block.label().to_owned(),
                description: block.description().to_owned(),
                component: None,
            })
            .collect();
        // A component put after the step reads nothing of it (DIAG-22).
        let data_source = !self.after_step
            && self
                .source
                .as_ref()
                .is_some_and(|(_, port)| !port.starts_with("__"));
        for component in library::offered_components(project, component) {
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
                block: Block::Component,
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

    /// The output a block taken from the menu is added from: a step from a
    /// step's own menu from its *Next* (DIAG-22), anything else from the
    /// menu's source.
    fn source_for(&self, block: Block) -> Option<(String, String)> {
        let (node, port) = self.source.clone()?;
        if self.after_step && adds_step(block) {
            Some((node, ORDER_PORT.to_owned()))
        } else {
            Some((node, port))
        }
    }

    /// Shows the popup. With `library`, the steps are followed by the block
    /// library's Variables and Operators sections (KTLO-05d). `component`, a
    /// component's ID, offers that component's steps (UX-21, UX-22). With
    /// `shared`, the menu opened on the empty board ends with *Save shared
    /// values as variables*, outside the scrolling list (LAY-25).
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        graph: &Graph,
        project: &Project,
        library: Option<&library::Context<'_>>,
        component: Option<&str>,
        shared: Option<Shared>,
    ) -> Outcome {
        // The opening click belongs to the toolbar/card, outside the new popup.
        // Preserve this before requesting search focus clears the initial flag.
        let opening = self.focus_search;
        let mut outcome = Outcome::Open;
        let source_node = self
            .source
            .as_ref()
            .and_then(|(id, _)| graph.nodes.iter().find(|node| &node.id == id));
        let actions = self.actions(graph, project, component);
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
                                            format!("AFTER: {}", node.display_name().to_uppercase())
                                        }
                                        None => "STEP LIBRARY".into(),
                                    })
                                    .size(10.0)
                                    .color(theme::MUTED),
                                );
                                ui.label(
                                    RichText::new(if source_node.is_some() {
                                        "What should happen next?"
                                    } else if component.is_some() {
                                        "Add a step to the component"
                                    } else {
                                        "Add a step to the scenario"
                                    })
                                    .family(theme::bold_family())
                                    .size(14.0),
                                );
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                                if ui
                                    .add(
                                        crate::widgets::icon_button(Icon::Close, "Close").compact(),
                                    )
                                    .clicked()
                                {
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
                        // First, above the steps the menu adds, where the
                        // menu of a card low on the board still shows it.
                        if let Some(moves) = &self.moves {
                            for (later, icon, label, keys, refused) in [
                                (false, Icon::ChevronUp, MOVE_EARLIER, "Alt+↑", &moves.earlier),
                                (true, Icon::ChevronDown, MOVE_LATER, "Alt+↓", &moves.later),
                            ] {
                                // The keys stand right in the entry, seen
                                // without a hover.
                                let entry = ui
                                    .horizontal(|ui| {
                                        let entry = ui
                                            .add_enabled(
                                                refused.is_none(),
                                                crate::widgets::action(icon, label).quiet(),
                                            )
                                            .on_hover_text(format!(
                                                "Run `{}` one step {} in its braces, with its checks. {keys}",
                                                moves.handle,
                                                if later { "later" } else { "earlier" }
                                            ))
                                            .on_disabled_hover_text(
                                                refused.clone().unwrap_or_default(),
                                            );
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.label(
                                                    RichText::new(keys)
                                                        .size(11.0)
                                                        .color(theme::MUTED),
                                                );
                                            },
                                        );
                                        entry
                                    })
                                    .inner;
                                if entry.clicked() {
                                    outcome = Outcome::Add(Command::MoveStep {
                                        id: moves.id.clone(),
                                        later,
                                    });
                                }
                            }
                            ui.separator();
                        }
                        if let Some(id) = &self.out_of_branch {
                            let entry = ui
                                .add(crate::widgets::action(Icon::ArrowLeft, MOVE_OUT).quiet())
                                .on_hover_text(
                                    "Write it after the condition's braces: it then runs whichever way the condition goes, when what it reads exists.",
                                );
                            if entry.clicked() {
                                outcome = Outcome::Add(Command::MoveOutOfBranch(id.clone()));
                            }
                            ui.separator();
                        }
                        let search = crate::widgets::named(
                            ui.add(
                                crate::widgets::text_edit(&mut self.search)
                                    .hint_text("Find an action…")
                                    .desired_width(f32::INFINITY),
                            ),
                            "Find an action",
                        );
                        if self.focus_search {
                            search.request_focus();
                            self.focus_search = false;
                        }
                        let query = self.search.to_lowercase();
                        egui::ScrollArea::vertical()
                            .max_height(320.0)
                            .show(ui, |ui| {
                                let matching: Vec<&Action> = actions
                                    .iter()
                                    .filter(|action| {
                                        format!("{} {}", action.name, action.description)
                                            .to_lowercase()
                                            .contains(&query)
                                    })
                                    .collect();
                                if matching.is_empty() {
                                    ui.add_space(6.0);
                                    ui.label(
                                        RichText::new("No matching steps").color(theme::MUTED),
                                    );
                                }
                                for action in matching {
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
                                        outcome = Outcome::Add(if self.bar {
                                            Command::AddStep {
                                                block: action.block,
                                                component: action.component.clone(),
                                                position,
                                            }
                                        } else if let Some(after) =
                                            self.source.clone().filter(|_| self.on_line)
                                        {
                                            // On the line it was opened on
                                            // (DIAG-06b), as a step dropped
                                            // there lands (DIAG-06c).
                                            Command::AddOnLine {
                                                block: action.block,
                                                component: action.component.clone(),
                                                position,
                                                after,
                                            }
                                        } else {
                                            Command::AddNode {
                                                block: action.block,
                                                component: action.component.clone(),
                                                position,
                                                source: self.source_for(action.block),
                                            }
                                        });
                                    }
                                }
                                if let Some(context) = library.filter(|_| !self.on_line) {
                                    let matches = |text: &str| text.to_lowercase().contains(&query);
                                    let mut actions = Vec::new();
                                    ui.add_space(6.0);
                                    library::variables_section(ui, context, &matches, &mut actions);
                                    ui.add_space(6.0);
                                    library::operators_section(
                                        ui,
                                        &mut self.open_groups,
                                        &query,
                                        &mut actions,
                                    );
                                    if let Some(action) = actions.pop() {
                                        outcome = Outcome::Library(action);
                                    }
                                }
                            });
                        if let Some(shared) = shared.filter(|_| self.source.is_none()) {
                            ui.add_space(4.0);
                            ui.separator();
                            let enabled = shared.editable && shared.values > 0;
                            let entry = ui
                                .add_enabled(
                                    enabled,
                                    crate::widgets::action(
                                        Icon::Variable,
                                        &shared_entry(shared.values),
                                    )
                                    .quiet(),
                                )
                                .on_hover_text(
                                    "Every value of a request or a component call that three or more steps read, one of them far, is saved in a variable after its step, and those steps read the variable. One undo.",
                                )
                                .on_disabled_hover_text(if !shared.editable {
                                    "The board cannot be edited now."
                                } else {
                                    "No value of a request or a component call is read by three or more steps, one of them far, without a variable."
                                });
                            if entry.clicked() {
                                outcome = Outcome::SaveShared;
                            }
                        }
                    });
            });
        if ctx.input(|input| input.key_pressed(Key::Escape)) {
            outcome = Outcome::Close;
        }
        // Clicking elsewhere closes the menu.
        if area.response.clicked_elsewhere() && matches!(outcome, Outcome::Open) && !opening {
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
    let color = theme::block_color(action.block);
    ui.painter()
        .rect_filled(icon, 6.0, color.gamma_multiply(0.12));
    theme::block_icon(ui.painter(), icon, action.block, color);
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
    crate::widgets::named(
        response.on_hover_cursor(egui::CursorIcon::PointingHand),
        &action.name,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_offer_control_ordering_instead_of_a_fabricated_data_port() {
        let project = apiforge_core::demos::shop_project();
        let export = Block::Export.create(Position { x: 0.0, y: 0.0 }, None);
        let id = export.id.clone();
        let graph = Graph {
            nodes: vec![export],
            edges: vec![],
            proxies: Vec::new(),
        };
        assert_eq!(
            default_source(&graph, &project, &id),
            Some((id, ORDER_PORT.into()))
        );
    }

    /// UX-21: a card's *+* and a dropped wire, which open the menu from an
    /// output, offer a component *Export* and not *Data condition*; a
    /// scenario's menu from the same output is unchanged.
    #[test]
    fn a_components_menu_from_an_output_offers_export_and_no_condition() {
        let project = apiforge_core::demos::shop_project();
        let graph = &project.scenarios[0].graph;
        let source = (graph.nodes[0].id.clone(), ORDER_PORT.to_owned());
        let menu = ActionMenu::new(Pos2::ZERO, Pos2::ZERO, Some(source));
        let blocks = |component| {
            menu.actions(graph, &project, component)
                .into_iter()
                .filter(|action| action.component.is_none())
                .map(|action| action.block)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            blocks(Some("component")),
            [
                Block::Request,
                Block::Branch,
                Block::Value,
                Block::Comparison,
                Block::Object,
                Block::Variable,
                Block::Export,
            ]
        );
        assert_eq!(
            blocks(None),
            [
                Block::Request,
                Block::Branch,
                Block::Value,
                Block::Comparison,
                Block::Condition,
                Block::Object,
                Block::Variable,
            ]
        );
    }

    #[test]
    fn opening_click_keeps_step_menu_open_but_a_later_outside_click_closes_it() {
        let context = egui::Context::default();
        theme::apply(&context);
        let project = apiforge_core::demos::shop_project();
        let graph = &project.scenarios[0].graph;
        let mut popup = None;
        let mut button = egui::Rect::NOTHING;
        let mut frame = |events| {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 600.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let response = ui.add(crate::widgets::action(Icon::Plus, "Add step"));
                    button = response.rect;
                    if response.clicked() {
                        popup = Some(ActionMenu::new(
                            egui::pos2(100.0, 100.0),
                            egui::Pos2::ZERO,
                            None,
                        ));
                    }
                    if let Some(menu) = &mut popup
                        && matches!(
                            menu.show(&context, graph, &project, None, None, None),
                            Outcome::Close
                        )
                    {
                        popup = None;
                    }
                },
            );
            output.textures_delta.clear();
            (button, popup.is_some())
        };
        let at = frame(vec![]).0.center();
        let click = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(vec![egui::Event::PointerMoved(at), click(at, true)]);
        assert!(
            frame(vec![click(at, false)]).1,
            "opening click dismissed the menu"
        );
        assert!(frame(vec![]).1);
        let outside = egui::pos2(700.0, 550.0);
        frame(vec![
            egui::Event::PointerMoved(outside),
            click(outside, true),
        ]);
        assert!(
            !frame(vec![click(outside, false)]).1,
            "later outside click must dismiss the menu"
        );
    }
}
