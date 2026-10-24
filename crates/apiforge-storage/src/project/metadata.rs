//! Deliberate replacement of derived metadata preserves the uninterpreted
//! original before installation. Ordinary saves cannot erase unknown formats.
use super::{io as disk, *};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub(super) const HISTORY: &str = ".apiforge/local/metadata-history";
const DIAGRAM: &str = ".apiforge/diagram.json";

#[derive(Clone)]
pub struct DiagramRecoveryPreview {
    root: PathBuf,
    project_id: String,
    snapshot: ProjectSnapshot,
}
impl DiagramRecoveryPreview {
    pub fn original_bytes(&self) -> Option<&[u8]> {
        self.snapshot
            .files
            .get(&ProjectPath::new(DIAGRAM).expect("constant path"))
            .and_then(|bytes| bytes.as_deref())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetadataReplacement {
    pub generation: u64,
    pub preserved_copy: Option<ProjectPath>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HistoryIndex {
    format_version: u32,
    project_id: String,
    operation_id: String,
    original_path: ProjectPath,
    original_state: FileState,
    copy_path: ProjectPath,
}

impl ProjectStore {
    pub fn preview_diagram_recovery(&self) -> Result<DiagramRecoveryPreview> {
        let _worker = self.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
        let snapshot = self.stable_read(|| {
            Ok(vec![
                ProjectPath::new(MANIFEST_PATH)?,
                ProjectPath::new(DIAGRAM)?,
            ])
        })?;
        let manifest = ProjectManifest::from_bytes(
            snapshot.files[&ProjectPath::new(MANIFEST_PATH)?]
                .as_deref()
                .ok_or(ProjectError::InvalidMetadata)?,
        )?;
        Ok(DiagramRecoveryPreview {
            root: self.root().into(),
            project_id: manifest.id,
            snapshot,
        })
    }

    /// This is explicit metadata recovery, not a new source checkpoint. The
    /// project analyser remains responsible for source/context/semantic provenance.
    pub fn replace_diagram_checkpoint(
        &self,
        preview: &DiagramRecoveryPreview,
        replacement: &DiagramFile,
    ) -> Result<MetadataReplacement> {
        self.writable()?;
        let _worker = self.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
        if preview.root != self.root() {
            return Err(ProjectError::InvalidCheckpoint);
        }
        let current = self.stable_read(|| Ok(preview.snapshot.files.keys().cloned().collect()))?;
        if current.generation != preview.snapshot.generation
            || current.files != preview.snapshot.files
        {
            return Err(ProjectError::Conflict {
                path: self.root().join(DIAGRAM),
            });
        }
        let bytes = replacement.to_bytes()?;
        let (mut changes, preserved_copy) = match preview.original_bytes() {
            Some(bytes) => self.plan_metadata_history(&preview.project_id, DIAGRAM, bytes)?,
            None => (Vec::new(), None),
        };
        changes.push(ProjectChange::Write {
            path: ProjectPath::new(DIAGRAM)?,
            expected: preview
                .snapshot
                .state(&ProjectPath::new(DIAGRAM)?)
                .ok_or(ProjectError::InvalidCheckpoint)?,
            bytes,
        });
        let generation = self.commit_locked(changes, |_| Ok(()))?;
        Ok(MetadataReplacement {
            generation,
            preserved_copy,
        })
    }

    pub(super) fn plan_metadata_history(
        &self,
        project_id: &str,
        original: &str,
        bytes: &[u8],
    ) -> Result<(Vec<ProjectChange>, Option<ProjectPath>)> {
        if bytes.len() > MAX_JSON_BYTES {
            return Err(ProjectError::Limit);
        }
        self.check_secrets(bytes)?;
        let operation = uuid::Uuid::new_v4().to_string();
        let directory = format!("{HISTORY}/{operation}");
        let copy = ProjectPath::new(format!("{directory}/original.bin"))?;
        let index = HistoryIndex {
            format_version: 1,
            project_id: project_id.into(),
            operation_id: operation,
            original_path: ProjectPath::new(original)?,
            original_state: FileState::of(bytes),
            copy_path: copy.clone(),
        };
        let index_bytes = super::codec::encode(&index, MAX_JSON_BYTES)?;
        self.check_secrets(&index_bytes)?;
        let mut changes = Vec::new();
        match self.state(HISTORY)? {
            FileState::Missing => changes.push(ProjectChange::CreateDirectory {
                path: ProjectPath::new(HISTORY)?,
            }),
            FileState::Directory => {}
            _ => {
                return Err(ProjectError::Conflict {
                    path: self.root().join(HISTORY),
                });
            }
        }
        changes.push(ProjectChange::CreateDirectory {
            path: ProjectPath::new(directory.clone())?,
        });
        changes.push(ProjectChange::Write {
            path: copy.clone(),
            expected: FileState::Missing,
            bytes: bytes.to_vec(),
        });
        changes.push(ProjectChange::Write {
            path: ProjectPath::new(format!("{directory}/index.json"))?,
            expected: FileState::Missing,
            bytes: index_bytes,
        });
        Ok((changes, Some(copy)))
    }

    pub(super) fn preserve_local_metadata(
        &self,
        project_id: &str,
        original: &str,
        bytes: &[u8],
    ) -> Result<Option<ProjectPath>> {
        let (changes, copy) = self.plan_metadata_history(project_id, original, bytes)?;
        for change in changes {
            match change {
                ProjectChange::CreateDirectory { path } => {
                    disk::create_directory(&super::path::resolve(self.root(), path.as_str())?)?
                }
                ProjectChange::Write { path, bytes, .. } => disk::atomic(
                    &super::path::resolve(self.root(), path.as_str())?,
                    &bytes,
                    true,
                )?,
                _ => return Err(ProjectError::InvalidMetadata),
            }
        }
        // A previous failed attempt may have left the history root visible but
        // not durable. Flush its name before replacing any original metadata.
        let history = super::path::resolve(self.root(), HISTORY)?;
        disk::sync_directory(&history)?;
        disk::sync_parent(&history)?;
        Ok(copy)
    }
}

#[cfg(test)]
#[path = "metadata_tests.rs"]
mod tests;
