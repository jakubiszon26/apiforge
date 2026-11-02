//! The window's run mode, per person and per project (UX-02). Acceptance
//! seam: every project is in Simulation until the implementation.

pub use apiforge_core::run::RunMode;
use std::path::{Path, PathBuf};

/// A run mode file is small; anything larger is not one.
const MAX_BYTES: u64 = 1024 * 1024;
/// The file's name in the application's configuration directory.
const FILE: &str = "run-mode.json";

#[derive(Clone, Debug, Default)]
pub struct RunModeStore {
    path: Option<PathBuf>,
}

impl RunModeStore {
    #[cfg(test)]
    pub fn in_configuration(configuration: &Path) -> Self {
        Self::at(Some(path_in(configuration)))
    }

    fn at(path: Option<PathBuf>) -> Self {
        Self { path }
    }

    pub fn mode(&self, _project: &str) -> RunMode {
        let _ = &self.path;
        RunMode::Simulation
    }

    pub fn set(&mut self, _project: &str, _mode: RunMode) {}
}

fn path_in(configuration: &Path) -> PathBuf {
    configuration.join("apiforge").join(FILE)
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
            format!(r#"{{"version": 1, "projects": {{"project-a": {{"mode": "fast"}}}}}}"#)
                .into_bytes(),
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
