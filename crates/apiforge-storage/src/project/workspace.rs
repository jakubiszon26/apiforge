//! Local view state of a folder project: the last mode, open tabs and where each
//! view was. It is a convenience, never user work: an unreadable, foreign or
//! future file is ignored and replaced by the next write. Writes use the project
//! writer lock and are refused in read-only sessions.
use super::{codec, recovery::TextSelection, *};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub(super) const WORKSPACE_PATH: &str = ".apiforge/local/workspace.json";
const MAX_WORKSPACE_BYTES: usize = 1024 * 1024;
const MAX_TABS: usize = 256;
const MAX_EXPANDED: usize = 4096;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ViewMode {
    /// New projects start with their sources.
    #[default]
    Code,
    Diagram,
    Split,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CameraState {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TabState {
    pub document_id: String,
    /// Directional selection in the source; checked against the text on use.
    pub selection: TextSelection,
    /// Vertical scroll offset of the code view, in points.
    pub scroll: f32,
    /// Whether the diagram shows a scenario's cleanup graph.
    #[serde(default)]
    pub cleanup: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<CameraState>,
}

/// Width of the block library when a view state does not record one.
pub const DEFAULT_LIBRARY_WIDTH: f32 = 240.0;

fn default_library_width() -> f32 {
    DEFAULT_LIBRARY_WIDTH
}

/// Panel sizes are the one part of the view state that ignores unknown fields
/// (KTLO-05c): a size added by a newer build is dropped, not a reason to lose
/// the whole view.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PanelSizes {
    /// Width of the document list beside the code.
    pub documents: f32,
    /// Width of the code beside the diagram in Split.
    pub split_code: f32,
    /// Height of the diagnostics below the code.
    pub diagnostics: f32,
    /// Width of the block library beside the board.
    #[serde(default = "default_library_width")]
    pub library: f32,
    /// Whether the block library is folded to its rail (KTLO-17). Written only
    /// when set, so an expanded library leaves the file as before.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub library_collapsed: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceState {
    pub format_version: u32,
    pub project_id: String,
    pub mode: ViewMode,
    /// Tab order; one tab per document.
    pub tabs: Vec<TabState>,
    pub active: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panels: Option<PanelSizes>,
    /// Explorer folders shown expanded, as project-relative paths.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expanded: Vec<String>,
}

impl WorkspaceState {
    pub fn new(project_id: impl Into<String>) -> Self {
        Self {
            format_version: 1,
            project_id: project_id.into(),
            mode: ViewMode::default(),
            tabs: Vec::new(),
            active: None,
            panels: None,
            expanded: Vec::new(),
        }
    }

    fn validate(&self) -> Result<()> {
        if self.format_version != 1 {
            return Err(ProjectError::UnsupportedVersion);
        }
        if self.tabs.len() > MAX_TABS || self.expanded.len() > MAX_EXPANDED {
            return Err(ProjectError::Limit);
        }
        if self
            .expanded
            .iter()
            .any(|path| super::path::validate_relative(path).is_err())
        {
            return Err(ProjectError::InvalidMetadata);
        }
        let mut seen = HashSet::new();
        for tab in &self.tabs {
            if tab.document_id.is_empty() || !seen.insert(tab.document_id.as_str()) {
                return Err(ProjectError::InvalidMetadata);
            }
            let finite_camera = tab.camera.is_none_or(|camera| {
                camera.x.is_finite()
                    && camera.y.is_finite()
                    && camera.zoom.is_finite()
                    && camera.zoom > 0.0
            });
            if !tab.scroll.is_finite() || tab.scroll < 0.0 || !finite_camera {
                return Err(ProjectError::InvalidMetadata);
            }
        }
        if self
            .active
            .as_ref()
            .is_some_and(|active| !seen.contains(active.as_str()))
        {
            return Err(ProjectError::InvalidMetadata);
        }
        if let Some(panels) = &self.panels
            && ![
                panels.documents,
                panels.split_code,
                panels.diagnostics,
                panels.library,
            ]
            .iter()
            .all(|size| size.is_finite() && *size >= 0.0)
        {
            return Err(ProjectError::InvalidMetadata);
        }
        Ok(())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_WORKSPACE_BYTES {
            return Err(ProjectError::Limit);
        }
        let state: Self =
            serde_json::from_slice(bytes).map_err(|_| ProjectError::InvalidMetadata)?;
        state.validate()?;
        Ok(state)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        codec::encode(self, MAX_WORKSPACE_BYTES)
    }
}

impl ProjectStore {
    /// The view state of the last session of this project. `None` when there is
    /// none, or when it is unreadable, unsupported or belongs to another project:
    /// a view state never blocks opening the project.
    pub fn load_workspace_state(&self, project_id: &str) -> Result<Option<WorkspaceState>> {
        let path = ProjectPath::new(WORKSPACE_PATH)?;
        let snapshot = self.read(std::slice::from_ref(&path))?;
        Ok(snapshot.files[&path]
            .as_deref()
            .and_then(|bytes| WorkspaceState::from_bytes(bytes).ok())
            .filter(|state| state.project_id == project_id))
    }

    /// Replaces the view state atomically. Read-only sessions cannot write it.
    pub fn save_workspace_state(&self, state: &WorkspaceState) -> Result<()> {
        self.writable()?;
        let bytes = state.to_bytes()?;
        self.check_secrets(&bytes)?;
        let _worker = self.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
        self.atomic(&path::resolve(self.root(), WORKSPACE_PATH)?, &bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, ProjectStore) {
        let directory = tempfile::tempdir().unwrap();
        let store = ProjectStore::open(directory.path()).unwrap();
        (directory, store)
    }

    fn state() -> WorkspaceState {
        WorkspaceState {
            mode: ViewMode::Split,
            tabs: vec![TabState {
                document_id: "doc".into(),
                selection: TextSelection {
                    anchor: 4,
                    caret: 1,
                },
                scroll: 120.0,
                cleanup: true,
                camera: Some(CameraState {
                    x: -10.0,
                    y: 4.5,
                    zoom: 0.75,
                }),
            }],
            active: Some("doc".into()),
            panels: Some(PanelSizes {
                documents: 220.0,
                split_code: 480.0,
                diagnostics: 140.0,
                library: 260.0,
                library_collapsed: true,
            }),
            expanded: vec!["flows".into(), "flows/nested".into()],
            ..WorkspaceState::new("project")
        }
    }

    #[test]
    fn view_state_round_trips_for_its_project_only() {
        let (_directory, store) = store();
        assert_eq!(store.load_workspace_state("project").unwrap(), None);
        store.save_workspace_state(&state()).unwrap();
        assert_eq!(
            store.load_workspace_state("project").unwrap(),
            Some(state())
        );
        assert_eq!(store.load_workspace_state("another").unwrap(), None);
    }

    #[test]
    fn damaged_or_future_view_state_is_ignored_and_replaced() {
        let (directory, store) = store();
        let path = directory.path().join(WORKSPACE_PATH);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        for bytes in [
            b"{".to_vec(),
            br#"{"formatVersion":2,"projectId":"project","mode":"code","tabs":[],"active":null}"#
                .to_vec(),
            br#"{"formatVersion":1,"projectId":"project","mode":"code","tabs":[],"active":"x"}"#
                .to_vec(),
        ] {
            std::fs::write(&path, bytes).unwrap();
            assert_eq!(store.load_workspace_state("project").unwrap(), None);
        }
        store.save_workspace_state(&state()).unwrap();
        assert_eq!(
            store.load_workspace_state("project").unwrap(),
            Some(state())
        );
    }

    #[test]
    fn invalid_view_state_is_never_written() {
        let (_directory, store) = store();
        let mut duplicate = state();
        duplicate.tabs.push(duplicate.tabs[0].clone());
        let mut infinite = state();
        infinite.tabs[0].scroll = f32::INFINITY;
        let mut zoom = state();
        zoom.tabs[0].camera = Some(CameraState {
            x: 0.0,
            y: 0.0,
            zoom: 0.0,
        });
        for invalid in [duplicate, infinite, zoom] {
            assert!(store.save_workspace_state(&invalid).is_err());
        }
        assert_eq!(store.load_workspace_state("project").unwrap(), None);
    }

    /// Writes `panels` into an otherwise valid view state on disk, loads it, saves
    /// it again and returns the panel sizes as written by this build.
    fn panels_through_load_and_save(panels: serde_json::Value) -> serde_json::Value {
        let (directory, store) = store();
        let path = directory.path().join(WORKSPACE_PATH);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let state = serde_json::json!({
            "formatVersion": 1,
            "projectId": "project",
            "mode": "diagram",
            "tabs": [],
            "active": null,
            "panels": panels,
        });
        std::fs::write(&path, serde_json::to_vec(&state).unwrap()).unwrap();
        let loaded = store
            .load_workspace_state("project")
            .unwrap()
            .expect("the view state loads");
        assert_eq!(loaded.mode, ViewMode::Diagram);
        store.save_workspace_state(&loaded).unwrap();
        let written: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        written["panels"].clone()
    }

    /// KTLO-05c criterion 1: the library panel's width round-trips.
    #[test]
    fn the_library_panel_width_round_trips() {
        let panels = serde_json::json!({
            "documents": 220.0,
            "splitCode": 480.0,
            "diagnostics": 140.0,
            "library": 287.5,
        });
        assert_eq!(panels_through_load_and_save(panels.clone()), panels);
    }

    /// KTLO-05c criterion 2: a view state written before the library width was
    /// remembered loads with the default width (240, the panel's former fixed
    /// default).
    #[test]
    fn a_view_state_without_a_library_width_loads_with_the_default() {
        let panels = panels_through_load_and_save(serde_json::json!({
            "documents": 220.0,
            "splitCode": 480.0,
            "diagnostics": 140.0,
        }));
        assert_eq!(
            panels,
            serde_json::json!({
                "documents": 220.0,
                "splitCode": 480.0,
                "diagnostics": 140.0,
                "library": 240.0,
            })
        );
    }

    /// KTLO-05c criterion 3: a panel size from a newer build is ignored; the
    /// known sizes keep their values. The rest of the view state still rejects
    /// unknown fields (the manifest's own check is
    /// `schema_rejects_unknown_versions_fields_and_collisions`).
    #[test]
    fn an_unknown_panel_size_is_dropped_and_the_known_ones_kept() {
        let panels = panels_through_load_and_save(serde_json::json!({
            "documents": 220.0,
            "splitCode": 480.0,
            "diagnostics": 140.0,
            "library": 300.0,
            "inspectorFromTheFuture": 99.0,
        }));
        assert_eq!(
            panels,
            serde_json::json!({
                "documents": 220.0,
                "splitCode": 480.0,
                "diagnostics": 140.0,
                "library": 300.0,
            })
        );
        for unknown in [
            br#"{"formatVersion":1,"projectId":"project","mode":"code","tabs":[],"active":null,"future":1}"#
                .to_vec(),
            br#"{"formatVersion":1,"projectId":"project","mode":"code","tabs":[{"documentId":"doc","selection":{"anchor":0,"caret":0},"scroll":0.0,"future":1}],"active":null}"#
                .to_vec(),
        ] {
            assert!(
                WorkspaceState::from_bytes(&unknown).is_err(),
                "unknown fields outside the panel sizes are still rejected"
            );
        }
    }

    /// KTLO-17 criterion 6: a collapsed block library round-trips with the
    /// panel sizes.
    #[test]
    fn the_library_collapsed_flag_round_trips() {
        let panels = serde_json::json!({
            "documents": 220.0,
            "splitCode": 480.0,
            "diagnostics": 140.0,
            "library": 287.5,
            "libraryCollapsed": true,
        });
        assert_eq!(panels_through_load_and_save(panels.clone()), panels);
    }

    /// KTLO-17 criterion 6: a view state written before the flag existed opens
    /// the library expanded; an expanded library adds nothing to the file.
    #[test]
    fn a_view_state_without_the_collapsed_flag_opens_expanded() {
        let panels = serde_json::json!({
            "documents": 220.0,
            "splitCode": 480.0,
            "diagnostics": 140.0,
            "library": 240.0,
        });
        assert_eq!(panels_through_load_and_save(panels.clone()), panels);
        let expanded = panels_through_load_and_save(serde_json::json!({
            "documents": 220.0,
            "splitCode": 480.0,
            "diagnostics": 140.0,
            "library": 240.0,
            "libraryCollapsed": false,
        }));
        assert_eq!(expanded, panels);
    }

    /// KTLO-17 criterion 6 (KTLO-05c criterion 3 extended): the panel sizes this
    /// build writes with the flag load in a build without it, whose panel sizes
    /// are KTLO-05c's: tolerant of unknown fields, with no `libraryCollapsed`.
    #[test]
    fn a_view_state_with_the_collapsed_flag_loads_in_a_build_without_it() {
        #[derive(Debug, Deserialize, PartialEq)]
        #[serde(rename_all = "camelCase")]
        struct PanelSizesOfKtlo05c {
            documents: f32,
            split_code: f32,
            diagnostics: f32,
            #[serde(default = "default_library_width")]
            library: f32,
        }
        let written = panels_through_load_and_save(serde_json::json!({
            "documents": 220.0,
            "splitCode": 480.0,
            "diagnostics": 140.0,
            "library": 300.0,
            "libraryCollapsed": true,
        }));
        assert_eq!(
            written["libraryCollapsed"],
            serde_json::json!(true),
            "this build writes the flag"
        );
        let older: PanelSizesOfKtlo05c = serde_json::from_value(written).unwrap();
        assert_eq!(
            older,
            PanelSizesOfKtlo05c {
                documents: 220.0,
                split_code: 480.0,
                diagnostics: 140.0,
                library: 300.0,
            },
            "the older build keeps every size it knows"
        );
    }

    /// KTLO-11 criterion 6: the chosen *Files* tab of the left panel in
    /// Diagram round-trips with the panel sizes.
    #[test]
    fn the_files_tab_round_trips() {
        let panels = serde_json::json!({
            "documents": 220.0,
            "splitCode": 480.0,
            "diagnostics": 140.0,
            "library": 380.0,
            "libraryFiles": true,
        });
        assert_eq!(panels_through_load_and_save(panels.clone()), panels);
        let collapsed = serde_json::json!({
            "documents": 220.0,
            "splitCode": 480.0,
            "diagnostics": 140.0,
            "library": 380.0,
            "libraryCollapsed": true,
            "libraryFiles": true,
        });
        assert_eq!(
            panels_through_load_and_save(collapsed.clone()),
            collapsed,
            "the tab is kept with the rail (KTLO-17)"
        );
    }

    /// KTLO-11 criterion 6: a view state without the field opens on *Library*,
    /// and *Library* adds nothing to the file.
    #[test]
    fn a_view_state_without_the_files_tab_opens_on_the_library() {
        let panels = serde_json::json!({
            "documents": 220.0,
            "splitCode": 480.0,
            "diagnostics": 140.0,
            "library": 240.0,
        });
        assert_eq!(panels_through_load_and_save(panels.clone()), panels);
        let library = panels_through_load_and_save(serde_json::json!({
            "documents": 220.0,
            "splitCode": 480.0,
            "diagnostics": 140.0,
            "library": 240.0,
            "libraryFiles": false,
        }));
        assert_eq!(library, panels);
    }

    /// KTLO-11 criterion 6 (KTLO-05c's tolerance of unknown fields): the panel
    /// sizes this build writes with the *Files* tab load in a build without
    /// the field, whose panel sizes are KTLO-17's.
    #[test]
    fn a_view_state_with_the_files_tab_loads_in_a_build_without_it() {
        #[derive(Debug, Deserialize, PartialEq)]
        #[serde(rename_all = "camelCase")]
        struct PanelSizesOfKtlo17 {
            documents: f32,
            split_code: f32,
            diagnostics: f32,
            #[serde(default = "default_library_width")]
            library: f32,
            #[serde(default)]
            library_collapsed: bool,
        }
        let written = panels_through_load_and_save(serde_json::json!({
            "documents": 220.0,
            "splitCode": 480.0,
            "diagnostics": 140.0,
            "library": 400.0,
            "libraryCollapsed": true,
            "libraryFiles": true,
        }));
        assert_eq!(
            written["libraryFiles"],
            serde_json::json!(true),
            "this build writes the field"
        );
        let older: PanelSizesOfKtlo17 = serde_json::from_value(written).unwrap();
        assert_eq!(
            older,
            PanelSizesOfKtlo17 {
                documents: 220.0,
                split_code: 480.0,
                diagnostics: 140.0,
                library: 400.0,
                library_collapsed: true,
            },
            "the older build keeps every size and flag it knows"
        );
    }

    #[test]
    fn a_read_only_session_does_not_write_view_state() {
        let (directory, _writer) = store();
        let reader = ProjectStore::open(directory.path()).unwrap();
        assert!(reader.is_read_only());
        assert!(matches!(
            reader.save_workspace_state(&state()),
            Err(ProjectError::ReadOnly)
        ));
    }
}
