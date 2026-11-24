//! The *Checks* tab of a step's Inspector (LAY-08): the checks that read the
//! step, in words, with their sources, status and moment. A request and a call
//! of a component both have it, because both are subjects of a check (LAY-20).
//!
//! A check with several subjects is listed under each (Q6); a gate is listed
//! under its subject and counted apart; a check with no subject is listed
//! nowhere. A check of the scenario's Cleanup graph is listed under the Main
//! step it reads, with the moment word *Cleanup* and counted apart, so that
//! aggregation hides no failure (LAY-20).
//!
//! The tab's first line, *Add check*, opens the step's outputs as *Output
//! data* writes them, and a field there opens DIAG-08's menu of four checks
//! (DIAG-15); a check's card selects the check.

use super::Context;
use crate::icons::Icon;
use crate::report::Requirement;
use crate::{theme, widgets};
use apiforge_core::{
    edit::FieldCheck,
    format::{Binding, Graph, Node, OptionalJson, Port, Project},
    graph::{check_subjects_in, output_ports, sequence},
    model::NodeKind,
    path::JsonPath,
    run::{RunResult, RunStatus, StepResult, StepStatus},
    template_text::Scope,
};
use eframe::egui::{self, Color32, RichText, Ui};
use indexmap::IndexSet;
use serde_json::Value;
use std::collections::HashMap;

/// How a line of the list reads at a glance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tone {
    Quiet,
    Passed,
    Failed,
}

impl Tone {
    fn color(self) -> Color32 {
        match self {
            Self::Quiet => theme::MUTED,
            Self::Passed => theme::SUCCESS,
            Self::Failed => theme::DANGER,
        }
    }
}

/// Where a check stands in a run, for the line above the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Passed,
    Failed,
    NotRun,
    Waiting,
}

/// One check as the list says it under one subject.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Row {
    /// The check's step, which its card selects.
    id: String,
    handle: String,
    /// The name and the moment: `receipt_total — Waiting for receipt · Shared`.
    title: String,
    predicate: String,
    status: String,
    tone: Tone,
    outcome: Outcome,
    sources: Vec<String>,
    shared_with: Vec<String>,
    gate: bool,
    /// Listed under a Main step but run in Cleanup: counted apart, and its
    /// card stays on the Cleanup board (LAY-20).
    cleanup: bool,
}

/// Names a step by its handle, wherever the check found it: a Cleanup check
/// may read a step of the main graph. `nodes` are every node a check may
/// read, the graph its predicate is written against.
struct Names<'a> {
    graphs: Vec<&'a Graph>,
    nodes: &'a Graph,
    project: &'a Project,
}

impl Names<'_> {
    fn step(&self, id: &str) -> String {
        self.graphs
            .iter()
            .flat_map(|graph| &graph.nodes)
            .find(|node| node.id == id)
            .map_or_else(|| id.to_owned(), |node| node.handle.clone())
    }

    fn reference(&self, id: &str, port: &str, path: &str) -> String {
        let mut text = self.step(id);
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

    /// What the check holds, as its card writes it (DIAG-02): the signs of
    /// the code, the step's name and leading `body.` left out under the one
    /// step it reads, which the Inspector is of and the row's sources name
    /// in full. A long value written in the check is cut at 80 characters,
    /// as the list has no hover.
    fn sentence(&self, node: &Node) -> String {
        let scope = Scope {
            nodes: &self.nodes.nodes,
            project: self.project,
            request: Some(node),
        };
        let own = crate::canvas::own_step(node, self.nodes);
        crate::canvas::check_text(node, scope, self.nodes, own).capped(80)
    }

    /// Everything the check reads, once each, in the order it reads it.
    fn sources(&self, node: &Node) -> Vec<String> {
        let sources: IndexSet<String> = node
            .inputs
            .values()
            .flat_map(Binding::references)
            .map(|source| self.reference(source.node_id, source.port, source.path))
            .collect();
        sources.into_iter().collect()
    }
}

fn json(value: &OptionalJson) -> Option<String> {
    match value {
        OptionalJson::Present(value) => Some(value.to_string()),
        OptionalJson::Missing => None,
    }
}

/// The result of a check in words. A failed comparison names what `check`
/// required, as its card in Run results does (DIAG-03).
fn status(step: Option<&StepResult>, check: &Node) -> (String, Tone, Outcome) {
    let Some(step) = step else {
        return ("Not run yet".to_owned(), Tone::Quiet, Outcome::Waiting);
    };
    match step.status {
        StepStatus::Passed => ("✓ Passed".to_owned(), Tone::Passed, Outcome::Passed),
        StepStatus::Failed | StepStatus::Error => {
            let label = step.status.label();
            let text = match (json(&step.actual), json(&step.expected)) {
                (Some(actual), Some(expected)) => {
                    let expected = Requirement::of(check).map_or(expected, |requirement| {
                        requirement.text_with(&step.expected, Value::to_string)
                    });
                    format!("✕ {label}: got {actual}, expected {expected}")
                }
                _ => match &step.message {
                    Some(message) => format!("✕ {label}: {message}"),
                    None => format!("✕ {label}"),
                },
            };
            (text, Tone::Failed, Outcome::Failed)
        }
        other => {
            let text = match &step.message {
                Some(message) => format!("– {} ({message})", other.label()),
                None => format!("– {}", other.label()),
            };
            (text, Tone::Quiet, Outcome::NotRun)
        }
    }
}

/// When the check runs, as far as `run` says: its result once it has one, else
/// what it still waits for, which is its latest subject.
fn moment(check: &Node, latest: &str, names: &Names<'_>, run: Option<&RunResult>) -> String {
    if let Some(step) = run.and_then(|run| run.step(&check.id)) {
        return step.status.label().to_owned();
    }
    match run {
        Some(run) if run.status != RunStatus::Running => "Not run".to_owned(),
        Some(run) if run.step(latest).is_some() => "Next".to_owned(),
        _ => format!("Waiting for {}", names.step(latest)),
    }
}

/// A check listed under a Main step but run in Cleanup names its phase until it
/// has a result of its own; then the result takes that place (LAY-20).
fn cleanup_moment(check: &Node, run: Option<&RunResult>) -> String {
    match run.and_then(|run| run.step(&check.id)) {
        Some(step) => step.status.label().to_owned(),
        None => "Cleanup".to_owned(),
    }
}

/// The checks of one graph that read the step `subject`, in the order the engine
/// reaches them. `lookup` is where a check's sources are resolved: the Cleanup
/// pass resolves them in both graphs, because its subjects are Main steps.
/// `position` is the board's order, which says which subject comes last.
fn pass(
    checks: &Graph,
    lookup: &[&Graph],
    names: &Names<'_>,
    position: &HashMap<&str, usize>,
    subject: &str,
    run: Option<&RunResult>,
    cleanup: bool,
) -> Vec<Row> {
    sequence(checks)
        .iter()
        .filter_map(|id| checks.nodes.iter().find(|node| node.id == *id))
        .filter_map(|check| {
            let subjects = check_subjects_in(lookup, check).subjects;
            let own = subjects
                .iter()
                .find(|candidate| candidate.step == subject)?;
            let latest = subjects
                .iter()
                .map(|candidate| candidate.step)
                .max_by_key(|step| position.get(step))?;
            let (status, tone, outcome) = status(run.and_then(|run| run.step(&check.id)), check);
            let moment = if cleanup {
                cleanup_moment(check, run)
            } else {
                moment(check, latest, names, run)
            };
            let mut title = format!("{} — {moment}", check.handle);
            // Under its latest subject the line *Shared with* says it alone.
            if own.shared && latest != subject {
                title.push_str(" · Shared");
            }
            if own.gate {
                title.push_str(" · Gate");
            }
            Some(Row {
                id: check.id.clone(),
                handle: check.handle.clone(),
                title,
                predicate: names.sentence(check),
                status,
                tone,
                outcome,
                sources: names.sources(check),
                shared_with: subjects
                    .iter()
                    .filter(|candidate| candidate.step != subject)
                    .map(|candidate| names.step(candidate.step))
                    .collect(),
                gate: own.gate,
                cleanup,
            })
        })
        .collect()
}

fn is_check(node: &Node) -> bool {
    matches!(node.kind, NodeKind::Assert | NodeKind::Condition)
}

/// The checks of the step `subject`: those of its own board, then those the
/// scenario's Cleanup graph holds for it, which `cleanup` names while the board
/// shows Main (LAY-20).
fn rows(
    board: &Graph,
    graph: &Graph,
    cleanup: Option<&Graph>,
    project: &Project,
    subject: &str,
    run: Option<&RunResult>,
) -> Vec<Row> {
    let mut graphs = vec![board, graph];
    graphs.extend(cleanup);
    // A Cleanup check listed here is written against Main's steps and its
    // own graph's (LAY-20).
    let merged;
    let nodes = match cleanup.filter(|graph| graph.nodes.iter().any(is_check)) {
        Some(cleanup) => {
            let mut nodes = graph.nodes.clone();
            nodes.extend(
                cleanup
                    .nodes
                    .iter()
                    .filter(|node| !graph.nodes.iter().any(|known| known.id == node.id))
                    .cloned(),
            );
            merged = Graph {
                nodes,
                edges: Vec::new(),
                proxies: Vec::new(),
            };
            &merged
        }
        None => graph,
    };
    let names = Names {
        graphs,
        nodes,
        project,
    };
    let position: HashMap<&str, usize> = sequence(board)
        .iter()
        .enumerate()
        .map(|(index, id)| (*id, index))
        .collect();
    let mut rows = pass(board, &[board], &names, &position, subject, run, false);
    // A Cleanup graph without a check needs no pass of its own.
    if let Some(cleanup) = cleanup.filter(|graph| graph.nodes.iter().any(is_check)) {
        rows.extend(pass(
            cleanup,
            &[board, cleanup],
            &names,
            &position,
            subject,
            run,
            true,
        ));
    }
    rows
}

fn plural(count: usize, one: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{count} {one}s")
    }
}

/// The line above the list. Gates and the checks of Cleanup are counted apart
/// from the checks; a run turns the count of checks into what became of them.
fn summary(rows: &[Row], run: bool) -> (String, Tone) {
    let checks: Vec<&Row> = rows
        .iter()
        .filter(|row| !row.gate && !row.cleanup)
        .collect();
    let count = |outcome: Outcome| checks.iter().filter(|row| row.outcome == outcome).count();
    let (failed, passed) = (count(Outcome::Failed), count(Outcome::Passed));
    let (not_run, waiting) = (count(Outcome::NotRun), count(Outcome::Waiting));
    let mut parts = Vec::new();
    if run && failed > 0 {
        parts.push(format!("✕ {failed}/{} failed", checks.len()));
    } else if run && !checks.is_empty() {
        parts.push(format!("{passed}/{} ✓", checks.len()));
    } else if !checks.is_empty() {
        parts.push(plural(checks.len(), "check"));
    }
    if run && not_run > 0 {
        parts.push(format!("{not_run} skipped"));
    }
    if run && waiting > 0 {
        parts.push(format!("{waiting} waiting"));
    }
    let shared = checks
        .iter()
        .filter(|row| !row.shared_with.is_empty())
        .count();
    if shared > 0 {
        parts.push(format!("{shared} shared"));
    }
    let in_cleanup = rows.iter().filter(|row| row.cleanup).count();
    if in_cleanup > 0 {
        parts.push(format!("{in_cleanup} in Cleanup"));
    }
    let gates = rows.iter().filter(|row| row.gate && !row.cleanup).count();
    if gates > 0 {
        parts.push(plural(gates, "gate"));
    }
    let tone = if rows.iter().any(|row| row.tone == Tone::Failed) {
        Tone::Failed
    } else if run && failed == 0 && waiting == 0 && not_run == 0 {
        Tone::Passed
    } else {
        Tone::Quiet
    };
    (parts.join(" · "), tone)
}

/// A check's card. A click, or Return while it has the focus, selects the
/// check (DIAG-15); a check of Cleanup stands on the other board, so its
/// card only reads. Selecting is no edit: where the Inspector is drawn
/// disabled, on a read-only board or a locked document, the card still
/// senses its click and its focus, and stays as faded as the rest.
fn row(ui: &mut Ui, row: &Row, actions: &mut Vec<super::Action>) {
    let border = if row.tone == Tone::Failed {
        theme::DANGER.gamma_multiply(0.5)
    } else {
        theme::BORDER
    };
    let mut frame = egui::Frame::new()
        .fill(theme::PANEL)
        .stroke(egui::Stroke::new(1.0, border))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .begin(ui);
    {
        let ui = &mut frame.content_ui;
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 1.0;
        // The card takes the click, not a selection of its text.
        ui.style_mut().interaction.selectable_labels = false;
        ui.label(
            RichText::new(&row.title)
                .family(theme::bold_family())
                .size(theme::FONT_BODY)
                .color(theme::TEXT),
        );
        if !row.shared_with.is_empty() {
            ui.label(
                RichText::new(format!("Shared with: {}", row.shared_with.join(", ")))
                    .size(theme::FONT_META)
                    .color(theme::LINK),
            );
        }
        ui.label(
            RichText::new(&row.predicate)
                .size(theme::FONT_LABEL)
                .color(theme::LINK),
        );
        ui.label(
            RichText::new(&row.status)
                .size(theme::FONT_LABEL)
                .color(row.tone.color()),
        );
        ui.label(
            RichText::new(format!("Sources: {}", row.sources.join(", ")))
                .size(theme::FONT_META)
                .color(theme::MUTED),
        );
    }
    let rect = frame.allocate_space(ui).rect;
    let name = format!("Check {}", row.handle);
    let id = ui.id().with(("check", &row.handle));
    if row.cleanup {
        ui.interact(rect, id, egui::Sense::hover())
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &name));
        frame.paint(ui);
        return;
    }
    let response = enabled(ui, rect, id.with("sensing"))
        .interact(rect, id, egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
    if response.hovered() || response.has_focus() {
        frame.frame.fill = theme::FIELD;
    }
    if response.has_focus() {
        frame.frame.stroke = egui::Stroke::new(1.5, theme::SELECTION);
    }
    frame.paint(ui);
    if response.clicked() {
        actions.push(super::Action::Select(row.id.clone()));
    }
}

/// A `Ui` over `rect` of `ui`'s layer and clip that senses input where `ui`
/// is disabled, for what is no edit there: selecting a check.
fn enabled(ui: &Ui, rect: egui::Rect, id: egui::Id) -> Ui {
    let mut builder = egui::UiBuilder::new()
        .layer_id(ui.layer_id())
        .max_rect(rect)
        .accessibility_parent(ui.unique_id());
    if ui.is_sizing_pass() {
        builder = builder.sizing_pass();
    }
    let mut sensing = Ui::new(ui.ctx().clone(), id, builder);
    sensing.set_clip_rect(ui.clip_rect());
    sensing
}

/// The list of the selected step.
pub(super) fn show(ui: &mut Ui, context: &Context<'_>, actions: &mut Vec<super::Action>) {
    let rows = rows(
        context.board,
        context.graph,
        context.cleanup,
        context.project,
        &context.node.id,
        context.run,
    );
    if rows.is_empty() {
        // *Add check* above the sentence (DIAG-15).
        add_check(ui, context, true, actions);
        ui.add_space(4.0);
        widgets::hint(ui, "No check reads this step yet.");
        return;
    }
    // Three rows, one of them shared, fit the Inspector of the smallest window.
    ui.spacing_mut().item_spacing.y = 3.0;
    let (text, tone) = summary(&rows, context.run.is_some());
    // *Add check* shares the line above the list with its count, so that
    // three rows still fit the smallest window (DIAG-15, criterion 7).
    // Its words stand where the line holds them beside the count and the
    // switch; where it does not, as at 960 × 540, the button is its icon,
    // named and explained on its hover, as Run results' *Check* is (REP-08).
    let words = {
        let label = egui::FontId::proportional(theme::FONT_LABEL);
        let button = add_words_width(text_width(ui, "Add check".to_owned(), label.clone()));
        let count = text_width(
            ui,
            text.clone(),
            egui::FontId::new(theme::FONT_LABEL, theme::bold_family()),
        );
        let switch = context.expand_here.map_or(0.0, |switch| {
            12.0 + 8.0 + text_width(ui, expand_text(switch).2, label)
        });
        button + ui.spacing().item_spacing.x + count + switch <= ui.available_width()
    };
    // The line is as high as its button, no higher.
    ui.spacing_mut().interact_size.y = ADD_HEIGHT;
    let line = ui
        .horizontal(|ui| {
            add_check(ui, context, words, actions);
            ui.label(
                RichText::new(text)
                    .family(theme::bold_family())
                    .size(theme::FONT_LABEL)
                    .color(tone.color()),
            )
        })
        .inner;
    // The step's own switch of its checks, against the board's setting
    // (LAY-11, LAY-30).
    if let Some(switch) = context.expand_here {
        expand_here(ui, line.rect, switch, actions);
    }
    for line in &rows {
        row(ui, line, actions);
    }
}

/// What the menu says of a request with no sample and no run (DIAG-15).
const NO_FIELDS: &str = "Run the request or add a sample response to list its fields.";
/// A row of the menu: Output data's height (DIAG-10).
const MENU_ROW: f32 = 26.0;
/// How far a child's row starts right of its parent's, as in Output data.
const MENU_INDENT: f32 = 14.0;
/// The width of the fold mark, and the space between a row's parts.
const MENU_FOLD: f32 = 14.0;
const MENU_GAP: f32 = 8.0;
/// The most rows the menu draws in one frame, and of one branch.
const MENU_ROWS: usize = 400;
const MENU_BRANCH: usize = 100;
/// The deepest level the menu opens; a field there has no fold mark.
const MENU_DEPTH: usize = 10;

/// A check chosen in the menu: the output, the field's path and the check.
type Chosen = (String, String, FieldCheck);

/// The outputs of the step and their values as *Output data* shows them:
/// the last run's, else the sample's (DIAG-09, DIAG-10).
fn outputs(context: &Context<'_>) -> Vec<(Port, Option<Value>)> {
    output_ports(context.node, context.project)
        .into_iter()
        .map(|port| {
            let value = match context.step {
                Some(step) => step.outputs.get(&port.name).cloned(),
                None => super::response::sample_output(context.node, &port.name, context.project),
            };
            (port, value)
        })
        .collect()
}

/// *Add check*, the tab's first line, with no check and with checks
/// (DIAG-15): it opens *Add a check of*, the step's outputs as Output data
/// writes them, and a field there opens DIAG-08's menu of four checks.
/// Where the step takes no edit it is disabled and says why (DIAG-09); a
/// step with no output has none. `words` says whether the button shows its
/// words beside its icon.
fn add_check(ui: &mut Ui, context: &Context<'_>, words: bool, actions: &mut Vec<super::Action>) {
    let outputs = outputs(context);
    if outputs.is_empty() {
        return;
    }
    let id = context.document.with(("add-check", &context.node.id));
    let enabled = context.read_only.is_none();
    let mut open = enabled && ui.data(|data| data.get_temp::<bool>(id).unwrap_or(false));
    let button = ui.add_enabled(enabled, |ui: &mut Ui| add_button(ui, words));
    let button = match context.read_only {
        Some(why) => button.on_disabled_hover_text(why),
        None => button,
    };
    let toggled = button.clicked();
    if toggled {
        open = !open;
    }
    let menu_id = id.with("menu");
    let mut fields = Fields {
        context,
        menu_left: ui.ctx().read_response(menu_id).map(|menu| menu.rect.left()),
        open_field: None,
        chosen: None,
        escape: false,
        budget: MENU_ROWS,
        truncated: false,
    };
    // As wide as the Inspector leaves it, within the window.
    let room = ui.ctx().content_rect().right() - button.rect.left() - 40.0;
    // The menu opens below the button and nowhere else: its list scrolls in
    // the room down to the window's edge (criterion 4).
    let shown = egui::Popup::from_response(&button)
        .id(menu_id)
        .open_bool(&mut open)
        .kind(egui::PopupKind::Menu)
        .layout(egui::Layout::top_down_justified(egui::Align::Min))
        .style(egui::containers::menu::menu_style)
        .align(egui::RectAlign::BOTTOM_START)
        .align_alternatives(&[])
        .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
        .show(|ui| {
            ui.set_width(room.clamp(220.0, 320.0));
            fields.menu(ui, &outputs);
        });
    if let Some(shown) = shown {
        // A click outside the menu closes it, unless it is on the button,
        // which toggles it, or in the menu of the field it opened.
        let in_field_menu = |ctx: &egui::Context| {
            fields.open_field.is_some_and(|field| {
                ctx.input(|input| input.pointer.interact_pos())
                    .and_then(|at| ctx.layer_id_at(at))
                    .is_some_and(|layer| layer.id == field)
            })
        };
        if fields.escape
            || fields.chosen.is_some()
            || (shown.response.clicked_elsewhere() && !toggled && !in_field_menu(ui.ctx()))
        {
            open = false;
        }
    }
    ui.data_mut(|data| data.insert_temp(id, open));
    if let Some((port, path, check)) = fields.chosen {
        actions.push(super::Action::AssertField { port, path, check });
    }
}

/// The button *Add check*: a small framed button, so that the line it
/// shares with the count stays low enough for three checks below it in the
/// smallest window (LAY-08, criterion 5); its icon alone where `words`
/// is false, named and explained on its hover.
fn add_button(ui: &mut Ui, words: bool) -> egui::Response {
    const NAME: &str = "Add check";
    const HOVER: &str = "Check a field of this step's output";
    let galley = words.then(|| {
        ui.painter().layout_no_wrap(
            NAME.to_owned(),
            egui::FontId::proportional(theme::FONT_LABEL),
            theme::TEXT,
        )
    });
    let width = galley
        .as_ref()
        .map_or(ADD_HEIGHT, |galley| add_words_width(galley.size().x));
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, ADD_HEIGHT), egui::Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), NAME));
    let enabled = ui.is_enabled();
    if ui.is_rect_visible(rect) {
        let fill = if enabled && (response.hovered() || response.has_focus()) {
            theme::FIELD
        } else {
            theme::WHITE
        };
        let stroke = if response.has_focus() {
            egui::Stroke::new(1.5, theme::SELECTION)
        } else {
            egui::Stroke::new(1.0, theme::BORDER)
        };
        ui.painter()
            .rect(rect, 4.0, fill, stroke, egui::StrokeKind::Inside);
        let colour = if enabled { theme::TEXT } else { theme::MUTED };
        let icon = egui::Rect::from_center_size(
            egui::pos2(
                if words {
                    rect.left() + ADD_PADDING + 6.0
                } else {
                    rect.center().x
                },
                rect.center().y,
            ),
            egui::vec2(12.0, 12.0),
        );
        Icon::Plus.paint(ui.painter(), icon, colour);
        if let Some(galley) = galley {
            let at = egui::pos2(icon.right() + 4.0, rect.center().y - galley.size().y / 2.0);
            ui.painter().galley(at, galley, colour);
        }
    }
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if words {
        response.on_hover_text(HOVER)
    } else {
        response.on_hover_text(format!("{NAME}\n{HOVER}"))
    }
}

/// The button's height, and the space at each end of it.
const ADD_HEIGHT: f32 = 20.0;
const ADD_PADDING: f32 = 6.0;

/// The width of *Add check* with words `text` wide.
fn add_words_width(text: f32) -> f32 {
    ADD_PADDING + 12.0 + 4.0 + text + ADD_PADDING
}

/// The menu *Add a check of* while it is drawn.
struct Fields<'a, 'b> {
    context: &'a Context<'b>,
    /// The menu's left edge in the last frame, which a field's menu stands
    /// beside.
    menu_left: Option<f32>,
    /// The popup of the field whose menu is open.
    open_field: Option<egui::Id>,
    chosen: Option<Chosen>,
    /// Escape closed the menu.
    escape: bool,
    budget: usize,
    /// A row was left out for the budget.
    truncated: bool,
}

impl Fields<'_, '_> {
    fn menu(&mut self, ui: &mut Ui, outputs: &[(Port, Option<Value>)]) {
        widgets::name_menu(ui, "Add check menu");
        widgets::caption(ui, "ADD A CHECK OF");
        let request = self.context.node.kind == NodeKind::Request;
        let has = |name: &str| {
            outputs
                .iter()
                .any(|(port, value)| port.name == name && value.is_some())
        };
        if request && self.context.step.is_none() && !has("body") {
            widgets::hint(ui, NO_FIELDS);
        } else if outputs.iter().all(|(_, value)| value.is_none()) {
            widgets::hint(ui, "Run the step to see its data.");
        }
        let bottom = ui.ctx().content_rect().bottom();
        let room = (bottom - ui.cursor().top() - 16.0).max(3.0 * MENU_ROW);
        egui::ScrollArea::vertical()
            .id_salt("add-check-fields")
            .max_height(room)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 0.0;
                for (port, value) in outputs {
                    if let Some(value) = value {
                        self.row(
                            ui,
                            port,
                            value,
                            &JsonPath::root(),
                            port.display_name(),
                            0,
                            true,
                        );
                    }
                }
                if self.truncated {
                    widgets::hint(
                        ui,
                        "Field display limit reached. Collapse a branch to inspect other fields.",
                    );
                }
            });
        // Escape closes a field's menu first, which takes the key; then this.
        if self.open_field.is_none()
            && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.escape = true;
        }
    }

    /// The row of `value` at `path` of `port`, named `name`, and its open
    /// children. A row is the field's name and its value as Output data
    /// writes them; a click opens DIAG-08's menu of the field beside the
    /// list; ▸ opens an object or a list in place.
    #[allow(clippy::too_many_arguments)]
    fn row(
        &mut self,
        ui: &mut Ui,
        port: &Port,
        value: &Value,
        path: &JsonPath,
        name: &str,
        depth: usize,
        supported: bool,
    ) {
        if self.budget == 0 {
            self.truncated = true;
            return;
        }
        self.budget -= 1;
        let text = path.to_string();
        // The check's sentence names the output where the row is the whole
        // of it, as the Inspector's ✓ does; the row's name is its path.
        let subject = if text.is_empty() { &port.name } else { &text };
        let label = if text.is_empty() { name } else { &text };
        let children = match value {
            Value::Array(items) => items.len(),
            Value::Object(fields) => fields.len(),
            _ => 0,
        };
        let context = self.context;
        let fold_id =
            context
                .document
                .with(("add-check-fold", &context.node.id, &port.name, &text));
        let mut open = ui
            .data(|data| data.get_temp::<bool>(fold_id))
            .unwrap_or(depth == 0);

        let sense = if supported {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        };
        // The row starts where its level does, as its name does in Output data.
        let (line, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), MENU_ROW),
            egui::Sense::hover(),
        );
        let rect = line.with_min_x(line.left() + depth as f32 * MENU_INDENT);
        let response = ui.interact(rect, fold_id.with("row"), sense);
        let preview = super::fields::tree_preview(value, 200);
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, supported, label));
        ui.ctx()
            .accesskit_node_builder(response.id, |node| node.set_value(preview.as_str()));
        let popup = egui::Popup::default_response_id(&response);
        let field_open = egui::Popup::is_id_open(ui.ctx(), popup);
        if field_open {
            self.open_field = Some(popup);
        }
        if ui.is_rect_visible(rect) {
            self.paint(
                ui,
                rect,
                depth,
                name,
                value,
                &preview,
                response.hovered() || field_open,
                supported,
            );
        }
        // Fields deeper than the menu opens are read in Output data.
        let folds = children > 0 && depth < MENU_DEPTH;
        if folds {
            let x = rect.left();
            let mark = egui::Rect::from_center_size(
                egui::pos2(x + MENU_FOLD / 2.0, rect.center().y),
                egui::vec2(MENU_FOLD, 20.0),
            );
            let word = if open { "Collapse" } else { "Expand" };
            let toggle = ui.interact(mark, fold_id.with("mark"), egui::Sense::click());
            let named = format!("{word} {label}");
            toggle
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &named));
            ui.painter().text(
                mark.center(),
                egui::Align2::CENTER_CENTER,
                if open { "▾" } else { "▸" },
                egui::FontId::proportional(theme::FONT_LABEL),
                if toggle.hovered() {
                    theme::TEXT
                } else {
                    theme::MUTED
                },
            );
            if toggle.clicked() {
                open = !open;
                ui.data_mut(|data| data.insert_temp(fold_id, open));
            }
        }
        // DIAG-08's menu opens beside the list, level with the row: its
        // anchor is a strip left of the list as wide as the menu was.
        let width = ui
            .ctx()
            .read_response(popup)
            .map_or(220.0, |menu| menu.rect.width());
        let left = self.menu_left.unwrap_or(rect.left() - 8.0) - 4.0;
        let mut anchor = response.clone();
        anchor.rect = egui::Rect::from_min_max(
            egui::pos2(left - width, rect.top()),
            egui::pos2(left, rect.top()),
        );
        anchor.interact_rect = anchor.rect;
        // The checks it offers are written against the board the step is
        // on, as their cards will read them (DIAG-19).
        let field = super::fields::Field {
            node: &context.node.id,
            port: &port.name,
            path: &text,
            board: Some((context.graph, context.project)),
            shown: subject,
        };
        if let Some(check) = super::fields::check_menu(&anchor, field, value) {
            self.chosen = Some((port.name.clone(), text.clone(), check));
        }

        if !open || !folds {
            return;
        }
        let reserved = |key: &str| ["__proto__", "constructor", "prototype"].contains(&key);
        match value {
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate().take(MENU_BRANCH) {
                    let mut child = path.clone();
                    child.push_index(index);
                    self.row(
                        ui,
                        port,
                        item,
                        &child,
                        &format!("[{index}]"),
                        depth + 1,
                        supported,
                    );
                }
            }
            Value::Object(fields) => {
                for (key, item) in fields.iter().take(MENU_BRANCH) {
                    let mut child = path.clone();
                    child.push_key(key.clone());
                    let supported = supported && !reserved(key);
                    self.row(ui, port, item, &child, key, depth + 1, supported);
                }
            }
            _ => {}
        }
        if children > MENU_BRANCH {
            ui.horizontal(|ui| {
                ui.add_space((depth + 1) as f32 * MENU_INDENT + MENU_FOLD + MENU_GAP);
                widgets::hint(
                    ui,
                    format!("{} more in Output data", children - MENU_BRANCH),
                );
            });
        }
    }

    /// A row's look: Output data's name and value (DIAG-10), the hover of
    /// a menu's item, and at its end the mark of a menu that opens beside.
    #[allow(clippy::too_many_arguments)]
    fn paint(
        &self,
        ui: &Ui,
        rect: egui::Rect,
        depth: usize,
        name: &str,
        value: &Value,
        preview: &str,
        lit: bool,
        supported: bool,
    ) {
        if lit && supported {
            ui.painter()
                .rect_filled(rect, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
        }
        let middle = rect.center().y;
        let chevron = egui::Rect::from_center_size(
            egui::pos2(rect.right() - 10.0, middle),
            egui::vec2(12.0, 12.0),
        );
        if supported {
            Icon::ChevronRight.paint(ui.painter(), chevron, theme::MUTED);
        }
        let name_x = rect.left() + MENU_FOLD + MENU_GAP;
        let room = (chevron.left() - MENU_GAP - name_x).max(0.0);
        let name_text = if depth == 0 {
            RichText::new(name)
                .size(theme::FONT_BODY)
                .color(theme::TEXT)
        } else {
            RichText::new(name)
                .monospace()
                .size(theme::FONT_LABEL)
                .color(theme::LINK)
        };
        let name = line(ui, name_text, (room * 0.6).max(room.min(24.0)));
        let value_x = name_x + name.size().x + MENU_GAP;
        let value = line(
            ui,
            RichText::new(preview)
                .monospace()
                .size(theme::FONT_LABEL)
                .color(super::fields::tree_colour(value)),
            (chevron.left() - MENU_GAP - value_x).max(0.0),
        );
        // Names and values sit a point above the middle, as in Output data.
        for (x, galley) in [(name_x, name), (value_x, value)] {
            let at = egui::pos2(x, middle - 1.0 - galley.size().y / 2.0);
            ui.painter().galley(at, galley, theme::TEXT);
        }
    }
}

/// `text` on one line of at most `width`, cut with an ellipsis.
fn line(ui: &Ui, text: RichText, width: f32) -> std::sync::Arc<egui::Galley> {
    egui::WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        width,
        egui::FontSelection::Default,
    )
}

/// The switch's name, hover and the text it shows.
fn expand_text(switch: crate::canvas::ChecksHere) -> (&'static str, &'static str, String) {
    let (name, hover) = if switch.board_folds {
        (
            "Expand checks here",
            "Show this step's checks as cards under it on the board",
        )
    } else {
        (
            "Collapse checks here",
            "Fold this step's checks into its line on the board",
        )
    };
    let text = if switch.chosen {
        format!("✓ {name}")
    } else {
        name.to_owned()
    };
    (name, hover, text)
}

/// The width `text` takes in `font`.
fn text_width(ui: &Ui, text: String, font: egui::FontId) -> f32 {
    ui.painter()
        .layout_no_wrap(text, font, theme::TEXT)
        .size()
        .x
}

/// *Expand checks here* while the board folds checks, *Collapse checks
/// here* while it draws them, pressed while the step's own choice differs
/// from the board's (LAY-30); at the right end of the line above the list
/// when it fits there, so that the three rows below still fit the smallest
/// window (LAY-08, criterion 5); on a line of its own when it does not.
fn expand_here(
    ui: &mut Ui,
    line: egui::Rect,
    switch: crate::canvas::ChecksHere,
    actions: &mut Vec<super::Action>,
) {
    let (name, hover, text) = expand_text(switch);
    let expanded = switch.chosen;
    let galley = ui.painter().layout_no_wrap(
        text,
        egui::FontId::proportional(theme::FONT_LABEL),
        theme::LINK,
    );
    let size = galley.size() + egui::vec2(8.0, 0.0);
    let right = ui.max_rect().right();
    let rect = if line.right() + 12.0 + size.x <= right {
        egui::Rect::from_min_size(
            egui::pos2(right - size.x, line.center().y - size.y / 2.0),
            size,
        )
    } else {
        ui.allocate_space(size).1
    };
    let response = ui
        .interact(
            rect,
            ui.id().with("expand-checks-here"),
            egui::Sense::click(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(hover);
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, expanded, name));
    if expanded || response.hovered() {
        ui.painter().rect_filled(rect, 4.0, theme::ACCENT_SOFT);
    }
    ui.painter()
        .galley(rect.min + egui::vec2(4.0, 0.0), galley, theme::LINK);
    if response.clicked() {
        actions.push(super::Action::ExpandChecksHere(!expanded));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_core::{
        format::Position,
        graph::create_node,
        run::{Phase, RunMode},
    };

    fn project() -> Project {
        apiforge_core::demos::empty_project()
    }

    fn request(id: &str) -> Node {
        let mut node = create_node(NodeKind::Request, Position { x: 0.0, y: 0.0 }, None);
        node.id = id.into();
        node.handle = id.into();
        node
    }

    /// `shared` reads `first` and `second`; `second` waits for `first`.
    fn board() -> Graph {
        let mut second = request("second");
        second
            .inputs
            .insert("url".into(), Binding::reference("first", "body", "id"));
        let mut shared = create_node(NodeKind::Assert, Position { x: 0.0, y: 0.0 }, None);
        shared.id = "shared".into();
        shared.handle = "shared".into();
        shared
            .inputs
            .insert("actual".into(), Binding::reference("first", "body", "a"));
        shared
            .inputs
            .insert("expected".into(), Binding::reference("second", "body", "a"));
        Graph {
            nodes: vec![request("first"), second, shared],
            edges: Vec::new(),
            proxies: Vec::new(),
        }
    }

    fn run(board: &Graph, done: &[&str], status: RunStatus) -> RunResult {
        RunResult {
            id: "run".into(),
            scenario_id: "scenario".into(),
            mode: RunMode::Simulation,
            started_at: String::new(),
            finished_at: None,
            status,
            steps: done
                .iter()
                .map(|id| {
                    let node = board.nodes.iter().find(|node| node.id == *id).unwrap();
                    let mut step = StepResult::new(node, Phase::Main);
                    step.status = StepStatus::Passed;
                    step
                })
                .collect(),
            diagnostic: false,
            snapshot: None,
            error: None,
            notice: None,
            running: None,
        }
    }

    fn title(board: &Graph, subject: &str, run: Option<&RunResult>) -> String {
        rows(board, board, None, &project(), subject, run)[0]
            .title
            .clone()
    }

    /// The moment follows a run under way: a shared check still waits for its
    /// later subject when the earlier one has finished.
    #[test]
    fn the_moment_follows_a_run_under_way() {
        let board = board();
        let early = run(&board, &["first"], RunStatus::Running);
        assert_eq!(
            title(&board, "first", Some(&early)),
            "shared — Waiting for second · Shared"
        );
        let late = run(&board, &["first", "second"], RunStatus::Running);
        assert_eq!(
            title(&board, "first", Some(&late)),
            "shared — Next · Shared"
        );
        assert_eq!(title(&board, "second", Some(&late)), "shared — Next");
        let finished = run(&board, &["first", "second"], RunStatus::Passed);
        assert_eq!(
            title(&board, "second", Some(&finished)),
            "shared — Not run",
            "a finished run that left the check out never reaches it"
        );
    }

    /// The summary says what became of the checks, and gates stand apart.
    #[test]
    fn the_summary_counts_what_became_of_the_checks() {
        let line = |gate: bool, outcome: Outcome, tone: Tone, shared: bool| Row {
            id: String::new(),
            handle: String::new(),
            title: String::new(),
            predicate: String::new(),
            status: String::new(),
            tone,
            outcome,
            sources: Vec::new(),
            shared_with: if shared {
                vec!["cart".into()]
            } else {
                Vec::new()
            },
            gate,
            cleanup: false,
        };
        let passed = line(false, Outcome::Passed, Tone::Passed, false);
        let waiting = line(false, Outcome::Waiting, Tone::Quiet, true);
        let failed = line(false, Outcome::Failed, Tone::Failed, false);
        let skipped = line(false, Outcome::NotRun, Tone::Quiet, false);
        let gate = line(true, Outcome::Waiting, Tone::Quiet, false);

        assert_eq!(
            summary(&[waiting.clone(), gate.clone()], false),
            ("1 check · 1 shared · 1 gate".to_owned(), Tone::Quiet)
        );
        assert_eq!(
            summary(std::slice::from_ref(&gate), false).0,
            "1 gate",
            "no check to count"
        );
        assert_eq!(
            summary(&[passed.clone(), passed.clone(), waiting], true),
            ("2/3 ✓ · 1 waiting · 1 shared".to_owned(), Tone::Quiet)
        );
        assert_eq!(
            summary(&[failed, skipped, passed.clone()], true),
            ("✕ 1/3 failed · 1 skipped".to_owned(), Tone::Failed)
        );
        assert_eq!(
            summary(std::slice::from_ref(&passed), true),
            ("1/1 ✓".to_owned(), Tone::Passed)
        );

        // A check of Cleanup listed under a Main step is counted apart, like a
        // gate, and its failure still colours the line (LAY-20).
        let in_cleanup = Row {
            cleanup: true,
            ..line(false, Outcome::Waiting, Tone::Quiet, false)
        };
        assert_eq!(
            summary(&[passed.clone(), passed.clone(), in_cleanup.clone()], false),
            ("2 checks · 1 in Cleanup".to_owned(), Tone::Quiet)
        );
        assert_eq!(
            summary(&[passed.clone(), in_cleanup, gate], false),
            ("1 check · 1 in Cleanup · 1 gate".to_owned(), Tone::Quiet)
        );
        let failed_in_cleanup = Row {
            cleanup: true,
            ..line(false, Outcome::Failed, Tone::Failed, false)
        };
        assert_eq!(
            summary(&[passed, failed_in_cleanup], true),
            ("1/1 ✓ · 1 in Cleanup".to_owned(), Tone::Failed),
            "the count leaves it out, the colour does not"
        );
    }

    /// A Cleanup check that reads a Main step is listed under that step, with
    /// the moment word *Cleanup* until it has a result (LAY-20).
    #[test]
    fn a_cleanup_check_is_listed_under_the_main_step_it_reads() {
        let board = board();
        let mut kept = create_node(NodeKind::Assert, Position { x: 0.0, y: 0.0 }, None);
        kept.id = "kept".into();
        kept.handle = "kept".into();
        kept.inputs
            .insert("actual".into(), Binding::reference("first", "body", "id"));
        kept.inputs.insert(
            "expected".into(),
            Binding::literal(serde_json::json!("c-1")),
        );
        let cleanup = Graph {
            nodes: vec![request("removed"), kept],
            edges: Vec::new(),
            proxies: Vec::new(),
        };

        let listed = rows(&board, &board, Some(&cleanup), &project(), "first", None);
        let titles: Vec<&str> = listed.iter().map(|row| row.title.as_str()).collect();
        assert_eq!(
            titles,
            ["shared — Waiting for second · Shared", "kept — Cleanup"],
            "the Cleanup check comes after the Main checks"
        );
        assert_eq!(
            listed.last().unwrap().sources,
            ["first.body.id"],
            "the Cleanup check names the Main step it reads"
        );
        assert_eq!(
            summary(&listed, false).0,
            "1 check · 1 shared · 1 in Cleanup"
        );

        // Nothing is listed under a step of the Cleanup graph itself: that
        // board lists its own checks.
        assert!(rows(&board, &board, Some(&cleanup), &project(), "removed", None).is_empty());
        // The result takes the moment's place once the check has one.
        let done = run(&cleanup, &["kept"], RunStatus::Passed);
        assert_eq!(
            rows(
                &board,
                &board,
                Some(&cleanup),
                &project(),
                "first",
                Some(&done)
            )
            .last()
            .unwrap()
            .title,
            "kept — Passed"
        );
    }
}
