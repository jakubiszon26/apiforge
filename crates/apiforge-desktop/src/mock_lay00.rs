//! LAY-00 round-3 mock-up switches (throwaway branch `mock/lay-00-round-3`).
//! The design review views in `folder::lay00r3_review` turn them on for their
//! own thread; with every switch off the window behaves as on `main`.
#![allow(dead_code)]

use apiforge_core::run::StepResult;
use eframe::egui::Rect;
use std::cell::RefCell;
use std::collections::HashMap;

/// Where the camera is placed by hand.
#[derive(Clone, Debug)]
pub enum Anchor {
    /// The top-left corner of these spine rows (0-based, inclusive).
    Rows(usize, usize),
    /// Centred on these cards (node ids).
    Cards(Vec<String>),
}

#[derive(Clone, Default, Debug)]
pub struct Flags {
    /// Shape D with every round-3 proposal.
    pub on: bool,
    /// Checks with a subject are folded into their subjects' aggregates.
    pub collapsed: bool,
    /// The camera: this zoom on this anchor; `None` fits the whole board.
    pub view: Option<(f32, Anchor)>,
    /// Columns per row of the spine.
    pub columns: usize,
    /// A run in progress: this step runs.
    pub running: Option<String>,
    /// The hovered wire (its id), traced.
    pub trace: Option<String>,
    /// Tinted bands behind the branches of an `if`.
    pub bands: bool,
    /// The Inspector lists the checks of this step.
    pub inspector: Option<String>,
    /// The router's clearance from cards it passes.
    pub clearance: Option<f32>,
    /// Steps that show no result although the run has finished.
    pub not_run: Vec<String>,
    /// A row of the Inspector's checks list under the pointer: the check.
    pub hover_check: Option<String>,
    /// The input row under the pointer: card and input; traces its wire.
    pub trace_row: Option<(String, String)>,
    /// Off-screen markers for the selection's related cards.
    pub markers: bool,
    /// The person jumped away from this step: a chip leads back.
    pub jumped_from: Option<String>,
}

thread_local! {
    static FLAGS: RefCell<Flags> = RefCell::new(Flags::default());
    static STEPS: RefCell<Option<Vec<StepResult>>> = const { RefCell::new(None) };
    static RECTS: RefCell<HashMap<String, Rect>> = RefCell::new(HashMap::new());
    static INSPECTOR: std::cell::Cell<Option<Rect>> = const { std::cell::Cell::new(None) };
}

pub fn get() -> Flags {
    FLAGS.with(|flags| flags.borrow().clone())
}

pub fn on() -> bool {
    FLAGS.with(|flags| flags.borrow().on)
}

pub fn set(flags: Flags) {
    FLAGS.with(|cell| *cell.borrow_mut() = flags);
}

pub fn update(change: impl FnOnce(&mut Flags)) {
    FLAGS.with(|cell| change(&mut cell.borrow_mut()));
}

/// Fake step results the board shows instead of the document's run.
pub fn set_steps(steps: Option<Vec<StepResult>>) {
    STEPS.with(|cell| *cell.borrow_mut() = steps);
}

pub fn steps() -> Option<Vec<StepResult>> {
    STEPS.with(|cell| cell.borrow().clone())
}

/// Where each card was drawn last frame, in points.
pub fn record(id: &str, rect: Rect) {
    RECTS.with(|cell| {
        cell.borrow_mut().insert(id.to_owned(), rect);
    });
}

pub fn rect_of(id: &str) -> Option<Rect> {
    RECTS.with(|cell| cell.borrow().get(id).copied())
}

pub fn clear_rects() {
    RECTS.with(|cell| cell.borrow_mut().clear());
}

pub fn set_inspector(rect: Option<Rect>) {
    INSPECTOR.with(|cell| cell.set(rect));
}

pub fn inspector_rect() -> Option<Rect> {
    INSPECTOR.with(std::cell::Cell::get)
}

/// The router's parameters, with the mock-up's clearance when set.
pub fn routing() -> apiforge_layout::Routing {
    let mut routing = apiforge_layout::Routing::default();
    if let Some(clearance) = get().clearance {
        routing.margin = clearance;
    }
    routing
}
