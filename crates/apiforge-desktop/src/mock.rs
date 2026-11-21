//! Switches of the 0.2.1 mock-ups (throwaway branch `mock/rel-0.2.1`): each
//! picks the variant a review capture draws. 0 is what v0.2.0 ships.
use std::sync::atomic::{AtomicU8, Ordering};

/// The selected card: 1 ink frame, 2 blue frame with a soft halo, 3 thin
/// accent frame; none has handles.
pub static SELECTION: AtomicU8 = AtomicU8::new(0);
/// The run mode menu: 1 whole rows, 2 commands, 3 a switch in the header.
pub static RUN_MENU: AtomicU8 = AtomicU8::new(0);
/// A check from a field of Output data: 1 a menu on the button, 2 a type
/// chip on the row.
pub static TYPE_CHECK: AtomicU8 = AtomicU8::new(0);

pub fn get(switch: &AtomicU8) -> u8 {
    switch.load(Ordering::Relaxed)
}

#[cfg(test)]
pub fn set(switch: &AtomicU8, variant: u8) {
    switch.store(variant, Ordering::Relaxed);
}

/// The colour of the selection and of the selected step's wires.
pub fn selection_colour() -> eframe::egui::Color32 {
    match get(&SELECTION) {
        1 => eframe::egui::Color32::from_rgb(38, 38, 38),
        2 => eframe::egui::Color32::from_rgb(44, 108, 214),
        _ => crate::theme::ACCENT,
    }
}
