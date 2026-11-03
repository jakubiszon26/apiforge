//! The notices under the tab strip (UX-12, #222).

use std::time::Instant;

/// The time notices age by. The window reads the system clock; a test build
/// holds it still until a test steps it, so a test of a notice's lifetime
/// never depends on how fast the machine is.
pub(super) struct Clock {
    held: Option<Instant>,
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            held: cfg!(test).then(Instant::now),
        }
    }
}

impl Clock {
    pub(super) fn now(&self) -> Instant {
        self.held.unwrap_or_else(Instant::now)
    }

    /// Moves a held clock on by `by`: a test seam.
    #[cfg(test)]
    pub(super) fn step(&mut self, by: std::time::Duration) {
        self.held = Some(self.now() + by);
    }
}
