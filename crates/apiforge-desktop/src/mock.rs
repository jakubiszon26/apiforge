//! Switches of the Output data mock-ups (throwaway branch
//! `mock/output-data`): each picks the variant a review capture draws. 0 is
//! what `main` ships.
use eframe::egui::{self, Color32};
use serde_json::Value;
use std::sync::atomic::{AtomicU8, Ordering};

/// A row of the output tree: 1 one line with the value coloured by its
/// type, 2 one line with a column of type words, 3 the value as JSON text.
pub static ROWS: AtomicU8 = AtomicU8::new(0);
/// The row's actions: 0 on every row, 1 on the pointed row only.
pub static ACTIONS: AtomicU8 = AtomicU8::new(0);
/// The root: 0 a `$` row under the port's heading, 1 the heading is the row.
pub static ROOT: AtomicU8 = AtomicU8::new(0);
/// 1: the check button of a row opens DIAG-08's menu.
pub static MENU: AtomicU8 = AtomicU8::new(0);
/// 1: the long value is opened over several lines.
pub static LONG: AtomicU8 = AtomicU8::new(0);

/// The port's name as its heading shows it, for the row that replaces it.
pub static HEADING: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

pub fn get(switch: &AtomicU8) -> u8 {
    switch.load(Ordering::Relaxed)
}

#[cfg(test)]
pub fn set(switch: &AtomicU8, variant: u8) {
    switch.store(variant, Ordering::Relaxed);
}

/// The branches the captures show open besides the root.
pub fn opened(path: &str) -> bool {
    matches!(path, "items" | "items[0]")
}

/// The row the captures treat as pointed at.
pub fn pointed(path: &str) -> bool {
    path == "total"
}

pub fn value_colour(value: &Value) -> Color32 {
    match value {
        Value::String(_) => Color32::from_rgb(46, 125, 78),
        Value::Number(_) => Color32::from_rgb(176, 92, 40),
        Value::Bool(_) | Value::Null => Color32::from_rgb(112, 76, 182),
        _ => crate::theme::MUTED,
    }
}

/// Thin vertical guides, one per level of nesting, through `row`.
pub fn guides(ui: &egui::Ui, row: egui::Rect, depth: usize, step: f32) {
    for level in 0..depth {
        let x = row.left() + level as f32 * step + 7.0;
        ui.painter().line_segment(
            [egui::pos2(x, row.top() - 2.0), egui::pos2(x, row.bottom() + 2.0)],
            egui::Stroke::new(1.0, crate::theme::BORDER),
        );
    }
}
