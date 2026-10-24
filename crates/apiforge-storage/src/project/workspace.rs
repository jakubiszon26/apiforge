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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelSizes {
    /// Width of the document list beside the code.
    pub documents: f32,
    /// Width of the code beside the diagram in Split.
    pub split_code: f32,
    /// Height of the diagnostics below the code.
    pub diagnostics: f32,
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
        }
    }

    fn validate(&self) -> Result<()> {
        if self.format_version != 1 {
            return Err(ProjectError::UnsupportedVersion);
        }
        if self.tabs.len() > MAX_TABS {
            return Err(ProjectError::Limit);
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
            && ![panels.documents, panels.split_code, panels.diagnostics]
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
            }),
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
