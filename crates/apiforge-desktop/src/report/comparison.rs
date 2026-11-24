//! The two sides of a check, EXPECTED over ACTUAL in one column, each one
//! line of JSON wrapped to at most [`ROWS`] rows, what differs of a failed
//! equality marked (REP-07b, form A of the owner's decision of 2026-11-24).

use super::{
    JSON_SHOWN_CHARS, Requirement, StepRow, compact_json, detail::CheckDetail, expected_text,
    text_width,
};
use crate::{labels, not_run, theme, widgets};
use apiforge_core::{
    format::OptionalJson,
    graph::{VALUE_INPUT, condition_inputs},
    js::strict_equals,
    model::{NodeKind, Predicate},
    run::{StepResult, StepStatus},
};
use eframe::egui::{
    self, Color32, RichText, Stroke, Ui,
    text::{LayoutJob, TextFormat},
};
use serde_json::{Map, Value};
use std::{borrow::Cow, ops::Range, rc::Rc, sync::Arc};
use unicode_segmentation::GraphemeCursor;

/// The most rows a side takes; past that it ends with ….
const ROWS: usize = 6;
/// The most characters of a side that are laid out: more than [`ROWS`] rows
/// hold in any window.
const SHOWN_CHARS: usize = 4000;

/// One side as it is written.
#[derive(Debug)]
struct Side {
    /// What is laid out, at most [`SHOWN_CHARS`] characters.
    shown: String,
    /// How much of it is the line's; a … follows where the line was cut.
    kept: usize,
    /// What of it differs from the other side.
    mark: Option<Range<usize>>,
    /// The kind of its value, where the two kinds differ.
    kind: Option<&'static str>,
    missing: bool,
    /// The whole value, for the accessible name and the hover.
    whole: String,
}

impl Side {
    fn new(line: &str, whole: String, missing: bool) -> Self {
        let mut kept = line.len();
        if let Some((end, _)) = line.char_indices().nth(SHOWN_CHARS) {
            kept = end;
            while !boundary(line, kept) {
                kept -= 1;
            }
        }
        let mut shown = line[..kept].to_owned();
        if kept < line.len() {
            shown.push('…');
        }
        Self {
            shown,
            kept,
            mark: None,
            kind: None,
            missing,
            whole,
        }
    }

    /// What is laid out: before the mark, marked and after it.
    fn pieces(&self) -> (&str, &str, &str) {
        let mark = self.mark.clone().unwrap_or(0..0);
        (
            &self.shown[..mark.start],
            &self.shown[mark.clone()],
            &self.shown[mark.end..],
        )
    }

    /// Marks `range` of the line, as far as it is laid out.
    fn mark(&mut self, range: Range<usize>) {
        let range = range.start.min(self.kept)..range.end.min(self.kept);
        self.mark = (!range.is_empty()).then_some(range);
    }
}

/// The sides of the check shown, written for one run, row, status and
/// requirement.
pub(super) struct Kept {
    run: String,
    row: String,
    of: (StepStatus, Option<Requirement>),
    written: Rc<Comparison>,
}

impl Kept {
    /// The sides of `row`, written again when they are not `kept`'s.
    pub fn of(
        kept: &mut Option<Self>,
        run: &str,
        row: &StepRow<'_>,
        requirement: Option<Requirement>,
    ) -> Rc<Comparison> {
        let of = (row.step.status, requirement);
        if let Some(kept) = kept
            && kept.run == run
            && kept.row == row.key
            && kept.of == of
        {
            return Rc::clone(&kept.written);
        }
        let written = Rc::new(Comparison::of(requirement, row.step));
        *kept = Some(Self {
            run: run.to_owned(),
            row: row.key.clone(),
            of,
            written: Rc::clone(&written),
        });
        written
    }
}

/// The title of a check's details, its predicate in at most [`ROWS`] rows;
/// its hover says what it compared with every name, and the whole predicate
/// where the title is cut.
pub(super) fn title(ui: &mut Ui, check: &CheckDetail<'_>) {
    let mut job = LayoutJob::single_section(
        labels::truncate(&check.predicate, SHOWN_CHARS),
        TextFormat::simple(
            egui::FontId::monospace(13.5),
            ui.visuals().strong_text_color(),
        ),
    );
    job.wrap.max_width = ui.available_width();
    job.wrap.max_rows = ROWS;
    job.wrap.overflow_character = Some('…');
    let galley = ui.painter().layout_job(job);
    let cut = galley.elided;
    let title = ui.add(egui::Label::new(galley).show_tooltip_when_elided(false));
    let title = widgets::named(title, check.predicate.as_str());
    if check.checked.is_some() || cut {
        title.on_hover_ui(|ui| {
            if let Some(checked) = &check.checked {
                ui.label(checked);
            }
            if cut {
                ui.label(labels::truncate(&check.predicate, JSON_SHOWN_CHARS));
            }
        });
    }
}

/// The two sides of one check, written once, not every frame.
#[derive(Debug)]
pub(super) struct Comparison {
    expected: Side,
    actual: Side,
}

impl Comparison {
    /// `None` for `requirement` is a step whose node is not found.
    pub fn of(requirement: Option<Requirement>, step: &StepResult) -> Self {
        let predicate = match requirement {
            Some(Requirement::One { predicate, .. }) => Some(predicate),
            _ => None,
        };
        // A Data condition that passes a value on reports that value, not
        // the one it compared.
        let compared = step.kind != NodeKind::Condition || !step.inputs.contains_key(VALUE_INPUT);
        let sides = sides(requirement, step);
        let (step_expected, step_actual) = (&*sides.0, &*sides.1);
        let values = match (step_expected, step_actual) {
            (OptionalJson::Present(expected), OptionalJson::Present(actual)) if compared => {
                Some((expected, actual))
            }
            _ => None,
        };
        // Only a failed equality marks what differs; the members of an
        // object are compared by name, so both sides have them in the order
        // received.
        let differ = values.filter(|(expected, actual)| {
            predicate == Some(Predicate::Equals)
                && (step.status == StepStatus::Failed || not_run::unmet(step))
                && !strict_equals(Some(expected), Some(actual))
        });
        let written = match differ {
            Some((expected, actual)) => {
                Cow::Owned(OptionalJson::Present(aligned(expected, actual)))
            }
            None => Cow::Borrowed(step_expected),
        };
        let line = match requirement {
            Some(requirement) => requirement.text_with(&written, one_line),
            None => super::requirement_text(Predicate::Equals, &written, 0.0, one_line),
        };
        let whole = match requirement {
            Some(requirement) => requirement.text(step_expected),
            None => expected_text(Predicate::Equals, step_expected, 0.0),
        };
        let mut expected = Side::new(&line, whole, false);
        let (actual_line, whole, missing) = match step_actual {
            OptionalJson::Present(value) => (one_line(value), compact_json(value), false),
            OptionalJson::Missing => ("(missing value)".into(), "(missing value)".into(), true),
        };
        let mut actual = Side::new(&actual_line, whole, missing);
        if differ.is_some() {
            let (left, right) = difference(&line, &actual_line);
            expected.mark(left);
            actual.mark(right);
        }
        // `is` names a type and has no value on its expected side.
        if let Some((left, right)) = values
            && predicate
                .is_some_and(|predicate| predicate.takes_expected() && predicate != Predicate::Type)
            && kind(left) != kind(right)
        {
            expected.kind = Some(kind(left));
            actual.kind = Some(kind(right));
        }
        Self { expected, actual }
    }

    /// The box, as wide as the details; `from` is where the expected value
    /// came from.
    pub fn show(&self, ui: &mut Ui, from: Option<&str>) {
        egui::Frame::new()
            .fill(theme::FIELD)
            .stroke(Stroke::new(1.0, theme::BORDER))
            .corner_radius(6)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let column = ["EXPECTED", "ACTUAL"]
                    .into_iter()
                    .map(|caption| text_width(ui, caption, theme::FONT_META))
                    .fold(0.0, f32::max)
                    + 16.0;
                row(ui, "EXPECTED", column, &self.expected, from, theme::SUCCESS);
                row(ui, "ACTUAL", column, &self.actual, None, theme::DANGER);
            });
    }
}

/// Whether the run ended a comparison because one side had no value: the
/// step keeps neither side, and its inputs hold the other.
pub(super) fn missing(requirement: Option<Requirement>, step: &StepResult) -> bool {
    let takes = matches!(
        requirement,
        Some(Requirement::One { predicate, .. }) if predicate.takes_expected()
    );
    let (actual, expected) = inputs(step);
    takes
        && step.status == StepStatus::Error
        && step.expected.is_missing()
        && step.actual.is_missing()
        && step.inputs.contains_key(&actual) != step.inputs.contains_key(&expected)
}

/// The inputs of a check's one comparison.
fn inputs(step: &StepResult) -> (String, String) {
    match step.kind {
        NodeKind::Condition => condition_inputs(1),
        _ => ("actual".into(), "expected".into()),
    }
}

/// The expected and the actual side of `step`.
fn sides(
    requirement: Option<Requirement>,
    step: &StepResult,
) -> (Cow<'_, OptionalJson>, Cow<'_, OptionalJson>) {
    if !missing(requirement, step) {
        return (Cow::Borrowed(&step.expected), Cow::Borrowed(&step.actual));
    }
    let (actual, expected) = inputs(step);
    let side = |name: &str| {
        Cow::Owned(match step.inputs.get(name) {
            Some(value) => OptionalJson::Present(value.clone()),
            None => OptionalJson::Missing,
        })
    };
    (side(&expected), side(&actual))
}

fn row(ui: &mut Ui, caption: &str, column: f32, side: &Side, from: Option<&str>, tone: Color32) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.allocate_ui_with_layout(
            egui::vec2(column, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(column);
                ui.add_space(1.0);
                widgets::subcaption(ui, caption);
            },
        );
        ui.vertical(|ui| value(ui, side, from, tone));
    });
}

fn value(ui: &mut Ui, side: &Side, from: Option<&str>, tone: Color32) {
    let width = ui.available_width();
    let galley = ui.painter().layout_job(job(ui, side, tone, width));
    let last = galley.rows.last().map(|row| row.rect());
    let response = ui.add(egui::Label::new(Arc::clone(&galley)).show_tooltip_when_elided(false));
    // Named by its whole text, which the label may cut.
    let response = widgets::named(response, side.whole.as_str()).on_hover_ui(|ui| {
        ui.label(labels::truncate(&side.whole, JSON_SHOWN_CHARS));
    });
    let note = match (side.kind, from) {
        (None, None) => return,
        (Some(kind), None) => kind.to_owned(),
        (None, Some(from)) => format!("from {from}"),
        (Some(kind), Some(from)) => format!("{kind} · from {from}"),
    };
    // Beside the end of the value, or under it where that has no room.
    let text = RichText::new(&note)
        .size(theme::FONT_META)
        .color(theme::MUTED);
    let size = egui::vec2(text_width(ui, &note, theme::FONT_META), 0.0);
    match last {
        Some(last) if last.right() + 10.0 + size.x <= width => {
            let at = response.rect.min + egui::vec2(last.right() + 10.0, last.top());
            let rect = egui::Rect::from_min_size(at, egui::vec2(size.x + 1.0, last.height()));
            ui.put(rect, egui::Label::new(text).extend());
        }
        _ => {
            ui.label(text);
        }
    }
}

fn job(ui: &Ui, side: &Side, tone: Color32, width: f32) -> LayoutJob {
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let plain = TextFormat {
        font_id: font.clone(),
        color: if side.missing {
            theme::MUTED
        } else {
            theme::TEXT
        },
        italics: side.missing,
        ..Default::default()
    };
    let marked = TextFormat {
        font_id: egui::FontId::new(font.size, theme::bold_mono_family()),
        color: theme::TEXT,
        background: tint(tone),
        expand_bg: 0.0,
        underline: Stroke::new(1.5, underline(tone)),
        ..Default::default()
    };
    let (before, differs, after) = side.pieces();
    let mut job = LayoutJob::default();
    for (text, format) in [(before, &plain), (differs, &marked), (after, &plain)] {
        if !text.is_empty() {
            job.append(text, 0.0, format.clone());
        }
    }
    job.wrap.max_width = width;
    job.wrap.max_rows = ROWS;
    job.wrap.overflow_character = Some('…');
    job
}

/// The colour behind what differs. A wrapped row's is drawn over the line
/// under the row above, so it lets that line through.
fn tint(tone: Color32) -> Color32 {
    tone.gamma_multiply(0.25)
}

/// The line under what differs: darker than its tone, so that it stands out
/// of the tint.
fn underline(tone: Color32) -> Color32 {
    tone.lerp_to_gamma(theme::TEXT, 0.35)
}

/// A value on one line.
fn one_line(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// The kind of a value, in the words of the owner's board.
fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "true or false",
        Value::Number(_) => "number",
        Value::String(_) => "text",
        Value::Array(_) => "list",
        Value::Object(_) => "object",
    }
}

/// `value` with the members of its objects in the order `other` has them,
/// those it has alone last.
fn aligned(value: &Value, other: &Value) -> Value {
    match (value, other) {
        (Value::Object(fields), Value::Object(others)) => {
            let mut result = Map::new();
            for (name, other) in others {
                if let Some(field) = fields.get(name) {
                    result.insert(name.clone(), aligned(field, other));
                }
            }
            for (name, field) in fields {
                if !others.contains_key(name) {
                    result.insert(name.clone(), field.clone());
                }
            }
            Value::Object(result)
        }
        (Value::Array(items), Value::Array(others)) => Value::Array(
            items
                .iter()
                .enumerate()
                .map(|(index, item)| match others.get(index) {
                    Some(other) => aligned(item, other),
                    None => item.clone(),
                })
                .collect(),
        ),
        _ => value.clone(),
    }
}

/// What of each text differs: from the first character that differs to the
/// last, no character and no escape split.
fn difference(left: &str, right: &str) -> (Range<usize>, Range<usize>) {
    let mut prefix = left
        .bytes()
        .zip(right.bytes())
        .take_while(|(left, right)| left == right)
        .count();
    while !(boundary(left, prefix) && boundary(right, prefix)) {
        prefix -= 1;
    }
    let most = left.len().min(right.len()) - prefix;
    let mut suffix = left
        .bytes()
        .rev()
        .zip(right.bytes().rev())
        .take(most)
        .take_while(|(left, right)| left == right)
        .count();
    while !(boundary(left, left.len() - suffix) && boundary(right, right.len() - suffix)) {
        suffix -= 1;
    }
    (prefix..left.len() - suffix, prefix..right.len() - suffix)
}

/// Whether `at` is between two characters as they are read (grapheme
/// clusters) and inside no escape of JSON.
fn boundary(text: &str, at: usize) -> bool {
    text.is_char_boundary(at)
        && !inside_escape(text.as_bytes(), at)
        && GraphemeCursor::new(at, text.len(), true)
            .is_boundary(text, 0)
            .unwrap_or(true)
}

/// Whether `at` is after the backslash of `\n` or of `\u0000` and before
/// its end.
fn inside_escape(text: &[u8], at: usize) -> bool {
    (1..=5).any(|back| {
        let Some(start) = at.checked_sub(back) else {
            return false;
        };
        // A backslash after an odd number of them is escaped itself.
        let opens = text[..=start.min(text.len() - 1)]
            .iter()
            .rev()
            .take_while(|byte| **byte == b'\\')
            .count()
            % 2
            == 1;
        opens && (back == 1 || text.get(start + 1) == Some(&b'u'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn differs<'a>(left: &'a str, right: &'a str) -> (&'a str, &'a str) {
        let (a, b) = difference(left, right);
        (&left[a], &right[b])
    }

    #[test]
    fn a_difference_splits_no_character_and_no_escape() {
        assert_eq!(differs("abc", "axc"), ("b", "x"));
        assert_eq!(differs("25", "250"), ("", "0"));
        assert_eq!(differs("", "a"), ("", "a"));
        assert_eq!(differs("aa", "aaa"), ("", "a"));
        assert_eq!(differs("same", "same"), ("", ""));
        // One byte of two is the same.
        assert_eq!(differs("é", "è"), ("é", "è"));
        // A letter with and without a combining accent.
        assert_eq!(differs("xey", "xe\u{301}y"), ("e", "e\u{301}"));
        // Flags share their first letter.
        assert_eq!(differs("🇵🇱", "🇵🇹"), ("🇵🇱", "🇵🇹"));
        assert_eq!(differs(r#""a\nb""#, r#""a\tb""#), (r"\n", r"\t"));
        assert_eq!(
            differs(r#""a\u0001b""#, r#""a\u0002b""#),
            (r"\u0001", r"\u0002")
        );
        // An escaped backslash, then a letter.
        assert_eq!(differs(r#""\\n""#, r#""\\t""#), ("n", "t"));
        assert_eq!(differs(r#""\\u1""#, r#""\\u2""#), ("1", "2"));
    }

    fn step(expected: Value, actual: Value) -> StepResult {
        let at = apiforge_core::format::Position { x: 0.0, y: 0.0 };
        let node = apiforge_core::graph::create_node(NodeKind::Assert, at, None);
        let mut step = StepResult::new(&node, apiforge_core::run::Phase::Main);
        step.status = StepStatus::Failed;
        step.expected = OptionalJson::Present(expected);
        step.actual = OptionalJson::Present(actual);
        step
    }

    fn one(predicate: Predicate) -> Option<Requirement> {
        Some(Requirement::One {
            predicate,
            tolerance: 0.0,
        })
    }

    fn marks(comparison: &Comparison) -> [Option<&str>; 2] {
        [&comparison.expected, &comparison.actual]
            .map(|side| side.mark.clone().map(|mark| &side.shown[mark]))
    }

    #[test]
    fn members_in_another_order_are_no_difference() {
        let left = json!({"a": [{"x": 1, "y": 2}], "b": 3, "only": 4});
        let right = json!({"b": 3, "a": [{"y": 5, "x": 1}], "more": 6});
        let comparison = Comparison::of(one(Predicate::Equals), &step(left.clone(), right));
        assert_eq!(
            comparison.expected.shown,
            r#"{"b":3,"a":[{"y":2,"x":1}],"only":4}"#
        );
        assert_eq!(
            comparison.expected.whole,
            serde_json::to_string_pretty(&left).unwrap(),
            "the name and the hover are the value as written"
        );
    }

    #[test]
    fn only_a_failed_equality_of_two_values_is_marked() {
        let failed = step(json!(30), json!(25));
        assert_eq!(
            marks(&Comparison::of(one(Predicate::Equals), &failed)),
            [Some("30"), Some("25")]
        );
        for predicate in [
            Predicate::NotEquals,
            Predicate::Greater,
            Predicate::Less,
            Predicate::Approximately,
            Predicate::Contains,
            Predicate::Type,
            Predicate::Exists,
            Predicate::NotEmpty,
        ] {
            assert_eq!(
                marks(&Comparison::of(one(predicate), &failed)),
                [None, None],
                "{predicate:?}"
            );
        }
        let several = Requirement::Several {
            count: 2,
            mode: apiforge_core::model::Match::All,
        };
        for requirement in [None, Some(several)] {
            assert_eq!(marks(&Comparison::of(requirement, &failed)), [None, None]);
        }
        let mut passed = failed.clone();
        passed.status = StepStatus::Passed;
        assert_eq!(
            marks(&Comparison::of(one(Predicate::Equals), &passed)),
            [None, None],
            "an if that took its else is no failure"
        );
        let mut missing = failed.clone();
        missing.actual = OptionalJson::Missing;
        let comparison = Comparison::of(one(Predicate::Equals), &missing);
        assert_eq!(marks(&comparison), [None, None]);
        assert!(comparison.actual.missing);
        assert_eq!(comparison.actual.shown, "(missing value)");
        // A secret of both sides is written alike.
        let redacted = step(json!("[redacted]"), json!("[redacted]"));
        assert_eq!(
            marks(&Comparison::of(one(Predicate::Equals), &redacted)),
            [None, None]
        );
        // A Data condition not met, unless it reports a value passed on.
        let mut unmet = failed.clone();
        unmet.kind = NodeKind::Condition;
        unmet.status = StepStatus::Blocked;
        assert_eq!(
            marks(&Comparison::of(one(Predicate::Equals), &unmet)),
            [Some("30"), Some("25")]
        );
        unmet.inputs.insert(VALUE_INPUT.into(), json!(25));
        assert_eq!(
            marks(&Comparison::of(one(Predicate::Equals), &unmet)),
            [None, None]
        );
    }

    #[test]
    fn a_comparison_without_one_value_shows_the_other() {
        let mut step = step(json!(1), json!(1));
        step.status = StepStatus::Error;
        step.expected = OptionalJson::Missing;
        step.actual = OptionalJson::Missing;
        let equals = one(Predicate::Equals);
        assert!(!missing(equals, &step), "neither side is known");
        step.inputs.insert("expected".into(), json!("a"));
        assert!(missing(equals, &step));
        assert!(!missing(one(Predicate::Exists), &step));
        assert!(!missing(None, &step));
        let comparison = Comparison::of(equals, &step);
        assert_eq!(comparison.expected.shown, r#""a""#);
        assert_eq!(comparison.actual.shown, "(missing value)");
        assert_eq!(marks(&comparison), [None, None]);
        step.inputs.insert("actual".into(), json!("b"));
        assert!(!missing(equals, &step), "another error");
    }

    #[test]
    fn the_kinds_are_named_where_they_differ() {
        let kinds = |predicate, left, right| {
            let comparison = Comparison::of(one(predicate), &step(left, right));
            (comparison.expected.kind, comparison.actual.kind)
        };
        assert_eq!(
            kinds(Predicate::Equals, json!(31), json!("31")),
            (Some("number"), Some("text"))
        );
        assert_eq!(
            kinds(Predicate::Equals, json!([]), json!({})),
            (Some("list"), Some("object"))
        );
        assert_eq!(
            kinds(Predicate::Less, json!(true), json!(null)),
            (Some("true or false"), Some("null"))
        );
        assert_eq!(kinds(Predicate::Equals, json!(1), json!(2)), (None, None));
        assert_eq!(
            kinds(Predicate::Type, json!("number"), json!(1)),
            (None, None)
        );
    }

    #[test]
    fn a_very_long_value_is_laid_out_in_part() {
        let long = "ż".repeat(4_000_000);
        let comparison = Comparison::of(
            one(Predicate::Equals),
            &step(json!(format!("a{long}")), json!(format!("b{long}"))),
        );
        for side in [&comparison.expected, &comparison.actual] {
            assert_eq!(side.shown.chars().count(), SHOWN_CHARS + 1);
            assert!(side.shown.ends_with('…'));
            assert_eq!(side.whole.chars().count(), 4_000_003);
        }
        assert_eq!(marks(&comparison), [Some("a"), Some("b")]);
        let comparison = Comparison::of(
            one(Predicate::Equals),
            &step(json!(format!("{long}a")), json!(format!("{long}b"))),
        );
        assert_eq!(marks(&comparison), [None, None], "past what is laid out");
        // A combining character after the last one laid out is not split.
        let text = format!("{}e\u{301}xx", "a".repeat(SHOWN_CHARS - 2));
        let side = Side::new(&one_line(&json!(text)), String::new(), false);
        assert!(side.shown.ends_with("aa…"), "{}", &side.shown[3990..]);
    }

    /// Second test in the real window, 2026-11-24, finding 3: the tint of a
    /// row stands under the line of the row above, and covers none of it.
    #[test]
    fn a_rows_tint_leaves_the_line_above_it() {
        for tone in [theme::SUCCESS, theme::DANGER] {
            assert!(!tint(tone).is_opaque());
        }
    }

    /// Review of #613: what is cut off may be as long as the … in its place.
    #[test]
    fn a_mark_stays_within_what_is_laid_out() {
        let check = |left: &str, right: &str| {
            let comparison =
                Comparison::of(one(Predicate::Equals), &step(json!(left), json!(right)));
            for side in [&comparison.expected, &comparison.actual] {
                let line = serde_json::from_str::<String>(&side.whole)
                    .map(|text| one_line(&json!(text)))
                    .unwrap();
                let kept = match line.chars().count() > SHOWN_CHARS {
                    true => side.shown.strip_suffix('…').expect("it is cut"),
                    false => side.shown.as_str(),
                };
                assert!(line.starts_with(kept), "what is kept is its beginning");
                let (before, marked, after) = side.pieces();
                assert_eq!(format!("{before}{marked}{after}"), side.shown);
                assert!(
                    before.len() + marked.len() <= kept.len(),
                    "{} and {} of {}",
                    before.len(),
                    marked.len(),
                    kept.len()
                );
            }
        };
        let a = "a".repeat(SHOWN_CHARS - 1);
        check(&format!("{a}é"), &format!("{a}è"));
        let units = [
            "b",
            "é",
            "€",
            "😀",
            "e\u{301}",
            "🇵🇱",
            "👩‍💻",
            "\n",
            "\u{1}",
            "\\",
            "\"",
        ];
        for filler in ["a", "€"] {
            for length in SHOWN_CHARS - 5..=SHOWN_CHARS + 1 {
                let text = filler.repeat(length);
                for (at, left) in units.iter().enumerate() {
                    let right = units[(at + 1) % units.len()];
                    check(&format!("{text}{left}"), &format!("{text}{right}"));
                    check(&format!("{text}{left}{right}"), &format!("{text}{right}"));
                }
            }
        }
    }

    #[test]
    fn the_marks_are_read_on_their_tint() {
        fn luminance(colour: Color32) -> f32 {
            let linear = |channel: u8| {
                let value = f32::from(channel) / 255.0;
                if value <= 0.04045 {
                    value / 12.92
                } else {
                    ((value + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(colour.r()) + 0.7152 * linear(colour.g()) + 0.0722 * linear(colour.b())
        }
        let contrast = |a: Color32, b: Color32| {
            let (a, b) = (luminance(a), luminance(b));
            (a.max(b) + 0.05) / (a.min(b) + 0.05)
        };
        for tone in [theme::SUCCESS, theme::DANGER] {
            let tint = theme::FIELD.blend(tint(tone));
            assert!(contrast(theme::TEXT, tint) >= 4.5);
            assert!(contrast(underline(tone), tint) >= 3.0);
        }
        assert!(contrast(theme::MUTED, theme::FIELD) >= 4.5);
    }
}
