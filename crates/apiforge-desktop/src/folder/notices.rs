//! The notices under the tab strip (UX-12, #222). A notice that is not an
//! error clears itself five seconds after it is raised or at the next action,
//! whichever comes first. An error stays until it is dismissed and belongs to
//! the tab that was active when it was raised: another tab shows its own
//! banner, never that one.

use eframe::egui;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// How long a notice that is not an error stays when nothing is done.
pub(super) const NOTICE_LIFETIME: Duration = Duration::from_secs(5);

/// A message under the tabs; errors stay until dismissed.
pub(super) struct Notice {
    pub(super) text: String,
    pub(super) error: bool,
    raised: Instant,
}

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
    pub(super) fn step(&mut self, by: Duration) {
        self.held = Some(self.now() + by);
    }
}

/// A tab's notices, or the window's under the key `None` (raised with no
/// tab open, or passed on by a closed tab).
type Key = Option<String>;

/// At most one error and one confirmation per tab, by the tab the notice was
/// raised on. A confirmation shows over its tab's error for its lifetime;
/// the error is shown again after it. The window's notices show on a tab
/// that has none of its own.
#[derive(Default)]
pub(super) struct Notices {
    errors: HashMap<Key, Notice>,
    confirmations: HashMap<Key, Notice>,
    /// While an operation's outcome is applied, the tab that was active when
    /// the operation was asked for: its notices belong there.
    pub(super) origin: Option<Key>,
    pub(super) clock: Clock,
}

impl Notices {
    /// Shows `text` on `tab`. An error replaces the tab's error and its
    /// confirmation; a confirmation replaces only the tab's confirmation.
    pub(super) fn raise(&mut self, tab: Option<&str>, text: String, error: bool) {
        let key: Key = tab.map(str::to_owned);
        let notice = Notice {
            text,
            error,
            raised: self.clock.now(),
        };
        if error {
            self.confirmations.remove(&key);
            self.errors.insert(key, notice);
        } else {
            self.confirmations.insert(key, notice);
        }
    }

    /// Where the notice shown while `tab` is active is kept: the tab's own
    /// first, a confirmation over an error, then the window's.
    fn slot(&self, tab: Option<&str>) -> Option<(bool, Key)> {
        let own: Key = tab.map(str::to_owned);
        let keys = if own.is_some() {
            vec![own, None]
        } else {
            vec![None]
        };
        keys.into_iter().find_map(|key| {
            if self.confirmations.contains_key(&key) {
                Some((false, key))
            } else if self.errors.contains_key(&key) {
                Some((true, key))
            } else {
                None
            }
        })
    }

    /// The notice shown while `tab` is active.
    pub(super) fn shown(&self, tab: Option<&str>) -> Option<&Notice> {
        let (error, key) = self.slot(tab)?;
        if error {
            self.errors.get(&key)
        } else {
            self.confirmations.get(&key)
        }
    }

    /// Dismisses the notice shown while `tab` is active.
    pub(super) fn dismiss(&mut self, tab: Option<&str>) {
        match self.slot(tab) {
            Some((true, key)) => self.errors.remove(&key),
            Some((false, key)) => self.confirmations.remove(&key),
            None => None,
        };
    }

    /// A tab renamed or moved keeps its notices.
    pub(super) fn rename(&mut self, from: &str, to: &str) {
        let (from, to): (Key, Key) = (Some(from.to_owned()), Some(to.to_owned()));
        for notices in [&mut self.errors, &mut self.confirmations] {
            if let Some(notice) = notices.remove(&from) {
                notices.insert(to.clone(), notice);
            }
        }
    }

    /// Every notice goes, as when another project is opened.
    pub(super) fn clear(&mut self) {
        self.errors.clear();
        self.confirmations.clear();
    }

    /// Once a frame, before anything acts: a confirmation goes when its five
    /// seconds are up or when `acted` (the user did something this frame).
    /// Another tab becoming active only hides it, so a tab the window opens
    /// by itself clears nothing; a switch by the user is a click or a key, an
    /// action. A closed tab's notices pass to the window: its error unless
    /// the window holds an error already, its confirmation unless the
    /// window's is newer. Returns how long until the next confirmation
    /// expires, for a repaint then.
    pub(super) fn tend(&mut self, open: impl Fn(&str) -> bool, acted: bool) -> Option<Duration> {
        let now = self.clock.now();
        let closed = |notices: &HashMap<Key, Notice>| -> Vec<Key> {
            notices
                .keys()
                .filter(|tab| tab.as_deref().is_some_and(|tab| !open(tab)))
                .cloned()
                .collect()
        };
        for tab in closed(&self.errors) {
            if let Some(notice) = self.errors.remove(&tab) {
                self.errors.entry(None).or_insert(notice);
            }
        }
        for tab in closed(&self.confirmations) {
            if let Some(notice) = self.confirmations.remove(&tab)
                && self
                    .confirmations
                    .get(&None)
                    .is_none_or(|window| window.raised <= notice.raised)
            {
                self.confirmations.insert(None, notice);
            }
        }
        self.confirmations.retain(|_, notice| {
            !acted && now.saturating_duration_since(notice.raised) < NOTICE_LIFETIME
        });
        self.confirmations
            .values()
            .map(|notice| {
                NOTICE_LIFETIME.saturating_sub(now.saturating_duration_since(notice.raised))
            })
            .min()
    }
}

/// Whether the user did something this frame: pressed a pointer button or a
/// key, typed or pasted text, or clicked through assistive technology.
/// Moving the pointer or scrolling is not an action.
pub(super) fn acted(input: &egui::InputState) -> bool {
    input.events.iter().any(|event| match event {
        egui::Event::PointerButton { pressed, .. } | egui::Event::Key { pressed, .. } => *pressed,
        egui::Event::Text(_) | egui::Event::Paste(_) => true,
        egui::Event::AccessKitActionRequest(request) => {
            request.action == egui::accesskit::Action::Click
        }
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notices() -> Notices {
        Notices::default()
    }

    fn shown(notices: &Notices, tab: Option<&str>) -> Option<String> {
        notices.shown(tab).map(|notice| notice.text.clone())
    }

    #[test]
    fn a_confirmation_raised_with_no_tab_shows_on_a_tab_the_window_opens() {
        let mut notices = notices();
        notices.raise(None, "Created a project.".into(), false);
        assert!(notices.tend(|_| true, false).is_some());
        assert_eq!(
            shown(&notices, Some("a")).as_deref(),
            Some("Created a project.")
        );
        notices.tend(|_| true, true);
        assert_eq!(shown(&notices, Some("a")), None, "an action clears it");
    }

    #[test]
    fn a_confirmation_is_hidden_on_another_tab_and_passes_on_when_its_tab_closes() {
        let mut notices = notices();
        notices.raise(Some("a"), "Moved to the trash.".into(), false);
        notices.tend(|_| true, false);
        assert_eq!(shown(&notices, Some("b")), None);
        assert!(shown(&notices, Some("a")).is_some(), "hidden, not gone");
        notices.tend(|tab| tab == "b", false);
        assert_eq!(
            shown(&notices, Some("b")).as_deref(),
            Some("Moved to the trash."),
            "its tab closed: the window shows it"
        );
        notices.clock.step(NOTICE_LIFETIME);
        notices.tend(|tab| tab == "b", false);
        assert_eq!(shown(&notices, Some("b")), None, "for the rest of its time");
    }

    #[test]
    fn a_confirmation_shows_over_an_error_which_comes_back_after_it() {
        let mut notices = notices();
        notices.raise(Some("a"), "error".into(), true);
        notices.raise(Some("a"), "Saved 1 file.".into(), false);
        assert_eq!(shown(&notices, Some("a")).as_deref(), Some("Saved 1 file."));
        notices.clock.step(NOTICE_LIFETIME);
        notices.tend(|_| true, false);
        assert_eq!(shown(&notices, Some("a")).as_deref(), Some("error"));
        notices.raise(Some("a"), "Saved 1 file.".into(), false);
        notices.dismiss(Some("a"));
        assert_eq!(
            shown(&notices, Some("a")).as_deref(),
            Some("error"),
            "x dismissed the confirmation"
        );
    }

    #[test]
    fn a_window_error_shows_where_a_tab_has_none_and_yields_to_its_own() {
        let mut notices = notices();
        notices.raise(None, "window".into(), true);
        notices.raise(Some("a"), "a".into(), true);
        assert_eq!(shown(&notices, Some("a")).as_deref(), Some("a"));
        assert_eq!(shown(&notices, Some("b")).as_deref(), Some("window"));
        notices.dismiss(Some("a"));
        assert_eq!(shown(&notices, Some("a")).as_deref(), Some("window"));
    }

    #[test]
    fn a_closed_tabs_error_passes_to_the_window_unless_it_holds_one() {
        let mut notices = notices();
        notices.raise(None, "Created a project.".into(), false);
        notices.raise(Some("a"), "a".into(), true);
        notices.tend(|tab| tab == "b", false);
        notices.dismiss(Some("b"));
        assert_eq!(
            shown(&notices, Some("b")).as_deref(),
            Some("a"),
            "kept under the window's confirmation"
        );
        notices.raise(Some("b"), "b".into(), true);
        notices.tend(|_| false, false);
        assert_eq!(
            shown(&notices, None).as_deref(),
            Some("a"),
            "the window keeps its own"
        );
    }

    #[test]
    fn a_renamed_tab_keeps_its_notices() {
        let mut notices = notices();
        notices.raise(Some("a"), "a".into(), true);
        notices.raise(Some("a"), "Saved 1 file.".into(), false);
        notices.rename("a", "b");
        notices.tend(|tab| tab == "b", false);
        assert_eq!(shown(&notices, Some("b")).as_deref(), Some("Saved 1 file."));
        notices.dismiss(Some("b"));
        assert_eq!(shown(&notices, Some("b")).as_deref(), Some("a"));
        assert!(
            notices.errors.contains_key(&Some("b".to_owned())),
            "still the tab's own"
        );
    }

    #[test]
    fn the_wait_is_what_is_left_of_the_five_seconds() {
        let mut notices = notices();
        notices.raise(Some("a"), "Saved 1 file.".into(), false);
        notices.clock.step(Duration::from_secs(2));
        assert_eq!(notices.tend(|_| true, false), Some(Duration::from_secs(3)));
        notices.raise(Some("a"), "error".into(), true);
        notices.raise(Some("a"), "Saved 1 file.".into(), false);
        notices.dismiss(Some("a"));
        assert_eq!(
            notices.tend(|_| true, false),
            None,
            "errors wait for nothing"
        );
    }
}
