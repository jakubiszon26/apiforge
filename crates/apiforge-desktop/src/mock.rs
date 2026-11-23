//! Switches of the REQ check-making mock-ups (DIAG-12, DIAG-15, DIAG-16).
//! Mock branch only: never merged.
use crate::icons::Icon;
use crate::theme;
use eframe::egui;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering::Relaxed};

/// DIAG-15: 0 today, 1 a button and a menu, 2 the response in the tab.
pub static ADD_CHECK: AtomicU8 = AtomicU8::new(0);
/// DIAG-16: 0 today, 1 a row *Use*, 2 a path field, 3 the hint only.
pub static TYPED_PATH: AtomicU8 = AtomicU8::new(0);
/// DIAG-16: what the dialog opens with typed.
pub static PREFILL: AtomicBool = AtomicBool::new(false);
/// DIAG-12: 0 today, 1 the ✓ filled, 2 a chip, 3 a line at the edge.
pub static MARK: AtomicU8 = AtomicU8::new(0);
pub static RESULTS: AtomicBool = AtomicBool::new(false);
pub static OPEN: AtomicBool = AtomicBool::new(false);
pub static PORT: Mutex<String> = Mutex::new(String::new());

pub fn add_check() -> u8 {
    ADD_CHECK.load(Relaxed)
}
pub fn typed_path() -> u8 {
    TYPED_PATH.load(Relaxed)
}
pub fn mark_mode() -> u8 {
    MARK.load(Relaxed)
}

pub struct Mark {
    pub checks: &'static [&'static str],
    pub chip: &'static str,
    pub failed: bool,
    pub inside: bool,
}

impl Mark {
    pub fn colour(&self) -> egui::Color32 {
        if !RESULTS.load(Relaxed) {
            theme::LINK
        } else if self.failed {
            theme::DANGER
        } else {
            theme::SUCCESS
        }
    }
    pub fn words(&self) -> String {
        if self.inside {
            "1 check inside".to_owned()
        } else if RESULTS.load(Relaxed) && self.failed {
            "1 failed".to_owned()
        } else if self.checks.len() == 1 {
            "1 check".to_owned()
        } else {
            format!("{} checks", self.checks.len())
        }
    }
}

/// The checks of the board fixture that read the field at `path` of the
/// output being drawn.
pub fn mark(path: &str) -> Option<Mark> {
    if mark_mode() == 0 {
        return None;
    }
    let port = PORT.lock().unwrap().clone();
    let results = RESULTS.load(Relaxed);
    let one = |checks, chip| Mark {
        checks,
        chip,
        failed: false,
        inside: false,
    };
    Some(match (port.as_str(), path) {
        ("status", "") => one(&["status == 201"], "== 201"),
        ("body", "total") => one(&["total is \"number\"", "total == 172.5"], "2 checks"),
        ("body", "items") => one(&["nonempty items"], "nonempty"),
        ("body", "owner.name") => one(&["owner.name == \"Ann\""], "== \"Ann\""),
        ("body", "paid") => Mark {
            failed: results,
            ..one(&["paid == true"], "== true")
        },
        ("body", "owner") if !OPEN.load(Relaxed) => Mark {
            inside: true,
            ..one(&[], "1 inside")
        },
        _ => return None,
    })
}

/// The ✓ of a field a check reads: a filled square, or an outlined one on
/// a folded field whose children are read.
pub fn badge(painter: &egui::Painter, centre: egui::Pos2, mark: &Mark) {
    let rect = egui::Rect::from_center_size(centre, egui::vec2(18.0, 18.0));
    let colour = mark.colour();
    let icon = egui::Rect::from_center_size(centre, egui::vec2(13.0, 13.0));
    if mark.inside {
        painter.rect_filled(rect, 5.0, theme::WHITE);
        painter.rect_stroke(
            rect,
            5.0,
            egui::Stroke::new(1.4, colour),
            egui::StrokeKind::Inside,
        );
        painter.circle_filled(centre, 3.0, colour);
    } else {
        painter.rect_filled(rect, 5.0, colour);
        let icon_kind = if mark.failed && RESULTS.load(Relaxed) {
            Icon::Close
        } else {
            Icon::Check
        };
        icon_kind.paint(painter, icon, theme::WHITE);
    }
}

/// A chip saying what the check holds; returns its width.
pub fn chip(ui: &egui::Ui, right: f32, left: Option<f32>, middle: f32, mark: &Mark) -> f32 {
    let colour = mark.colour();
    let galley = ui.painter().layout_no_wrap(
        mark.chip.to_owned(),
        egui::FontId::monospace(theme::FONT_META),
        colour,
    );
    let width = galley.size().x + 22.0;
    let x = left.unwrap_or(right - width);
    let rect = egui::Rect::from_min_size(egui::pos2(x, middle - 9.0), egui::vec2(width, 18.0));
    // What the value loses to the chip.
    ui.painter().rect_filled(
        rect.expand2(egui::vec2(6.0, 3.0)),
        0.0,
        ui.visuals().panel_fill,
    );
    ui.painter()
        .rect_filled(rect, 9.0, colour.gamma_multiply(0.12));
    let icon = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 10.0, middle),
        egui::vec2(10.0, 10.0),
    );
    if mark.failed && RESULTS.load(Relaxed) {
        Icon::Close.paint(ui.painter(), icon, colour);
    } else {
        Icon::Check.paint(ui.painter(), icon, colour);
    }
    ui.painter().galley(
        egui::pos2(rect.left() + 17.0, middle - galley.size().y / 2.0),
        galley,
        colour,
    );
    width
}

/// A line at the row's left edge.
pub fn gutter(ui: &egui::Ui, row: egui::Rect, mark: &Mark) {
    let painter = ui
        .ctx()
        .layer_painter(ui.layer_id())
        .with_clip_rect(ui.clip_rect().expand(10.0));
    let rect = egui::Rect::from_min_max(
        egui::pos2(row.left() - 7.0, row.top() + 3.0),
        egui::pos2(row.left() - 4.0, row.bottom() - 3.0),
    );
    if mark.inside {
        painter.rect_stroke(
            rect,
            1.5,
            egui::Stroke::new(1.0, mark.colour()),
            egui::StrokeKind::Inside,
        );
    } else {
        painter.rect_filled(rect, 1.5, mark.colour());
    }
}

static MENUS: Mutex<Vec<(egui::Id, String, String, bool)>> = Mutex::new(Vec::new());

/// Remembers which field the button `id` stands on, for its menu.
pub fn remember(id: egui::Id, path: &str) {
    let port = PORT.lock().unwrap().clone();
    let mut menus = MENUS.lock().unwrap();
    menus.retain(|(known, ..)| *known != id);
    menus.push((id, port, path.to_owned(), RESULTS.load(Relaxed)));
}

pub fn menu_mark(id: egui::Id) -> Option<Mark> {
    let (port, path, results) = MENUS
        .lock()
        .unwrap()
        .iter()
        .find(|(known, ..)| *known == id)
        .map(|(_, port, path, results)| (port.clone(), path.clone(), *results))?;
    *PORT.lock().unwrap() = port;
    RESULTS.store(results, Relaxed);
    OPEN.store(true, Relaxed);
    mark(&path)
}
