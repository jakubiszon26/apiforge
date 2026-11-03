//! A component's settings in the Inspector (UX-33, COMP-00 Q4), opened by
//! *Component settings* in the board's header row: the name, the name in
//! code, the description, the action and the lists of the inputs and
//! outputs. Edits are live, as an input's are (UX-32): each is written to the
//! code as it is made, through the ordinary model edit. The lists lead to
//! UX-32's panels rather than editing inputs a second way.

use super::inputs::{draft, heading, keep_draft, side_panel, unnamed_field_label};
use super::*;
use apiforge_core::format::{Component, Contract};
use apiforge_core::graph::component_outputs;
use apiforge_core::handle::is_valid_handle;
use apiforge_core::model::ValueType;

const ACTION_LABEL: &str = "Offered after a block that outputs (action)";
const ACTION_HOVER: &str = "Dropped after a block's output, the component is connected through \
     this input. It changes nothing in a run.";
const NO_ACTION: &str = "No action";
const ROW_HEIGHT: f32 = 36.0;
/// The room a required mark takes after an input's name in a row.
const MARK_ROOM: f32 = 12.0;

/// What the settings asked for in one frame.
enum Change {
    Name(String),
    Handle(String),
    Description(String),
    /// The input that feeds the action, or none.
    Action(Option<String>),
    /// Open UX-32's *New input* form.
    AddInput,
    /// Open the panel of this input.
    Open(String),
}

impl FolderApp {
    /// The settings of the component in `document_id` beside its board.
    pub(super) fn component_settings_panel(
        &mut self,
        ui: &mut egui::Ui,
        document_id: &str,
        component: &Component,
        editable: bool,
    ) {
        let project = self.context_project();
        let (published, taken) = self.handles(component);
        let id = egui::Id::new(("component-settings", document_id));
        let (close, changes) = side_panel(ui, |ui| {
            let mut changes = ui
                .add_enabled_ui(editable, |ui| {
                    settings_form(ui, id, component, published, &taken, &project.contracts)
                })
                .inner;
            inputs_list(ui, component, &project.contracts, editable, &mut changes);
            outputs_list(ui, component, &project.contracts);
            changes
        });
        if close {
            self.component_settings.remove(document_id);
        }
        // The frame's changes, applied in turn as one edit.
        let mut next = component.clone();
        for change in changes {
            match change {
                // Going to an input is navigation, free on a read-only board.
                Change::Open(name) => self.open_input(document_id, name),
                _ if !editable => {}
                Change::AddInput => self.open_new_input(document_id),
                Change::Name(name) => next.name = name,
                Change::Handle(handle) => next.handle = handle,
                Change::Description(description) => next.description = description,
                Change::Action(input) => next = edit::set_action(&next, input.as_deref()),
            }
        }
        if next != *component {
            self.edit_component(document_id, next);
        }
    }

    /// Whether `component` has been published, and the names in code of the
    /// project's other components, working or published.
    fn handles(&self, component: &Component) -> (bool, Vec<String>) {
        let Some(session) = self.session.as_ref() else {
            return (false, Vec::new());
        };
        let revisions = &session.manifest().revisions;
        let published = revisions
            .iter()
            .any(|revision| revision.component_id == component.id);
        let mut taken: Vec<String> = revisions
            .iter()
            .filter(|revision| revision.component_id != component.id)
            .map(|revision| revision.handle.clone())
            .collect();
        taken.extend(
            session
                .documents()
                .iter()
                .filter_map(|document| match document.model() {
                    Some(Document::Component(other)) if other.id != component.id => {
                        Some(other.handle.clone())
                    }
                    _ => None,
                }),
        );
        (published, taken)
    }
}

/// Why `handle` cannot be the component's name in code, if it cannot.
fn handle_problem(component: &Component, taken: &[String], handle: &str) -> Option<String> {
    if !is_valid_handle(handle) {
        Some(format!(
            "Not a valid name. Use letters, digits and _, starting with a letter or _; not a \
             reserved word. The code keeps {}.",
            component.handle
        ))
    } else if handle != component.handle && taken.iter().any(|other| other == handle) {
        Some(format!(
            "Another component is already named {handle}. The code keeps {} until the name is \
             unique.",
            component.handle
        ))
    } else {
        None
    }
}

/// `product · object · Product`: an input as the action offers it.
fn action_text(
    input: &str,
    value_type: ValueType,
    contract: Option<&str>,
    contracts: &[Contract],
) -> String {
    let mut text = format!("{input} · {}", value_type.as_str());
    if let Some(id) = contract {
        let name = contracts
            .iter()
            .find(|contract| contract.id == id)
            .map_or(id, |contract| contract.name.as_str());
        text = format!("{text} · {name}");
    }
    text
}

/// Draws the name, the name in code, the description and the action;
/// returns what they asked for.
fn settings_form(
    ui: &mut egui::Ui,
    id: egui::Id,
    component: &Component,
    published: bool,
    taken: &[String],
    contracts: &[Contract],
) -> Vec<Change> {
    let mut changes = Vec::new();
    heading(ui, "COMPONENT", &component.name);

    // The name: written as typed unless it is empty.
    widgets::field_label(ui, "Name");
    let name_id = id.with("name");
    let mut name = draft(ui, name_id, &component.id).unwrap_or_else(|| component.name.clone());
    let response = widgets::validated(ui, name.trim().is_empty(), |ui| {
        widgets::named(
            ui.add(
                widgets::text_edit(&mut name)
                    .id(name_id.with("field"))
                    .desired_width(f32::INFINITY),
            ),
            "Component name",
        )
    });
    keep_draft(ui, name_id, &component.id, &name, response.has_focus());
    if name.trim().is_empty() {
        widgets::field_error(
            ui,
            format!(
                "A component needs a name. The code keeps {}.",
                component.name
            ),
        );
    } else if response.changed() && name != component.name {
        changes.push(Change::Name(name));
    }

    // The name in code: a scenario's call names it, so a publication fixes
    // it (ADR 0010).
    if published {
        widgets::hint(
            ui,
            format!("In code: {} · fixed once published", component.handle),
        );
    } else {
        unnamed_field_label(ui, "Name in code");
        let handle_id = id.with("handle");
        let mut handle =
            draft(ui, handle_id, &component.id).unwrap_or_else(|| component.handle.clone());
        let outlined = handle_problem(component, taken, &handle).is_some();
        let response = widgets::validated(ui, outlined, |ui| {
            widgets::named(
                ui.add(
                    widgets::text_edit(&mut handle)
                        .id(handle_id.with("field"))
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY),
                ),
                "Name in code",
            )
        });
        keep_draft(ui, handle_id, &component.id, &handle, response.has_focus());
        match handle_problem(component, taken, &handle) {
            Some(problem) => widgets::field_error(ui, problem),
            None => {
                widgets::hint(
                    ui,
                    "Scenarios call the component by this name. It is fixed once the \
                     component is published.",
                );
                if response.changed() && handle != component.handle {
                    changes.push(Change::Handle(handle));
                }
            }
        }
    }

    widgets::field_label(ui, "Description");
    let mut description = component.description.clone();
    if widgets::named(
        ui.add(
            widgets::text_area(&mut description)
                .id(id.with("description"))
                .hint_text("What the component does")
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        ),
        "Component description",
    )
    .changed()
    {
        changes.push(Change::Description(description));
    }

    // The action: the input a block's output feeds when the component is
    // dropped after it. The language has one `action` line.
    unnamed_field_label(ui, ACTION_LABEL);
    let current = component.action.as_ref().map(|action| action.input.clone());
    let mut chosen = current.clone();
    let selected = component.action.as_ref().map_or_else(
        || NO_ACTION.to_owned(),
        |action| {
            action_text(
                &action.input,
                action.value_type,
                action.contract_id.as_deref(),
                contracts,
            )
        },
    );
    let combo = egui::ComboBox::from_id_salt(id.with("action"))
        .selected_text(selected)
        .width(ui.available_width().min(280.0))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut chosen, None, NO_ACTION);
            for port in &component.inputs {
                ui.selectable_value(
                    &mut chosen,
                    Some(port.name.clone()),
                    action_text(
                        &port.name,
                        port.value_type,
                        port.contract_id.as_deref(),
                        contracts,
                    ),
                );
            }
        });
    widgets::named(combo.response, ACTION_LABEL).on_hover_text(ACTION_HOVER);
    if chosen != current {
        changes.push(Change::Action(chosen));
    }
    changes
}

/// A section's title and count, as *Inputs 2*.
fn list_heading(ui: &mut egui::Ui, title: &str, count: usize) {
    ui.label(
        RichText::new(title)
            .size(theme::FONT_BODY)
            .family(theme::bold_family()),
    );
    ui.label(
        RichText::new(count.to_string())
            .size(theme::FONT_LABEL)
            .color(theme::MUTED),
    );
}

/// One row of a list: the name, a required mark, and the detail at the right.
/// A row that opens something is a button named `accessible`.
fn list_row(
    ui: &mut egui::Ui,
    view: &canvas::InputView,
    accessible: &str,
    opens: bool,
) -> egui::Response {
    let sense = if opens {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_HEIGHT), sense);
    let kind = if opens {
        egui::WidgetType::Button
    } else {
        egui::WidgetType::Label
    };
    response.widget_info(|| egui::WidgetInfo::labeled(kind, ui.is_enabled(), accessible));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let fill = if opens && response.hovered() {
            theme::BORDER
        } else {
            theme::FIELD
        };
        painter.rect_filled(rect, 6, fill);
        if response.has_focus() {
            painter.rect_stroke(
                rect.shrink(0.5),
                6,
                egui::Stroke::new(1.5, theme::ACCENT),
                egui::StrokeKind::Inside,
            );
        }
        let inner = rect.shrink2(egui::vec2(theme::SPACE_MD, 0.0));
        let detail = painter.layout_no_wrap(
            view.detail(),
            egui::FontId::proportional(theme::FONT_LABEL),
            theme::MUTED,
        );
        // The detail keeps its place at the right; a long name gives way,
        // with an ellipsis, and its required mark stays after it.
        let detail_left = (inner.right() - detail.size().x).max(inner.center().x);
        let mark = if view.required { MARK_ROOM } else { 0.0 };
        let name_font = egui::FontId::new(theme::FONT_BODY, theme::bold_family());
        let full = painter
            .layout_no_wrap(view.name.clone(), name_font.clone(), theme::TEXT)
            .size()
            .x;
        let name_right = (inner.left() + full).min(detail_left - theme::SPACE_SM - mark);
        let name_room =
            egui::Rect::from_min_max(inner.left_top(), egui::pos2(name_right, inner.bottom()));
        widgets::paint_label(
            painter,
            name_room,
            &view.name,
            name_font,
            theme::TEXT,
            egui::Align::Min,
        );
        if view.required && name_room.width() >= 2.0 {
            painter.text(
                egui::pos2(name_room.right() + 4.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                "*",
                egui::FontId::proportional(theme::FONT_BODY),
                theme::DANGER,
            );
        }
        let detail_room =
            egui::Rect::from_min_max(egui::pos2(detail_left, inner.top()), inner.right_bottom());
        painter
            .with_clip_rect(detail_room.intersect(painter.clip_rect()))
            .galley(
                egui::pos2(detail_left, rect.center().y - detail.size().y / 2.0),
                detail,
                theme::MUTED,
            );
    }
    ui.add_space(2.0);
    response
}

/// *Inputs* with *+ Add input*, and a row per input that opens its panel.
fn inputs_list(
    ui: &mut egui::Ui,
    component: &Component,
    contracts: &[Contract],
    editable: bool,
    changes: &mut Vec<Change>,
) {
    ui.add_space(theme::SPACE_LG);
    ui.horizontal(|ui| {
        list_heading(ui, "Inputs", component.inputs.len());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(
                    editable,
                    widgets::action(Icon::Plus, "Add input")
                        .quiet()
                        .compact()
                        .named("Add input to the component"),
                )
                .on_hover_text("Declare another input in the Inspector")
                .clicked()
            {
                changes.push(Change::AddInput);
            }
        });
    });
    ui.add_space(4.0);
    if component.inputs.is_empty() {
        widgets::hint(ui, "No inputs yet. Steps read an input as param.name.");
    }
    for port in &component.inputs {
        let view = canvas::InputView::of(port, contracts);
        if list_row(ui, &view, &format!("Input {}", view.text()), true)
            .on_hover_text("Open this input in the Inspector")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            changes.push(Change::Open(port.name.clone()));
        }
    }
}

/// *Outputs*: one row per Export block, which is where an output is edited.
fn outputs_list(ui: &mut egui::Ui, component: &Component, contracts: &[Contract]) {
    let outputs = component_outputs(component);
    ui.add_space(theme::SPACE_LG);
    ui.horizontal(|ui| list_heading(ui, "Outputs", outputs.len()));
    ui.add_space(4.0);
    for port in &outputs {
        let view = canvas::InputView {
            required: false,
            ..canvas::InputView::of(port, contracts)
        };
        list_row(ui, &view, &format!("Output {}", view.text()), false);
    }
    widgets::hint(ui, "Outputs are the Export blocks on the board.");
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_core::demos::shop_project;

    fn cart() -> Component {
        shop_project()
            .components
            .into_iter()
            .find(|component| component.name == "Add to cart")
            .expect("the shop's Add to cart")
    }

    #[test]
    fn a_name_in_code_must_be_valid_and_free() {
        let cart = cart();
        let taken = vec!["product".to_owned()];
        assert_eq!(handle_problem(&cart, &taken, "add_item"), None);
        assert_eq!(handle_problem(&cart, &taken, &cart.handle), None);
        assert!(
            handle_problem(&cart, &taken, "1 bad")
                .unwrap()
                .starts_with("Not a valid name.")
        );
        assert!(
            handle_problem(&cart, &taken, "product")
                .unwrap()
                .starts_with("Another component is already named product.")
        );
    }

    #[test]
    fn an_action_reads_its_input_type_and_contract() {
        let contracts = vec![Contract {
            id: "c-1".into(),
            name: "Product".into(),
            ..shop_project().contracts[0].clone()
        }];
        assert_eq!(
            action_text("product", ValueType::Object, Some("c-1"), &contracts),
            "product · object · Product"
        );
        assert_eq!(
            action_text("item", ValueType::Object, Some("missing"), &contracts),
            "item · object · missing"
        );
        assert_eq!(
            action_text("quantity", ValueType::Number, None, &contracts),
            "quantity · number"
        );
    }
}
