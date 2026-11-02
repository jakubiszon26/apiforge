//! UX-00 mock-up switches (throwaway branch `mock/ux-00`). The design review
//! views in `folder::ux00_review` turn them on for their own thread; with
//! every switch off the window behaves as on `main`.
#![allow(dead_code)]

use eframe::egui::Color32;
use std::cell::{Cell, RefCell};

/// UX-02: the run mode the header's Run button runs.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum RunMode {
    /// As on `main`: *Run*, and *Run live requests* in *Run options*.
    #[default]
    Off,
    Simulation,
    Live,
}

/// UX-02: the colour of *Run simulation*.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum SimulationColour {
    /// The SIMULATION badge's colour in Run results, `theme::LINK`.
    #[default]
    Badge,
    /// A violet, for badge and button together.
    Violet,
}

/// UX-04: where *Running unsaved code* is shown.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Unsaved {
    /// As on `main`: a notice in the band under the header.
    #[default]
    Band,
    /// In the header, just left of the run controls.
    BesideRun,
    /// In the header, after the project name and its badges.
    BesideTitle,
}

/// UX-03: the status at the right of the PROBLEMS caption.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Status {
    /// As on `main`: red while the code is invalid.
    #[default]
    Current,
    /// The same words, never red.
    Muted,
    /// Only the save state, never red; problems are in the list below.
    SaveStateOnly,
}

/// UX-05: Run results dragged by its edge.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Drag {
    #[default]
    Off,
    /// Mid-drag, near the top: the drop zone shows.
    Mid,
    /// Released at the top: the panel fills the workspace, grip at its top.
    Full,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Flags {
    pub run_mode: RunMode,
    pub colour: SimulationColour,
    pub unsaved: Unsaved,
    pub status: Status,
    pub drag: Drag,
    /// UX-01: a check added from Run results keeps the results usable.
    pub checks_kept: bool,
    /// UX-02: *Run options* as a chevron beside the Run button.
    pub chevron_options: bool,
    /// UX-03: no focus line at the gutter's edge while the editor has focus.
    pub no_focus_line: bool,
}

pub const VIOLET: Color32 = Color32::from_rgb(112, 76, 182);

thread_local! {
    static FLAGS: Cell<Flags> = Cell::new(Flags::default());
    static CHECKED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

pub fn get() -> Flags {
    FLAGS.with(Cell::get)
}

pub fn set(flags: Flags) {
    FLAGS.with(|cell| cell.set(flags));
}

/// The colour of the simulation badge and of *Run simulation*.
pub fn simulation_colour() -> Color32 {
    match get().colour {
        SimulationColour::Badge => crate::theme::LINK,
        SimulationColour::Violet => VIOLET,
    }
}

fn key(node: &str, port: &str, path: &str) -> String {
    format!("{node}:{port}:{path}")
}

pub fn add_check(node: &str, port: &str, path: &str) {
    CHECKED.with(|checked| checked.borrow_mut().push(key(node, port, path)));
}

pub fn is_checked(node: &str, port: &str, path: &str) -> bool {
    CHECKED.with(|checked| checked.borrow().contains(&key(node, port, path)))
}

pub fn checks() -> usize {
    CHECKED.with(|checked| checked.borrow().len())
}

pub fn clear_checks() {
    CHECKED.with(|checked| checked.borrow_mut().clear());
}

pub fn checked() -> Vec<String> {
    CHECKED.with(|checked| checked.borrow().clone())
}
