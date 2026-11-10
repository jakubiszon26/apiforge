//! User preferences: how the application looks, not what a document contains.
//!
//! They live in the user's configuration directory, away from the workspace and its
//! writer lock, because they belong to the person rather than to a project. Every
//! read and write is best effort: a preference that cannot be loaded falls back to
//! the default, and one that cannot be written is simply not remembered. Nothing
//! here is worth interrupting the user for, and nothing here is worth a prompt.
//!
//! Several windows share the file. A change takes a lock file beside it, reads
//! the file again and replaces only the preference that changed, so a window
//! never reverts what another window changed, even when both save at once; the
//! last window to change the same preference wins. Each write is atomic, so a
//! crash never leaves a half-written file behind.

use crate::canvas::WireStyle;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The file format. An unknown version is replaced by the defaults rather than
/// guessed at, and is overwritten by the next change.
const VERSION: u32 = 1;

/// A preferences file is tiny; anything larger is not one.
const MAX_BYTES: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum StoredWireStyle {
    #[default]
    Orthogonal,
    /// *Curves*, withdrawn until it draws curves (UX-28). A file that stores it
    /// still loads, as orthogonal, and is never written with it again.
    Spline,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Stored {
    version: u32,
    wire_style: StoredWireStyle,
    bridges: bool,
    code_panel: bool,
    /// Absent from a file written before LAY-16, which reads as on.
    #[serde(default = "on")]
    compact_cards: bool,
}

fn on() -> bool {
    true
}

/// One preference, named so that a change writes only that one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preference {
    WireStyle,
    Bridges,
    CodePanel,
    CompactCards,
}

/// The preferences of this session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub wire_style: WireStyle,
    /// Whether a wire crossing is drawn as a bridge.
    pub bridges: bool,
    /// Whether the code panel is open beside the board.
    pub code_panel: bool,
    /// Whether a request card shows only what it uses (LAY-16).
    pub compact_cards: bool,
    /// Where to write them, or `None` when the system has no configuration
    /// directory and preferences last only for this session.
    path: Option<PathBuf>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            wire_style: WireStyle::default(),
            bridges: true,
            code_panel: false,
            compact_cards: true,
            path: default_path(),
        }
    }
}

impl Preferences {
    /// Reads the user's preferences from the usual place.
    pub fn load() -> Self {
        Self::at(default_path())
    }

    /// Reads preferences kept in `path`, falling back to the defaults for anything
    /// missing, unreadable or written by a version this one does not know. `None`
    /// is a session without a file, where preferences last until the window closes.
    pub fn at(path: Option<PathBuf>) -> Self {
        let mut preferences = Self {
            path,
            ..Self::default()
        };
        if let Some(stored) = preferences.path.as_deref().and_then(read) {
            preferences.wire_style = match stored.wire_style {
                StoredWireStyle::Orthogonal | StoredWireStyle::Spline => WireStyle::Orthogonal,
            };
            preferences.bridges = stored.bridges;
            preferences.code_panel = stored.code_panel;
            preferences.compact_cards = stored.compact_cards;
        }
        preferences
    }

    /// Writes the preference that changed, and says nothing if it cannot. The
    /// file is read again first and only `changed` is replaced in it, so what
    /// another window wrote since this one started is kept; a file that cannot
    /// be read is replaced by this window's preferences. The read and the write
    /// happen under a lock shared by every window, so two windows saving at once
    /// both keep their change.
    pub fn save(&self, changed: Preference) {
        let Some(path) = &self.path else {
            return;
        };
        // Held until this function returns. Without it the change is still
        // written, only no longer serialised against another window.
        let _lock = lock(path);
        let mut stored = read(path).unwrap_or_else(|| self.stored());
        // A stored *Curves* is never written again (UX-28).
        stored.wire_style = StoredWireStyle::Orthogonal;
        match changed {
            Preference::WireStyle => {
                stored.wire_style = match self.wire_style {
                    WireStyle::Orthogonal => StoredWireStyle::Orthogonal,
                }
            }
            Preference::Bridges => stored.bridges = self.bridges,
            Preference::CodePanel => stored.code_panel = self.code_panel,
            Preference::CompactCards => stored.compact_cards = self.compact_cards,
        }
        let Ok(bytes) = serde_json::to_vec_pretty(&stored) else {
            return;
        };
        let _ = apiforge_storage::write_atomic(path, &bytes);
    }

    /// This window's preferences as the file holds them.
    fn stored(&self) -> Stored {
        Stored {
            version: VERSION,
            wire_style: match self.wire_style {
                WireStyle::Orthogonal => StoredWireStyle::Orthogonal,
            },
            bridges: self.bridges,
            code_panel: self.code_panel,
            compact_cards: self.compact_cards,
        }
    }
}

/// The lock that serialises every window's read-modify-write of `path`: a file
/// beside it that is never removed, so every window locks the same one. The
/// lock is released when the returned file is dropped. Before the first save
/// the directory does not exist yet and there is no lock: `write_atomic`
/// creates the directory, so that it can make the new directory durable too.
fn lock(path: &std::path::Path) -> Option<std::fs::File> {
    let file = std::fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path.with_extension("lock"))
        .ok()?;
    file.lock().ok()?;
    Some(file)
}

/// What `path` holds, or `None` when it is missing, unreadable, oversized or
/// written by a version this one does not know.
fn read(path: &std::path::Path) -> Option<Stored> {
    let file = std::fs::File::open(path).ok()?;
    if file
        .metadata()
        .is_ok_and(|metadata| metadata.len() > MAX_BYTES)
    {
        return None;
    }
    let stored = serde_json::from_slice::<Stored>(&std::fs::read(path).ok()?).ok()?;
    (stored.version == VERSION).then_some(stored)
}

/// Tests never read the developer's own preferences: a suite whose result
/// depends on the machine it runs on measures nothing. The tests that cover
/// persistence pass an explicit path instead.
#[cfg(test)]
fn default_path() -> Option<PathBuf> {
    None
}

#[cfg(not(test))]
fn default_path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("apiforge/preferences.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_survive_a_save_and_a_read() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preferences.json");
        let mut preferences = Preferences::at(Some(path.clone()));
        preferences.bridges = false;
        preferences.save(Preference::Bridges);
        preferences.code_panel = true;
        preferences.save(Preference::CodePanel);
        let reloaded = Preferences::at(Some(path));
        assert_eq!(reloaded.wire_style, WireStyle::Orthogonal);
        assert!(!reloaded.bridges);
        assert!(reloaded.code_panel);
    }

    /// LAY-16 criterion 4: *Compact cards* is on by default and remembered;
    /// a file written before it existed loads with it on.
    #[test]
    fn compact_cards_is_remembered_and_on_by_default() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preferences.json");
        assert!(Preferences::at(Some(path.clone())).compact_cards);
        let mut preferences = Preferences::at(Some(path.clone()));
        preferences.compact_cards = false;
        preferences.save(Preference::CompactCards);
        assert!(!Preferences::at(Some(path.clone())).compact_cards);

        std::fs::write(
            &path,
            br#"{"version": 1, "wireStyle": "orthogonal", "bridges": false, "codePanel": true}"#,
        )
        .unwrap();
        let earlier = Preferences::at(Some(path));
        assert!(!earlier.bridges, "an earlier file is still read");
        assert!(earlier.compact_cards);
    }

    /// A window that started before another changed *Compact cards* keeps that
    /// change when it saves a different preference.
    #[test]
    fn a_change_replaces_only_its_own_preference() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preferences.json");
        let mut stale = Preferences::at(Some(path.clone()));
        let mut other = Preferences::at(Some(path.clone()));
        other.compact_cards = false;
        other.save(Preference::CompactCards);
        stale.bridges = false;
        stale.save(Preference::Bridges);
        let reloaded = Preferences::at(Some(path));
        assert!(!reloaded.compact_cards, "the other window's change is kept");
        assert!(!reloaded.bridges, "this window's change is written");
    }

    /// Two windows saving at once both keep their change: a save waits for
    /// the lock every window takes around its read-modify-write, so it reads
    /// the file only after the other window's write.
    #[test]
    fn a_save_waits_for_another_window_saving_at_the_same_moment() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preferences.json");
        let mut waiting = Preferences::at(Some(path.clone()));
        waiting.bridges = false;

        // The other window is between its read and its write.
        let other = lock(&path).expect("the lock file can be taken");
        let (saved, done) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            waiting.save(Preference::Bridges);
            saved.send(()).unwrap();
        });
        assert!(
            done.recv_timeout(std::time::Duration::from_millis(300))
                .is_err(),
            "a save must not read the file while another window holds the lock"
        );
        let mut writer = Preferences::at(Some(path.clone()));
        writer.compact_cards = false;
        std::fs::write(&path, serde_json::to_vec(&writer.stored()).unwrap()).unwrap();
        drop(other);
        done.recv().unwrap();
        thread.join().unwrap();

        let reloaded = Preferences::at(Some(path));
        assert!(!reloaded.compact_cards, "the other window's change is kept");
        assert!(!reloaded.bridges, "the waiting window's change is written");
    }

    #[test]
    fn an_unreadable_or_newer_file_leaves_the_defaults_in_place() {
        let directory = tempfile::tempdir().unwrap();
        for (name, content) in [
            ("broken.json", br#"not json at all"#.as_slice()),
            (
                "newer.json",
                br#"{"version": 9, "wireStyle": "spline", "bridges": false}"#,
            ),
            (
                "extra.json",
                br#"{"version": 1, "wireStyle": "spline", "bridges": false, "future": 1}"#,
            ),
        ] {
            let path = directory.path().join(name);
            std::fs::write(&path, content).unwrap();
            let preferences = Preferences::at(Some(path));
            assert_eq!(
                preferences.wire_style,
                WireStyle::Orthogonal,
                "{name} must not be trusted"
            );
            assert!(preferences.bridges, "{name} must not be trusted");
        }
    }

    #[test]
    fn an_oversized_file_is_not_read() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preferences.json");
        let mut padded =
            br#"{"version": 1, "wireStyle": "spline", "bridges": false, "codePanel": true}"#
                .to_vec();
        padded.resize(MAX_BYTES as usize + 1, b' ');
        std::fs::write(&path, padded).unwrap();
        let preferences = Preferences::at(Some(path));
        assert!(preferences.bridges);
        assert!(!preferences.code_panel);
    }

    #[test]
    fn a_missing_configuration_directory_keeps_preferences_for_the_session() {
        let mut session = Preferences::at(None);
        session.bridges = false;
        // Saving does nothing and must not panic.
        session.save(Preference::Bridges);
        assert!(!session.bridges);
        assert!(Preferences::at(None).bridges);
    }
}
