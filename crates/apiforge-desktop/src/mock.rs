//! REQ-00 mock-up: the *Steps* presentation of a scenario, drawn from the
//! real graph and the real run. Mock branch only: never merged.
use crate::canvas::{check_text, own_step};
use crate::theme;
use crate::widgets;
use apiforge_core::format::{Binding, Graph, Node, Project, TemplatePart};
use apiforge_core::graph::{relations, sequence};
use apiforge_core::model::NodeKind;
use apiforge_core::run::{RunResult, StepResult, StepStatus};
use apiforge_core::template_text::{Scope, print_reference, print_template};
use eframe::egui::{self, Color32, RichText, Ui};
use indexmap::IndexMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering::Relaxed};

/// The Steps presentation is chosen.
pub static ON: AtomicBool = AtomicBool::new(false);
/// Question 1: 0 against the common request, 1 against the step above,
/// 2 against the first request.
pub static SENDS: AtomicU8 = AtomicU8::new(0);
/// Question 4: 0 three columns, 1 the list as a rail, 2 Request | Response
/// as tabs, 3 the response under the request.
pub static FOLD: AtomicU8 = AtomicU8::new(0);
/// Question 5: 0 every step a row, 1 requests only.
pub static KINDS: AtomicU8 = AtomicU8::new(0);
/// Question 7: 0 nothing, 1 `+` between cards and a row at the end, 2 a
/// button on the bar.
pub static ADD: AtomicU8 = AtomicU8::new(0);
/// Question 8: 0 a section of the list, 1 the Main | Cleanup tabs.
pub static CLEANUP: AtomicU8 = AtomicU8::new(0);
/// 0 Body, 1 Headers, 2 Auth.
pub static TAB: AtomicU8 = AtomicU8::new(0);
/// Question 3: 0 the last run, 1 sent alone, 2 the document changed.
pub static ALONE: AtomicU8 = AtomicU8::new(0);
/// Questions 6 and 8: the card's menu is open.
pub static MENU: AtomicBool = AtomicBool::new(false);
/// Question 2: 0 today, 1 a link to Steps, 2 a summary and the link.
pub static INSPECTOR: AtomicU8 = AtomicU8::new(0);
/// The handle of the step shown as selected; empty for none.
pub static SELECTED: Mutex<String> = Mutex::new(String::new());

pub fn on() -> bool {
    ON.load(Relaxed)
}

pub struct View<'a> {
    pub main: &'a Graph,
    pub cleanup: Option<&'a Graph>,
    pub project: &'a Project,
    pub run: Option<&'a RunResult>,
    pub file: &'a str,
    /// The ids of the Cleanup steps; `cleanup` also holds Main's nodes, so
    /// that what a Cleanup step reads has a name.
    pub cleanup_own: Vec<String>,
}

struct Row<'a> {
    node: &'a Node,
    number: usize,
    depth: usize,
    branch: Option<&'static str>,
    checks: Vec<&'a Node>,
    cleanup: bool,
}

fn scope<'a>(graph: &'a Graph, project: &'a Project) -> Scope<'a> {
    Scope {
        nodes: &graph.nodes,
        project,
        request: None,
    }
}

fn text(binding: &Binding, s: Scope<'_>) -> String {
    match binding {
        Binding::Literal { value } => serde_json::to_string(value).unwrap_or_default(),
        Binding::Object { fields } => format!(
            "{{ {} }}",
            fields
                .iter()
                .map(|(key, value)| format!("{key}: {}", text(value, s)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Binding::Array { items } => format!(
            "[{}]",
            items
                .iter()
                .map(|item| text(item, s))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Binding::Template { .. } => print_template(binding, s).unwrap_or_else(|| "…".into()),
        other => print_reference(other, s).unwrap_or_else(|| "…".into()),
    }
}

fn json(binding: &Binding, s: Scope<'_>) -> serde_json::Value {
    match binding {
        Binding::Literal { value } => value.clone(),
        Binding::Object { fields } => serde_json::Value::Object(
            fields
                .iter()
                .map(|(key, value)| (key.clone(), json(value, s)))
                .collect(),
        ),
        Binding::Array { items } => {
            serde_json::Value::Array(items.iter().map(|item| json(item, s)).collect())
        }
        Binding::Template { .. } => serde_json::Value::String(text(binding, s)),
        other => serde_json::Value::String(format!("{{{{{}}}}}", text(other, s))),
    }
}

/// What a request sends, one entry per leaf: `url`, `header Name`,
/// `query name`, and the body's fields by path.
fn sent(node: &Node, s: Scope<'_>) -> IndexMap<String, String> {
    fn leaves(prefix: &str, value: &serde_json::Value, out: &mut IndexMap<String, String>) {
        match value {
            serde_json::Value::Object(fields) if !fields.is_empty() => {
                for (key, value) in fields {
                    let path = if prefix.is_empty() || prefix.ends_with(' ') {
                        format!("{prefix}{key}")
                    } else {
                        format!("{prefix}.{key}")
                    };
                    leaves(&path, value, out);
                }
            }
            other => {
                out.insert(prefix.to_owned(), other.to_string());
            }
        }
    }
    let mut out = IndexMap::new();
    for (input, prefix) in [("headers", "header "), ("query", "query "), ("body", "")] {
        if let Some(binding) = node.inputs.get(input) {
            leaves(prefix, &json(binding, s), &mut out);
        }
    }
    out
}

fn url(node: &Node, s: Scope<'_>) -> String {
    node.inputs
        .get("url")
        .map(|binding| text(binding, s))
        .unwrap_or_default()
}

fn path(url: &str) -> String {
    match url.split_once("://") {
        Some((_, rest)) => rest
            .find('/')
            .map_or(String::new(), |at| rest[at..].to_owned()),
        None => url.to_owned(),
    }
}

fn shape(node: &Node, s: Scope<'_>) -> String {
    let url = url(node, s);
    let mut out = format!("{:?} ", node.config.method);
    let mut inside = false;
    for part in url.split("{{") {
        if inside {
            out.push_str("{}");
            out.push_str(part.split_once("}}").map_or("", |(_, rest)| rest));
        } else {
            out.push_str(part);
        }
        inside = true;
    }
    out
}

fn method(node: &Node) -> &'static str {
    node.config.method.map_or("GET", crate::labels::method)
}

fn reads(node: &Node, s: Scope<'_>) -> Vec<String> {
    fn walk(binding: &Binding, s: Scope<'_>, out: &mut Vec<String>) {
        match binding {
            Binding::Literal { .. } => {}
            Binding::Object { fields } => fields.values().for_each(|value| walk(value, s, out)),
            Binding::Array { items } => items.iter().for_each(|item| walk(item, s, out)),
            Binding::Expression { inputs, .. } => {
                inputs.values().for_each(|value| walk(value, s, out));
            }
            Binding::Template { parts } => parts.iter().for_each(|part| {
                if let TemplatePart::Value { binding } = part {
                    walk(binding, s, out);
                }
            }),
            other => {
                let name = text(other, s);
                if !out.contains(&name) {
                    out.push(name);
                }
            }
        }
    }
    let mut out = Vec::new();
    node.inputs
        .values()
        .for_each(|binding| walk(binding, s, &mut out));
    out
}

fn rows<'a>(graph: &'a Graph, cleanup: bool, own: Option<&[String]>) -> Vec<Row<'a>> {
    let order = sequence(graph);
    let node = |id: &str| graph.nodes.iter().find(|node| node.id == id).unwrap();
    // The branch a step stands in: the `if` and its side, by the branch
    // edge that leads to it or to a step before it in the same braces.
    let mut inside: IndexMap<&str, (usize, &'static str)> = IndexMap::new();
    for id in &order {
        for edge in graph.edges.iter().filter(|edge| edge.target == *id) {
            let depth = inside
                .get(edge.source.as_str())
                .map_or(0, |(depth, _)| *depth);
            let source = node(&edge.source);
            match edge.branch {
                Some(apiforge_core::model::Branch::True) => {
                    inside.insert(id, (depth + 1, "then"));
                }
                Some(apiforge_core::model::Branch::False) => {
                    inside.insert(id, (depth + 1, "else"));
                }
                None if source.kind != NodeKind::If => {
                    if let Some(place) = inside.get(edge.source.as_str()).copied() {
                        inside.entry(id).or_insert(place);
                    }
                }
                None => {}
            }
        }
    }
    let mut out: Vec<Row<'a>> = Vec::new();
    let mut last_branch: Option<(usize, &'static str)> = None;
    for (index, id) in order.iter().enumerate() {
        let step = node(id);
        if step.kind == NodeKind::Start || own.is_some_and(|own| !own.contains(&step.id)) {
            continue;
        }
        if step.kind == NodeKind::Assert
            && let Some(owner) = own_step(step, graph)
            && let Some(row) = out.iter_mut().find(|row| row.node.id == owner)
        {
            row.checks.push(step);
            continue;
        }
        let place = inside.get(id).copied().or_else(|| {
            // A check that reads a step of a branch stands in that branch.
            (step.kind == NodeKind::Assert)
                .then(|| own_step(step, graph).and_then(|owner| inside.get(owner).copied()))
                .flatten()
        });
        out.push(Row {
            node: step,
            number: index + 1,
            depth: place.map_or(0, |(depth, _)| depth),
            branch: place
                .filter(|place| last_branch != Some(*place))
                .map(|(_, side)| side),
            checks: Vec::new(),
            cleanup,
        });
        last_branch = place;
    }
    out
}

fn result<'a>(run: Option<&'a RunResult>, node: &Node) -> Option<&'a StepResult> {
    run?.steps.iter().find(|step| step.node_id == node.id)
}

fn status_colour(status: StepStatus) -> Color32 {
    match status {
        StepStatus::Passed => theme::SUCCESS,
        StepStatus::Failed | StepStatus::Error => theme::DANGER,
        _ => theme::MUTED,
    }
}

fn mono(text: impl Into<String>) -> RichText {
    RichText::new(text).monospace().size(theme::FONT_LABEL)
}

fn caption(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(10.0).strong().color(theme::MUTED));
}

fn badge(ui: &mut Ui, number: usize, colour: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 11.0, colour);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        number.to_string(),
        egui::FontId::proportional(10.5),
        theme::WHITE,
    );
}

fn mark(ui: &mut Ui, status: Option<StepStatus>) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
    let Some(status) = status else {
        ui.painter()
            .circle_stroke(rect.center(), 5.0, egui::Stroke::new(1.2, theme::SOCKET));
        return;
    };
    let colour = status_colour(status);
    ui.painter()
        .circle_filled(rect.center(), 7.0, colour.gamma_multiply(0.16));
    let icon = match status {
        StepStatus::Passed => crate::icons::Icon::Check,
        StepStatus::Failed | StepStatus::Error => crate::icons::Icon::Close,
        _ => crate::icons::Icon::Minus,
    };
    icon.paint(ui.painter(), rect.shrink(3.0), colour);
}

fn cut(text: &str, limit: usize) -> String {
    crate::labels::truncate(text, limit)
}

/// What the SENDS column says of `row` under the rule of question 1.
fn sends(row: &Row<'_>, all: &[Row<'_>], view: &View<'_>, graph: &Graph) -> Vec<(String, bool)> {
    let s = scope(graph, view.project);
    let mine = sent(row.node, s);
    let group: Vec<&Row<'_>> = all
        .iter()
        .filter(|other| {
            other.node.kind == NodeKind::Request && shape(other.node, s) == shape(row.node, s)
        })
        .collect();
    let one = |key: &str, value: &str| format!("{key}: {}", cut(value, 36));
    let whole = || {
        if mine.is_empty() {
            vec![("no body, no headers".to_owned(), true)]
        } else {
            mine.iter()
                .take(4)
                .map(|(key, value)| (one(key, value), false))
                .collect()
        }
    };
    let against = |base: &IndexMap<String, String>, base_url: &str| {
        let mut out = Vec::new();
        let own_url = url(row.node, s);
        if own_url != base_url {
            out.push((format!("path {}", path(&own_url)), false));
        }
        for (key, value) in &mine {
            if base.get(key) != Some(value) {
                out.push((one(key, value), false));
            }
        }
        for key in base.keys().filter(|key| !mine.contains_key(*key)) {
            out.push((format!("no {key}"), false));
        }
        out
    };
    let mut out = match SENDS.load(Relaxed) {
        0 => {
            if group.len() < 3 {
                return whole();
            }
            // The common request: per place, what most of the group sends.
            let mut common = IndexMap::new();
            let mut keys: Vec<String> = Vec::new();
            for other in &group {
                for key in sent(other.node, s).keys() {
                    if !keys.contains(key) {
                        keys.push(key.clone());
                    }
                }
            }
            for key in keys {
                let mut counts: IndexMap<String, usize> = IndexMap::new();
                for other in &group {
                    if let Some(value) = sent(other.node, s).get(&key) {
                        *counts.entry(value.clone()).or_default() += 1;
                    }
                }
                if let Some((value, count)) = counts.iter().max_by_key(|(_, count)| **count)
                    && *count * 2 > group.len()
                {
                    common.insert(key, value.clone());
                }
            }
            let mut urls: IndexMap<String, usize> = IndexMap::new();
            for other in &group {
                *urls.entry(url(other.node, s)).or_default() += 1;
            }
            let common_url = urls
                .iter()
                .max_by_key(|(_, count)| **count)
                .map(|(url, _)| url.clone())
                .unwrap_or_default();
            against(&common, &common_url)
        }
        1 => {
            let above = all
                .iter()
                .take_while(|other| other.node.id != row.node.id)
                .filter(|other| other.node.kind == NodeKind::Request)
                .last();
            match above {
                Some(above) => against(&sent(above.node, s), &url(above.node, s)),
                None => return whole(),
            }
        }
        _ => match group.first() {
            Some(first) if first.node.id != row.node.id => {
                against(&sent(first.node, s), &url(first.node, s))
            }
            _ => return whole(),
        },
    };
    if out.is_empty() {
        out.push(("nothing differs".to_owned(), true));
    }
    if out.len() > 4 {
        let more = out.len() - 3;
        out.truncate(3);
        out.push((format!("+ {more} more"), true));
    }
    out
}

fn check_line(ui: &mut Ui, check: &Node, graph: &Graph, view: &View<'_>, limit: usize) {
    let s = Scope {
        request: Some(check),
        ..scope(graph, view.project)
    };
    let words = check_text(check, s, graph, own_step(check, graph)).capped(limit);
    let step = result(view.run, check);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        mark(ui, step.map(|step| step.status));
        ui.label(mono(cut(&words, limit + 30)));
        if let Some(step) = step.filter(|step| step.status == StepStatus::Failed)
            && let apiforge_core::format::OptionalJson::Present(actual) = &step.actual
        {
            ui.label(
                mono(format!("— actual {}", cut(&actual.to_string(), 24))).color(theme::DANGER),
            );
        }
    });
}

fn outcome(row: &Row<'_>, view: &View<'_>) -> Option<(String, Color32)> {
    let step = result(view.run, row.node)?;
    let failed = row
        .checks
        .iter()
        .filter(|check| {
            result(view.run, check).is_some_and(|step| step.status == StepStatus::Failed)
        })
        .count();
    Some(if failed > 0 {
        (
            format!("{failed} of {} failed", row.checks.len()),
            theme::DANGER,
        )
    } else {
        (step.status.label().to_owned(), status_colour(step.status))
    })
}

fn received(row: &Row<'_>, view: &View<'_>) -> Option<String> {
    result(view.run, row.node)?
        .outputs
        .get("status")
        .map(ToString::to_string)
}

fn selected(row: &Row<'_>) -> bool {
    *SELECTED.lock().unwrap() == row.node.handle
}

fn plus_line(ui: &mut Ui) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 14.0), egui::Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        egui::Stroke::new(1.0, theme::SELECTION),
    );
    let centre = egui::pos2(rect.left() + 30.0, rect.center().y);
    ui.painter().circle_filled(centre, 9.0, theme::SELECTION);
    crate::icons::Icon::Plus.paint(
        ui.painter(),
        egui::Rect::from_center_size(centre, egui::vec2(10.0, 10.0)),
        theme::WHITE,
    );
}

fn add_row(ui: &mut Ui, words: &str) {
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.label(RichText::new(format!("+  {words}")).color(theme::LINK));
        });
}

fn section(ui: &mut Ui, title: &str, note: &str) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(11.0).strong().color(theme::TEXT));
        ui.label(RichText::new(note).size(11.0).color(theme::MUTED));
    });
    ui.add_space(2.0);
}

/// One step in the wide list: a card for a request, a line for the rest.
fn card(ui: &mut Ui, row: &Row<'_>, all: &[Row<'_>], view: &View<'_>, graph: &Graph) {
    let s = scope(graph, view.project);
    let width = ui.available_width();
    ui.horizontal_top(|ui| {
        ui.add_space(row.depth as f32 * 26.0);
        if let Some(branch) = row.branch {
            ui.label(
                RichText::new(branch)
                    .size(10.0)
                    .strong()
                    .color(if branch == "then" {
                        theme::BRANCH_YES
                    } else {
                        theme::BRANCH_NO
                    }),
            );
        } else if row.depth > 0 {
            ui.add_space(27.0);
        }
        badge(
            ui,
            row.number,
            if row.cleanup {
                theme::MUTED
            } else {
                theme::TEXT
            },
        );
        let room = width - row.depth as f32 * 26.0 - if row.depth > 0 { 70.0 } else { 36.0 };
        let stroke = if selected(row) {
            egui::Stroke::new(2.0, theme::ACCENT)
        } else {
            egui::Stroke::new(1.0, theme::BORDER)
        };
        egui::Frame::new()
            .fill(theme::WHITE)
            .stroke(stroke)
            .corner_radius(6)
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.set_width(room - 22.0);
                ui.vertical(|ui| match row.node.kind {
                    NodeKind::Request => request_card(ui, row, all, view, graph),
                    NodeKind::If => {
                        let words = check_text(row.node, s, graph, None).capped(60);
                        ui.horizontal(|ui| {
                            ui.label(mono("if").strong().color(theme::SIMULATION));
                            ui.label(mono(words));
                            ui.label(mono(row.node.handle.clone()).color(theme::MUTED));
                            if let Some((words, colour)) = outcome(row, view) {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| widgets::pill(ui, &words, colour),
                                );
                            }
                        });
                    }
                    NodeKind::Component => {
                        ui.horizontal(|ui| {
                            ui.label(mono("call").strong().color(theme::OPERATOR));
                            ui.label(RichText::new(row.node.display_name()).strong());
                            let call = row
                                .node
                                .inputs
                                .iter()
                                .map(|(key, value)| format!("{key}: {}", text(value, s)))
                                .collect::<Vec<_>>()
                                .join(", ");
                            ui.label(mono(cut(&format!("({call})"), 40)).color(theme::MUTED));
                            let read = relations(graph, row.node).readers.len();
                            if read > 0 {
                                ui.label(
                                    mono(format!("SETS {} · read by {read}", row.node.handle))
                                        .color(theme::LINK),
                                );
                            }
                            if let Some((words, colour)) = outcome(row, view) {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| widgets::pill(ui, &words, colour),
                                );
                            }
                        });
                    }
                    NodeKind::Assert => check_line(ui, row.node, graph, view, 70),
                    _ => {
                        let value = row
                            .node
                            .inputs
                            .values()
                            .next()
                            .map(|binding| text(binding, s))
                            .unwrap_or_default();
                        ui.horizontal(|ui| {
                            ui.label(mono("value").strong().color(theme::VARIABLE));
                            ui.label(mono(format!(
                                "{} = {}",
                                row.node.handle,
                                cut(&value, if width < 700.0 { 44 } else { 90 })
                            )));
                        });
                    }
                });
            });
    });
}

fn request_card(ui: &mut Ui, row: &Row<'_>, all: &[Row<'_>], view: &View<'_>, graph: &Graph) {
    let s = scope(graph, view.project);
    let narrow = ui.available_width() < 640.0;
    ui.horizontal(|ui| {
        ui.label(mono(method(row.node)).strong().color(theme::LINK));
        ui.label(RichText::new(cut(row.node.display_name(), 30)).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some((words, colour)) = outcome(row, view) {
                widgets::pill(ui, &words, colour);
            }
            if let Some(status) = received(row, view) {
                widgets::pill(ui, &status, theme::MUTED);
            }
            let room = ((ui.available_width() - 8.0) / 7.3).max(8.0) as usize;
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.label(mono(cut(&path(&url(row.node, s)), room)).color(theme::MUTED));
            });
        });
    });
    ui.add_space(2.0);
    let readers = relations(graph, row.node).readers;
    let total = ui.available_width();
    let columns = if narrow {
        [total * 0.24, total * 0.30, total * 0.46]
    } else {
        [total * 0.18, total * 0.30, total * 0.52]
    };
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.allocate_ui_with_layout(
            egui::vec2(columns[0], 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(columns[0]);
                ui.vertical(|ui| {
                    caption(ui, "READS");
                    let read = reads(row.node, s);
                    if read.is_empty() {
                        ui.label(mono("nothing").color(theme::MUTED));
                    }
                    for name in read {
                        ui.label(mono(cut(&name, (columns[0] / 7.4) as usize)).color(theme::LINK));
                    }
                });
            },
        );
        ui.allocate_ui_with_layout(
            egui::vec2(columns[1], 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(columns[1]);
                ui.vertical(|ui| {
                    let limit = ((columns[1] - 14.0) / 7.4) as usize;
                    if readers.is_empty() {
                        let peers = all
                            .iter()
                            .filter(|other| {
                                other.node.kind == NodeKind::Request
                                    && shape(other.node, s) == shape(row.node, s)
                            })
                            .count();
                        caption(
                            ui,
                            match SENDS.load(Relaxed) {
                                0 if peers < 3 => "SENDS",
                                0 => "SENDS · differs from the common request",
                                1 => "SENDS · differs from the step above",
                                _ => "SENDS · differs from the first request",
                            },
                        );
                        for (line, quiet) in sends(row, all, view, graph) {
                            let line = mono(cut(&line, limit));
                            ui.label(if quiet {
                                line.color(theme::MUTED)
                            } else {
                                line
                            });
                        }
                    } else {
                        caption(ui, "SETS · read by a later step");
                        let mut seen: Vec<String> = Vec::new();
                        for feed in &readers {
                            let field = if feed.path.is_empty() {
                                feed.output.to_owned()
                            } else {
                                format!("{}.{}", feed.output, feed.path)
                            };
                            let reader = graph
                                .nodes
                                .iter()
                                .find(|node| node.id == feed.step)
                                .map_or("?", |node| node.handle.as_str());
                            let line = format!("{field} → {reader}");
                            if !seen.contains(&line) {
                                seen.push(line);
                            }
                        }
                        for line in seen.iter().take(4) {
                            ui.label(mono(cut(line, limit)));
                        }
                    }
                });
            },
        );
        ui.allocate_ui_with_layout(
            egui::vec2(columns[2], 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(columns[2]);
                ui.vertical(|ui| {
                    caption(ui, "CHECKS");
                    if row.checks.is_empty() {
                        ui.label(mono("none").color(theme::MUTED));
                    }
                    let limit = ((columns[2] - 30.0) / 7.4) as usize;
                    for check in &row.checks {
                        check_line(ui, check, graph, view, limit.saturating_sub(18).max(20));
                    }
                });
            },
        );
    });
}

/// One step in the narrow list beside the request.
fn list_row(ui: &mut Ui, row: &Row<'_>, view: &View<'_>, rail: bool) {
    let fill = if selected(row) {
        theme::ACCENT_SOFT
    } else {
        Color32::TRANSPARENT
    };
    egui::Frame::new()
        .fill(fill)
        .corner_radius(5)
        .inner_margin(egui::Margin::symmetric(4, 2))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                if !rail {
                    ui.add_space(row.depth as f32 * 12.0);
                }
                badge(
                    ui,
                    row.number,
                    if row.cleanup {
                        theme::MUTED
                    } else {
                        theme::TEXT
                    },
                );
                let worst = row
                    .checks
                    .iter()
                    .filter_map(|check| result(view.run, check))
                    .map(|step| step.status)
                    .find(|status| *status == StepStatus::Failed)
                    .or_else(|| result(view.run, row.node).map(|step| step.status));
                mark(ui, worst);
                if rail {
                    return;
                }
                let name = match row.node.kind {
                    NodeKind::If => format!("if {}", row.node.handle),
                    _ => row.node.display_name().to_owned(),
                };
                let room = ((ui.available_width() - 58.0 - row.depth as f32 * 12.0) / 6.8).max(6.0)
                    as usize;
                let weight = RichText::new(cut(&name, room)).size(theme::FONT_LABEL);
                ui.label(if row.node.kind == NodeKind::Request {
                    weight.strong()
                } else {
                    weight.color(theme::MUTED)
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !row.checks.is_empty() {
                        ui.label(mono(format!("✓{}", row.checks.len())).color(theme::MUTED));
                    }
                    if let Some(status) = received(row, view) {
                        ui.label(mono(status).color(theme::MUTED));
                    }
                });
            });
        });
}

fn tabs(ui: &mut Ui, names: &[&str], active: usize) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 14.0;
        for (index, name) in names.iter().enumerate() {
            let mut chosen = active;
            widgets::tab_value(ui, &mut chosen, index, name);
        }
    });
}

fn code(ui: &mut Ui, value: &serde_json::Value, height: f32) {
    let height = height.max(36.0);
    let pretty = serde_json::to_string_pretty(value).unwrap_or_default();
    egui::Frame::new()
        .fill(theme::PANEL)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(height);
            let room = ((ui.available_width() - 34.0) / 7.3) as usize;
            let lines = (height / 16.0).max(3.0) as usize;
            let all: Vec<&str> = pretty.lines().collect();
            for (index, line) in all.iter().take(lines).enumerate() {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    ui.label(mono(format!("{:>2}", index + 1)).color(theme::SOCKET));
                    let colour = if line.contains("{{") {
                        theme::SIMULATION
                    } else {
                        theme::TEXT
                    };
                    ui.label(mono(cut(line, room)).color(colour));
                });
            }
            if all.len() > lines {
                ui.label(
                    mono(format!("   … {} more lines", all.len() - lines)).color(theme::MUTED),
                );
            }
        });
}

fn small_button(ui: &mut Ui, words: &str, primary: bool) {
    egui::Frame::new()
        .fill(if primary {
            theme::SIMULATION
        } else {
            theme::WHITE
        })
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.label(
                RichText::new(words)
                    .size(theme::FONT_LABEL)
                    .color(if primary { theme::WHITE } else { theme::TEXT }),
            );
        });
}

fn request_pane(ui: &mut Ui, row: &Row<'_>, view: &View<'_>, graph: &Graph, height: f32) {
    let s = scope(graph, view.project);
    let narrow = ui.available_width() < 520.0;
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    mono(format!("{} ▾", method(row.node)))
                        .strong()
                        .color(theme::LINK),
                );
                let address = url(row.node, s);
                let room = ((ui.available_width() - 8.0) / 7.3) as usize;
                let shown = if address.chars().count() > room {
                    format!("…{}", path(&address))
                } else {
                    address
                };
                ui.label(mono(cut(&shown, room)));
            });
        });
    ui.horizontal(|ui| {
        small_button(ui, "▶ Run to here", true);
        small_button(ui, "Send alone", false);
    });
    let headers = sent(row.node, s)
        .keys()
        .filter(|key| key.starts_with("header "))
        .count();
    let tab = TAB.load(Relaxed) as usize;
    let header_tab = format!("Headers {headers}");
    if narrow {
        tabs(ui, &["Body", &header_tab, "Auth", "More ▾"], tab);
    } else {
        tabs(
            ui,
            &["Body", &header_tab, "Auth", "Query", "Sample", "Settings"],
            tab,
        );
    }
    ui.add_space(4.0);
    match tab {
        0 => {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("JSON ▾")
                        .size(theme::FONT_LABEL)
                        .color(theme::MUTED),
                );
                small_button(ui, "Prettify", false);
                ui.label(
                    RichText::new("✓ valid")
                        .size(theme::FONT_LABEL)
                        .color(theme::SUCCESS),
                );
            });
            let body = row
                .node
                .inputs
                .get("body")
                .map_or(serde_json::Value::Null, |binding| json(binding, s));
            code(ui, &body, height - 132.0);
        }
        1 => {
            for (key, value) in sent(row.node, s)
                .iter()
                .filter(|(key, _)| key.starts_with("header "))
            {
                field_pair(
                    ui,
                    key.trim_start_matches("header "),
                    value.trim_matches('"'),
                );
            }
            field_pair(ui, "name", "value");
            ui.label(
                RichText::new("Authorization is set on the Auth tab.")
                    .size(theme::FONT_META)
                    .color(theme::MUTED),
            );
        }
        _ => {
            let auth = sent(row.node, s)
                .get("header Authorization")
                .map(|value| value.trim_matches('"').to_owned());
            ui.horizontal(|ui| {
                ui.label("Type");
                small_button(
                    ui,
                    match &auth {
                        Some(value) if value.starts_with("Bearer ") => "Bearer token ▾",
                        Some(_) => "Free header ▾",
                        None => "None ▾",
                    },
                    false,
                );
            });
            if let Some(value) = &auth {
                field_pair(ui, "Token", value.trim_start_matches("Bearer "));
                ui.label(mono(format!("writes  Authorization: \"{value}\"")).color(theme::MUTED));
                ui.label(
                    RichText::new("A real secret belongs in the environment: {{env.TOKEN}}.")
                        .size(theme::FONT_META)
                        .color(theme::WARNING),
                );
            } else {
                ui.label(mono("writes no Authorization header").color(theme::MUTED));
            }
        }
    }
}

fn field_pair(ui: &mut Ui, name: &str, value: &str) {
    ui.horizontal(|ui| {
        let total = ui.available_width();
        for (words, width) in [(name, total * 0.36), (value, total * 0.58)] {
            egui::Frame::new()
                .fill(theme::FIELD)
                .corner_radius(4)
                .inner_margin(egui::Margin::symmetric(6, 4))
                .show(ui, |ui| {
                    ui.set_width(width - 12.0);
                    ui.vertical(|ui| ui.label(mono(cut(words, ((width - 16.0) / 7.3) as usize))));
                });
        }
    });
}

fn banner(ui: &mut Ui, words: &str, action: &str, colour: Color32) {
    egui::Frame::new()
        .fill(colour.gamma_multiply(0.12))
        .corner_radius(5)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format!("{words} "))
                        .size(theme::FONT_LABEL)
                        .color(theme::TEXT),
                );
                ui.label(
                    RichText::new(action)
                        .size(theme::FONT_LABEL)
                        .strong()
                        .color(theme::LINK),
                );
            });
        });
}

fn response_pane(ui: &mut Ui, row: &Row<'_>, view: &View<'_>, graph: &Graph, height: f32) {
    let step = result(view.run, row.node);
    let alone = ALONE.load(Relaxed);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Response").strong());
        ui.add_space(6.0);
        match alone {
            1 => widgets::pill(ui, "SENT ALONE · simulation", theme::WARNING),
            _ => widgets::pill(ui, "SIMULATION · sample", theme::SIMULATION),
        };
        if let Some(status) = received(row, view) {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{} ms", step.map_or(0, |step| step.duration_ms)))
                        .size(theme::FONT_LABEL)
                        .color(theme::MUTED),
                );
                widgets::pill(ui, &status, theme::MUTED);
            });
        }
    });
    match alone {
        1 => banner(
            ui,
            "Sent alone with the values of the last run. Not part of a run: the run's verdict is unchanged.",
            "Show the run's response",
            theme::WARNING,
        ),
        2 => banner(
            ui,
            "The request changed after this run. This is the response to what was sent then.",
            "Run to here",
            theme::WARNING,
        ),
        _ => {}
    }
    let Some(step) = step else {
        ui.add_space(8.0);
        ui.label(
            RichText::new("Not run yet. Run to here, or send this request alone.")
                .color(theme::MUTED),
        );
        checks_box(ui, row, view, graph);
        return;
    };
    if ui.available_width() < 420.0 {
        tabs(ui, &["Body", "Headers", "Sent"], 0);
    } else {
        tabs(ui, &["Body", "Headers", "Request sent"], 0);
    }
    ui.add_space(4.0);
    let checks = row.checks.len().min(5) as f32 * 21.0 + 34.0;
    let banner_room = if alone > 0 { 40.0 } else { 0.0 };
    code(
        ui,
        step.outputs.get("body").unwrap_or(&serde_json::Value::Null),
        (height - 78.0 - checks - banner_room).max(52.0),
    );
    checks_box(ui, row, view, graph);
}

fn checks_box(ui: &mut Ui, row: &Row<'_>, view: &View<'_>, graph: &Graph) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Checks").strong());
        ui.add_space(6.0);
        let done: Vec<_> = row
            .checks
            .iter()
            .filter_map(|check| result(view.run, check))
            .collect();
        if !done.is_empty() {
            let passed = done
                .iter()
                .filter(|step| step.status == StepStatus::Passed)
                .count();
            let all = passed == done.len();
            ui.label(
                RichText::new(format!("{passed} of {} passed", done.len()))
                    .size(theme::FONT_LABEL)
                    .color(if all { theme::SUCCESS } else { theme::DANGER }),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new("+ Add check")
                    .size(theme::FONT_LABEL)
                    .color(theme::LINK),
            );
        });
    });
    let limit = ((ui.available_width() - 40.0) / 7.3) as usize;
    for check in row.checks.iter().take(5) {
        check_line(ui, check, graph, view, limit.saturating_sub(16).max(18));
    }
}

fn menu(ctx: &egui::Context, at: egui::Pos2) {
    egui::Area::new(egui::Id::new("req00-menu"))
        .fixed_pos(at)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(theme::WHITE)
                .stroke(egui::Stroke::new(1.0, theme::BORDER))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.set_width(210.0);
                    for (entry, key) in [
                        ("Run to here", ""),
                        ("Send this request alone", ""),
                        ("Add step after", ""),
                        ("Move earlier", "Alt+↑"),
                        ("Move later", "Alt+↓"),
                        ("Move to Cleanup", ""),
                        ("Show in Canvas", ""),
                        ("Delete", "Del"),
                    ] {
                        ui.horizontal(|ui| {
                            ui.label(entry);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| ui.label(RichText::new(key).color(theme::MUTED)),
                            );
                        });
                    }
                });
        });
}

/// The whole presentation, in place of the board.
pub fn steps(ui: &mut Ui, view: &View<'_>) {
    ui.spacing_mut().interact_size.y = 18.0;
    ui.spacing_mut().item_spacing.y = 3.0;
    let main = rows(view.main, false, None);
    let mut cleanup = view
        .cleanup
        .map(|graph| rows(graph, true, Some(&view.cleanup_own)))
        .unwrap_or_default();
    for (index, row) in cleanup.iter_mut().enumerate() {
        row.number = index + 1;
    }
    let tabs_for_cleanup = CLEANUP.load(Relaxed) == 1;
    let requests_only = KINDS.load(Relaxed) == 1;
    let add = ADD.load(Relaxed);
    let checks: usize = main
        .iter()
        .chain(&cleanup)
        .map(|row| row.checks.len())
        .sum();
    egui::Frame::new()
        .fill(theme::WHITE)
        .inner_margin(egui::Margin::symmetric(12, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.set_height(theme::CONTROL_HEIGHT);
                ui.label(RichText::new(view.file).strong());
                if tabs_for_cleanup {
                    let mut chosen = 0;
                    widgets::tab_value(ui, &mut chosen, 0, "Main");
                    widgets::tab_value(ui, &mut chosen, 1, format!("Cleanup {}", cleanup.len()));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if add == 2 {
                        small_button(ui, "+ Add step", false);
                    }
                    egui::Frame::new()
                        .stroke(egui::Stroke::new(1.0, theme::BORDER))
                        .corner_radius(6)
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            for (name, chosen) in [("Canvas", false), ("Steps", true)] {
                                egui::Frame::new()
                                    .fill(if chosen { theme::FIELD } else { theme::WHITE })
                                    .corner_radius(6)
                                    .inner_margin(egui::Margin::symmetric(9, 4))
                                    .show(ui, |ui| {
                                        let words = RichText::new(name).size(theme::FONT_LABEL);
                                        ui.label(if chosen { words.strong() } else { words });
                                    });
                            }
                        });
                    if ui.available_width() > 190.0 {
                        ui.label(
                            RichText::new(format!(
                                "{} steps · {checks} checks",
                                main.len() + cleanup.len()
                            ))
                            .size(theme::FONT_LABEL)
                            .color(theme::MUTED),
                        );
                    }
                });
            });
        });
    let shown = |row: &&Row<'_>| !requests_only || row.node.kind == NodeKind::Request;
    let picked = main
        .iter()
        .map(|row| (row, view.main))
        .chain(
            cleanup
                .iter()
                .map(|row| (row, view.cleanup.unwrap_or(view.main))),
        )
        .find(|(row, _)| selected(row));
    let area = ui.available_rect_before_wrap();
    let Some((picked, picked_graph)) = picked else {
        // Nothing selected: the wide cards.
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let mut hidden: Vec<String> = Vec::new();
                    for (index, row) in main.iter().enumerate() {
                        if !shown(&row) {
                            hidden.push(row.node.handle.clone());
                            continue;
                        }
                        if !hidden.is_empty() {
                            ui.label(
                                mono(format!("     · not shown: {}", hidden.join(", ")))
                                    .color(theme::MUTED),
                            );
                            hidden.clear();
                        }
                        card(ui, row, &main, view, view.main);
                        if add == 1 && index == 1 {
                            plus_line(ui);
                        }
                    }
                    if !hidden.is_empty() {
                        ui.label(
                            mono(format!("     · not shown: {}", hidden.join(", ")))
                                .color(theme::MUTED),
                        );
                    }
                    if add == 1 {
                        add_row(ui, "Add step");
                    }
                    if !tabs_for_cleanup
                        && (view.cleanup.is_some())
                        && (!cleanup.is_empty() || add == 1)
                    {
                        section(
                            ui,
                            "CLEANUP",
                            "runs after the test, also when a check fails",
                        );
                        for row in cleanup.iter().filter(shown) {
                            card(ui, row, &cleanup, view, view.cleanup.unwrap());
                        }
                        if add == 1 {
                            add_row(ui, "Add cleanup step");
                        }
                    }
                });
        });
        if MENU.load(Relaxed) {
            menu(
                ui.ctx(),
                egui::pos2(area.right() - 250.0, area.top() + 96.0),
            );
        }
        return;
    };
    let fold = FOLD.load(Relaxed);
    let list = if fold == 1 {
        54.0
    } else {
        (area.width() * 0.24).clamp(190.0, 300.0)
    };
    let rest = area.width() - list - 2.0;
    let height = area.height();
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.allocate_ui_with_layout(
            egui::vec2(list, height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(list);
                egui::Frame::new()
                    .fill(theme::WHITE)
                    .inner_margin(egui::Margin::symmetric(4, 6))
                    .show(ui, |ui| {
                        ui.set_min_height(height - 12.0);
                        ui.set_width(list - 8.0);
                        egui::ScrollArea::vertical()
                            .id_salt("req00-list")
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = 1.0;
                                for row in main.iter().filter(shown) {
                                    list_row(ui, row, view, fold == 1);
                                }
                                if !tabs_for_cleanup && !cleanup.is_empty() {
                                    if fold == 1 {
                                        ui.separator();
                                    } else {
                                        section(ui, "CLEANUP", "");
                                    }
                                    for row in cleanup.iter().filter(shown) {
                                        list_row(ui, row, view, fold == 1);
                                    }
                                }
                                if add == 1 && fold != 1 {
                                    add_row(ui, "Add step");
                                }
                            });
                    });
            },
        );
        let pane = |ui: &mut Ui, width: f32, height: f32, request: bool| {
            ui.allocate_ui_with_layout(
                egui::vec2(width, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(width);
                    egui::Frame::new()
                        .fill(theme::WHITE)
                        .stroke(egui::Stroke::new(1.0, theme::BORDER))
                        .inner_margin(egui::Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.set_width(width - 22.0);
                            ui.set_min_height(height - 14.0);
                            ui.set_max_height(height - 14.0);
                            ui.vertical(|ui| {
                                if request {
                                    request_pane(ui, picked, view, picked_graph, height - 14.0);
                                } else {
                                    response_pane(ui, picked, view, picked_graph, height - 14.0);
                                }
                            });
                        });
                },
            );
        };
        match fold {
            2 => {
                ui.vertical(|ui| {
                    egui::Frame::new()
                        .fill(theme::WHITE)
                        .inner_margin(egui::Margin::symmetric(10, 2))
                        .show(ui, |ui| {
                            ui.set_width(rest - 20.0);
                            tabs(ui, &["Request", "Response 422 · 4 of 4"], 0);
                        });
                    pane(ui, rest, height - 34.0, true);
                });
            }
            3 => {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    pane(ui, rest, height * 0.5, true);
                    pane(ui, rest, height * 0.5, false);
                });
            }
            _ => {
                pane(ui, rest * 0.5, height, true);
                pane(ui, rest * 0.5, height, false);
            }
        }
    });
    if MENU.load(Relaxed) {
        menu(
            ui.ctx(),
            egui::pos2(area.left() + list - 30.0, area.top() + 60.0),
        );
    }
}
