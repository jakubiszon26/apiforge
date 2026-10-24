use super::{ProjectError, Result};
use serde::{Deserialize, Deserializer, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// A portable, normalised project-relative path. Storage control files cannot be
/// addressed through this type; callers cannot overwrite the lock or head.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ProjectPath(String);

impl ProjectPath {
    pub fn new(path: impl Into<String>) -> Result<Self> {
        let path = path.into();
        validate_relative(&path)?;
        let lower = path.to_lowercase();
        if lower.split('/').any(|part| part == ".git") {
            return Err(ProjectError::InvalidPath);
        }
        if lower.split('/').any(|part| part == ".apiforge") {
            let permitted = path == ".apiforge/diagram.json"
                || path == ".apiforge/revisions"
                || path.starts_with(".apiforge/revisions/")
                || path == ".apiforge/local/trash"
                || path.starts_with(".apiforge/local/trash/")
                || path == ".apiforge/local/conflicts"
                || path.starts_with(".apiforge/local/conflicts/")
                || matches!(
                    path.as_str(),
                    ".apiforge/local/recovery.json"
                        | ".apiforge/local/recovery.json.bak"
                        | ".apiforge/local/workspace.json"
                        | ".apiforge/local/last-valid.json"
                );
            if !permitted || lower.split('/').filter(|part| *part == ".apiforge").count() != 1 {
                return Err(ProjectError::InvalidPath);
            }
        }
        Ok(Self(path))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn working_source(path: impl Into<String>) -> Result<Self> {
        let path = Self::new(path)?;
        if path
            .0
            .split('/')
            .any(|part| part.eq_ignore_ascii_case(".apiforge"))
            || !path.0.ends_with(".afs")
        {
            return Err(ProjectError::InvalidPath);
        }
        Ok(path)
    }

    pub(super) fn is_revision(&self) -> bool {
        self.0.starts_with(".apiforge/revisions/")
    }

    pub(super) fn is_conflict_archive(&self) -> bool {
        self.0 == ".apiforge/local/conflicts" || self.0.starts_with(".apiforge/local/conflicts/")
    }
}

impl<'de> Deserialize<'de> for ProjectPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?)
            .map_err(|_| serde::de::Error::custom("invalid project-relative path"))
    }
}

pub(super) fn validate_relative(path: &str) -> Result<()> {
    if path.is_empty() || path.len() > 4096 || path.contains('\\') {
        return Err(ProjectError::InvalidPath);
    }
    for part in path.split('/') {
        let stem = part.split('.').next().unwrap_or_default().to_uppercase();
        if part.is_empty()
            || matches!(part, "." | "..")
            || part.ends_with(['.', ' '])
            || part
                .chars()
                .any(|c| c.is_control() || ":*?\"<>|".contains(c))
            || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || ["COM", "LPT"].iter().any(|prefix| {
                stem.strip_prefix(prefix).is_some_and(|suffix| {
                    ["1", "2", "3", "4", "5", "6", "7", "8", "9", "¹", "²", "³"].contains(&suffix)
                })
            })
        {
            return Err(ProjectError::InvalidPath);
        }
    }
    Ok(())
}

fn reject_link(path: &Path, metadata: &fs::Metadata) -> Result<()> {
    let link = metadata.file_type().is_symlink();
    #[cfg(windows)]
    let link = {
        use std::os::windows::fs::MetadataExt;
        link || metadata.file_attributes() & 0x400 != 0
    };
    if link {
        return Err(ProjectError::UnsafePath {
            path: path.to_owned(),
        });
    }
    Ok(())
}

/// Validate existing ancestors, including those above the root. Repeated directly
/// before I/O; this narrows ordinary races, not hostile ancestor replacement.
pub(super) fn check_boundary(path: &Path) -> Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(ProjectError::InvalidPath);
        }
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => reject_link(&current, &metadata)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(ProjectError::io(&current, error)),
        }
    }
    Ok(())
}

pub(super) fn resolve(root: &Path, relative: &str) -> Result<PathBuf> {
    validate_relative(relative)?;
    check_boundary(root)?;
    let mut current = root.to_owned();
    for part in relative.split('/') {
        // Enforce Windows-compatible case collisions on Linux as well.
        match fs::read_dir(&current) {
            Ok(entries) => {
                for entry in entries {
                    let entry = entry.map_err(|error| ProjectError::io(&current, error))?;
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    if name != part && name.to_lowercase() == part.to_lowercase() {
                        return Err(ProjectError::UnsafePath {
                            path: current.join(part),
                        });
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(ProjectError::io(&current, error)),
        }
        current.push(part);
        check_boundary(&current)?;
    }
    Ok(current)
}
