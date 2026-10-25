//! Opaque, bounded export when damaged transaction intent cannot be interpreted.
use super::{codec, path, *};
use serde::Serialize;
use std::{fs, io::Read};

#[derive(Serialize)]
struct EvidenceFile {
    path: String,
    state: FileState,
    /// Exact unsigned bytes; binary or corrupt input is never decoded as text.
    bytes: Vec<u8>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Evidence {
    format_version: u32,
    files: Vec<EvidenceFile>,
    unavailable: Vec<String>,
}

impl ProjectStore {
    /// Explicit export only. This is evidence for manual recovery, not a replay
    /// plan or a saved generation. No project file or original is modified.
    pub fn export_transaction_evidence(&self) -> Result<Vec<u8>> {
        let _worker = self.owner.worker.lock().unwrap_or_else(|p| p.into_inner());
        let mut evidence = Evidence {
            format_version: 1,
            files: Vec::new(),
            unavailable: Vec::new(),
        };
        let mut remaining = MAX_TRANSACTION_BYTES;
        self.collect_evidence(
            ".apiforge/local/head.json",
            0,
            &mut remaining,
            &mut evidence,
        )?;
        self.collect_evidence(
            ".apiforge/local/transactions",
            2,
            &mut remaining,
            &mut evidence,
        )?;
        codec::encode(&evidence, MAX_TRANSACTION_BYTES)
    }

    fn collect_evidence(
        &self,
        relative: &str,
        depth: u8,
        remaining: &mut usize,
        evidence: &mut Evidence,
    ) -> Result<()> {
        if evidence.files.len() + evidence.unavailable.len() >= MAX_TREE_ENTRIES {
            return Err(ProjectError::Limit);
        }
        let absolute = match path::resolve(self.root(), relative) {
            Ok(path) => path,
            Err(_) => {
                evidence.unavailable.push(relative.into());
                return Ok(());
            }
        };
        let metadata = match fs::symlink_metadata(&absolute) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(_) => {
                evidence.unavailable.push(relative.into());
                return Ok(());
            }
        };
        if metadata.is_dir() && depth > 0 {
            let entries = match fs::read_dir(&absolute) {
                Ok(entries) => entries,
                Err(_) => {
                    evidence.unavailable.push(relative.into());
                    return Ok(());
                }
            };
            let mut names = Vec::new();
            for entry in entries.take(MAX_TREE_ENTRIES + 1) {
                let entry = entry.map_err(|error| ProjectError::io(&absolute, error))?;
                let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                    evidence
                        .unavailable
                        .push(format!("{relative}/[non-UTF-8 name]"));
                    continue;
                };
                names.push(name);
            }
            if names.len() > MAX_TREE_ENTRIES {
                return Err(ProjectError::Limit);
            }
            names.sort();
            for name in names {
                self.collect_evidence(
                    &format!("{relative}/{name}"),
                    depth - 1,
                    remaining,
                    evidence,
                )?;
            }
        } else if metadata.is_file() {
            if metadata.len() > *remaining as u64 {
                return Err(ProjectError::Limit);
            }
            let read: Result<Vec<u8>> = (|| {
                path::check_boundary(&absolute)?;
                let file = fs::File::open(&absolute)
                    .map_err(|error| ProjectError::io(&absolute, error))?;
                let mut bytes = Vec::new();
                file.take(*remaining as u64 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|error| ProjectError::io(&absolute, error))?;
                Ok(bytes)
            })();
            match read {
                Ok(bytes) => {
                    if bytes.len() > *remaining {
                        return Err(ProjectError::Limit);
                    }
                    *remaining -= bytes.len();
                    evidence.files.push(EvidenceFile {
                        path: relative.into(),
                        state: FileState::of(&bytes),
                        bytes,
                    });
                }
                Err(_) => evidence.unavailable.push(relative.into()),
            }
        } else {
            evidence.unavailable.push(relative.into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrupt_intent_and_binary_staging_export_exactly_without_replay_or_local_writes() {
        let directory = tempfile::tempdir().unwrap();
        let transaction = directory.path().join(".apiforge/local/transactions/broken");
        fs::create_dir_all(&transaction).unwrap();
        fs::write(
            directory.path().join(".apiforge/local/head.json"),
            b"future head",
        )
        .unwrap();
        fs::write(
            transaction.join("transaction.json"),
            b"corrupt descriptor\r\n",
        )
        .unwrap();
        fs::write(transaction.join("0"), [255, 0, 254]).unwrap();
        let store = ProjectStore::open_read_only(directory.path()).unwrap();
        let bytes = store.export_transaction_evidence().unwrap();
        let exported: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let files = exported["files"].as_array().unwrap();
        assert_eq!(files.len(), 3);
        let binary = files
            .iter()
            .find(|file| file["path"].as_str().unwrap().ends_with("/0"))
            .unwrap();
        assert_eq!(binary["bytes"], serde_json::json!([255, 0, 254]));
        assert_eq!(
            fs::read(transaction.join("transaction.json")).unwrap(),
            b"corrupt descriptor\r\n"
        );
        assert!(
            !directory
                .path()
                .join(".apiforge/local/project.lock")
                .exists()
        );
    }

    #[cfg(unix)]
    #[test]
    fn evidence_never_follows_a_staged_link_outside_the_project() {
        let directory = tempfile::tempdir().unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        fs::write(outside.path(), b"outside bytes").unwrap();
        let transaction = directory.path().join(".apiforge/local/transactions/id");
        fs::create_dir_all(&transaction).unwrap();
        std::os::unix::fs::symlink(outside.path(), transaction.join("0")).unwrap();
        let store = ProjectStore::open_read_only(directory.path()).unwrap();
        let bytes = store.export_transaction_evidence().unwrap();
        let exported: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(exported["files"].as_array().unwrap().is_empty());
        assert_eq!(exported["unavailable"].as_array().unwrap().len(), 1);
        assert!(!String::from_utf8(bytes).unwrap().contains("outside bytes"));
    }
}
