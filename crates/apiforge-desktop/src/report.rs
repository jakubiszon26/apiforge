//! The bottom panel: live progress and step-by-step results of a scenario run.
//!
//! Draws the toggle bar, run summary, hierarchical step list (including component
//! inner steps and the cleanup phase) and the selected step's comparison, outputs,
//! inputs and HTTP request snapshot.

use crate::{dnd::Payload, labels, theme, widgets};
use apiforge_core::{
    format::{Binding, OptionalJson},
    js::{json_stringify, type_name},
    model::ErrorPolicy,
    path::JsonPath,
    run::{Phase, RunMode, RunResult, RunStatus, StepResult, StepStatus},
};
use eframe::egui::{self, Color32, Id, RichText, Sense, Stroke, Ui};
use serde_json::{Map, Value};
use std::collections::HashSet;

const RESERVED_KEYS: [&str; 3] = ["__proto__", "constructor", "prototype"];

/// Requests from the report panel to the application.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    SelectNode(String),
    AssertField {
        node_id: String,
        port: String,
        path: String,
        value: Value,
    },
    AssignField {
        node_id: String,
        port: String,
        path: String,
    },
    SetErrorPolicy(ErrorPolicy),
    ExportReport,
}

/// Everything the report panel needs to draw one frame.
pub struct Context<'a> {
    pub run: Option<&'a RunResult>,
    pub selected_node_id: Option<&'a str>,
    pub error_policy: ErrorPolicy,
    pub running: bool,
    pub stale: bool,
    pub files_busy: bool,
}

/// A step in the flattened step list; component children have `depth > 0` and keep
/// the top-level component node's identifier as `root_id`.
#[derive(Clone, Debug, PartialEq)]
pub struct StepRow<'a> {
    pub step: &'a StepResult,
    pub key: String,
    pub root_id: String,
    pub depth: usize,
}

/// Flattens top-level steps and any nested component steps in execution order.
pub fn flatten_steps(steps: &[StepResult]) -> Vec<StepRow<'_>> {
    let mut out = Vec::new();
    flatten_into(steps, None, "", 0, &mut out);
    out
}

fn flatten_into<'a>(
    steps: &'a [StepResult],
    root_id: Option<&str>,
    prefix: &str,
    depth: usize,
    out: &mut Vec<StepRow<'a>>,
) {
    for (index, step) in steps.iter().enumerate() {
        let key = format!("{prefix}{index}:{}", step.node_id);
        let root = root_id.unwrap_or(&step.node_id);
        out.push(StepRow {
            step,
            key: key.clone(),
            root_id: root.to_owned(),
            depth,
        });
        if let Some(children) = &step.children {
            let child_prefix = format!("{key}/");
            flatten_into(children, Some(root), &child_prefix, depth + 1, out);
        }
    }
}

/// Chooses the active step row following the reference priority:
/// explicit key -> selected board node -> first failed/errored step -> first step.
pub fn resolve_selected<'a, 'b>(
    rows: &'b [StepRow<'a>],
    selected_key: Option<&str>,
    selected_node_id: Option<&str>,
) -> Option<&'b StepRow<'a>> {
    rows.iter()
        .find(|row| Some(row.key.as_str()) == selected_key)
        .or_else(|| selected_node_id.and_then(|id| rows.iter().find(|row| row.step.node_id == id)))
        .or_else(|| {
            rows.iter()
                .find(|row| matches!(row.step.status, StepStatus::Error | StepStatus::Failed))
        })
        .or_else(|| rows.first())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DetailTab {
    #[default]
    Outputs,
    Inputs,
    Request,
}

pub struct ReportPanel {
    pub open: bool,
    height: f32,
    last_run_id: Option<String>,
    last_selected_node_id: Option<String>,
    selected_key: Option<String>,
    tab: DetailTab,
    expanded: HashSet<String>,
}

impl Default for ReportPanel {
    fn default() -> Self {
        Self {
            open: true,
            height: 260.0,
            last_run_id: None,
            last_selected_node_id: None,
            selected_key: None,
            tab: DetailTab::Outputs,
            expanded: HashSet::new(),
        }
    }
}

impl ReportPanel {
    pub fn show(&mut self, ui: &mut Ui, context: &Context<'_>) -> Vec<Action> {
        let mut actions = Vec::new();
        self.sync_selection(context.run, context.selected_node_id);

        egui::Panel::bottom("report")
            .resizable(self.open)
            .default_size(if self.open { self.height } else { 40.0 })
            .size_range(if self.open {
                160.0..=560.0
            } else {
                40.0..=40.0
            })
            .frame(
                egui::Frame::new()
                    .fill(theme::WHITE)
                    .inner_margin(egui::Margin::symmetric(24, 8)),
            )
            .show(ui, |ui| {
                self.toggle_bar(ui, context, &mut actions);
                if self.open {
                    self.height = (ui.available_height() + 40.0).clamp(160.0, 560.0);
                    ui.separator();
                    match context.run {
                        None => empty_state(ui),
                        Some(run) => self.run_contents(ui, run, context, &mut actions),
                    }
                }
            });
        actions
    }

    fn sync_selection(&mut self, run: Option<&RunResult>, selected_node_id: Option<&str>) {
        let run_id = run.map(|run| run.id.as_str());
        let run_changed = self.last_run_id.as_deref() != run_id;
        let node_changed = self.last_selected_node_id.as_deref() != selected_node_id;
        if run_changed {
            self.last_run_id = run_id.map(str::to_owned);
            self.selected_key = None;
            self.expanded.clear();
        }
        if node_changed {
            self.last_selected_node_id = selected_node_id.map(str::to_owned);
        }
        if (run_changed || node_changed)
            && let (Some(run), Some(node_id)) = (run, selected_node_id)
        {
            let rows = flatten_steps(&run.steps);
            if let Some(item) = rows
                .iter()
                .find(|row| row.depth == 0 && row.step.node_id == node_id)
            {
                self.selected_key = Some(item.key.clone());
            }
        }
    }

    fn toggle_bar(&mut self, ui: &mut Ui, context: &Context<'_>, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            let arrow = if self.open { "▾" } else { "▸" };
            let verb = if self.open { "Hide" } else { "Show" };
            if ui
                .add(egui::Button::new(format!("{arrow}  {verb} run")).frame(false))
                .clicked()
            {
                self.open = !self.open;
            }
            if let Some(run) = context.run {
                let (label, color) = run_pill(run.status);
                widgets::pill(ui, label, color);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut policy = context.error_policy;
                ui.add_enabled_ui(!context.running, |ui| {
                    egui::ComboBox::from_id_salt("report-error-policy")
                        .selected_text(error_policy_label(policy))
                        .width(190.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut policy,
                                ErrorPolicy::Continue,
                                error_policy_label(ErrorPolicy::Continue),
                            );
                            ui.selectable_value(
                                &mut policy,
                                ErrorPolicy::Stop,
                                error_policy_label(ErrorPolicy::Stop),
                            );
                        });
                });
                if policy != context.error_policy {
                    actions.push(Action::SetErrorPolicy(policy));
                }
                ui.add_space(8.0);
                if context.stale {
                    ui.label(
                        RichText::new(
                            "The graph changed after this run — the report shows an earlier version",
                        )
                        .size(11.0)
                        .color(theme::WARNING),
                    );
                } else {
                    widgets::hint(ui, "Results and data of every step");
                }
            });
        });
    }

    fn run_contents(
        &mut self,
        ui: &mut Ui,
        run: &RunResult,
        context: &Context<'_>,
        actions: &mut Vec<Action>,
    ) {
        header_row(ui, run, context.files_busy, actions);
        meta_row(ui, run);
        if let Some(notice) = &run.notice {
            egui::Frame::new()
                .fill(theme::ACCENT_SOFT)
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(notice).size(11.5).color(theme::TEXT));
                });
        }
        if let Some(error) = &run.error {
            widgets::error_box(ui, std::slice::from_ref(error));
        }
        ui.add_space(4.0);

        let rows = flatten_steps(&run.steps);
        let selected = resolve_selected(
            &rows,
            self.selected_key.as_deref(),
            context.selected_node_id,
        )
        .cloned();
        if self.tab == DetailTab::Request
            && selected
                .as_ref()
                .is_none_or(|row| row.step.request.is_none())
        {
            self.tab = DetailTab::Outputs;
        }

        let height = ui.available_height().max(80.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(280.0, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    self.step_list(ui, &rows, selected.as_ref(), actions);
                },
            );
            ui.separator();
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    self.step_details(ui, &run.id, selected.as_ref(), actions);
                },
            );
        });
    }

    fn step_list(
        &mut self,
        ui: &mut Ui,
        rows: &[StepRow<'_>],
        selected: Option<&StepRow<'_>>,
        actions: &mut Vec<Action>,
    ) {
        egui::ScrollArea::vertical()
            .id_salt("report-steps")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if rows.is_empty() {
                    widgets::hint(ui, "Waiting for the first step…");
                    return;
                }
                for (index, row) in rows.iter().enumerate() {
                    if row.step.phase == Phase::Cleanup
                        && row.depth == 0
                        && (index == 0 || rows[index - 1].step.phase != Phase::Cleanup)
                    {
                        ui.add_space(6.0);
                        widgets::caption(ui, "AFTER TEST · CLEANUP");
                        ui.add_space(2.0);
                    }
                    let is_active = selected.is_some_and(|active| active.key == row.key);
                    if step_row_button(ui, row, is_active).clicked() {
                        self.selected_key = Some(row.key.clone());
                        actions.push(Action::SelectNode(row.root_id.clone()));
                    }
                }
            });
    }

    fn step_details(
        &mut self,
        ui: &mut Ui,
        run_id: &str,
        selected: Option<&StepRow<'_>>,
        actions: &mut Vec<Action>,
    ) {
        let Some(selected) = selected else {
            widgets::hint(ui, "Results will appear while the scenario runs.");
            return;
        };
        let step = selected.step;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(&step.name)
                        .size(14.0)
                        .family(theme::bold_family()),
                );
                if selected.depth > 0 {
                    widgets::hint(ui, format!("Step inside component · {}", step.node_id));
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                widgets::pill(ui, step.status.label(), status_color(step.status));
            });
        });

        if let Some(message) = &step.message {
            let color = status_color(step.status);
            egui::Frame::new()
                .fill(color.gamma_multiply(0.08))
                .stroke(Stroke::new(1.0, color.gamma_multiply(0.25)))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(message).size(11.5).color(theme::TEXT));
                });
        }

        if !step.expected.is_missing() || !step.actual.is_missing() {
            comparison_box(ui, &step.expected, &step.actual);
        }

        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.tab,
                DetailTab::Outputs,
                format!("Outputs  {}", step.outputs.len()),
            );
            ui.selectable_value(
                &mut self.tab,
                DetailTab::Inputs,
                format!("Inputs  {}", step.inputs.len()),
            );
            if step.request.is_some() {
                ui.selectable_value(&mut self.tab, DetailTab::Request, "HTTP request");
            }
            if selected.depth == 0 && self.tab == DetailTab::Outputs {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    widgets::hint(ui, "Choose a field → add a check");
                });
            }
        });
        ui.separator();

        egui::ScrollArea::vertical()
            .id_salt(("report-detail", run_id, &selected.key, self.tab as u8))
            .auto_shrink([false, false])
            .show(ui, |ui| match self.tab {
                DetailTab::Request => {
                    if let Some(request) = &step.request {
                        ui.horizontal(|ui| {
                            widgets::pill(ui, &request.method, theme::ACCENT);
                            ui.label(RichText::new(&request.url).monospace().size(11.5));
                        });
                        ui.add_space(4.0);
                        let headers = Value::Object(
                            request
                                .headers
                                .iter()
                                .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                                .collect::<Map<String, Value>>(),
                        );
                        let branch = format!("{run_id}:{}:request:headers", selected.key);
                        self.value_tree(ui, &branch, "headers", &headers, None, actions);
                        if let OptionalJson::Present(body) = &request.body {
                            let branch = format!("{run_id}:{}:request:body", selected.key);
                            self.value_tree(ui, &branch, "body", body, None, actions);
                        }
                    }
                }
                DetailTab::Outputs | DetailTab::Inputs => {
                    let map = if self.tab == DetailTab::Inputs {
                        &step.inputs
                    } else {
                        &step.outputs
                    };
                    if map.is_empty() {
                        widgets::hint(
                            ui,
                            if self.tab == DetailTab::Inputs {
                                "No inputs for this step."
                            } else {
                                "No outputs for this step."
                            },
                        );
                        return;
                    }
                    let interactive = self.tab == DetailTab::Outputs && selected.depth == 0;
                    for (port, value) in map {
                        let branch = format!("{run_id}:{}:{}:{port}", selected.key, self.tab as u8);
                        let target = interactive.then_some((step.node_id.as_str(), port.as_str()));
                        self.value_tree(ui, &branch, port, value, target, actions);
                    }
                }
            });
    }

    #[allow(clippy::too_many_arguments)]
    fn value_tree(
        &mut self,
        ui: &mut Ui,
        branch_id: &str,
        label: &str,
        value: &Value,
        interactive: Option<(&str, &str)>,
        actions: &mut Vec<Action>,
    ) {
        self.value_branch(
            ui,
            branch_id,
            label,
            value,
            &JsonPath::root(),
            0,
            true,
            interactive,
            actions,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn value_branch(
        &mut self,
        ui: &mut Ui,
        branch_id: &str,
        label: &str,
        value: &Value,
        path: &JsonPath,
        depth: usize,
        valid_path: bool,
        interactive: Option<(&str, &str)>,
        actions: &mut Vec<Action>,
    ) {
        let path_text = path.to_string();
        let expandable = match value {
            Value::Array(items) => !items.is_empty(),
            Value::Object(fields) => !fields.is_empty(),
            _ => false,
        };
        let key = format!("{branch_id}:{path_text}");
        // Top-level ports start open; nested containers open on demand.
        let is_open = if depth == 0 {
            !self.expanded.contains(&format!("closed:{key}"))
        } else {
            self.expanded.contains(&key)
        };

        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * 14.0);
            let toggle = if expandable {
                if is_open { "▾" } else { "▸" }
            } else {
                "·"
            };
            if ui
                .add_enabled(
                    expandable,
                    egui::Button::new(RichText::new(toggle).size(11.0))
                        .frame(false)
                        .min_size(egui::vec2(16.0, 18.0)),
                )
                .clicked()
            {
                let marker = if depth == 0 {
                    format!("closed:{key}")
                } else {
                    key.clone()
                };
                if !self.expanded.remove(&marker) {
                    self.expanded.insert(marker);
                }
            }

            let can_drag = interactive.is_some() && valid_path;
            let key_color = if can_drag { theme::LINK } else { theme::TEXT };
            let sense = if can_drag {
                Sense::click_and_drag()
            } else {
                Sense::hover()
            };
            let key_response = ui.add(
                egui::Label::new(RichText::new(label).monospace().color(key_color))
                    .sense(sense)
                    .truncate(),
            );
            if let Some((node_id, port)) = interactive
                && valid_path
            {
                let drag_label = if path_text.is_empty() {
                    port.to_owned()
                } else {
                    path_text.clone()
                };
                key_response.dnd_set_drag_payload(Payload::Variable {
                    label: labels::truncate(&drag_label, 200),
                    binding: Binding::reference(node_id, port, path_text.clone()),
                });
            }
            let hover = if path_text.is_empty() {
                format!("(whole value) · {}", type_name(Some(value)))
            } else {
                format!("{path_text} · {}", type_name(Some(value)))
            };
            key_response.on_hover_text(hover);

            ui.label(RichText::new(":").color(theme::MUTED));
            let summary = describe_value(value);
            ui.add(
                egui::Label::new(
                    RichText::new(labels::truncate(&summary, 72))
                        .monospace()
                        .size(11.0)
                        .color(theme::MUTED),
                )
                .truncate(),
            )
            .on_hover_text(summary);

            if let Some((node_id, port)) = interactive {
                if valid_path {
                    let target_name = if path_text.is_empty() {
                        label
                    } else {
                        path_text.as_str()
                    };
                    if ui
                        .small_button("+ Check")
                        .on_hover_text(format!("Check {target_name}"))
                        .clicked()
                    {
                        actions.push(Action::AssertField {
                            node_id: node_id.to_owned(),
                            port: port.to_owned(),
                            path: path_text.clone(),
                            value: value.clone(),
                        });
                    }
                    if ui
                        .small_button("To variable")
                        .on_hover_text(format!("Assign {target_name} to a variable"))
                        .clicked()
                    {
                        actions.push(Action::AssignField {
                            node_id: node_id.to_owned(),
                            port: port.to_owned(),
                            path: path_text.clone(),
                        });
                    }
                } else {
                    ui.label(RichText::new("custom key").size(10.5).color(theme::MUTED))
                        .on_hover_text("Reserved field name. Select the parent object instead.");
                }
            }
        });

        if !expandable || !is_open || depth >= 10 {
            return;
        }
        match value {
            Value::Array(items) => {
                for (index, child) in items.iter().take(100).enumerate() {
                    let mut child_path = path.clone();
                    child_path.push_index(index);
                    let child_label = index.to_string();
                    self.value_branch(
                        ui,
                        branch_id,
                        &child_label,
                        child,
                        &child_path,
                        depth + 1,
                        valid_path,
                        interactive,
                        actions,
                    );
                }
            }
            Value::Object(fields) => {
                for (field, child) in fields.iter().take(100) {
                    let mut child_path = path.clone();
                    child_path.push_key(field.clone());
                    let child_valid = valid_path && is_supported_key(field);
                    self.value_branch(
                        ui,
                        branch_id,
                        field,
                        child,
                        &child_path,
                        depth + 1,
                        child_valid,
                        interactive,
                        actions,
                    );
                }
            }
            _ => {}
        }
    }
}

fn is_supported_key(key: &str) -> bool {
    !RESERVED_KEYS.contains(&key)
}

fn describe_value(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Array(items) => format!("List ({})", items.len()),
        Value::Object(fields) => format!("Object ({})", fields.len()),
        scalar => json_stringify(scalar),
    }
}

fn empty_state(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(16.0);
        ui.label(
            RichText::new("Your scenario is ready for its first run")
                .size(13.5)
                .family(theme::bold_family()),
        );
        ui.add_space(2.0);
        ui.label(
            RichText::new(
                "Run a simulation or the live API. Each step's data and comparison results will appear here.",
            )
            .size(11.5)
            .color(theme::MUTED),
        );
    });
}

fn header_row(ui: &mut Ui, run: &RunResult, files_busy: bool, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Test run")
                .size(13.0)
                .family(theme::bold_family()),
        );
        match run.mode {
            RunMode::Simulation => {
                widgets::pill(ui, "SIMULATION", theme::LINK);
            }
            RunMode::Live => {
                widgets::pill(ui, "LIVE API", theme::ACCENT);
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(!files_busy, egui::Button::new("Export").small())
                .on_hover_text("Export JSON report with masked secrets")
                .clicked()
            {
                actions.push(Action::ExportReport);
            }
            let passed = run
                .steps
                .iter()
                .filter(|step| step.status == StepStatus::Passed)
                .count();
            let failed = run
                .steps
                .iter()
                .filter(|step| matches!(step.status, StepStatus::Failed | StepStatus::Error))
                .count();
            if failed > 0 {
                ui.label(
                    RichText::new(format!("{failed} failed"))
                        .size(11.5)
                        .strong()
                        .color(theme::DANGER),
                );
            }
            ui.label(
                RichText::new(format!("{passed} passed"))
                    .size(11.5)
                    .strong()
                    .color(theme::SUCCESS),
            );
        });
    });
}

fn meta_row(ui: &mut Ui, run: &RunResult) {
    ui.horizontal_wrapped(|ui| {
        let time = short_time(&run.started_at);
        let timing = match total_duration_ms(run) {
            Some(ms) => format!("{time} · {ms} ms"),
            None => time.to_owned(),
        };
        ui.label(RichText::new(timing).size(11.0).color(theme::MUTED));
        ui.label(RichText::new("·").size(11.0).color(theme::MUTED));
        let summary = match run.status {
            RunStatus::Running => "Running…",
            RunStatus::Passed => "Finished — all checks passed",
            RunStatus::Cancelled => "Cancelled",
            RunStatus::Failed => "Finished with errors",
        };
        ui.label(RichText::new(summary).size(11.0).color(theme::MUTED));
        if run.diagnostic {
            ui.label(RichText::new("·").size(11.0).color(theme::MUTED));
            ui.label(
                RichText::new(
                    "Diagnostic run · may use saved inputs · does not pass the full test",
                )
                .size(11.0)
                .strong()
                .color(theme::WARNING),
            );
        }
        if run.mode == RunMode::Simulation {
            ui.label(RichText::new("·").size(11.0).color(theme::MUTED));
            ui.label(
                RichText::new("Sample responses — no requests sent")
                    .size(11.0)
                    .color(theme::MUTED),
            );
        }
    });
}

fn comparison_box(ui: &mut Ui, expected: &OptionalJson, actual: &OptionalJson) {
    egui::Frame::new()
        .fill(theme::FIELD)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let col_width = ((ui.available_width() - 28.0) * 0.5).max(80.0);
                ui.allocate_ui(egui::vec2(col_width, 36.0), |ui| {
                    ui.vertical(|ui| {
                        widgets::subcaption(ui, "EXPECTED");
                        let text = match expected {
                            OptionalJson::Present(value) => compact_json(value),
                            OptionalJson::Missing => "—".into(),
                        };
                        ui.add(
                            egui::Label::new(
                                RichText::new(labels::truncate(&text, 120)).monospace(),
                            )
                            .truncate(),
                        )
                        .on_hover_text(text);
                    });
                });
                ui.label(RichText::new("→").color(theme::MUTED));
                ui.allocate_ui(egui::vec2(col_width, 36.0), |ui| {
                    ui.vertical(|ui| {
                        widgets::subcaption(ui, "ACTUAL");
                        let text = match actual {
                            OptionalJson::Present(value) => compact_json(value),
                            OptionalJson::Missing => "(missing value)".into(),
                        };
                        ui.add(
                            egui::Label::new(
                                RichText::new(labels::truncate(&text, 120)).monospace(),
                            )
                            .truncate(),
                        )
                        .on_hover_text(text);
                    });
                });
            });
        });
}

fn compact_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| json_stringify(value))
}

fn step_row_button(ui: &mut Ui, row: &StepRow<'_>, active: bool) -> egui::Response {
    let indent = row.depth as f32 * 14.0;
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 36.0), Sense::click());
    let painter = ui.painter();
    if active {
        painter.rect_filled(rect, 6.0, theme::ACCENT_SOFT);
    } else if response.hovered() {
        painter.rect_filled(rect, 6.0, theme::BG);
    }

    let status_color = status_color(row.step.status);
    let badge_center = egui::pos2(rect.left() + 14.0 + indent, rect.center().y);
    painter.circle_filled(badge_center, 8.0, status_color.gamma_multiply(0.15));
    painter.text(
        badge_center,
        egui::Align2::CENTER_CENTER,
        status_mark(row.step.status),
        egui::FontId::proportional(10.5),
        status_color,
    );

    let text_left = rect.left() + 28.0 + indent;
    let name_color = if active { theme::ACCENT } else { theme::TEXT };
    painter.text(
        egui::pos2(text_left, rect.top() + 11.0),
        egui::Align2::LEFT_CENTER,
        widgets::ellipsis(&row.step.name, 26),
        egui::FontId::new(12.0, theme::bold_family()),
        name_color,
    );
    let sub = if row.depth > 0 {
        format!("Inside component · {}", row.step.status.label())
    } else {
        row.step.status.label().to_owned()
    };
    painter.text(
        egui::pos2(text_left, rect.top() + 25.0),
        egui::Align2::LEFT_CENTER,
        sub,
        egui::FontId::proportional(10.0),
        theme::MUTED,
    );
    painter.text(
        egui::pos2(rect.right() - 8.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        format!("{} ms", row.step.duration_ms),
        egui::FontId::monospace(10.5),
        theme::MUTED,
    );
    let _ = Id::new(&row.key);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn run_pill(status: RunStatus) -> (&'static str, Color32) {
    match status {
        RunStatus::Passed => ("PASSED", theme::SUCCESS),
        RunStatus::Running => ("RUNNING", theme::ACCENT),
        RunStatus::Cancelled => ("CANCELLED", theme::WARNING),
        RunStatus::Failed => ("FAILED", theme::DANGER),
    }
}

fn status_color(status: StepStatus) -> Color32 {
    match status {
        StepStatus::Passed => theme::SUCCESS,
        StepStatus::Failed | StepStatus::Error => theme::DANGER,
        StepStatus::Cancelled => theme::WARNING,
        StepStatus::Blocked | StepStatus::Skipped => theme::MUTED,
    }
}

fn status_mark(status: StepStatus) -> &'static str {
    match status {
        StepStatus::Passed => "✓",
        StepStatus::Failed | StepStatus::Error => "✕",
        StepStatus::Blocked | StepStatus::Skipped | StepStatus::Cancelled => "−",
    }
}

fn error_policy_label(policy: ErrorPolicy) -> &'static str {
    match policy {
        ErrorPolicy::Continue => "Continue remaining steps",
        ErrorPolicy::Stop => "Stop on error",
    }
}

/// Extracts `HH:MM:SS` from an RFC 3339 timestamp when present.
fn short_time(rfc3339: &str) -> &str {
    rfc3339
        .split_once('T')
        .and_then(|(_, rest)| rest.get(..8))
        .unwrap_or(rfc3339)
}

/// Computes the elapsed milliseconds when the run has finished.
fn total_duration_ms(run: &RunResult) -> Option<u64> {
    let finished = run.finished_at.as_deref()?;
    if let (Some(start), Some(end)) = (day_millis(&run.started_at), day_millis(finished))
        && end >= start
    {
        return Some(end - start);
    }
    Some(run.steps.iter().map(|step| step.duration_ms).sum())
}

fn day_millis(rfc3339: &str) -> Option<u64> {
    let (_, time) = rfc3339.split_once('T')?;
    let hours: u64 = time.get(0..2)?.parse().ok()?;
    let minutes: u64 = time.get(3..5)?.parse().ok()?;
    let seconds: u64 = time.get(6..8)?.parse().ok()?;
    let millis: u64 = if time.as_bytes().get(8) == Some(&b'.') {
        let digits: String = time[9..]
            .chars()
            .take_while(char::is_ascii_digit)
            .take(3)
            .collect();
        match digits.len() {
            1 => digits.parse::<u64>().ok()? * 100,
            2 => digits.parse::<u64>().ok()? * 10,
            3 => digits.parse::<u64>().ok()?,
            _ => 0,
        }
    } else {
        0
    };
    Some(((hours * 60 + minutes) * 60 + seconds) * 1000 + millis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_core::{model::NodeKind, run::RequestSnapshot};
    use indexmap::IndexMap;
    use serde_json::json;

    fn sample_run() -> RunResult {
        let mut child =
            StepResult::new("inner-req", "Inner request", NodeKind::Request, Phase::Main);
        child.outputs.insert("status".into(), json!(200));

        let mut comp =
            StepResult::new("comp-1", "Auth component", NodeKind::Component, Phase::Main);
        comp.duration_ms = 12;
        comp.outputs.insert(
            "token".into(),
            json!({"value": "abc", "__proto__": "ignored"}),
        );
        comp.children = Some(vec![child]);

        let mut check = StepResult::new("assert-1", "Check status", NodeKind::Assert, Phase::Main);
        check.status = StepStatus::Failed;
        check.message = Some("Expected 200, got 500".into());
        check.expected = OptionalJson::Present(json!(200));
        check.actual = OptionalJson::Present(json!(500));
        check.request = Some(RequestSnapshot {
            method: "POST".into(),
            url: "http://localhost:3333/orders".into(),
            headers: IndexMap::from([("content-type".into(), "application/json".into())]),
            body: OptionalJson::Present(json!({"sku": "A1"})),
        });

        let cleanup = StepResult::new("clean-1", "Delete order", NodeKind::Request, Phase::Cleanup);

        RunResult {
            id: "run-1".into(),
            scenario_id: "scen-1".into(),
            mode: RunMode::Simulation,
            started_at: "2026-04-19T12:00:00.100Z".into(),
            finished_at: Some("2026-04-19T12:00:00.245Z".into()),
            status: RunStatus::Failed,
            steps: vec![comp, check, cleanup],
            diagnostic: true,
            snapshot: None,
            error: Some("Step failed".into()),
            notice: Some("Using saved inputs from the previous run.".into()),
        }
    }

    #[test]
    fn flatten_steps_preserves_component_children_and_root_id() {
        let run = sample_run();
        let rows = flatten_steps(&run.steps);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].step.node_id, "comp-1");
        assert_eq!(rows[0].root_id, "comp-1");
        assert_eq!(rows[0].depth, 0);
        assert_eq!(rows[1].step.node_id, "inner-req");
        assert_eq!(rows[1].root_id, "comp-1");
        assert_eq!(rows[1].depth, 1);
        assert_eq!(rows[2].step.node_id, "assert-1");
        assert_eq!(rows[3].step.phase, Phase::Cleanup);
    }

    #[test]
    fn resolve_selected_prefers_explicit_then_board_then_failed() {
        let run = sample_run();
        let rows = flatten_steps(&run.steps);
        let default = resolve_selected(&rows, None, None).unwrap();
        assert_eq!(default.step.node_id, "assert-1");

        let by_node = resolve_selected(&rows, None, Some("clean-1")).unwrap();
        assert_eq!(by_node.step.node_id, "clean-1");

        let by_key = resolve_selected(&rows, Some(&rows[1].key), Some("clean-1")).unwrap();
        assert_eq!(by_key.step.node_id, "inner-req");
    }

    #[test]
    fn duration_and_short_time_parse_rfc3339_timestamps() {
        let run = sample_run();
        assert_eq!(short_time(&run.started_at), "12:00:00");
        assert_eq!(total_duration_ms(&run), Some(145));
    }

    #[test]
    fn report_panel_renders_empty_and_populated_states_headlessly() {
        let mut panel = ReportPanel::default();
        let run = sample_run();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        for _ in 0..2 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                let empty = Context {
                    run: None,
                    selected_node_id: None,
                    error_policy: ErrorPolicy::Continue,
                    running: false,
                    stale: false,
                    files_busy: false,
                };
                assert!(panel.show(ui, &empty).is_empty());
            });
            out.textures_delta.clear();
        }
        for _ in 0..2 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                let populated = Context {
                    run: Some(&run),
                    selected_node_id: Some("comp-1"),
                    error_policy: ErrorPolicy::Stop,
                    running: false,
                    stale: true,
                    files_busy: false,
                };
                let _ = panel.show(ui, &populated);
            });
            out.textures_delta.clear();
        }
    }
}
