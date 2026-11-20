//! LANG-15: a project's manifest rewritten as one of another language, as a
//! project written by another build has it: `formatVersion: 1` without the
//! field for language 1, else `formatVersion: 2` with `languageVersion`.
//!
//! Shared by path (`#[path]`), so it depends only on `apiforge-storage`.
#![allow(dead_code)]

use apiforge_storage::project::{MANIFEST_PATH, ProjectManifest};
use std::{fs, path::Path};

/// Rewrites the manifest of the project at `root` as one of `language`. A
/// manifest already in that language is left alone, byte for byte.
pub fn write_language(root: &Path, language: u32) {
    let path = root.join(MANIFEST_PATH);
    let mut manifest = ProjectManifest::from_bytes(&fs::read(&path).unwrap()).unwrap();
    if manifest.language() == language {
        return;
    }
    (manifest.format_version, manifest.language_version) = match language {
        1 => (1, None),
        _ => (2, Some(language)),
    };
    fs::write(&path, manifest.to_public_bytes().unwrap()).unwrap();
}

/// The language the manifest of the project at `root` names.
pub fn language_of(root: &Path) -> u32 {
    ProjectManifest::from_bytes(&fs::read(root.join(MANIFEST_PATH)).unwrap())
        .unwrap()
        .language()
}
