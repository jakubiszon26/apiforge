//! The person's consent to an MCP agent's live requests, per project (MCP-09,
//! ADR 0014 decision 6).
//!
//! It belongs to the person and to this machine, not to the project: it is
//! kept beside `preferences.json` in the user's configuration directory, keyed
//! by the project's manifest id, never in the manifest (which Git carries to
//! other people) and never under the project folder (which an observing window
//! may not write). The default is off.
//!
//! As with the preferences, every read and write is best effort: a file that
//! cannot be read is no consent at all, and a consent that cannot be written
//! lasts for this window only. Several windows share the file without a lock,
//! as they share the preferences: a change reads it again and replaces only
//! its own project's entry, written atomically, and each window reads it
//! again once a second, so a change made in another window is followed. Two
//! changes in the same instant can still lose one of them.

use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// The file format. An unknown version is no consent.
const VERSION: u32 = 1;
/// A consent file is small; anything larger is not one.
const MAX_BYTES: u64 = 1024 * 1024;
/// The file's name in the application's configuration directory.
const FILE: &str = "agent-consent.json";

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    version: u32,
    #[serde(default)]
    projects: BTreeMap<String, Entry>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Entry {
    live: bool,
    /// When the person last changed it, as RFC 3339 in UTC.
    changed_at: String,
}

/// The consents this window knows, and where they are kept.
#[derive(Clone, Debug, Default)]
pub struct AgentConsent {
    /// `None` when the system has no configuration directory, and in tests
    /// that do not name one: consent then lasts for this window only.
    path: Option<PathBuf>,
    /// The file's entries as last read.
    projects: BTreeMap<String, Entry>,
    /// Choices this window could not keep in the file: they last for the
    /// window and win over the file.
    unsaved: BTreeMap<String, Entry>,
    /// When the file was last read, so a change another window made is
    /// followed within a second.
    read_at: Option<Instant>,
}

/// What the file holds.
enum Read {
    Entries(BTreeMap<String, Entry>),
    /// A version this build does not know: never overwritten.
    Newer,
}

impl AgentConsent {
    /// The consents kept in the user's configuration directory.
    pub fn load() -> Self {
        Self::at(default_path())
    }

    /// The consents kept under `configuration`, the directory
    /// `XDG_CONFIG_HOME` names: `apiforge/agent-consent.json` in it. Tests
    /// name their own.
    #[cfg(test)]
    pub fn in_configuration(configuration: &Path) -> Self {
        Self::at(Some(path_in(configuration)))
    }

    fn at(path: Option<PathBuf>) -> Self {
        let mut consent = Self {
            path,
            ..Self::default()
        };
        consent.reread();
        consent
    }

    /// Whether the person allowed the agent's live requests in the project
    /// whose manifest id is `project`. Off for a project never set.
    pub fn live(&self, project: &str) -> bool {
        self.unsaved
            .get(project)
            .or_else(|| self.projects.get(project))
            .is_some_and(|entry| entry.live)
    }

    /// Reads the file again when it was last read over a second ago: another
    /// window may have changed a project's consent.
    pub fn follow(&mut self) {
        if self
            .read_at
            .is_none_or(|read| read.elapsed() >= Duration::from_secs(1))
        {
            self.reread();
        }
    }

    fn reread(&mut self) {
        self.read_at = Some(Instant::now());
        if let Some(path) = &self.path {
            // A newer version's file is no consent for this build.
            self.projects = match read(path) {
                Read::Entries(projects) => projects,
                Read::Newer => BTreeMap::new(),
            };
        }
    }

    /// The person's choice for `project`, kept at once. The file is read
    /// again first and only this project's entry replaced; a file written
    /// by a newer version is left alone, and a choice that cannot be written
    /// lasts for this window.
    pub fn set(&mut self, project: &str, live: bool) {
        let entry = Entry {
            live,
            changed_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        };
        let mut newer = false;
        let written = self.path.as_deref().and_then(|path| match read(path) {
            Read::Newer => {
                newer = true;
                None
            }
            Read::Entries(mut projects) => {
                projects.insert(project.to_owned(), entry.clone());
                let stored = Stored {
                    version: VERSION,
                    projects,
                };
                let bytes = serde_json::to_vec_pretty(&stored).ok()?;
                apiforge_storage::write_atomic(path, &bytes).ok()?;
                Some(stored.projects)
            }
        });
        match written {
            Some(projects) => {
                self.projects = projects;
                self.unsaved.remove(project);
            }
            None => {
                // A newer version's file is no consent for this build, as
                // when it is read again.
                if newer {
                    self.projects.clear();
                }
                self.unsaved.insert(project.to_owned(), entry);
            }
        }
        self.read_at = Some(Instant::now());
    }
}

fn path_in(configuration: &Path) -> PathBuf {
    configuration.join("apiforge").join(FILE)
}

/// What `path` holds: nothing when it is missing, unreadable, oversized or
/// not JSON of this shape, which the next change replaces as preferences
/// are; a newer version is said so, and kept.
fn read(path: &Path) -> Read {
    let Ok(file) = std::fs::File::open(path) else {
        return Read::Entries(BTreeMap::new());
    };
    if file
        .metadata()
        .is_ok_and(|metadata| metadata.len() > MAX_BYTES)
    {
        return Read::Entries(BTreeMap::new());
    }
    let Some(stored) = std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
    else {
        return Read::Entries(BTreeMap::new());
    };
    match stored.get("version").and_then(serde_json::Value::as_u64) {
        Some(version) if version > u64::from(VERSION) => Read::Newer,
        _ => Read::Entries(
            serde_json::from_value::<Stored>(stored)
                .ok()
                .filter(|stored| stored.version == VERSION)
                .map(|stored| stored.projects)
                .unwrap_or_default(),
        ),
    }
}

/// Tests never read the developer's own consent; those that cover it name a
/// directory of their own.
#[cfg(test)]
fn default_path() -> Option<PathBuf> {
    None
}

/// Beside `preferences.json`.
#[cfg(not(test))]
fn default_path() -> Option<PathBuf> {
    Some(path_in(&dirs::config_dir()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_consent_is_kept_per_project_beside_the_preferences() {
        let configuration = tempfile::tempdir().unwrap();
        let mut consent = AgentConsent::in_configuration(configuration.path());
        assert!(!consent.live("project-a"), "off for a project never set");
        consent.set("project-a", true);
        let path = configuration.path().join("apiforge/agent-consent.json");
        assert_eq!(
            path.parent(),
            Some(configuration.path().join("apiforge").as_path()),
            "the directory of preferences.json"
        );
        let reread = AgentConsent::in_configuration(configuration.path());
        assert!(reread.live("project-a"));
        assert!(!reread.live("project-b"));
    }

    #[test]
    fn a_change_keeps_what_another_window_wrote_meanwhile() {
        let configuration = tempfile::tempdir().unwrap();
        let mut first = AgentConsent::in_configuration(configuration.path());
        let mut second = AgentConsent::in_configuration(configuration.path());
        first.set("project-a", true);
        second.set("project-b", true);
        let reread = AgentConsent::in_configuration(configuration.path());
        assert!(reread.live("project-a"));
        assert!(reread.live("project-b"));
        second.set("project-a", false);
        assert!(!AgentConsent::in_configuration(configuration.path()).live("project-a"));
    }

    #[test]
    fn an_unreadable_newer_or_oversized_file_is_no_consent() {
        let configuration = tempfile::tempdir().unwrap();
        let path = path_in(configuration.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let entry = r#"{"live": true, "changedAt": "2026-11-01T10:00:00Z"}"#;
        let mut oversized =
            format!(r#"{{"version": 1, "projects": {{"project-a": {entry}}}}}"#).into_bytes();
        oversized.resize(MAX_BYTES as usize + 1, b' ');
        for content in [
            b"not json".to_vec(),
            format!(r#"{{"version": 2, "projects": {{"project-a": {entry}}}}}"#).into_bytes(),
            oversized,
        ] {
            std::fs::write(&path, content).unwrap();
            assert!(!AgentConsent::in_configuration(configuration.path()).live("project-a"));
        }
    }

    #[test]
    fn a_newer_file_is_kept_and_the_choice_lasts_for_the_window() {
        let configuration = tempfile::tempdir().unwrap();
        let path = path_in(configuration.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let newer = br#"{"version": 2, "projects": {}}"#;
        std::fs::write(&path, newer).unwrap();
        let mut consent = AgentConsent::in_configuration(configuration.path());
        consent.set("project-a", true);
        consent.set("project-b", true);
        assert!(consent.live("project-a") && consent.live("project-b"));
        consent.reread();
        assert!(consent.live("project-a"), "kept for the window");
        assert_eq!(std::fs::read(&path).unwrap(), newer, "never overwritten");
    }

    #[test]
    fn another_windows_change_is_followed() {
        let configuration = tempfile::tempdir().unwrap();
        let mut first = AgentConsent::in_configuration(configuration.path());
        let mut second = AgentConsent::in_configuration(configuration.path());
        first.set("project-a", true);
        second.follow();
        assert!(!second.live("project-a"), "not read again within a second");
        second.read_at = Instant::now().checked_sub(Duration::from_secs(2));
        second.follow();
        assert!(second.live("project-a"), "read again after a second");
        first.set("project-a", false);
        second.read_at = Instant::now().checked_sub(Duration::from_secs(2));
        second.follow();
        assert!(!second.live("project-a"));
    }

    #[test]
    fn a_newer_file_written_meanwhile_is_no_consent() {
        let configuration = tempfile::tempdir().unwrap();
        let mut consent = AgentConsent::in_configuration(configuration.path());
        let mut other = AgentConsent::in_configuration(configuration.path());
        other.set("project-a", true);
        consent.reread();
        assert!(consent.live("project-a"));
        std::fs::write(
            path_in(configuration.path()),
            br#"{"version": 2, "projects": {}}"#,
        )
        .unwrap();
        consent.reread();
        assert!(!consent.live("project-a"));
        other.set("project-b", true);
        assert!(
            !other.live("project-a"),
            "a change reads the newer file as none"
        );
        assert!(other.live("project-b"), "the change lasts for the window");
    }

    #[test]
    fn without_a_directory_consent_lasts_for_the_window() {
        let mut consent = AgentConsent::at(None);
        consent.set("project-a", true);
        assert!(consent.live("project-a"));
        assert!(!AgentConsent::at(None).live("project-a"));
    }
}
