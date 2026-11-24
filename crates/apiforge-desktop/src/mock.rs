//! Switches of the WS-05 and REQ-15 mock-ups. Mock branch only: never merged.
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering::Relaxed};

/// WS-05: 0 today, 1 A (header, hover in Files), 2 B (Files by title), 3 C
/// (header only).
pub static TITLE: AtomicU8 = AtomicU8::new(0);
/// WS-05, the sub-question: the title in the header is being edited.
pub static EDIT: AtomicBool = AtomicBool::new(false);
/// REQ-15: 0 today, 1 A (the action), 2 B (the action, the section moved up).
pub static SAMPLE: AtomicU8 = AtomicU8::new(0);
/// REQ-15: the state after the action was taken.
pub static OPENED: AtomicBool = AtomicBool::new(false);
/// The title of every document, by its path.
pub static TITLES: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

pub fn title() -> u8 {
    TITLE.load(Relaxed)
}
pub fn sample() -> u8 {
    SAMPLE.load(Relaxed)
}
/// Frames the opened section still asks to be scrolled to.
pub static SCROLL: AtomicU8 = AtomicU8::new(0);
pub fn scroll() -> bool {
    SCROLL
        .fetch_update(Relaxed, Relaxed, |left| left.checked_sub(1))
        .is_ok()
}
pub fn opened() -> bool {
    OPENED.load(Relaxed)
}
pub fn title_of(path: &str) -> Option<String> {
    TITLES.lock().unwrap().as_ref()?.get(path).cloned()
}
pub const REFUSAL: &str = "No sample response for the simulation.";
pub const ACTION: &str = "Add a sample response";
