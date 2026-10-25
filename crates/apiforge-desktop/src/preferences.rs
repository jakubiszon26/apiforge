//! User preferences: how the application looks, not what a document contains.
//!
//! They live in the user's configuration directory, away from the workspace and its
//! writer lock, because they belong to the person rather than to a project. Every
//! read and write is best effort: a preference that cannot be loaded falls back to
//! the default, and one that cannot be written is simply not remembered. Nothing
//! here is worth interrupting the user for, and nothing here is worth a prompt.
//!
//! Several windows share the file without coordination, so the last window to
//! change a preference wins. Each write is atomic, so a crash never leaves a
//! half-written file behind.

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
    Spline,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Stored {
    version: u32,
    wire_style: StoredWireStyle,
    bridges: bool,
    code_panel: bool,
}

/// The preferences of this session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub wire_style: WireStyle,
    /// Whether a wire crossing is drawn as a bridge.
    pub bridges: bool,
    /// Whether the code panel is open beside the board.
    pub code_panel: bool,
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
        let Some(path) = preferences.path.clone() else {
            return preferences;
        };
        let path = path.as_path();
        let Ok(file) = std::fs::File::open(path) else {
            return preferences;
        };
        if file
            .metadata()
            .is_ok_and(|metadata| metadata.len() > MAX_BYTES)
        {
            return preferences;
        }
        let Ok(stored) = std::fs::read(path) else {
            return preferences;
        };
        let Ok(stored) = serde_json::from_slice::<Stored>(&stored) else {
            return preferences;
        };
        if stored.version != VERSION {
            return preferences;
        }
        preferences.wire_style = match stored.wire_style {
            StoredWireStyle::Orthogonal => WireStyle::Orthogonal,
            StoredWireStyle::Spline => WireStyle::Spline,
        };
        preferences.bridges = stored.bridges;
        preferences.code_panel = stored.code_panel;
        preferences
    }

    /// Writes the preferences, and says nothing if it cannot.
    pub fn save(&self) {
        let Some(path) = &self.path else {
            return;
        };
        let stored = Stored {
            version: VERSION,
            wire_style: match self.wire_style {
                WireStyle::Orthogonal => StoredWireStyle::Orthogonal,
                WireStyle::Spline => StoredWireStyle::Spline,
            },
            bridges: self.bridges,
            code_panel: self.code_panel,
        };
        let Ok(bytes) = serde_json::to_vec_pretty(&stored) else {
            return;
        };
        let _ = apiforge_storage::write_atomic(path, &bytes);
    }
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
        preferences.wire_style = WireStyle::Spline;
        preferences.bridges = false;
        preferences.code_panel = true;
        preferences.save();
        let reloaded = Preferences::at(Some(path));
        assert_eq!(reloaded.wire_style, WireStyle::Spline);
        assert!(!reloaded.bridges);
        assert!(reloaded.code_panel);
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
        assert_eq!(
            Preferences::at(Some(path)).wire_style,
            WireStyle::Orthogonal
        );
    }

    #[test]
    fn a_missing_configuration_directory_keeps_preferences_for_the_session() {
        let mut session = Preferences::at(None);
        session.wire_style = WireStyle::Spline;
        // Saving does nothing and must not panic.
        session.save();
        assert_eq!(session.wire_style, WireStyle::Spline);
        assert_eq!(Preferences::at(None).wire_style, WireStyle::Orthogonal);
    }
}
