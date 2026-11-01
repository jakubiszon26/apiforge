//! Following the holder's work while the window observes (MCP-16).
//!
//! Every commit the window follows names what it changed
//! (`ExternalOutcome::followed`). While following is pinned the window goes
//! to the document of the commit's main change, in the view the person is
//! in: a saved source shows its first changed line and marks the changed
//! lines in the code's gutter until the holder's next commit to that
//! document; a created document opens at its top; a published component's
//! tab comes to the front. Deletions and the settings are never followed,
//! nor is a run; a commit that deleted the active document leaves the
//! window on the tab closing chose. The person's own navigation to another document unpins
//! following; *Follow* in the bottom bar pins it again and goes to the last
//! change at once. Following starts pinned with every observation and ends
//! with it; nothing of it is persisted.
use super::*;
use apiforge_project::CommitChange;
use std::ops::Range;

/// The toggle on screen, and its accessible name (decision 5).
pub(super) const FOLLOW: &str = "Follow";
pub(super) const FOLLOW_NAME: &str = "Follow the agent";
pub(super) const PINNED_HINT: &str =
    "Follows the agent to every file it changes; click to stay where you are";
pub(super) const RELEASED_HINT: &str = "Click to follow the agent again";

#[derive(Default)]
pub(super) struct Follow {
    /// Whether the window observed when this state began: it belongs to one
    /// observation.
    observing: bool,
    pinned: bool,
    /// Where the holder's last followed change is: pinning again goes there.
    last: Option<Target>,
    /// Each document's lines changed by the holder's last commit to it,
    /// counted from zero.
    marks: HashMap<String, Range<usize>>,
    /// A document's first changed line, as a byte offset, waiting for the
    /// code view to show it.
    reveal: Option<(String, usize)>,
}

#[derive(Clone)]
struct Target {
    document_id: String,
    /// The changed lines of a saved source; nothing to scroll to otherwise.
    lines: Option<Range<usize>>,
}

impl Follow {
    /// The bar's toggle: `None` while the window does not observe.
    pub(super) fn toggle(&self) -> Option<bool> {
        self.observing.then_some(self.pinned)
    }

    /// The gutter's marks for `document_id`.
    pub(super) fn marks(&self, document_id: &str) -> Option<Range<usize>> {
        self.marks.get(document_id).cloned()
    }

    /// The offset the code view of `document_id` should show, once.
    pub(super) fn take_reveal(&mut self, document_id: &str) -> Option<usize> {
        if self
            .reveal
            .as_ref()
            .is_some_and(|(waiting, _)| waiting == document_id)
        {
            self.reveal.take().map(|(_, at)| at)
        } else {
            None
        }
    }
}

impl FolderApp {
    /// Following starts pinned whenever the window becomes an observer and
    /// is forgotten when it stops being one (decision 1).
    pub(super) fn track_observing(&mut self) {
        let observing = self.observing();
        if observing != self.follow.observing {
            self.follow = Follow {
                observing,
                pinned: observing,
                ..Follow::default()
            };
        }
    }

    /// The person is going to `document_id` by their own gesture: away from
    /// the active document, following lets go (decision 4).
    pub(super) fn navigated_to(&mut self, document_id: &str) {
        if self.workbench.active() != Some(document_id) {
            self.unpin();
        }
    }

    /// The person left the document following brought them to.
    pub(super) fn unpin(&mut self) {
        self.track_observing();
        if self.follow.observing && self.follow.pinned {
            self.follow.pinned = false;
            self.follow.reveal = None;
        }
    }

    /// *Follow*: unpins, or pins and goes to the last change at once.
    pub(super) fn toggle_follow(&mut self) {
        self.track_observing();
        if !self.follow.observing {
            return;
        }
        self.follow.pinned = !self.follow.pinned;
        if !self.follow.pinned {
            self.follow.reveal = None;
        } else if let Some(target) = self.follow.last.clone() {
            self.go_to(target);
        }
    }

    /// A commit the window followed. Its main change is the first of a
    /// created, saved, published or laid-out document, in that order;
    /// deletions and the settings are never followed. When the commit took
    /// the active document away (`active_closed`), the window stays on the
    /// tab closing chose (criterion 5).
    ///
    /// One reload can carry several commits that landed between two looks,
    /// and its diff no longer says which came last. Then a document other
    /// than the active one goes first: the active one changes in place,
    /// under the person's eyes, with its lines marked.
    pub(super) fn follow_commit(&mut self, changes: &[CommitChange], active_closed: bool) {
        self.track_observing();
        if !self.follow.observing {
            return;
        }
        // The marks of a document last until the holder's next commit to it.
        for change in changes {
            if let Some(document_id) = &change.document_id {
                self.follow.marks.remove(document_id);
                if self.follow.pinned
                    && change.kind == CommitChangeKind::Saved
                    && let Some(lines) = &change.lines
                {
                    self.follow.marks.insert(document_id.clone(), lines.clone());
                }
            }
        }
        let active = self.workbench.active().map(str::to_owned);
        let Some(change) = changes
            .iter()
            .filter(|change| {
                change.document_id.as_ref().is_some_and(|id| {
                    self.session
                        .as_ref()
                        .is_some_and(|session| session.document(id).is_some())
                })
            })
            .filter_map(|change| priority(change.kind).map(|rank| (rank, change)))
            .min_by_key(|(rank, change)| (change.document_id == active, *rank))
            .map(|(_, change)| change)
        else {
            return;
        };
        let target = Target {
            document_id: change.document_id.clone().unwrap_or_default(),
            lines: (change.kind == CommitChangeKind::Saved)
                .then(|| change.lines.clone())
                .flatten(),
        };
        self.follow.last = Some(target.clone());
        if self.follow.pinned && !active_closed {
            self.go_to(target);
        }
    }

    /// Activates the target's tab, opening it beside the active one, in the
    /// view the person is in; a saved source's changed lines are marked and
    /// its first one shown.
    fn go_to(&mut self, target: Target) {
        let Some(text) = self
            .session
            .as_ref()
            .and_then(|session| session.document(&target.document_id))
            .map(|document| document.text())
        else {
            return;
        };
        let at = target
            .lines
            .as_ref()
            .map(|lines| line_start(text, lines.start));
        if self.workbench.active() != Some(target.document_id.as_str()) {
            self.activate_tab(&target.document_id);
            self.touched();
        }
        if let Some(lines) = target.lines {
            self.follow.marks.insert(target.document_id.clone(), lines);
        }
        if let Some(at) = at {
            self.follow.reveal = Some((target.document_id, at));
        }
    }
}

/// Which change of a commit is followed first; `None` is never followed.
fn priority(kind: CommitChangeKind) -> Option<u8> {
    match kind {
        CommitChangeKind::Added => Some(0),
        CommitChangeKind::Saved => Some(1),
        CommitChangeKind::Published => Some(2),
        CommitChangeKind::Layout => Some(3),
        CommitChangeKind::Deleted | CommitChangeKind::Settings => None,
    }
}

/// The byte offset where line `line` (from zero) of `text` starts; the end
/// of the text for a line past it.
fn line_start(text: &str, line: usize) -> usize {
    if line == 0 {
        return 0;
    }
    text.match_indices('\n')
        .nth(line - 1)
        .map_or(text.len(), |(at, _)| at + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_starts_after_the_break_before_it() {
        let text = "a\nbb\nccc";
        assert_eq!(line_start(text, 0), 0);
        assert_eq!(line_start(text, 1), 2);
        assert_eq!(line_start(text, 2), 5);
        assert_eq!(line_start(text, 3), text.len());
    }

    #[test]
    fn a_creation_comes_before_a_save_a_publication_and_a_layout() {
        let mut kinds = [
            CommitChangeKind::Layout,
            CommitChangeKind::Published,
            CommitChangeKind::Saved,
            CommitChangeKind::Added,
        ];
        kinds.sort_by_key(|kind| priority(*kind));
        assert_eq!(
            kinds,
            [
                CommitChangeKind::Added,
                CommitChangeKind::Saved,
                CommitChangeKind::Published,
                CommitChangeKind::Layout,
            ]
        );
        assert_eq!(priority(CommitChangeKind::Deleted), None);
        assert_eq!(priority(CommitChangeKind::Settings), None);
    }
}
