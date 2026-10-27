//! The *Migration stopped* window (KTLO-03): the first and only window of a
//! start whose legacy workspace migration refused. It says which project and
//! scenario, that nothing was activated, and offers the previous workspace or
//! quitting. Nothing opens by itself.
use crate::folder::Refusal;
use crate::icons::Icon;
use crate::{app, theme, widgets};
use eframe::egui::{self, RichText};

/// The window's size in points: a 432-point text column and 24-point margins.
pub const WINDOW_SIZE: egui::Vec2 = egui::vec2(480.0, 276.0);
const MARGIN: i8 = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    OpenPrevious,
    Quit,
}

pub struct StopWindow {
    refusal: Refusal,
    /// Why the previous workspace could not be opened, if it could not.
    error: Option<String>,
    choice: Option<Choice>,
}

impl StopWindow {
    pub fn new(refusal: Refusal) -> Self {
        Self {
            refusal,
            error: None,
            choice: None,
        }
    }

    /// Draws the window into the whole of `ui`, which the native window sizes
    /// to [`WINDOW_SIZE`]. The root `ui` has no margin, so the window sets its own.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::BG).inner_margin(MARGIN))
            .show(ui, |ui| self.contents(ui));
    }

    fn contents(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("Migration stopped")
                .size(theme::FONT_TITLE)
                .family(theme::bold_family()),
        );
        ui.add_space(theme::SPACE_SM);
        ui.add(egui::Label::new(message(&self.refusal)).wrap());
        ui.add_space(theme::SPACE_MD);
        let note = match &self.error {
            Some(error) => format!("The previous workspace could not be opened: {error}"),
            None => "The previous workspace and the staged folders were left untouched, \
                     and nothing was activated."
                .to_owned(),
        };
        ui.add(
            egui::Label::new(
                RichText::new(note)
                    .size(theme::FONT_LABEL)
                    .color(theme::MUTED),
            )
            .wrap(),
        );
        // The buttons sit on the window's last row, whatever the text's length:
        // right to left, aligned to the bottom of the space that remains.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Max), |ui| {
            if ui.add(widgets::action(Icon::Close, "Quit")).clicked() {
                self.choice = Some(Choice::Quit);
            }
            if ui
                .add(widgets::action(Icon::Folder, "Open the previous workspace"))
                .clicked()
            {
                self.choice = Some(Choice::OpenPrevious);
            }
        });
    }

    pub fn take_choice(&mut self) -> Option<Choice> {
        self.choice.take()
    }
}

/// The paragraph: which project, why, and which scenario.
fn message(refusal: &Refusal) -> String {
    let Some(project) = &refusal.project else {
        return format!(
            "API Forge could not move the previous workspace into folders. {}",
            refusal.reason
        );
    };
    let mut text = format!(
        "API Forge could not move the project \u{201c}{project}\u{201d} from the previous \
         workspace into its own folder: {}.",
        refusal.reason
    );
    if let Some(scenario) = &refusal.scenario {
        text.push_str(&format!(
            " The scenario \u{201c}{scenario}\u{201d} did not read back the same from the \
             migrated folder."
        ));
    }
    text
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

    /// Draws the window. *Quit* closes it; *Open the previous workspace*
    /// returns the workspace window, as `--legacy` opens it, and grows the
    /// native window to that window's size, centred on the screen.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<app::App> {
        self.window.ui(ui);
        match self.window.take_choice()? {
            Choice::Quit => {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                None
            }
            Choice::OpenPrevious => match (self.previous)() {
                Ok(source) => {
                    let context = ui.ctx();
                    for command in [
                        egui::ViewportCommand::Resizable(true),
                        egui::ViewportCommand::MinInnerSize(crate::MIN_WINDOW_SIZE.into()),
                        egui::ViewportCommand::InnerSize(crate::DEFAULT_WINDOW_SIZE.into()),
                    ] {
                        context.send_viewport_cmd(command);
                    }
                    // Centred again at its new size, as the stop window was, so
                    // it does not grow off the screen from the dialog's corner.
                    let size = egui::Vec2::from(crate::DEFAULT_WINDOW_SIZE);
                    if let Some(monitor) = context.input(|input| input.viewport().monitor_size) {
                        let corner = ((monitor - size) / 2.0).max(egui::Vec2::ZERO);
                        context.send_viewport_cmd(egui::ViewportCommand::OuterPosition(
                            corner.to_pos2(),
                        ));
                    }
                    Some(app::App::new(context, source))
                }
                Err(error) => {
                    self.window.error = Some(error);
                    None
                }
            },
        }
    }
}

#[cfg(test)]
#[path = "stop_tests.rs"]
mod tests;
