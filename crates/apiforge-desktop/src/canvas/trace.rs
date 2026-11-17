//! Wire and input tracing (LAY-14).

#[cfg(test)]
use eframe::egui::Rect;

impl super::Canvas {
    /// The statements the last frame showed in boxes, with each box on
    /// screen.
    #[cfg(test)]
    pub(crate) fn debug_statements(&self) -> Vec<(String, Rect)> {
        Vec::new()
    }

    /// The board's area on screen in the last frame.
    #[cfg(test)]
    pub(crate) fn debug_board(&self) -> Rect {
        self.last_viewport
    }
}
