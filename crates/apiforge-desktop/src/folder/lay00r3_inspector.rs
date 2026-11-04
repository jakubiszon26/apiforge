//! LAY-00 round-3 mock-up (throwaway branch `mock/lay-00-round-3`): the
//! Inspector's *Checks* list after a click on a step's aggregate. Static.
use crate::canvas::lay00r3::{self as mock, Aggregate};
use crate::{icons::Icon, theme, widgets};
use apiforge_core::{
    format::{Binding, Graph, Node, OptionalJson},
    model::{NodeKind, Predicate},
    run::{StepResult, StepStatus},
};
use eframe::egui::{self, RichText};
use std::collections::HashMap;

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

fn value(json: &OptionalJson) -> Option<String> {
    match json {
        OptionalJson::Present(value) => Some(value.to_string()),
        OptionalJson::Missing => None,
    }
}

fn predicate_words(predicate: Predicate) -> &'static str {
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

/// The predicate in words: `checkout.body.total is approximately expected_total`.
fn sentence(graph: &Graph, node: &Node) -> String {
    if node.kind == NodeKind::Condition {
        return "all of its conditions hold (a gate: blocks what reads it)".to_owned();
    }
    let predicate = node.config.predicate.unwrap_or_default();
    let operands: Vec<String> = node
        .inputs
        .iter()
        .filter(|(name, _)| !name.contains("tolerance"))
        .take(2)
        .map(|(_, binding)| describe(graph, binding))
        .collect();
    match operands.as_slice() {
        [one] => format!("{one} {}", predicate_words(predicate)),
        [one, two] => format!("{one} {} {two}", predicate_words(predicate)),
        _ => predicate_words(predicate).to_owned(),
    }
}

fn status(step: Option<&StepResult>) -> (String, egui::Color32) {
    let Some(step) = step else {
        return ("Not run yet".to_owned(), theme::MUTED);
    };
    match step.status {
        StepStatus::Passed => ("✓ Passed".to_owned(), theme::SUCCESS),
        StepStatus::Failed | StepStatus::Error => {
            let actual = value(&step.actual);
            let expected = value(&step.expected);
            match (actual, expected) {
                (Some(actual), Some(expected)) => (
                    format!("✕ Failed: got {actual}, expected {expected}"),
                    theme::DANGER,
                ),
                _ => (
                    format!("✕ Failed: {}", step.message.clone().unwrap_or_default()),
                    theme::DANGER,
                ),
            }
        }
        other => (
            format!(
                "– {}{}",
                other.label(),
                step.message
                    .as_ref()
                    .map(|m| format!(" ({m})"))
                    .unwrap_or_default()
            ),
            theme::MUTED,
        ),
    }
}

fn row(
    ui: &mut egui::Ui,
    graph: &Graph,
    subject: &str,
    node: &Node,
    step: Option<&StepResult>,
    subjects_of: &HashMap<String, Vec<String>>,
    sequence: &HashMap<String, usize>,
    steps: Option<&[StepResult]>,
) {
    let number = sequence.get(&node.id).copied();
    let hovered = crate::mock_lay00::get().hover_check.as_deref() == Some(node.id.as_str());
    // When it runs: its result, or what it waits for (its latest subject).
    let all: Vec<String> = subjects_of.get(&node.id).cloned().unwrap_or_default();
    let latest = all.iter().max_by_key(|id| sequence.get(*id)).cloned();
    let finished = |id: &str| steps.is_some_and(|steps| steps.iter().any(|s| s.node_id == id));
    let mut when = match step {
        Some(step) => step.status.label().to_owned(),
        None => match &latest {
            Some(latest) if !finished(latest) => format!("Waiting for {}", handle_of(graph, latest)),
            _ => "Next".to_owned(),
        },
    };
    if all.len() > 1 {
        when.push_str(" · Shared");
    }
    if node.kind == NodeKind::Condition {
        when.push_str(" · Gate");
    }
    let (status_text, status_color) = status(step);
    let others: Vec<String> = subjects_of
        .get(&node.id)
        .into_iter()
        .flatten()
        .filter(|id| id.as_str() != subject)
        .map(|id| handle_of(graph, id))
        .collect();
    egui::Frame::new()
        .fill(theme::PANEL)
        .stroke(egui::Stroke::new(
            if hovered { 2.0 } else { 1.0 },
            if hovered {
                mock::RELATED
            } else if status_color == theme::DANGER {
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
                if let Some(number) = number {
                    ui.label(
                        RichText::new(format!(" {number} "))
                            .size(theme::FONT_META)
                            .color(theme::WHITE)
                            .background_color(if hovered {
                                mock::RELATED
                            } else {
                                egui::Color32::from_rgb(92, 92, 92)
                            }),
                    );
                }
                ui.label(
                    RichText::new(format!("{} — {when}", node.handle))
                        .family(theme::bold_family())
                        .size(theme::FONT_BODY)
                        .color(theme::TEXT),
                );
                if !others.is_empty() {
                    ui.label(
                        RichText::new(format!("Shared · also checks {}", others.join(", ")))
                            .size(theme::FONT_META)
                            .color(mock::RELATED)
                            .family(theme::bold_family()),
                    );
                }
            });
            ui.add_space(2.0);
            ui.label(
                RichText::new(sentence(graph, node))
                    .size(theme::FONT_LABEL)
                    .color(theme::LINK),
            );
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

/// The checks of `subject`, one row each, and *Expand checks here*.
pub fn show(ui: &mut egui::Ui, graph: &Graph, subject: &str, steps: Option<&[StepResult]>) {
    let checks = mock::checks_of(graph).remove(subject).unwrap_or_default();
    let subjects_of: HashMap<String, Vec<String>> = graph
        .nodes
        .iter()
        .filter(|node| matches!(node.kind, NodeKind::Assert | NodeKind::Condition))
        .map(|node| (node.id.clone(), mock::subjects(graph, node)))
        .collect();
    let sequence: HashMap<String, usize> = mock::sequence(graph)
        .into_iter()
        .enumerate()
        .map(|(i, id)| (id, i + 1))
        .collect();
    let aggregate = Aggregate::of(&checks, steps, &subjects_of, &mock::gates(graph));
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
        RichText::new(aggregate.text(true))
            .family(theme::bold_family())
            .size(theme::FONT_LABEL)
            .color(aggregate.color()),
    );
    ui.add_space(theme::SPACE_MD);
    for id in &checks {
        if let Some(node) = graph.nodes.iter().find(|node| &node.id == id) {
            let step = steps.and_then(|steps| steps.iter().find(|step| &step.node_id == id));
            row(
                ui,
                graph,
                subject,
                node,
                step,
                &subjects_of,
                &sequence,
                steps,
            );
            ui.add_space(theme::SPACE_SM);
        }
    }
    ui.add_space(theme::SPACE_SM);
    let _ = ui
        .add(widgets::action(Icon::Check, "Expand checks here"))
        .on_hover_text("Show this step's checks as cards on the board");
}
