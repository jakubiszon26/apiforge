//! The *Migration stopped* window (KTLO-03): the first and only window of a
//! start whose legacy workspace migration refused. It says which project and
//! scenario, that nothing was activated, and offers the previous workspace or
//! quitting. Nothing opens by itself.
use crate::app;
use crate::folder::Refusal;
use eframe::egui;

/// The window's size in points: a 432-point text column and 24-point margins.
pub const WINDOW_SIZE: egui::Vec2 = egui::vec2(480.0, 276.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    OpenPrevious,
    Quit,
}

pub struct StopWindow {
    refusal: Refusal,
    choice: Option<Choice>,
}

impl StopWindow {
    pub fn new(refusal: Refusal) -> Self {
        Self {
            refusal,
            choice: None,
        }
    }

    pub fn refusal(&self) -> &Refusal {
        &self.refusal
    }

    /// Draws the window into the whole of `ui`.
    pub fn ui(&mut self, _ui: &mut egui::Ui) {}

    pub fn take_choice(&mut self) -> Option<Choice> {
        self.choice.take()
    }
}

/// The stop window and what *Open the previous workspace* opens.
pub struct Stopped {
    window: StopWindow,
    previous: fn() -> Result<app::Source, String>,
}

impl Stopped {
    pub fn new(refusal: Refusal, previous: fn() -> Result<app::Source, String>) -> Self {
        Self {
            window: StopWindow::new(refusal),
            previous,
        }
    }

    /// Draws the window; returns the workspace window when the person chose
    /// to open the previous workspace.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<app::App> {
        self.window.ui(ui);
        let _ = self.previous;
        None
    }
}

#[cfg(test)]
#[path = "stop_tests.rs"]
mod tests;
