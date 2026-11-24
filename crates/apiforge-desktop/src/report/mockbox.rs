//! Mock-up of the comparison box of a check's details (review of REP-07b,
//! branch `mock/rep-07b-box`): today's box and three options, drawn with
//! static content for a comparison board. Not for merging.
use super::*;
use egui::text::{LayoutJob, TextFormat};
use std::sync::atomic::{AtomicU8, Ordering};

/// 0 today, 1 A, 2 B, 3 C.
pub static VARIANT: AtomicU8 = AtomicU8::new(0);

/// Mock of REP-07c: the message folded into the box, the pane opening on
/// the box and the response's first lines.
pub static FOLD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
thread_local! {
    pub static MESSAGE: std::cell::RefCell<Option<(String, Color32)>> = const { std::cell::RefCell::new(None) };
    pub static BOX: std::cell::Cell<Option<egui::Rect>> = const { std::cell::Cell::new(None) };
}

/// Height of the response's lines to keep in view, in points.
pub static LINES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(110);

pub fn fold() -> bool {
    FOLD.load(Ordering::Relaxed)
}

pub fn variant() -> u8 {
    VARIANT.load(Ordering::Relaxed)
}

/// One check of the board.
pub struct Case {
    pub predicate: &'static str,
    pub line: &'static str,
    pub checked: &'static str,
    /// The expected value as JSON.
    pub expected: &'static str,
    /// The actual value as JSON, or none where it is missing.
    pub actual: Option<&'static str>,
}

/// The head of a check's details as REP-07b draws it, and the box under it.
pub fn cell(ui: &mut Ui, case: &Case) {
    ui.horizontal(|ui| {
        let room = ui.available_width() - pill_width(ui, "Failed") - ui.spacing().item_spacing.x;
        ui.vertical(|ui| {
            ui.set_max_width(room.max(0.0));
            ui.add(
                egui::Label::new(
                    RichText::new(case.predicate)
                        .monospace()
                        .size(13.5)
                        .strong(),
                )
                .wrap(),
            );
            ui.label(
                RichText::new(case.line)
                    .size(theme::FONT_LABEL)
                    .color(theme::MUTED),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            widgets::pill(ui, "Failed", theme::DANGER);
        });
    });
    egui::Frame::new()
        .fill(theme::DANGER.gamma_multiply(0.08))
        .stroke(Stroke::new(1.0, theme::DANGER.gamma_multiply(0.25)))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new("The comparison failed.")
                    .size(12.0)
                    .color(theme::TEXT),
            );
        });
    let pretty = |text: &str| match serde_json::from_str::<Value>(text) {
        Ok(value) => compact_json(&value),
        Err(_) => text.to_owned(),
    };
    let actual = match case.actual {
        Some(text) => OptionalJson::Present(serde_json::from_str(text).unwrap()),
        None => OptionalJson::Missing,
    };
    if variant() == 0 {
        today_box(ui, Some(case.checked), &pretty(case.expected), &actual);
    } else {
        draw(ui, Some(case.checked), case.expected, &actual);
    }
}

/// What was checked without its step and `body`: the line above names the
/// step.
fn relative(checked: &str) -> &str {
    match checked.split_once(".body.") {
        Some((_, rest)) => rest,
        None => checked.split_once('.').map_or(checked, |(_, rest)| rest),
    }
}

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

/// The characters two texts begin with and end with alike.
fn alike(left: &str, right: &str) -> (usize, usize) {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    let prefix = left
        .iter()
        .zip(&right)
        .take_while(|(left, right)| left == right)
        .count();
    let most = left.len().min(right.len()) - prefix;
    let suffix = left
        .iter()
        .rev()
        .zip(right.iter().rev())
        .take_while(|(left, right)| left == right)
        .count()
        .min(most);
    (prefix, suffix)
}

fn mono(ui: &Ui) -> egui::FontId {
    egui::TextStyle::Monospace.resolve(ui.style())
}

/// `text` with the part that differs from `other` marked in `tint`, and
/// `kind` after it.
fn value_job(
    ui: &Ui,
    text: &str,
    other: Option<&str>,
    tint: Color32,
    kind: Option<&str>,
    rows: usize,
) -> LayoutJob {
    let font = mono(ui);
    let plain = TextFormat {
        font_id: font.clone(),
        color: theme::TEXT,
        ..Default::default()
    };
    let mut job = LayoutJob::default();
    match other {
        Some(other) if other != text => {
            let (prefix, suffix) = alike(text, other);
            let chars: Vec<char> = text.chars().collect();
            let end = chars.len() - suffix;
            let part = |range: std::ops::Range<usize>| chars[range].iter().collect::<String>();
            job.append(&part(0..prefix), 0.0, plain.clone());
            if prefix < end {
                job.append(
                    &part(prefix..end),
                    0.0,
                    TextFormat {
                        font_id: egui::FontId::new(font.size, theme::bold_mono_family()),
                        color: theme::TEXT,
                        background: tint.gamma_multiply(0.22),
                        underline: Stroke::new(1.0, tint),
                        ..Default::default()
                    },
                );
            }
            job.append(&part(end..chars.len()), 0.0, plain);
        }
        _ => job.append(text, 0.0, plain),
    }
    if let Some(kind) = kind {
        job.append(
            kind,
            10.0,
            TextFormat {
                font_id: egui::FontId::proportional(theme::FONT_META),
                color: theme::MUTED,
                ..Default::default()
            },
        );
    }
    job.wrap.max_rows = rows;
    job
}

/// The path with its parents dimmed and its last name strong; `separator`
/// stands between the names.
fn path_job(ui: &Ui, path: &str, separator: &str) -> LayoutJob {
    let font = mono(ui);
    let dim = TextFormat {
        font_id: font.clone(),
        color: theme::MUTED,
        ..Default::default()
    };
    let strong = TextFormat {
        font_id: egui::FontId::new(font.size, theme::bold_mono_family()),
        color: theme::TEXT,
        ..Default::default()
    };
    let names: Vec<&str> = path.split('.').collect();
    let mut job = LayoutJob::default();
    for (index, name) in names.iter().enumerate() {
        let last = index + 1 == names.len();
        if index > 0 {
            job.append(separator, 0.0, dim.clone());
        }
        job.append(name, 0.0, if last { strong.clone() } else { dim.clone() });
    }
    job
}

/// A caption in the gutter and what it names beside it.
fn row(ui: &mut Ui, caption: &str, job: LayoutJob) {
    const GUTTER: f32 = 70.0;
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(GUTTER, 16.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(GUTTER);
                ui.add_space(2.0);
                widgets::subcaption(ui, caption);
            },
        );
        let width = ui.available_width();
        ui.allocate_ui_with_layout(
            egui::vec2(width, 16.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(width);
                ui.add(egui::Label::new(job).wrap());
            },
        );
    });
}

/// Options A, B and C.
pub fn draw(ui: &mut Ui, checked: Option<&str>, expected: &str, actual: &OptionalJson) {
    let variant = variant();
    let one_line = |value: &Value| serde_json::to_string(value).unwrap();
    let expected_value = serde_json::from_str::<Value>(expected).ok();
    // An operand that reads as a value is compared for equality.
    let expected_text = expected_value
        .as_ref()
        .map_or_else(|| expected.to_owned(), one_line);
    let (actual_text, actual_value) = match actual {
        OptionalJson::Present(value) => (one_line(value), Some(value)),
        OptionalJson::Missing => ("(missing value)".to_owned(), None),
    };
    let marked = expected_value.is_some() && actual_value.is_some();
    let kinds = match (&expected_value, actual_value) {
        (Some(expected), Some(actual)) if kind(expected) != kind(actual) => {
            Some((kind(expected), kind(actual)))
        }
        _ => None,
    };
    let rows = if variant == 3 { 4 } else { 6 };
    let expected_job = |ui: &Ui| {
        value_job(
            ui,
            &expected_text,
            marked.then_some(actual_text.as_str()),
            theme::SUCCESS,
            kinds.map(|kinds| kinds.0),
            rows,
        )
    };
    let actual_job = |ui: &Ui| {
        let mut job = value_job(
            ui,
            &actual_text,
            marked.then_some(expected_text.as_str()),
            theme::DANGER,
            kinds.map(|kinds| kinds.1),
            rows,
        );
        if actual_value.is_none() {
            for section in &mut job.sections {
                section.format.color = theme::MUTED;
                section.format.italics = true;
            }
        }
        job
    };
    let message = MESSAGE.with(|message| message.borrow_mut().take());
    let (fill, stroke) = match &message {
        Some((_, color)) => (color.gamma_multiply(0.06), color.gamma_multiply(0.3)),
        None => (theme::FIELD, theme::BORDER),
    };
    let frame = egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, stroke))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 5.0;
            if let Some((message, _)) = &message {
                ui.label(RichText::new(message).size(12.0).color(theme::TEXT));
            }
            match variant {
                // A: two rows, no CHECKED.
                1 => {
                    row(ui, "EXPECTED", expected_job(ui));
                    row(ui, "ACTUAL", actual_job(ui));
                }
                // B: the field as a breadcrumb over the two rows.
                2 => {
                    if let Some(checked) = checked {
                        row(ui, "FIELD", path_job(ui, relative(checked), " › "));
                    }
                    row(ui, "EXPECTED", expected_job(ui));
                    row(ui, "ACTUAL", actual_job(ui));
                }
                // C: today's arrangement, wrapped and marked.
                _ => {
                    if let Some(checked) = checked {
                        widgets::subcaption(ui, "CHECKED");
                        // A name to a line where the path is longer than one.
                        let path = relative(checked);
                        let fits =
                            text_width(ui, path, mono(ui).size) * 1.25 <= ui.available_width();
                        let job = path_job(ui, path, if fits { "." } else { "\n." });
                        ui.add(egui::Label::new(job).wrap());
                        ui.add_space(2.0);
                    }
                    ui.horizontal_top(|ui| {
                        let width = ((ui.available_width() - 28.0) * 0.5).max(80.0);
                        for (caption, job, arrow) in [
                            ("EXPECTED", expected_job(ui), true),
                            ("ACTUAL", actual_job(ui), false),
                        ] {
                            ui.allocate_ui_with_layout(
                                egui::vec2(width, 36.0),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    ui.set_width(width);
                                    widgets::subcaption(ui, caption);
                                    ui.add(egui::Label::new(job).wrap());
                                },
                            );
                            if arrow {
                                ui.label(RichText::new("→").color(theme::MUTED));
                            }
                        }
                    });
                }
            }
        });
    BOX.with(|rect| rect.set(Some(frame.response.rect)));
}
