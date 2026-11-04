//! LAY-00 mock-up (throwaway branch `mock/lay-00-cards`): the Inspector after
//! a click on a step's check aggregate. Static: nothing here edits.
use crate::{icons::Icon, mock_lay00 as mock, theme, widgets};
use apiforge_core::{
    format::{Binding, Graph, Node, OptionalJson},
    model::Predicate,
    run::{StepResult, StepStatus},
};
use eframe::egui::{self, RichText};

fn handle_of(graph: &Graph, id: &str) -> String {
    graph
        .nodes
        .iter()
        .find(|node| node.id == id)
        .map_or_else(|| id.to_owned(), |node| node.handle.clone())
}

fn describe(graph: &Graph, binding: &Binding) -> String {
    match binding {
        Binding::Reference {
            node_id,
            port,
            path,
        } => {
            let mut text = handle_of(graph, node_id);
            if port != "value" {
                text.push('.');
                text.push_str(port);
            }
            if !path.is_empty() {
                text.push('.');
                text.push_str(path);
            }
            text
        }
        Binding::Literal { value } => value.to_string(),
        Binding::Parameter { name, .. } => format!("input {name}"),
        Binding::Environment { name } => format!("env {name}"),
        _ => "…".to_owned(),
    }
}

fn symbol(predicate: Predicate) -> &'static str {
    match predicate {
        Predicate::Equals => "=",
        Predicate::NotEquals => "≠",
        Predicate::Greater => ">",
        Predicate::Less => "<",
        Predicate::Approximately => "≈",
        _ => "·",
    }
}

fn value(json: &OptionalJson) -> Option<String> {
    match json {
        OptionalJson::Present(value) => Some(value.to_string()),
        OptionalJson::Missing => None,
    }
}

fn status(step: Option<&StepResult>) -> (String, egui::Color32) {
    let Some(step) = step else {
        return ("Not run yet".to_owned(), theme::MUTED);
    };
    match step.status {
        StepStatus::Passed => ("✓ Passed".to_owned(), theme::SUCCESS),
        StepStatus::Failed | StepStatus::Error => {
            let actual =
                value(&step.actual).or_else(|| step.inputs.get("actual").map(|v| v.to_string()));
            let expected = value(&step.expected)
                .or_else(|| step.inputs.get("expected").map(|v| v.to_string()));
            match (actual, expected) {
                (Some(actual), Some(expected)) => {
                    (format!("✕ Failed: {actual} vs {expected}"), theme::DANGER)
                }
                _ => (
                    format!("✕ Failed: {}", step.message.clone().unwrap_or_default()),
                    theme::DANGER,
                ),
            }
        }
        other => (format!("– {}", other.label()), theme::WARNING),
    }
}

fn row(ui: &mut egui::Ui, graph: &Graph, node: &Node, step: Option<&StepResult>) {
    let predicate = node.config.predicate.unwrap_or_default();
    let operands: Vec<String> = node
        .inputs
        .iter()
        .filter(|(name, _)| !name.contains("tolerance"))
        .take(2)
        .map(|(_, binding)| describe(graph, binding))
        .collect();
    let expression = operands.join(&format!(" {} ", symbol(predicate)));
    let (status_text, status_color) = status(step);
    egui::Frame::new()
        .fill(theme::PANEL)
        .stroke(egui::Stroke::new(
            1.0,
            if status_color == theme::DANGER {
                theme::DANGER.gamma_multiply(0.5)
            } else {
                theme::BORDER
            },
        ))
        .corner_radius(8)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(&node.handle)
                        .family(theme::bold_family())
                        .size(theme::FONT_BODY)
                        .color(theme::TEXT),
                );
                ui.label(RichText::new("·").color(theme::MUTED));
                ui.label(
                    RichText::new(predicate_label(predicate))
                        .size(theme::FONT_LABEL)
                        .color(theme::MUTED),
                );
                ui.label(RichText::new("·").color(theme::MUTED));
                ui.label(
                    RichText::new(&expression)
                        .monospace()
                        .size(theme::FONT_LABEL)
                        .color(theme::LINK),
                );
            });
            ui.add_space(4.0);
            ui.label(
                RichText::new(status_text)
                    .family(theme::bold_family())
                    .size(theme::FONT_LABEL)
                    .color(status_color),
            );
            ui.add_space(2.0);
            let sources: Vec<String> = node
                .inputs
                .values()
                .flat_map(Binding::references)
                .map(|reference| {
                    describe(
                        graph,
                        &Binding::Reference {
                            node_id: reference.node_id.to_owned(),
                            port: reference.port.to_owned(),
                            path: reference.path.to_owned(),
                        },
                    )
                })
                .collect();
            ui.label(
                RichText::new(format!("Sources: {}", sources.join(", ")))
                    .size(theme::FONT_META)
                    .color(theme::MUTED),
            );
        });
}

fn predicate_label(predicate: Predicate) -> &'static str {
    match predicate {
        Predicate::Equals => "equals",
        Predicate::NotEquals => "does not equal",
        Predicate::Greater => "is greater than",
        Predicate::Less => "is less than",
        Predicate::Approximately => "is approximately",
        Predicate::Exists => "exists",
        Predicate::NotEmpty => "is not empty",
        Predicate::Contains => "contains",
        Predicate::Type => "has type",
        #[allow(unreachable_patterns)]
        _ => "checks",
    }
}

/// The checks of `subject`, one row each, and *Expand checks here*.
pub fn show(ui: &mut egui::Ui, graph: &Graph, subject: &str, steps: Option<&[StepResult]>) {
    let checks = mock::checks_of(graph).remove(subject).unwrap_or_default();
    let aggregate = mock::Aggregate::of(&checks, steps);
    ui.label(
        RichText::new("CHECKS OF")
            .size(theme::FONT_META)
            .color(theme::MUTED),
    );
    ui.label(
        RichText::new(handle_of(graph, subject))
            .family(theme::bold_family())
            .size(theme::FONT_PANEL_TITLE)
            .color(theme::TEXT),
    );
    ui.label(
        RichText::new(aggregate.text())
            .family(theme::bold_family())
            .size(theme::FONT_LABEL)
            .color(if aggregate.failed > 0 {
                theme::DANGER
            } else {
                theme::SUCCESS
            }),
    );
    ui.add_space(theme::SPACE_MD);
    for id in &checks {
        if let Some(node) = graph.nodes.iter().find(|node| &node.id == id) {
            let step = steps.and_then(|steps| steps.iter().find(|step| &step.node_id == id));
            row(ui, graph, node, step);
            ui.add_space(theme::SPACE_SM);
        }
    }
    ui.add_space(theme::SPACE_SM);
    let _ = ui
        .add(widgets::action(Icon::Check, "Expand checks here"))
        .on_hover_text("Show this step's checks as cards on the board");
}
