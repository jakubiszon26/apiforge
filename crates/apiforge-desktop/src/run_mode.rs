//! The run mode the header's Run button runs, per person and per project
//! (UX-02).
//!
//! It belongs to the person and to this machine, as MCP-09's consent does:
//! it is kept in `run-mode.json` beside `agent-consent.json` in the user's
//! configuration directory, keyed by the project's manifest id, never in the
//! manifest or under the project folder. A project never set is in
//! Simulation.
//!
//! Every read and write is best effort: a file that cannot be read is
//! Simulation for every project, and a choice that cannot be written lasts
//! for this window only. Windows share the file without a lock: a change
//! reads it again and replaces only its own project's entry, written
//! atomically. Unlike the consent, the file is not read again while the
//! window is open: the mode is the person's choice at this window, and the
//! last one written wins, as with the preferences. It is kept apart from
//! `consent.rs` so that MCP-09's file and its versions stay as they are.

pub use apiforge_core::run::RunMode;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

/// The file format. An unknown version is Simulation for every project.
const VERSION: u32 = 1;
/// A run mode file is small; anything larger is not one.
const MAX_BYTES: u64 = 1024 * 1024;
/// The file's name in the application's configuration directory.
const FILE: &str = "run-mode.json";

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
    /// `simulation` or `live`.
    mode: RunMode,
    /// When the person last changed it, as RFC 3339 in UTC.
    changed_at: String,
}

/// The run modes this window knows, and where they are kept.
#[derive(Clone, Debug, Default)]
pub struct RunModeStore {
    /// `None` when the system has no configuration directory, and in tests
    /// that do not name one: a choice then lasts for this window only.
    path: Option<PathBuf>,
    /// The file's entries as last read.
    projects: BTreeMap<String, Entry>,
    /// Choices this window could not keep in the file: they last for the
    /// window and win over the file.
    unsaved: BTreeMap<String, Entry>,
}

/// What the file holds.
enum Read {
    Entries(BTreeMap<String, Entry>),
    /// A version this build does not know: never overwritten.
    Newer,
}

impl RunModeStore {
    /// The run modes kept in the user's configuration directory.
    pub fn load() -> Self {
        Self::at(default_path())
    }

    /// The run modes kept under `configuration`, the directory
    /// `XDG_CONFIG_HOME` names: `apiforge/run-mode.json` in it. Tests name
    /// their own.
    #[cfg(test)]
    pub fn in_configuration(configuration: &Path) -> Self {
        Self::at(Some(path_in(configuration)))
    }

    fn at(path: Option<PathBuf>) -> Self {
        let projects = match path.as_deref().map(read) {
            Some(Read::Entries(projects)) => projects,
            Some(Read::Newer) | None => BTreeMap::new(),
        };
        Self {
            path,
            projects,
            unsaved: BTreeMap::new(),
        }
    }

    /// The mode of the project whose manifest id is `project`: Simulation
    /// for a project never set.
    pub fn mode(&self, project: &str) -> RunMode {
        self.unsaved
            .get(project)
            .or_else(|| self.projects.get(project))
            .map_or(RunMode::Simulation, |entry| entry.mode)
    }

    /// The person's choice for `project`, kept at once. The file is read
    /// again first and only this project's entry replaced; a file written
    /// by a newer version is left alone, and a choice that cannot be written
    /// lasts for this window.
    pub fn set(&mut self, project: &str, mode: RunMode) {
        let entry = Entry {
            mode,
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
                // A newer version's file is Simulation for this build, as
                // when it is read.
                if newer {
                    self.projects.clear();
                }
                self.unsaved.insert(project.to_owned(), entry);
            }
        }
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

/// Tests never read the developer's own run modes; those that cover them
/// name a directory of their own.
#[cfg(test)]
fn default_path() -> Option<PathBuf> {
    None
}

/// Beside `agent-consent.json` and `preferences.json`.
#[cfg(not(test))]
fn default_path() -> Option<PathBuf> {
    Some(path_in(&dirs::config_dir()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const LIVE_ENTRY: &str = r#"{"mode": "live", "changedAt": "2026-11-02T10:00:00Z"}"#;

    fn stored(configuration: &Path) -> Value {
        serde_json::from_slice(&std::fs::read(path_in(configuration)).unwrap()).unwrap()
    }

    #[test]
    fn a_mode_is_kept_per_project_beside_the_consent() {
        let configuration = tempfile::tempdir().unwrap();
        let mut store = RunModeStore::in_configuration(configuration.path());
        assert_eq!(
            store.mode("project-a"),
            RunMode::Simulation,
            "Simulation for a project never set"
        );
        store.set("project-a", RunMode::Live);
        assert_eq!(store.mode("project-a"), RunMode::Live);
        let path = configuration.path().join("apiforge/run-mode.json");
        assert_eq!(
            path.parent(),
            Some(configuration.path().join("apiforge").as_path()),
            "the directory of agent-consent.json"
        );
        let file = stored(configuration.path());
        assert_eq!(file["version"], 1, "{file}");
        assert_eq!(file["projects"]["project-a"]["mode"], "live", "{file}");
        assert!(
            file["projects"]["project-a"]["changedAt"]
                .as_str()
                .is_some_and(|at| at.ends_with('Z')),
            "{file}"
        );
        let reread = RunModeStore::in_configuration(configuration.path());
        assert_eq!(reread.mode("project-a"), RunMode::Live);
        assert_eq!(reread.mode("project-b"), RunMode::Simulation);
        store.set("project-a", RunMode::Simulation);
        assert_eq!(
            RunModeStore::in_configuration(configuration.path()).mode("project-a"),
            RunMode::Simulation
        );
        assert_eq!(
            stored(configuration.path())["projects"]["project-a"]["mode"],
            "simulation"
        );
    }

    #[test]
    fn a_write_keeps_the_other_projects_entries() {
        let configuration = tempfile::tempdir().unwrap();
        let mut first = RunModeStore::in_configuration(configuration.path());
        let mut second = RunModeStore::in_configuration(configuration.path());
        first.set("project-a", RunMode::Live);
        second.set("project-b", RunMode::Live);
        let reread = RunModeStore::in_configuration(configuration.path());
        assert_eq!(reread.mode("project-a"), RunMode::Live);
        assert_eq!(reread.mode("project-b"), RunMode::Live);
        second.set("project-a", RunMode::Simulation);
        let reread = RunModeStore::in_configuration(configuration.path());
        assert_eq!(reread.mode("project-a"), RunMode::Simulation);
        assert_eq!(reread.mode("project-b"), RunMode::Live);
    }

    #[test]
    fn an_unreadable_newer_or_oversized_file_is_simulation() {
        let configuration = tempfile::tempdir().unwrap();
        let path = path_in(configuration.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            format!(r#"{{"version": 1, "projects": {{"project-a": {LIVE_ENTRY}}}}}"#),
        )
        .unwrap();
        assert_eq!(
            RunModeStore::in_configuration(configuration.path()).mode("project-a"),
            RunMode::Live,
            "a readable file of this version is read"
        );
        let mut oversized =
            format!(r#"{{"version": 1, "projects": {{"project-a": {LIVE_ENTRY}}}}}"#).into_bytes();
        oversized.resize(MAX_BYTES as usize + 1, b' ');
        for content in [
            b"not json".to_vec(),
            br#"{"version": 1, "projects": {"project-a": {"mode": "fast"}}}"#.to_vec(),
            format!(r#"{{"version": 2, "projects": {{"project-a": {LIVE_ENTRY}}}}}"#).into_bytes(),
            oversized,
        ] {
            std::fs::write(&path, content).unwrap();
            assert_eq!(
                RunModeStore::in_configuration(configuration.path()).mode("project-a"),
                RunMode::Simulation
            );
        }
    }

    #[test]
    fn a_newer_file_is_never_overwritten_and_the_choice_lasts_for_the_window() {
        let configuration = tempfile::tempdir().unwrap();
        let path = path_in(configuration.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let newer = br#"{"version": 2, "projects": {}}"#;
        std::fs::write(&path, newer).unwrap();
        let mut store = RunModeStore::in_configuration(configuration.path());
        store.set("project-a", RunMode::Live);
        assert_eq!(
            store.mode("project-a"),
            RunMode::Live,
            "kept for the window"
        );
        assert_eq!(std::fs::read(&path).unwrap(), newer, "never overwritten");
        assert_eq!(
            RunModeStore::in_configuration(configuration.path()).mode("project-a"),
            RunMode::Simulation,
            "another window does not see it"
        );
    }

    #[test]
    fn without_a_directory_the_choice_lasts_for_the_window() {
        let mut store = RunModeStore::at(None);
        store.set("project-a", RunMode::Live);
        assert_eq!(store.mode("project-a"), RunMode::Live);
        assert_eq!(
            RunModeStore::at(None).mode("project-a"),
            RunMode::Simulation
        );
    }
}
