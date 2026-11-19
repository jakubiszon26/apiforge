//! The fix of a file written for language 1 (LANG-13). Not implemented yet:
//! this names the interface the acceptance tests use.
use crate::{ast::Diagnostic, lower::LowerContext};

/// An `@after` in a source.
pub const AFTER_REMOVED: &str = "`@after` is not part of the language: the steps of a block run in the order they are written; delete this line";
/// A main flow with a step and no `start`.
pub const START_MISSING: &str =
    "A flow with a step needs `start` as its first step; write `begin = start` above it";
/// A step written above `start`.
pub const START_NOT_FIRST: &str =
    "`start` must be the first step of the flow; move the steps written above it below it";
/// The fix is refused: a step nothing orders after `start` today.
pub const RUNS_NOWHERE: &str = "This step runs nowhere today: nothing orders it after `start`; write it where it should run or delete it, then delete the `@after` lines";
/// The fix is refused: a statement written elsewhere than where it runs.
pub const NOT_IN_PLACE: &str = "This statement is not written where it runs today; move it there, then delete the `@after` lines";

/// The text of a file after its fix, or as it was when the fix is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fixed {
    pub text: String,
    pub refused: Option<Diagnostic>,
}

impl Fixed {
    /// Whether the fix changed the text.
    pub fn changed(&self, text: &str) -> bool {
        self.text != text
    }
}

/// Not implemented yet: returns the text unchanged.
pub fn fix_source(text: &str, _context: &LowerContext<'_>) -> Fixed {
    Fixed {
        text: text.to_owned(),
        refused: None,
    }
}
