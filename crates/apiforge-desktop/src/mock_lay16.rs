//! LAY-16 mock-up switches (throwaway branch `mock/lay-16-inputs`): the
//! compact request card and the ways to reach the ports it does not show. With
//! the option `Today` the window behaves as on `main`.
#![allow(dead_code)]

use eframe::egui::{Pos2, Rect, Vec2};
use std::cell::RefCell;
use std::collections::HashMap;

/// What a compact card offers for the ports it hides.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Option_ {
    /// Today's geometry.
    #[default]
    Today,
    /// Hidden sockets on the card's edge while a wire is dragged or the card
    /// is selected; the box is unchanged.
    A,
    /// Nothing: wiring a hidden port needs *Compact cards* off or the
    /// Inspector.
    B,
    /// Every input row stays, and the first output row; `+` and *Next* go.
    C,
    /// A labelled tray beside the card's edge, outside the box, while a wire
    /// is dragged near the card or the card is selected.
    D,
}

#[derive(Clone, Debug, Default)]
pub struct Flags {
    pub option: Option_,
    /// The camera: this zoom, centred on these cards.
    pub camera: Option<(f32, Vec<String>)>,
    /// Cards moved for the view: id and world origin.
    pub place: Vec<(String, Pos2)>,
}

/// Where a card was drawn last frame: its screen rect and the camera.
#[derive(Clone, Copy, Debug)]
pub struct Drawn {
    pub rect: Rect,
    pub zoom: f32,
    pub pan: Vec2,
}

impl Drawn {
    pub fn to_screen(&self, world: Pos2) -> Pos2 {
        Pos2::new(world.x * self.zoom, world.y * self.zoom) + self.pan
    }
}

thread_local! {
    static FLAGS: RefCell<Flags> = RefCell::new(Flags::default());
    static DRAWN: RefCell<HashMap<String, Drawn>> = RefCell::new(HashMap::new());
    static WORLD: RefCell<HashMap<String, Rect>> = RefCell::new(HashMap::new());
    static TARGET: RefCell<Option<(String, String)>> = const { RefCell::new(None) };
}

/// The input a dragged wire would land on this frame: card and input.
pub fn set_target(target: Option<(String, String)>) {
    TARGET.with(|cell| *cell.borrow_mut() = target);
}

pub fn target() -> Option<(String, String)> {
    TARGET.with(|cell| cell.borrow().clone())
}

pub fn get() -> Flags {
    FLAGS.with(|flags| flags.borrow().clone())
}

pub fn option() -> Option_ {
    FLAGS.with(|flags| flags.borrow().option)
}

pub fn set(flags: Flags) {
    FLAGS.with(|cell| *cell.borrow_mut() = flags);
}

pub fn update(change: impl FnOnce(&mut Flags)) {
    FLAGS.with(|cell| change(&mut cell.borrow_mut()));
}

pub fn record(id: &str, rect: Rect, zoom: f32, pan: Vec2) {
    DRAWN.with(|cell| {
        cell.borrow_mut()
            .insert(id.to_owned(), Drawn { rect, zoom, pan });
    });
}

pub fn drawn(id: &str) -> Option<Drawn> {
    DRAWN.with(|cell| cell.borrow().get(id).copied())
}

pub fn record_world(id: &str, rect: Rect) {
    WORLD.with(|cell| {
        cell.borrow_mut().insert(id.to_owned(), rect);
    });
}

pub fn world(id: &str) -> Option<Rect> {
    WORLD.with(|cell| cell.borrow().get(id).copied())
}

pub fn clear() {
    DRAWN.with(|cell| cell.borrow_mut().clear());
    WORLD.with(|cell| cell.borrow_mut().clear());
}
