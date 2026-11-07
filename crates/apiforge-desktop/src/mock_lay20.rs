//! LAY-20 mock-up switch (throwaway branch `mock/lay-20-checks`): how a
//! Cleanup check that reads a Main step is counted under that step.
#![allow(dead_code)]

use std::cell::RefCell;

/// How the list of a Main step counts its Cleanup checks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Variant {
    /// In the same list and count, each row marked *Cleanup*.
    #[default]
    Counted,
    /// In the same list, counted apart like gates: `3 checks · 1 in Cleanup`.
    Apart,
    /// Under a heading *Cleanup* of its own, with its own count line.
    Section,
}

thread_local! {
    static VARIANT: RefCell<Variant> = const { RefCell::new(Variant::Counted) };
}

pub fn variant() -> Variant {
    VARIANT.with(|cell| *cell.borrow())
}

pub fn set(variant: Variant) {
    VARIANT.with(|cell| *cell.borrow_mut() = variant);
}
