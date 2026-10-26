//! The left sidebar: workspace and scenario navigation and the block library.

use crate::icons::Icon;
use crate::{dnd::Payload, theme, widgets};
use apiforge_core::{
    block::Block,
    format::{Binding, Project},
    graph::latest_components,
    operator::{Operator, OperatorGroup},
};
use eframe::egui::{self, RichText, Sense, Stroke};

/// Requests from the sidebar to the application.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    SwitchProject(String),
    NewProject,
    SwitchScenario(String),
    NewScenario,
    AddNode {
        block: Block,
        component: Option<(String, u32)>,
    },
    AddOperator(Operator),
    SelectNode(String),
    OpenExample(Example),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Example {
    Workflow,
    OperatorBlocks,
}

const BUILTIN: [Block; 7] = Block::BUILTIN;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Section {
    #[default]
    Blocks,
    Variables,
    Operators,
}

pub struct Library {
    section: Section,
    pub search: String,
    open_groups: Vec<OperatorGroup>,
}

impl Default for Library {
    fn default() -> Self {
        Self {
            search: String::new(),
            section: Section::Blocks,
            open_groups: default_open_groups(),
        }
    }
}

pub struct Context<'a> {
    pub project: &'a Project,
    pub projects: Vec<(String, String)>,
    pub scenario_id: &'a str,
    /// Variable nodes of the board: id and name.
    pub variables: Vec<(String, String)>,
    /// Parameters of the scenario, or arguments of the component being edited.
    pub parameters: Vec<String>,
    pub editing_component: bool,
    pub running: bool,
    pub document_locked: bool,
    /// The *Examples* entry opens demo scenarios into the workspace; the folder
    /// window leaves it out (KTLO-05a decision 2), the legacy window keeps it.
    pub show_examples: bool,
}

impl Library {
    pub fn show(&mut self, ui: &mut egui::Ui, context: &Context<'_>) -> Vec<Action> {
        let mut actions = Vec::new();
        brand(ui);
        ui.add_space(14.0);

        widgets::caption(ui, "WORKSPACE");
        ui.horizontal(|ui| {
            let mut selected = context.project.id.clone();
            ui.add_enabled_ui(!context.running, |ui| {
                egui::ComboBox::from_id_salt("project-picker")
                    .width(ui.available_width() - 36.0)
                    .selected_text(widgets::ellipsis(&context.project.name, 28))
                    .show_ui(ui, |ui| {
                        for (id, name) in &context.projects {
                            ui.selectable_value(&mut selected, id.clone(), name);
                        }
                    });
                if ui
                    .add(widgets::icon_button(Icon::Plus, "New project"))
                    .clicked()
                {
                    actions.push(Action::NewProject);
                }
            });
            if selected != context.project.id {
                actions.push(Action::SwitchProject(selected));
            }
        });
        ui.add_space(12.0);

        ui.horizontal(|ui| {
            widgets::caption(ui, "SCENARIOS");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(
                        !context.running,
                        widgets::icon_button(Icon::Plus, "New scenario").compact(),
                    )
                    .clicked()
                {
                    actions.push(Action::NewScenario);
                }
            });
        });
        for scenario in &context.project.scenarios {
            let active = scenario.id == context.scenario_id;
            let enabled = !context.running || active;
            if ui
                .add_enabled(enabled, widgets::NavItem::new(&scenario.name, active))
                .clicked()
                && !active
            {
                actions.push(Action::SwitchScenario(scenario.id.clone()));
            }
        }
        ui.add_space(12.0);

        actions.extend(self.show_blocks(ui, context));
        actions
    }

    /// The block library alone: search, the Blocks / Variables / Operators
    /// sections and the project's components, without workspace navigation.
    /// Shared by the legacy sidebar and the folder window's left panel.
    pub fn show_blocks(&mut self, ui: &mut egui::Ui, context: &Context<'_>) -> Vec<Action> {
        let mut actions = Vec::new();
        let components = latest_components(context.project);
        ui.horizontal(|ui| {
            let heading = ui.label(
                RichText::new("BLOCK LIBRARY")
                    .size(theme::FONT_META)
                    .family(theme::bold_family())
                    .color(theme::MUTED),
            );
            // The screen-reader name of the panel is the title case "Block
            // library", not the visually upper-cased caption.
            heading.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Label, true, "Block library")
            });
            ui.label(
                // A component's graph also offers the Export block.
                RichText::new(
                    (BUILTIN.len() + usize::from(context.editing_component) + components.len())
                        .to_string(),
                )
                .size(11.0)
                .color(theme::MUTED),
            );
        });
        widgets::named(
            ui.add_sized(
                [ui.available_width(), theme::CONTROL_HEIGHT],
                crate::widgets::text_edit(&mut self.search)
                    .hint_text("Search blocks…")
                    .desired_width(f32::INFINITY),
            ),
            "Search blocks",
        );
        ui.add_space(4.0);
        egui::Frame::new()
            .fill(theme::FIELD)
            .corner_radius(7)
            .inner_margin(3)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.spacing_mut().button_padding.x = 4.0;
                let width = (ui.available_width() - 8.0) / 3.0;
                ui.horizontal(|ui| {
                    for (section, title) in [
                        (Section::Blocks, "Blocks"),
                        (Section::Variables, "Variables"),
                        (Section::Operators, "Operators"),
                    ] {
                        let active = self.section == section;
                        let mut button = egui::Button::new(
                            RichText::new(title)
                                .size(theme::FONT_LABEL)
                                .color(if active { theme::TEXT } else { theme::MUTED }),
                        )
                        .selected(active)
                        .frame(active)
                        .corner_radius(5);
                        if active {
                            button = button
                                .fill(theme::WHITE)
                                .stroke(Stroke::new(1.0, theme::BORDER));
                        }
                        if ui.add_sized([width, 28.0], button).clicked() {
                            self.section = section;
                        }
                    }
                });
            });
        let query = self.search.to_lowercase();
        // Search spans categories, so an operator can be discovered from any tab.
        let all_sections = !query.is_empty();
        let show_blocks = all_sections || self.section == Section::Blocks;
        let show_variables = all_sections || self.section == Section::Variables;
        let show_operators = all_sections || self.section == Section::Operators;
        let matches = |text: &str| query.is_empty() || text.to_lowercase().contains(&query);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height((ui.available_height() - 68.0).max(80.0))
            .show(ui, |ui| {
                if context.document_locked {
                    ui.disable();
                }
                if show_blocks && !context.editing_component && context.show_examples {
                    widgets::action_menu(
                        ui,
                        widgets::action(Icon::Layers, "Examples").quiet(),
                        |ui| {
                            if ui
                                .add(
                                    widgets::action(Icon::Layers, "Ordering and IF / ELSE").quiet(),
                                )
                                .clicked()
                            {
                                actions.push(Action::OpenExample(Example::Workflow));
                                ui.close();
                            }
                            if ui
                                .add(widgets::action(Icon::Variable, "Operator blocks").quiet())
                                .clicked()
                            {
                                actions.push(Action::OpenExample(Example::OperatorBlocks));
                                ui.close();
                            }
                        },
                    );
                    ui.add_space(4.0);
                }
                if show_blocks {
                    widgets::subcaption(ui, "BASIC STEPS");
                    // Only a component's graph can pass values out.
                    let export = context.editing_component.then_some(Block::Export);
                    for block in BUILTIN.into_iter().chain(export) {
                        if matches(&format!("{} {}", block.label(), block.description())) {
                            let payload = Payload::Node {
                                block,
                                component: None,
                            };
                            if library_item(ui, block, block.label(), block.description(), payload)
                                .clicked()
                            {
                                actions.push(Action::AddNode {
                                    block,
                                    component: None,
                                });
                            }
                        }
                    }
                }
                if show_variables {
                    ui.add_space(6.0);
                    variables_section(ui, context, &matches, &mut actions);
                }

                ui.add_space(6.0);
                if show_operators {
                    operators_section(ui, &mut self.open_groups, &query, &mut actions);
                }
                if show_blocks {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        widgets::subcaption(ui, "PROJECT COMPONENTS");
                        ui.label(
                            RichText::new(components.len().to_string())
                                .size(11.0)
                                .color(theme::MUTED),
                        );
                    });
                    if components.is_empty() {
                        ui.label(
                            RichText::new(
                                "Select steps on the board and save them as a component.",
                            )
                            .size(11.0)
                            .color(theme::MUTED),
                        );
                    }
                    for component in components {
                        let description = if component.description.is_empty() {
                            format!("Component · r{}", component.revision)
                        } else {
                            component.description.clone()
                        };
                        if !matches(&format!("{} {description}", component.name)) {
                            continue;
                        }
                        let pinned = Some((component.id.clone(), component.revision));
                        let payload = Payload::Node {
                            block: Block::Component,
                            component: pinned.clone(),
                        };
                        if library_item(
                            ui,
                            Block::Component,
                            &component.name,
                            &description,
                            payload,
                        )
                        .clicked()
                        {
                            actions.push(Action::AddNode {
                                block: Block::Component,
                                component: pinned,
                            });
                        }
                    }
                }
            });
        actions
    }

    fn variables(
        ui: &mut egui::Ui,
        context: &Context<'_>,
        matches: &dyn Fn(&str) -> bool,
        actions: &mut Vec<Action>,
    ) {
        let environment = &context.project.environment;
        let mut chips: Vec<(String, String, Binding, Option<String>)> = Vec::new();
        for (id, name) in &context.variables {
            chips.push((
                name.clone(),
                "variable".into(),
                Binding::reference(id.clone(), "value", ""),
                Some(id.clone()),
            ));
        }
        for name in &context.parameters {
            let kind = if context.editing_component {
                "argument"
            } else {
                "parameter"
            };
            chips.push((
                name.clone(),
                kind.into(),
                Binding::parameter(name.clone()),
                None,
            ));
        }
        chips.push((
            "baseUrl".into(),
            "environment".into(),
            Binding::environment("baseUrl"),
            None,
        ));
        for name in environment
            .variables
            .keys()
            .chain(environment.secret_names.iter())
        {
            if chips
                .iter()
                .any(|(label, kind, ..)| label == name && kind != "variable")
            {
                continue;
            }
            let kind = if environment.secret_names.contains(name) {
                "secret"
            } else {
                "environment"
            };
            chips.push((
                name.clone(),
                kind.into(),
                Binding::environment(name.clone()),
                None,
            ));
        }
        ui.label(
            RichText::new(
                "Drag onto an argument, into a URL, query or header, or onto the board to show it there.",
            )
            .size(11.0)
            .color(theme::MUTED),
        );
        ui.horizontal_wrapped(|ui| {
            for (label, kind, binding, node) in chips {
                if !matches(&label) {
                    continue;
                }
                let response = widgets::variable_chip(
                    ui,
                    &label,
                    &kind,
                    Payload::Variable {
                        label: label.clone(),
                        binding,
                    },
                );
                if response.clicked()
                    && let Some(node) = node
                {
                    actions.push(Action::SelectNode(node));
                }
            }
        });
    }
}

/// The operator groups open before the person opens or closes any.
pub fn default_open_groups() -> Vec<OperatorGroup> {
    vec![OperatorGroup::Comparison, OperatorGroup::Logic]
}

/// The *Variables* section: its caption with *Add variable*, then a chip for
/// each variable, parameter and environment value matching the search.
/// Shared by the library and the board's *Add step* popup (KTLO-05d).
pub fn variables_section(
    ui: &mut egui::Ui,
    context: &Context<'_>,
    matches: &dyn Fn(&str) -> bool,
    actions: &mut Vec<Action>,
) {
    ui.horizontal(|ui| {
        widgets::subcaption(ui, "VARIABLES");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(widgets::icon_button(Icon::Plus, "Add variable").compact())
                .clicked()
            {
                actions.push(Action::AddNode {
                    block: Block::Variable,
                    component: None,
                });
            }
        });
    });
    Library::variables(ui, context, matches, actions);
}

/// The *Operators* section: a disclosure per group with its chips; every group
/// with a match is open while `query` (lower case) is not empty. Shared by the
/// library and the board's *Add step* popup (KTLO-05d).
pub fn operators_section(
    ui: &mut egui::Ui,
    open_groups: &mut Vec<OperatorGroup>,
    query: &str,
    actions: &mut Vec<Action>,
) {
    widgets::subcaption(ui, "OPERATORS");
    for group in OperatorGroup::ALL {
        let operators: Vec<Operator> = Operator::ALL
            .into_iter()
            .filter(|operator| {
                operator.spec().group == group && operator.label().to_lowercase().contains(query)
            })
            .collect();
        if operators.is_empty() {
            continue;
        }
        let open = !query.is_empty() || open_groups.contains(&group);
        let header = ui.add(widgets::Disclosure::new(
            group.label(),
            open,
            operators.len(),
        ));
        if header.clicked() {
            if let Some(index) = open_groups.iter().position(|item| *item == group) {
                open_groups.remove(index);
            } else {
                open_groups.push(group);
            }
        }
        if open {
            ui.horizontal_wrapped(|ui| {
                for operator in operators {
                    if operator_chip(ui, operator).clicked() {
                        actions.push(Action::AddOperator(operator));
                    }
                }
            });
        }
    }
}

fn brand(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(30.0, 30.0), Sense::hover());
        ui.painter().rect_filled(rect, 8.0, theme::ACCENT);
        theme::block_icon(ui.painter(), rect.shrink(3.0), Block::Branch, theme::WHITE);
        ui.label(
            RichText::new("apiforge")
                .size(20.0)
                .family(theme::bold_family()),
        );
    });
}

fn library_item(
    ui: &mut egui::Ui,
    block: Block,
    title: &str,
    description: &str,
    payload: Payload,
) -> egui::Response {
    let id = ui.id().with(("library", title, block));
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 48.0),
        Sense::click_and_drag(),
    );
    response.dnd_set_drag_payload(payload);
    let hovered = response.hovered() || response.dragged();
    if hovered {
        ui.painter().rect_filled(rect, 7.0, theme::BG);
        ui.painter().rect_stroke(
            rect,
            7.0,
            Stroke::new(1.0, theme::BORDER),
            egui::StrokeKind::Inside,
        );
    }
    let icon = egui::Rect::from_min_size(rect.min + egui::vec2(4.0, 10.0), egui::vec2(28.0, 28.0));
    let color = theme::block_color(block);
    ui.painter()
        .rect_filled(icon, 7.0, color.gamma_multiply(0.12));
    theme::block_icon(ui.painter(), icon, block, color);
    let right = rect.right() - if hovered { 22.0 } else { 8.0 };
    widgets::paint_label(
        ui.painter(),
        egui::Rect::from_min_max(
            rect.min + egui::vec2(44.0, 5.0),
            egui::pos2(right, rect.top() + 24.0),
        ),
        title,
        egui::FontId::new(theme::FONT_BODY, theme::bold_family()),
        theme::TEXT,
        egui::Align::Min,
    );
    widgets::paint_label(
        ui.painter(),
        egui::Rect::from_min_max(
            rect.min + egui::vec2(44.0, 25.0),
            egui::pos2(right, rect.top() + 43.0),
        ),
        description,
        egui::FontId::proportional(theme::FONT_META),
        theme::MUTED,
        egui::Align::Min,
    );
    if hovered {
        ui.painter().text(
            rect.right_center() - egui::vec2(10.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            "+",
            egui::FontId::proportional(13.0),
            theme::MUTED,
        );
    }
    let _ = id;
    widgets::named(
        response
            .on_hover_text(format!(
                "{description} — click to add, or drag onto the board"
            ))
            .on_hover_cursor(egui::CursorIcon::Grab),
        title,
    )
}

fn operator_chip(ui: &mut egui::Ui, operator: Operator) -> egui::Response {
    let text = egui::WidgetText::from(
        RichText::new(operator.label())
            .size(11.0)
            .color(theme::OPERATOR),
    );
    let galley = text.into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    let size = galley.size() + egui::vec2(16.0, 10.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    response.dnd_set_drag_payload(Payload::Operator(operator));
    let fill = if response.hovered() {
        theme::OPERATOR.gamma_multiply(0.18)
    } else {
        theme::OPERATOR.gamma_multiply(0.1)
    };
    ui.painter().rect_filled(rect, 6.0, fill);
    ui.painter()
        .galley(rect.min + egui::vec2(8.0, 5.0), galley, theme::OPERATOR);
    widgets::named(
        response
            .on_hover_text("Click to add as a step, or drag into an argument")
            .on_hover_cursor(egui::CursorIcon::Grab),
        operator.label(),
    )
}
