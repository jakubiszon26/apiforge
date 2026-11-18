//! Off-screen relations (LAY-15): chips on the board's edge for the related
//! cards out of view, the jump to one of them and *Back*.
#![cfg_attr(not(test), allow(dead_code, unused_imports))]

use eframe::egui::Rect;

/// The edge of the board a chip stands on, towards its card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

/// An edge chip as the last frame drew it.
#[cfg(test)]
#[derive(Clone, Debug)]
pub(crate) struct DrawnEdge {
    /// What it says: `Source: rooms.body.items[1].id ↑`, `↑ 3 sources`.
    pub text: String,
    /// Its button's accessible name.
    pub name: String,
    pub side: Side,
    /// Its box on screen.
    pub rect: Rect,
    /// The cards it stands for: one, or the members of a folded chip.
    pub targets: Vec<String>,
}

impl super::Canvas {
    /// The edge chips the last frame drew.
    #[cfg(test)]
    pub(crate) fn debug_edge_chips(&self) -> Vec<DrawnEdge> {
        Vec::new()
    }

    /// *Back* as the last frame drew it: its text and its box on screen.
    #[cfg(test)]
    pub(crate) fn debug_back(&self) -> Option<(String, Rect)> {
        None
    }
}
