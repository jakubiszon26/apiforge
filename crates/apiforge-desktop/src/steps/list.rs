//! The list of *Steps*: a card for each request, a line for every other step
//! (REQ-01a). It draws a [`Model`] beside the last run's results and returns
//! what a click asks for; a row out of sight keeps its place and its name and
//! is not laid out.

use super::{
    model::{Check, Column, Kind, Model, Request, Row},
    sends,
};
use crate::{
    not_run::{self, NotRun},
    report, theme,
};
use apiforge_core::{
    model::NodeKind,
    run::{CauseEnds, Phase, StepResult, StepStatus},
};
use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Ui, pos2, vec2,
};
use std::collections::HashMap;

/// The results of the run the rows are marked with.
pub struct Results<'a> {
    steps: HashMap<(Phase, &'a str), &'a StepResult>,
    ends: &'a CauseEnds,
}

impl<'a> Results<'a> {
    pub fn new(steps: &'a [StepResult], ends: &'a CauseEnds) -> Self {
        Self {
            steps: steps
                .iter()
                .map(|step| ((step.phase, step.node_id.as_str()), step))
                .collect(),
            ends,
        }
    }

    fn of(&self, phase: Phase, id: &str) -> Option<&'a StepResult> {
        self.steps.get(&(phase, id)).copied()
    }
}

/// What a click or a key asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Select {
        id: String,
        cleanup: bool,
    },
    /// Escape.
    Clear,
}

/// What a row says of the run.
struct Outcome {
    words: String,
    colour: Color32,
    /// Why the step did not run, in words and in a sentence.
    reason: Option<(String, String)>,
}

impl Outcome {
    fn of(step: &StepResult, results: &Results<'_>, checks: &[Check], phase: Phase) -> Self {
        if let Some(not_run) = NotRun::of(step, results.ends) {
            return Self {
                words: "Not run".into(),
                colour: theme::MUTED,
                reason: Some((not_run.words(), not_run.sentence())),
            };
        }
        let failed = checks
            .iter()
            .filter_map(|check| results.of(phase, &check.id))
            .filter(|check| matches!(check.status, StepStatus::Failed | StepStatus::Error))
            .count();
        let words = if not_run::unmet(step) {
            "Not met".to_owned()
        } else if step.status == StepStatus::Passed && failed > 0 {
            format!("{failed} of {} failed", checks.len())
        } else {
            step.status.label().to_owned()
        };
        Self {
            words,
            colour: if step.status == StepStatus::Passed && failed > 0 {
                theme::DANGER
            } else {
                report::status_color(step.status)
            },
            reason: None,
        }
    }

    /// The words of a row's accessible name.
    fn said(&self) -> String {
        match &self.reason {
            Some((words, _)) => format!("{} · {words}", self.words),
            None => self.words.clone(),
        }
    }
}

const MARGIN: f32 = 12.0;
const GAP: f32 = 4.0;
const BADGE: f32 = 22.0;
const BADGE_GAP: f32 = 8.0;
/// A branch's indent: the place of *then* or *else* and of its badge.
const INDENT: f32 = 55.0;
/// The least width a card keeps when branches indent it.
const LEAST_CARD: f32 = 320.0;
const PAD: egui::Vec2 = vec2(10.0, 6.0);
const LINE_HEIGHT: f32 = 30.0;
const HEAD_HEIGHT: f32 = 22.0;
const CAPTION_HEIGHT: f32 = 16.0;
const TEXT_HEIGHT: f32 = 18.0;
const CHECK_HEIGHT: f32 = 21.0;
const MARK: f32 = 20.0;
const PILL_HEIGHT: f32 = 19.0;
/// Narrower than this a card's columns stand one under the other.
const THREE_COLUMNS: f32 = 500.0;
/// From this width the columns are the board's shares of the card.
const WIDE: f32 = 800.0;
const COLUMN_GAP: f32 = 12.0;

const READS: &str = "READS";
const SENDS: &str = "SENDS";
const DIFFERS: &str = "SENDS · differs from the common request";
const DIFFERS_SHORT: &str = "SENDS · differs";
const SETS: &str = "SETS · read by a later step";
const SETS_SHORT: &str = "SETS";
const CHECKS: &str = "CHECKS";
/// The heading of Cleanup's rows.
pub const CLEANUP: &str = "CLEANUP · runs after the test, also when a check fails";

fn mono() -> FontId {
    FontId::monospace(11.5)
}

fn caption_font() -> FontId {
    FontId::proportional(10.0)
}

fn name_font() -> FontId {
    FontId::proportional(theme::FONT_BODY)
}

fn width_of(ui: &Ui, text: &str, font: FontId) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), font, theme::TEXT)
        .size()
        .x
}

/// One text on one line in `rect`, cut with … where `rect` ends; a cut text
/// is read in full on hover, beside `hover` when there is one. Returns the
/// width it took.
fn text(
    ui: &Ui,
    id: egui::Id,
    rect: Rect,
    text: &str,
    font: FontId,
    colour: Color32,
    hover: Option<&str>,
) -> f32 {
    let response = ui.interact(rect, id, Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text));
    if !ui.is_rect_visible(rect) {
        return rect.width();
    }
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, colour);
    job.wrap.max_width = rect.width();
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    let galley = ui.painter().layout_job(job);
    let cut = galley.elided;
    let width = galley.size().x;
    ui.painter().galley(
        pos2(rect.left(), rect.center().y - galley.size().y / 2.0),
        galley,
        colour,
    );
    match (cut, hover) {
        (_, Some(hover)) => {
            response.on_hover_text(hover);
        }
        (true, None) => {
            response.on_hover_text(text);
        }
        (false, None) => {}
    }
    width
}

/// The columns of a request's card.
#[derive(Clone, Copy)]
struct Columns {
    /// The widths of READS, of SENDS and of CHECKS.
    widths: [f32; 3],
    stacked: bool,
}

impl Columns {
    /// For a card of `width` inside, whose widest caption takes `caption`.
    fn of(width: f32, caption: f32) -> Self {
        if width < THREE_COLUMNS {
            return Self {
                widths: [width; 3],
                stacked: true,
            };
        }
        let reads = if width < WIDE {
            (width * 0.19).max(110.0)
        } else {
            width * 0.18
        };
        // The caption is one line (criterion 4).
        let sends = (width * 0.30).max((caption + COLUMN_GAP + 1.0).min(width * 0.36));
        Self {
            widths: [reads, sends, width - reads - sends],
            stacked: false,
        }
    }

    /// The columns' heights and the contents' height below the first line.
    fn height(&self, heights: [f32; 3]) -> f32 {
        if self.stacked {
            heights.iter().sum::<f32>() + 2.0 * GAP
        } else {
            heights.iter().copied().fold(0.0, f32::max)
        }
    }
}

/// The list's state: the selection it last showed.
#[derive(Default)]
pub struct List {
    shown: Option<(String, Option<String>)>,
}

/// What the rows are drawn from.
struct Drawing<'a> {
    document: &'a str,
    results: Option<&'a Results<'a>>,
    selected: Option<&'a str>,
    /// The widths of the two captions that have a short form, measured once
    /// a frame.
    differs: f32,
    sets: f32,
}

impl List {
    /// Draws `model` in the room left, and returns what was asked for.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        document: &str,
        model: &Model,
        results: Option<&Results<'_>>,
        selected: Option<&str>,
    ) -> Option<Action> {
        let mut action = None;
        let now = (document.to_owned(), selected.map(str::to_owned));
        // A step selected anywhere is brought into view once.
        let reveal = self.shown.as_ref() != Some(&now) && selected.is_some();
        self.shown = Some(now);
        let drawing = Drawing {
            document,
            results,
            selected,
            differs: width_of(ui, DIFFERS, caption_font()),
            sets: width_of(ui, SETS, caption_font()),
        };
        egui::ScrollArea::vertical()
            .id_salt(("steps-list", document))
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.add_space(MARGIN);
                if model.rows().next().is_none() {
                    let (rect, _) = ui.allocate_exact_size(
                        vec2(ui.available_width(), HEAD_HEIGHT),
                        Sense::hover(),
                    );
                    text(
                        ui,
                        ui.id().with("no-steps"),
                        rect.shrink2(vec2(MARGIN, 0.0)),
                        "No steps yet.",
                        FontId::proportional(theme::FONT_BODY),
                        theme::MUTED,
                        None,
                    );
                }
                for (rows, heading) in [(&model.main, None), (&model.cleanup, Some(CLEANUP))] {
                    if rows.is_empty() {
                        continue;
                    }
                    if let Some(heading) = heading {
                        ui.add_space(MARGIN);
                        let (rect, _) = ui.allocate_exact_size(
                            vec2(ui.available_width(), HEAD_HEIGHT),
                            Sense::hover(),
                        );
                        text(
                            ui,
                            ui.id().with("cleanup-heading"),
                            rect.shrink2(vec2(MARGIN, 0.0)),
                            heading,
                            FontId::proportional(theme::FONT_META),
                            theme::MUTED,
                            None,
                        );
                    }
                    for row in rows {
                        let response = drawing.row(ui, row);
                        if let Some(id) = response.1 {
                            action = Some(Action::Select {
                                id,
                                cleanup: row.phase == Phase::Cleanup,
                            });
                        }
                        if reveal && selected.is_some_and(|id| row.holds(id)) {
                            ui.scroll_to_rect(response.0.expand(GAP), None);
                        }
                        ui.add_space(GAP);
                    }
                }
                ui.add_space(MARGIN);
            });
        if action.is_none()
            && selected.is_some()
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.ctx().memory(|memory| memory.top_modal_layer().is_none())
            && !egui::Popup::is_any_open(ui.ctx())
            && ui.input(|input| input.key_pressed(egui::Key::Escape))
        {
            action = Some(Action::Clear);
        }
        action
    }
}

impl Drawing<'_> {
    /// Draws one row; returns its place and the step a click chose.
    fn row(&self, ui: &mut Ui, row: &Row) -> (Rect, Option<String>) {
        let full = ui.available_width() - 2.0 * MARGIN;
        let indent = (row.depth as f32 * INDENT)
            .min(full - BADGE - BADGE_GAP - LEAST_CARD)
            .max(0.0);
        let width = full - indent - BADGE - BADGE_GAP;
        let inside = width - 2.0 * PAD.x;
        let columns = Columns::of(inside, self.differs);
        let height = match &row.kind {
            Kind::Request(request) => 2.0 * PAD.y + HEAD_HEIGHT + columns.height(heights(request)),
            _ => LINE_HEIGHT,
        };
        let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
        let card = Rect::from_min_size(
            pos2(
                line.left() + MARGIN + indent + BADGE + BADGE_GAP,
                line.top(),
            ),
            vec2(width, height),
        );
        let id = ui.id().with((
            "steps-row",
            self.document,
            row.phase == Phase::Cleanup,
            &row.id,
        ));
        let result = self
            .results
            .and_then(|results| results.of(row.phase, &row.id));
        let outcome = self
            .results
            .zip(result)
            .map(|(results, step)| Outcome::of(step, results, row.checks(), row.phase));
        let response = ui.interact(card, id, Sense::click());
        let active = self.selected.is_some_and(|id| row.holds(id));
        response.widget_info(|| {
            let name = name(row, outcome.as_ref());
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name)
        });
        ui.ctx().accesskit_node_builder(id, |node| {
            node.set_selected(active);
            if let Some(said) = description(row) {
                node.set_description(said);
            }
        });
        if response.gained_focus() {
            ui.scroll_to_rect(card, None);
        }
        // A row out of sight, and its checks, are named and described for
        // assistive technology and the keyboard; nothing is laid out.
        let visible = ui.is_rect_visible(line);
        if visible {
            self.frame(ui, row, line, card, indent, &response, active);
        }
        let mut chosen = response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
            .then(|| row.id.clone());
        let inner = card.shrink2(PAD);
        let head = match row.kind {
            Kind::Request(_) => Rect::from_min_size(inner.min, vec2(inner.width(), HEAD_HEIGHT)),
            _ => Rect::from_min_size(
                pos2(inner.left(), card.top()),
                vec2(inner.width(), card.height()),
            ),
        };
        if visible {
            self.head(ui, id, row, head, result, outcome.as_ref());
        }
        if let Kind::Request(request) = &row.kind {
            let top = head.bottom();
            let contents = Rect::from_min_max(pos2(inner.left(), top), inner.max);
            chosen = self
                .columns(ui, id, row, request, contents, columns, visible)
                .or(chosen);
        }
        (line, chosen)
    }

    /// The badge, the branch's name and the card's frame.
    #[allow(clippy::too_many_arguments)]
    fn frame(
        &self,
        ui: &Ui,
        row: &Row,
        line: Rect,
        card: Rect,
        indent: f32,
        response: &Response,
        active: bool,
    ) {
        let painter = ui.painter();
        let stroke = if active {
            Stroke::new(2.0, theme::ACCENT)
        } else if response.has_focus() {
            Stroke::new(1.5, theme::ACCENT)
        } else if response.hovered() {
            Stroke::new(1.0, theme::SOCKET)
        } else {
            Stroke::new(1.0, theme::BORDER)
        };
        painter.rect(card, 6, theme::WHITE, stroke, egui::StrokeKind::Inside);
        if response.has_focus() && active {
            painter.rect_stroke(
                card.shrink(3.0),
                4,
                Stroke::new(1.0, theme::ACCENT),
                egui::StrokeKind::Inside,
            );
        }
        let badge = pos2(
            line.left() + MARGIN + indent + BADGE / 2.0,
            line.top() + LINE_HEIGHT / 2.0,
        );
        painter.circle_filled(
            badge,
            BADGE / 2.0,
            if row.phase == Phase::Cleanup {
                theme::MUTED
            } else {
                theme::TEXT
            },
        );
        painter.text(
            badge,
            Align2::CENTER_CENTER,
            row.number.to_string(),
            FontId::proportional(10.5),
            theme::WHITE,
        );
        if let Some(branch) = row.branch.filter(|_| row.opens) {
            painter.text(
                pos2(badge.x - BADGE / 2.0 - 5.0, badge.y),
                Align2::RIGHT_CENTER,
                branch,
                FontId::proportional(theme::FONT_META),
                if branch == "then" {
                    theme::BRANCH_YES
                } else {
                    theme::BRANCH_NO
                },
            );
        }
    }

    /// The first line of a row: what the step is, and what a run said of it.
    fn head(
        &self,
        ui: &Ui,
        id: egui::Id,
        row: &Row,
        rect: Rect,
        result: Option<&StepResult>,
        outcome: Option<&Outcome>,
    ) {
        let mut right = rect.right();
        if let Some(outcome) = outcome {
            let pill = pill(
                ui,
                pos2(right, rect.center().y),
                &outcome.words,
                outcome.colour,
            );
            right = pill.left() - 6.0;
            if let Some((_, sentence)) = &outcome.reason {
                ui.interact(pill, id.with("outcome"), Sense::hover())
                    .on_hover_text(sentence);
            }
            if matches!(row.kind, Kind::Request(_))
                && let Some(status) = result
                    .and_then(|step| step.outputs.get("status"))
                    .and_then(serde_json::Value::as_u64)
            {
                let pill = self::pill(
                    ui,
                    pos2(right, rect.center().y),
                    &status.to_string(),
                    theme::MUTED,
                );
                right = pill.left() - 6.0;
            }
        }
        let mut x = rect.left();
        // A check on a row of its own has its mark before it.
        if let Kind::Check { .. } = row.kind {
            check_mark(ui, pos2(x + 7.0, rect.center().y), result);
            x += MARK;
        }
        let actual = result.and_then(failed_value);
        let mut pieces: Vec<(&str, FontId, Color32)> = Vec::new();
        let reason = outcome
            .and_then(|outcome| outcome.reason.as_ref())
            .map(|(words, _)| words.as_str());
        let arguments;
        let set;
        match &row.kind {
            Kind::Request(request) => {
                pieces.push((request.method, mono(), theme::LINK));
                pieces.push((&row.name, name_font(), theme::TEXT));
                pieces.push((&request.path, mono(), theme::MUTED));
            }
            Kind::Call {
                arguments: given,
                readers,
            } => {
                arguments = format!("({given})");
                set = format!("SETS {} · read by {readers}", row.handle);
                pieces.push(("call", mono(), theme::OPERATOR));
                pieces.push((&row.name, name_font(), theme::TEXT));
                pieces.push((&arguments, mono(), theme::MUTED));
                if *readers > 0 {
                    pieces.push((&set, mono(), theme::LINK));
                }
            }
            Kind::Line { keyword, text } => {
                let colour = match *keyword {
                    "if" | "condition" => theme::SIMULATION,
                    _ => theme::VARIABLE,
                };
                pieces.push((keyword, mono(), colour));
                pieces.push((text, mono(), theme::TEXT));
                if *keyword == "if" {
                    pieces.push((&row.handle, mono(), theme::MUTED));
                }
            }
            Kind::Check { text } => pieces.push((text, mono(), theme::TEXT)),
        }
        let failed;
        if let Some(actual) = &actual
            && let Kind::Check { .. } = row.kind
        {
            failed = format!("— {actual}");
            pieces.push((&failed, mono(), theme::DANGER));
        }
        if let Some(reason) = reason {
            pieces.push((reason, FontId::proportional(theme::FONT_META), theme::MUTED));
        }
        let sentence = outcome
            .and_then(|outcome| outcome.reason.as_ref())
            .map(|(_, sentence)| sentence.as_str());
        let gaps = 8.0 * pieces.len().saturating_sub(1) as f32;
        let whole: Vec<f32> = pieces
            .iter()
            .map(|(text, font, _)| width_of(ui, text, font.clone()).ceil() + 1.0)
            .collect();
        let widths = shared(&whole, right - 6.0 - x - gaps);
        for (index, ((piece, font, colour), width)) in pieces.into_iter().zip(widths).enumerate() {
            let hover = (Some(piece) == reason).then_some(sentence).flatten();
            text(
                ui,
                id.with(("head", index)),
                Rect::from_min_size(pos2(x, rect.top()), vec2(width, rect.height())),
                piece,
                font,
                colour,
                hover,
            );
            x += width + 8.0;
        }
    }

    /// READS, SENDS or SETS, and CHECKS, of which alone the checks are there
    /// out of sight; returns the check a click chose.
    #[allow(clippy::too_many_arguments)]
    fn columns(
        &self,
        ui: &Ui,
        id: egui::Id,
        row: &Row,
        request: &Request,
        rect: Rect,
        columns: Columns,
        visible: bool,
    ) -> Option<String> {
        let heights = heights(request);
        let mut at = rect.min;
        let mut chosen = None;
        for (index, (width, height)) in columns.widths.into_iter().zip(heights).enumerate() {
            let room = if index == 2 || columns.stacked {
                width
            } else {
                width - COLUMN_GAP
            };
            let column = Rect::from_min_size(at, vec2(room, height));
            match index {
                0 | 1 if !visible => {}
                0 => reads(ui, id, request, column),
                1 => self.middle(ui, id, request, column),
                _ => chosen = self.checks(ui, id, row, request, column),
            }
            if columns.stacked {
                at.y += height + GAP;
            } else {
                at.x += width;
            }
        }
        chosen
    }

    /// SENDS, or SETS for a step a later step reads.
    fn middle(&self, ui: &Ui, id: egui::Id, request: &Request, column: Rect) {
        let fitting = |words: &'static str, width: f32, short: &'static str| {
            if width <= column.width() {
                words
            } else {
                short
            }
        };
        let quiet;
        let lines: Vec<(&str, bool)> = match &request.column {
            Column::Sends(sends) => {
                let words = match sends.common {
                    Some(_) => fitting(DIFFERS, self.differs, DIFFERS_SHORT),
                    None => SENDS,
                };
                let hover = sends.common.as_deref();
                caption(ui, id.with("sends"), column, words, hover);
                quiet = sends
                    .lines
                    .iter()
                    .map(|line| (line.text.as_str(), line.quiet))
                    .collect();
                quiet
            }
            Column::Sets(lines) => {
                caption(
                    ui,
                    id.with("sends"),
                    column,
                    fitting(SETS, self.sets, SETS_SHORT),
                    None,
                );
                lines.iter().map(|line| (line.as_str(), false)).collect()
            }
        };
        let (listed, more) = sends::cut(lines.len());
        for (index, (line, quiet)) in lines[..listed].iter().enumerate() {
            let colour = if *quiet { theme::MUTED } else { theme::TEXT };
            let rect = line_rect(column, index);
            text(
                ui,
                id.with(("send", index)),
                rect,
                line,
                mono(),
                colour,
                None,
            );
        }
        if more > 0 {
            let rest: Vec<&str> = lines[listed..].iter().map(|(line, _)| *line).collect();
            text(
                ui,
                id.with("more"),
                line_rect(column, listed),
                &format!("+ {more} more"),
                mono(),
                theme::MUTED,
                Some(&rest.join("\n")),
            );
        }
    }

    fn checks(
        &self,
        ui: &Ui,
        id: egui::Id,
        row: &Row,
        request: &Request,
        column: Rect,
    ) -> Option<String> {
        if ui.is_rect_visible(column) {
            caption(ui, id.with("checks"), column, CHECKS, None);
        }
        let top = column.top() + CAPTION_HEIGHT;
        if request.checks.is_empty() {
            if !ui.is_rect_visible(column) {
                return None;
            }
            let rect =
                Rect::from_min_size(pos2(column.left(), top), vec2(column.width(), TEXT_HEIGHT));
            text(
                ui,
                id.with("none"),
                rect,
                "none",
                mono(),
                theme::MUTED,
                None,
            );
            return None;
        }
        let mut chosen = None;
        for (index, check) in request.checks.iter().enumerate() {
            let rect = Rect::from_min_size(
                pos2(column.left(), top + index as f32 * CHECK_HEIGHT),
                vec2(column.width(), CHECK_HEIGHT),
            );
            if self.check(ui, id, row, check, rect) {
                chosen = Some(check.id.clone());
            }
        }
        chosen
    }

    /// One check under its request; whether it was clicked.
    fn check(&self, ui: &Ui, row_id: egui::Id, row: &Row, check: &Check, rect: Rect) -> bool {
        let id = row_id.with(("check", &check.id));
        let result = self
            .results
            .and_then(|results| results.of(row.phase, &check.id));
        let outcome = self
            .results
            .zip(result)
            .map(|(results, step)| Outcome::of(step, results, &[], row.phase));
        let actual = result.and_then(failed_value);
        let response = ui.interact(rect, id, Sense::click());
        response.widget_info(|| {
            let mut name = check.text.clone();
            if let Some(outcome) = &outcome {
                name.push_str(" · ");
                name.push_str(&outcome.said());
            }
            if let Some(actual) = &actual {
                name.push_str(" · ");
                name.push_str(actual);
            }
            name.push_str(&format!(" · {} of {}", check.handle, row.name));
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name)
        });
        let active = self.selected == Some(check.id.as_str());
        if response.gained_focus() {
            ui.scroll_to_rect(rect, None);
        }
        ui.ctx()
            .accesskit_node_builder(id, |node| node.set_selected(active));
        if ui.is_rect_visible(rect) {
            if active {
                ui.painter().rect_filled(rect, 4.0, theme::ACCENT_SOFT);
            } else if response.hovered() {
                ui.painter().rect_filled(rect, 4.0, theme::BG);
            }
            if response.has_focus() {
                ui.painter().rect_stroke(
                    rect.shrink(0.5),
                    4.0,
                    Stroke::new(1.5, theme::ACCENT),
                    egui::StrokeKind::Inside,
                );
            }
            check_mark(ui, pos2(rect.left() + 7.0, rect.center().y), result);
            let failed = actual.map(|actual| format!("— {actual}"));
            let mut widths = vec![width_of(ui, &check.text, mono()).ceil() + 1.0];
            if let Some(failed) = &failed {
                widths.push(width_of(ui, failed, mono()).ceil() + 1.0);
            }
            let room = rect.width() - MARK - if failed.is_some() { 6.0 } else { 0.0 };
            let widths = shared(&widths, room);
            let mut x = rect.left() + MARK;
            // The check's own hover says what is cut.
            let mut cut = false;
            let mut piece = |words: &str, width: f32, colour: Color32| {
                let mut job =
                    egui::text::LayoutJob::simple_singleline(words.to_owned(), mono(), colour);
                job.wrap.max_width = width;
                job.wrap.max_rows = 1;
                job.wrap.break_anywhere = true;
                job.wrap.overflow_character = Some('…');
                let galley = ui.painter().layout_job(job);
                cut |= galley.elided;
                ui.painter().galley(
                    pos2(x, rect.center().y - galley.size().y / 2.0),
                    galley,
                    colour,
                );
                x += width + 6.0;
            };
            piece(&check.text, widths[0], theme::TEXT);
            if let Some(failed) = &failed {
                piece(failed, widths[1], theme::DANGER);
            }
            let mut hover = Vec::new();
            if cut {
                hover.push(match &failed {
                    Some(failed) => format!("{} {failed}", check.text),
                    None => check.text.clone(),
                });
            }
            if let Some((words, sentence)) =
                outcome.as_ref().and_then(|outcome| outcome.reason.as_ref())
            {
                hover.push(format!("{words}\n{sentence}"));
            }
            let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
            if !hover.is_empty() {
                return response.on_hover_text(hover.join("\n")).clicked();
            }
            return response.clicked();
        }
        response.clicked()
    }
}

/// The widths of texts side by side in `room`: whole while they fit, else the
/// later ones give way first, each down to a third of the room at most.
fn shared(widths: &[f32], room: f32) -> Vec<f32> {
    let mut widths = widths.to_vec();
    let mut over = widths.iter().sum::<f32>() - room.max(0.0);
    let least = (room / 3.0).max(0.0);
    for width in widths.iter_mut().rev() {
        if over <= 0.0 {
            break;
        }
        let give = (*width - least).clamp(0.0, over);
        *width -= give;
        over -= give;
    }
    // Four texts of a third do not fit: the last go.
    for width in widths.iter_mut().rev() {
        if over <= 0.0 {
            break;
        }
        let give = width.min(over);
        *width -= give;
        over -= give;
    }
    widths
}

/// The name of a row for assistive technology.
fn name(row: &Row, outcome: Option<&Outcome>) -> String {
    let mut name = match row.phase {
        Phase::Main => format!("Step {} · {}", row.number, row.title()),
        Phase::Cleanup => format!("Cleanup step {} · {}", row.number, row.title()),
    };
    if let Kind::Check { .. } = row.kind {
        name.push_str(" · ");
        name.push_str(&row.name);
    }
    if let Some(outcome) = outcome {
        name.push_str(" · ");
        name.push_str(&outcome.said());
    }
    name
}

/// What a row says beside its name for assistive technology: its branch, and
/// what the request reads, sends or sets.
fn description(row: &Row) -> Option<String> {
    let mut said: Vec<String> = row.branch.iter().map(|it| format!("in {it}")).collect();
    if let Kind::Request(request) = &row.kind {
        if !request.reads.is_empty() {
            said.push(format!("reads {}", request.reads.join(", ")));
        }
        match &request.column {
            Column::Sends(sends) => {
                let lines: Vec<&str> = sends.lines.iter().map(|line| &*line.text).collect();
                said.push(format!("sends {}", lines.join(", ")));
            }
            Column::Sets(lines) => said.push(format!("sets {}", lines.join(", "))),
        }
    }
    (!said.is_empty()).then(|| said.join(" · "))
}

/// The heights of a request's three columns.
fn heights(request: &Request) -> [f32; 3] {
    let lines = match &request.column {
        Column::Sends(sends) => sends.lines.len(),
        Column::Sets(lines) => lines.len(),
    };
    let (listed, more) = sends::cut(lines);
    let checks = if request.checks.is_empty() {
        TEXT_HEIGHT
    } else {
        request.checks.len() as f32 * CHECK_HEIGHT
    };
    [
        CAPTION_HEIGHT + request.reads.len().max(1) as f32 * TEXT_HEIGHT,
        CAPTION_HEIGHT + (listed + usize::from(more > 0)) as f32 * TEXT_HEIGHT,
        CAPTION_HEIGHT + checks,
    ]
}

/// A failed check's value, as Run results writes it.
fn failed_value(step: &StepResult) -> Option<String> {
    (step.kind == NodeKind::Assert && step.status == StepStatus::Failed)
        .then(|| report::actual_text(step))
}

/// The mark of a check: REP-07a's, or an empty circle without a result.
fn check_mark(ui: &Ui, centre: Pos2, result: Option<&StepResult>) {
    match result {
        Some(step) => report::mark(ui.painter(), centre, 6.5, step.status),
        None => {
            ui.painter()
                .circle_stroke(centre, 5.5, Stroke::new(1.2, theme::SOCKET));
        }
    }
}

/// A rounded word whose right middle is `at`.
fn pill(ui: &Ui, at: Pos2, words: &str, colour: Color32) -> Rect {
    let galley = ui.painter().layout_no_wrap(
        words.to_owned(),
        FontId::proportional(theme::FONT_META),
        colour,
    );
    let size = vec2(galley.size().x + 16.0, PILL_HEIGHT);
    let rect = Rect::from_min_size(pos2(at.x - size.x, at.y - size.y / 2.0), size);
    ui.painter()
        .rect_filled(rect, PILL_HEIGHT / 2.0, colour.gamma_multiply(0.12));
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, colour);
    rect
}

fn caption(ui: &Ui, id: egui::Id, column: Rect, words: &str, hover: Option<&str>) {
    let rect = Rect::from_min_size(column.min, vec2(column.width(), CAPTION_HEIGHT));
    text(
        ui,
        id.with("caption"),
        rect,
        words,
        caption_font(),
        theme::MUTED,
        hover,
    );
}

fn reads(ui: &Ui, id: egui::Id, request: &Request, column: Rect) {
    caption(ui, id.with("reads"), column, READS, None);
    let nothing = ["nothing".to_owned()];
    let (lines, colour) = if request.reads.is_empty() {
        (nothing.as_slice(), theme::MUTED)
    } else {
        (request.reads.as_slice(), theme::LINK)
    };
    for (index, line) in lines.iter().enumerate() {
        let rect = line_rect(column, index);
        text(
            ui,
            id.with(("read", index)),
            rect,
            line,
            mono(),
            colour,
            None,
        );
    }
}

fn line_rect(column: Rect, index: usize) -> Rect {
    Rect::from_min_size(
        pos2(
            column.left(),
            column.top() + CAPTION_HEIGHT + index as f32 * TEXT_HEIGHT,
        ),
        vec2(column.width(), TEXT_HEIGHT),
    )
}

/// The *Steps | Canvas* switch: the words side by side, the chosen one
/// filled. Returns the choice a click made.
pub fn switch(ui: &mut Ui, steps: bool) -> Option<bool> {
    let mut chosen = None;
    let (rect, _) = ui.allocate_exact_size(switch_size(ui), Sense::hover());
    ui.painter().rect(
        rect,
        6,
        theme::WHITE,
        Stroke::new(1.0, theme::BORDER),
        egui::StrokeKind::Inside,
    );
    let mut x = rect.left() + SWITCH_PAD;
    for (words, on) in [(STEPS, true), (CANVAS, false)] {
        let width = width_of(ui, words, switch_font()) + SEGMENT_PAD;
        let segment = Rect::from_min_size(
            pos2(x, rect.top() + SWITCH_PAD),
            vec2(width, rect.height() - 2.0 * SWITCH_PAD),
        );
        x += width;
        let response = ui.interact(
            segment,
            ui.id().with(("steps-switch", words)),
            Sense::click(),
        );
        let selected = steps == on;
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, words)
        });
        if selected {
            ui.painter().rect_filled(segment, 5, theme::BORDER);
        } else if response.hovered() {
            ui.painter().rect_filled(segment, 5, theme::FIELD);
        }
        if response.has_focus() {
            ui.painter().rect_stroke(
                segment,
                5,
                Stroke::new(1.5, theme::ACCENT),
                egui::StrokeKind::Inside,
            );
        }
        ui.painter().text(
            segment.center(),
            Align2::CENTER_CENTER,
            words,
            switch_font(),
            if selected { theme::TEXT } else { theme::MUTED },
        );
        let hover = if on {
            "The scenario as a numbered list of its steps"
        } else {
            "The scenario as a board of cards and wires"
        };
        if response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(hover)
            .clicked()
            && !selected
        {
            chosen = Some(on);
        }
    }
    chosen
}

const STEPS: &str = "Steps";
const CANVAS: &str = "Canvas";
const SWITCH_PAD: f32 = 2.0;
const SEGMENT_PAD: f32 = 20.0;

fn switch_font() -> FontId {
    FontId::proportional(theme::FONT_LABEL)
}

/// The room the switch takes.
pub fn switch_size(ui: &Ui) -> egui::Vec2 {
    let words = width_of(ui, STEPS, switch_font()) + width_of(ui, CANVAS, switch_font());
    vec2(words + 2.0 * (SEGMENT_PAD + SWITCH_PAD), 28.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts_side_by_side_give_way_from_the_last() {
        assert_eq!(shared(&[50.0, 60.0], 200.0), [50.0, 60.0]);
        assert_eq!(shared(&[50.0, 160.0], 180.0), [50.0, 130.0]);
        // Neither below a third while the other can give.
        assert_eq!(shared(&[200.0, 200.0], 300.0), [200.0, 100.0]);
        assert_eq!(shared(&[300.0, 300.0], 300.0), [200.0, 100.0]);
        assert_eq!(
            shared(&[80.0, 90.0, 80.0, 90.0], 180.0).iter().sum::<f32>(),
            180.0
        );
    }
}
