//! WS-00 mock-up switches (throwaway branch `mock/ws-00`). The design review
//! views in `folder::ws00_review` turn them on for their own thread; with
//! every switch off the window behaves as on `main`.
#![allow(dead_code)]

use eframe::egui;
use std::cell::{Cell, RefCell};

/// Q1: one left sidebar (*Files* | *Library*) in Code, Diagram and Split.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Sidebar {
    /// As on `main`: Explorer in Code, Library | Files in Diagram, the block
    /// library alone in Split.
    #[default]
    Off,
    /// A: the same tabs everywhere; in Code the Library's blocks are shown
    /// disabled under the hint *Drag blocks onto the diagram*.
    LibraryDisabledInCode,
    /// B: the same tabs everywhere, but Code has no Library tab.
    LibraryHiddenInCode,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Flags {
    pub sidebar: Sidebar,
    /// Q2: in Split, the sidebar folds to its rail while the Inspector is
    /// open.
    pub fold_for_inspector: bool,
    /// Q3 A: the sidebar folds to its rail when the board would be narrower
    /// than [`MIN_BOARD`].
    pub fold_for_board: bool,
    /// Q3 B: the Inspector narrows to [`NARROW_INSPECTOR`] at the minimum
    /// window.
    pub narrow_inspector: bool,
    /// Q4: the *Add step* popup is capped at the board's height minus 24 px
    /// and its list scrolls under the search field.
    pub capped_popup: bool,
}

pub const MIN_BOARD: f32 = 480.0;
pub const NARROW_INSPECTOR: f32 = 280.0;
/// Windows narrower than this count as the minimum window for Q3 B.
pub const NARROW_WINDOW: f32 = 1100.0;

thread_local! {
    static FLAGS: RefCell<Flags> = RefCell::new(Flags::default());
    static BOARD: Cell<Option<egui::Rect>> = const { Cell::new(None) };
}

pub fn set(flags: Flags) {
    FLAGS.with(|cell| *cell.borrow_mut() = flags);
}

pub fn get() -> Flags {
    FLAGS.with(|cell| *cell.borrow())
}

pub fn sidebar() -> Sidebar {
    get().sidebar
}

/// The board's rect this frame, for the capped popup.
pub fn set_board(rect: egui::Rect) {
    BOARD.with(|cell| cell.set(Some(rect)));
}

pub fn board() -> Option<egui::Rect> {
    BOARD.with(Cell::get)
}

/// The Inspector's width at a window `width` wide.
pub fn inspector_width(width: f32) -> Option<f32> {
    (get().narrow_inspector && width < NARROW_WINDOW).then_some(NARROW_INSPECTOR)
}
