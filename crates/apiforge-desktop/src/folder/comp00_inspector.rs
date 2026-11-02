//! COMP-00 mock-up (throwaway branch `mock/comp-00`): the Inspector of a
//! component's input, of a new input and of the component itself, drawn with
//! the Inspector's own widgets. Values are shown, not kept.
use crate::icons::Icon;
use crate::inspector::fields::type_combo;
use crate::mock_comp00::{self as mock, BoardInput, Panel};
use crate::{theme, widgets};
use apiforge_core::format::{Component, OptionalJson, Project};
use apiforge_core::model::ValueType;
use eframe::egui::{self, Id, RichText, Ui};

fn heading(ui: &mut Ui, caption: &str, title: &str) {
    ui.label(RichText::new(caption).size(11.0).color(theme::MUTED));
    ui.label(
        RichText::new(title)
            .size(theme::FONT_PANEL_TITLE)
            .family(theme::bold_family()),
    );
    ui.add_space(theme::SPACE_SM);
    ui.separator();
}

fn field(ui: &mut Ui, id: Id, label: &str, text: &str, hint: &str) -> egui::Response {
    widgets::field_label(ui, label);
    let mut text = text.to_owned();
    widgets::named(
        ui.add(
            widgets::text_edit(&mut text)
                .id(id)
                .hint_text(hint)
                .desired_width(f32::INFINITY),
        ),
        label,
    )
}

pub fn show(ui: &mut Ui, component: &Component, project: &Project) {
    let flags = mock::get();
    let board = mock::Board::of(component);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_width((ui.available_width() - theme::SPACE_SM).max(0.0));
            match &flags.panel {
                Panel::Off => {}
                Panel::NewInput => new_input(ui, &flags.new_name),
                Panel::Input { name, draft_name } => {
                    if let Some(input) = board.inputs.iter().find(|input| &input.name == name) {
                        let port = component
                            .inputs
                            .iter()
                            .find(|port| &port.name == name)
                            .expect("the input exists");
                        input_form(ui, component, input, port.value_type, draft_name.as_deref());
                    }
                }
                Panel::Component => settings(ui, &board, project),
            }
        });
}

/// Q2 A: *+ Add input* opens this form with the name focused.
fn new_input(ui: &mut Ui, name: &str) {
    heading(ui, "COMPONENT INPUT", "New input");
    let id = Id::new("comp00-new-input");
    let response = field(ui, id.with("name"), "Name", name, "e.g. productId");
    response.request_focus();
    widgets::hint(
        ui,
        format!(
            "Steps read it as param.{}.",
            if name.is_empty() { "name" } else { name }
        ),
    );
    ui.columns(2, |columns| {
        widgets::field_label(&mut columns[0], "Type");
        let mut value_type = ValueType::String;
        type_combo(&mut columns[0], id.with("type"), &mut value_type);
        field(
            &mut columns[1],
            id.with("default"),
            "Default",
            "\"EUR\"",
            "None",
        );
    });
    ui.add_space(theme::SPACE_SM);
    ui.horizontal(|ui| {
        let mut required = false;
        ui.checkbox(&mut required, "Required")
            .on_hover_text("Callers must give a value");
        let mut exposed = true;
        ui.checkbox(&mut exposed, "Editable on the block");
    });
    ui.add_space(theme::SPACE_MD);
    ui.horizontal(|ui| {
        ui.add(widgets::primary_button("Add input"));
        ui.add(egui::Button::new("Cancel"));
    });
}

/// Q3: the selected input; a draft name that fails validation stays in the
/// field, outlined, and the code keeps the last valid name.
fn input_form(
    ui: &mut Ui,
    component: &Component,
    input: &BoardInput,
    value_type: ValueType,
    draft_name: Option<&str>,
) {
    heading(ui, "COMPONENT INPUT", &input.name);
    let id = Id::new(("comp00-input", &input.name));
    let duplicate = draft_name.filter(|draft| {
        component
            .inputs
            .iter()
            .any(|port| port.name == *draft && port.name != input.name)
    });
    widgets::field_label(ui, "Name");
    let mut name = draft_name.unwrap_or(&input.name).to_owned();
    let response = widgets::validated(ui, duplicate.is_some(), |ui| {
        widgets::named(
            ui.add(
                widgets::text_edit(&mut name)
                    .id(id.with("name"))
                    .desired_width(f32::INFINITY),
            ),
            "Name",
        )
    });
    if let Some(duplicate) = duplicate {
        response.request_focus();
        widgets::field_error(
            ui,
            format!(
                "Another input is already named {duplicate}. The code keeps {} until the name is unique.",
                input.name
            ),
        );
    } else {
        widgets::hint(ui, format!("Steps read it as param.{}.", input.name));
    }
    field(
        ui,
        id.with("label"),
        "Label on the block",
        input.label.as_deref().unwrap_or_default(),
        "Shown instead of the name",
    );
    let default = component
        .inputs
        .iter()
        .find(|port| port.name == input.name)
        .and_then(|port| match &port.default_value {
            OptionalJson::Present(value) => Some(value.to_string()),
            OptionalJson::Missing => None,
        })
        .unwrap_or_default();
    let mut value_type = value_type;
    ui.columns(2, |columns| {
        widgets::field_label(&mut columns[0], "Type");
        columns[0].horizontal(|ui| {
            type_combo(ui, id.with("type"), &mut value_type);
        });
        field(
            &mut columns[1],
            id.with("default"),
            "Default",
            &default,
            "None",
        );
    });
    if let Some((_, contract)) = input.type_label.split_once(" · ") {
        ui.horizontal(|ui| {
            widgets::hint(ui, "Contract");
            widgets::pill(ui, contract, theme::LINK);
        });
    }
    ui.add_space(theme::SPACE_SM);
    ui.horizontal(|ui| {
        let mut required = input.required;
        ui.checkbox(&mut required, "Required")
            .on_hover_text("Callers must give a value");
        let mut exposed = input.exposed;
        ui.checkbox(&mut exposed, "Editable on the block")
            .on_hover_text("@exposed: callers set it on the calling block");
    });
    let mut action = input.action;
    ui.checkbox(&mut action, "Fed by the previous block (action)");
    widgets::field_label(ui, "Used by");
    let users = users(component, &input.name);
    if users.is_empty() {
        widgets::hint(ui, "No step reads this input yet.");
    }
    for user in users {
        ui.add(widgets::action(Icon::ArrowLeft, &user).quiet());
    }
    ui.add_space(theme::SPACE_LG);
    ui.add(
        widgets::action(Icon::Trash, "Delete input")
            .quiet()
            .danger(),
    );
}

/// The steps that read `name`, as *Block → input*.
fn users(component: &Component, name: &str) -> Vec<String> {
    let mut users = Vec::new();
    for node in &component.graph.nodes {
        for (port, binding) in &node.inputs {
            let mut reads = false;
            binding.for_each_leaf(&mut |leaf, _| {
                if matches!(leaf, apiforge_core::format::Binding::Parameter { name: read, .. } if read == name)
                {
                    reads = true;
                }
            });
            if reads {
                let field = if port == "value" {
                    node.display_name().to_owned()
                } else {
                    format!("{} → {port}", node.display_name())
                };
                if !users.contains(&field) {
                    users.push(field);
                }
            }
        }
    }
    users
}

/// Q4: the component's settings, with nothing selected on its board.
fn settings(ui: &mut Ui, board: &mock::Board, project: &Project) {
    heading(ui, "COMPONENT", &board.name);
    let id = Id::new("comp00-component");
    field(ui, id.with("name"), "Name", &board.name, "");
    widgets::hint(
        ui,
        format!("In code: {} · fixed once published", board.handle),
    );
    widgets::field_label(ui, "Description");
    let mut description = board.description.clone();
    widgets::named(
        ui.add(
            widgets::text_area(&mut description)
                .id(id.with("description"))
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        ),
        "Description",
    );
    widgets::field_label(ui, "Offered after a block that outputs (action)");
    let selected = board.action.as_ref().map_or_else(
        || "Nothing: Library only".to_owned(),
        |action| {
            board
                .inputs
                .iter()
                .find(|input| &input.name == action)
                .map_or(action.clone(), |input| {
                    format!("{} · {}", input.name, input.type_label)
                })
        },
    );
    egui::ComboBox::from_id_salt(id.with("action"))
        .width(ui.available_width().min(260.0))
        .selected_text(selected)
        .show_ui(ui, |_| {});
    let _ = project;
    ui.add_space(theme::SPACE_MD);
    ui.horizontal(|ui| {
        widgets::section_inline(ui, "Inputs");
        ui.label(
            RichText::new(board.inputs.len().to_string())
                .size(11.0)
                .color(theme::MUTED),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(widgets::action(Icon::Plus, "Add input").quiet());
        });
    });
    for input in &board.inputs {
        row(ui, &input.name, input.required, &input.detail());
    }
    ui.add_space(theme::SPACE_MD);
    ui.horizontal(|ui| {
        widgets::section_inline(ui, "Outputs");
        ui.label(
            RichText::new(board.outputs.len().to_string())
                .size(11.0)
                .color(theme::MUTED),
        );
    });
    for (name, type_label) in &board.outputs {
        row(ui, name, false, type_label);
    }
    widgets::hint(ui, "Outputs are the Export blocks on the board.");
}

fn row(ui: &mut Ui, name: &str, required: bool, detail: &str) {
    egui::Frame::new()
        .fill(theme::FIELD)
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(name)
                        .size(theme::FONT_BODY)
                        .family(theme::bold_family()),
                );
                if required {
                    ui.label(RichText::new("*").color(theme::DANGER));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(detail)
                            .size(theme::FONT_LABEL)
                            .color(theme::MUTED),
                    );
                });
            });
        });
    ui.add_space(4.0);
}
