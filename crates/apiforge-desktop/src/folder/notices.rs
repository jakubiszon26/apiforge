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

/// One banner per tab, by the tab that was active when it was raised; the
/// key `None` holds one raised with no tab open, which the window shows
/// while the active tab has none of its own.
#[derive(Default)]
pub(super) struct Notices {
    banners: HashMap<Option<String>, Notice>,
    pub(super) clock: Clock,
}

impl Notices {
    /// Shows `text` on `tab`, in place of that tab's banner.
    pub(super) fn raise(&mut self, tab: Option<&str>, text: String, error: bool) {
        let raised = self.clock.now();
        self.banners.insert(
            tab.map(str::to_owned),
            Notice {
                text,
                error,
                raised,
            },
        );
    }

    /// The key of the banner shown while `tab` is active.
    fn key(&self, tab: Option<&str>) -> Option<Option<String>> {
        let own = tab.map(str::to_owned);
        if own.is_some() && self.banners.contains_key(&own) {
            return Some(own);
        }
        self.banners.contains_key(&None).then_some(None)
    }

    /// The banner shown while `tab` is active: its own, else the window's.
    pub(super) fn shown(&self, tab: Option<&str>) -> Option<&Notice> {
        self.key(tab).and_then(|key| self.banners.get(&key))
    }

    /// Dismisses the banner shown while `tab` is active.
    pub(super) fn dismiss(&mut self, tab: Option<&str>) {
        if let Some(key) = self.key(tab) {
            self.banners.remove(&key);
        }
    }

    /// A tab renamed or moved keeps its banner.
    pub(super) fn rename(&mut self, from: &str, to: &str) {
        if let Some(notice) = self.banners.remove(&Some(from.to_owned())) {
            self.banners.insert(Some(to.to_owned()), notice);
        }
    }

    /// Every banner goes, as when another project is opened.
    pub(super) fn clear(&mut self) {
        self.banners.clear();
    }

    /// Once a frame, before anything acts: a notice that is not an error
    /// goes when its five seconds are up, when `acted` (the user did
    /// something this frame) or when its tab is no longer the active one; a
    /// notice raised with no tab open stays across tabs the window opens by
    /// itself. The banners of tabs no longer open go. Returns how long until
    /// the next notice expires, for a repaint then.
    pub(super) fn tend(
        &mut self,
        active: Option<&str>,
        open: impl Fn(&str) -> bool,
        acted: bool,
    ) -> Option<Duration> {
        let now = self.clock.now();
        // A closed tab's error is not lost: the window shows it in place of a
        // confirmation, unless it already shows an error of its own.
        let closed: Vec<Option<String>> = self
            .banners
            .iter()
            .filter(|(tab, notice)| notice.error && tab.as_deref().is_some_and(|tab| !open(tab)))
            .map(|(tab, _)| tab.clone())
            .collect();
        for tab in closed {
            if let Some(notice) = self.banners.remove(&tab)
                && self.banners.get(&None).is_none_or(|window| !window.error)
            {
                self.banners.insert(None, notice);
            }
        }
        self.banners.retain(|tab, notice| {
            let tab = tab.as_deref();
            tab.is_none_or(&open)
                && (notice.error
                    || !(acted
                        || tab.is_some_and(|tab| Some(tab) != active)
                        || now.saturating_duration_since(notice.raised) >= NOTICE_LIFETIME))
        });
        self.banners
            .values()
            .filter(|notice| !notice.error)
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

    #[test]
    fn a_confirmation_raised_with_no_tab_stays_when_the_window_opens_one() {
        let mut notices = notices();
        notices.raise(None, "Created a project.".into(), false);
        assert!(notices.tend(Some("a"), |_| true, false).is_some());
        assert!(notices.shown(Some("a")).is_some());
        notices.tend(Some("a"), |_| true, true);
        assert!(notices.shown(Some("a")).is_none(), "an action clears it");
    }

    #[test]
    fn a_confirmation_goes_when_its_tab_is_left() {
        let mut notices = notices();
        notices.raise(Some("a"), "Saved 1 file.".into(), false);
        notices.tend(Some("b"), |_| true, false);
        notices.tend(Some("a"), |_| true, false);
        assert!(notices.shown(Some("a")).is_none());
    }

    #[test]
    fn a_window_error_shows_where_a_tab_has_none_and_yields_to_its_own() {
        let mut notices = notices();
        notices.tend(Some("a"), |_| true, false);
        notices.raise(None, "window".into(), true);
        notices.raise(Some("a"), "a".into(), true);
        assert_eq!(notices.shown(Some("a")).unwrap().text, "a");
        assert_eq!(notices.shown(Some("b")).unwrap().text, "window");
        notices.dismiss(Some("a"));
        assert_eq!(notices.shown(Some("a")).unwrap().text, "window");
    }

    #[test]
    fn a_closed_tabs_error_passes_to_the_window_and_its_confirmation_goes() {
        let mut notices = notices();
        notices.raise(Some("a"), "a".into(), true);
        notices.raise(Some("c"), "Saved 1 file.".into(), false);
        notices.tend(Some("b"), |tab| tab == "b", false);
        assert_eq!(notices.shown(Some("b")).unwrap().text, "a");
        assert_eq!(notices.banners.len(), 1, "the confirmation went");
        notices.raise(Some("b"), "b".into(), true);
        notices.tend(None, |_| false, false);
        assert_eq!(
            notices.shown(None).unwrap().text,
            "a",
            "the window keeps its own"
        );
    }

    #[test]
    fn a_closed_tabs_error_takes_the_place_of_the_windows_confirmation() {
        let mut notices = notices();
        notices.raise(None, "Created a project.".into(), false);
        notices.raise(Some("a"), "a".into(), true);
        notices.tend(None, |_| false, false);
        assert_eq!(notices.shown(None).unwrap().text, "a");
    }

    #[test]
    fn a_renamed_tab_keeps_its_error() {
        let mut notices = notices();
        notices.raise(Some("a"), "a".into(), true);
        notices.rename("a", "b");
        notices.tend(Some("b"), |tab| tab == "b", false);
        assert_eq!(notices.shown(Some("b")).unwrap().text, "a");
        assert!(
            notices.banners.contains_key(&Some("b".to_owned())),
            "still the tab's own"
        );
    }

    #[test]
    fn the_wait_is_what_is_left_of_the_five_seconds() {
        let mut notices = notices();
        notices.tend(Some("a"), |_| true, false);
        notices.raise(Some("a"), "Saved 1 file.".into(), false);
        notices.clock.step(Duration::from_secs(2));
        assert_eq!(
            notices.tend(Some("a"), |_| true, false),
            Some(Duration::from_secs(3))
        );
        notices.raise(Some("a"), "error".into(), true);
        assert_eq!(
            notices.tend(Some("a"), |_| true, false),
            None,
            "errors wait for nothing"
        );
    }
}
