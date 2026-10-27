//! The project directory as the explorer presents it. Scans list the real tree
//! without following links and hide `.git` and `.apiforge`; they are bounded so
//! that a large or deep folder cannot stall the worker that performs them.
use super::{path, *};
use std::{fs, io};

/// Entries beyond this are not listed and the scan reports truncation.
pub const MAX_TREE_ENTRIES: usize = 10_000;
/// Directories nested deeper than this are listed but not descended into.
pub const MAX_TREE_DEPTH: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeEntryKind {
    Directory,
    /// Sources and the registry carry a content hash; other files only a size,
    /// since the explorer never edits them.
    File {
        bytes: u64,
        sha256: Option<String>,
    },
    /// A symbolic link or junction. It is shown but never followed or changed.
    Link,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeEntry {
    /// Project-relative, `/`-separated.
    pub path: String,
    pub kind: TreeEntryKind,
    /// Whether the name is a valid portable project path. Other names are shown
    /// but cannot be the target or subject of a file operation.
    pub portable: bool,
}

impl TreeEntry {
    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }
    pub fn is_directory(&self) -> bool {
        self.kind == TreeEntryKind::Directory
    }
    pub fn state(&self) -> Option<FileState> {
        match &self.kind {
            TreeEntryKind::File {
                bytes,
                sha256: Some(sha256),
            } => Some(FileState::File {
                bytes: *bytes,
                sha256: sha256.clone(),
            }),
            TreeEntryKind::Directory => Some(FileState::Directory),
            _ => None,
        }
    }
}

/// One consistent listing: no transaction was active or committed while it was
/// taken. Entries are sorted with each directory directly before its contents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeScan {
    pub generation: u64,
    pub entries: Vec<TreeEntry>,
    pub truncated: bool,
}

impl TreeScan {
    pub fn entry(&self, path: &str) -> Option<&TreeEntry> {
        self.entries.iter().find(|entry| entry.path == path)
    }
}

/// Explorer order: a directory directly before its contents, directories before
/// files at every level, names by their lower-case spelling, then exactly.
pub fn tree_order(a: &TreeEntry, b: &TreeEntry) -> std::cmp::Ordering {
    let a_parts: Vec<_> = a.path.split('/').collect();
    let b_parts: Vec<_> = b.path.split('/').collect();
    for index in 0..a_parts.len().min(b_parts.len()) {
        let (left, right) = (a_parts[index], b_parts[index]);
        if left == right {
            continue;
        }
        let left_directory = index + 1 < a_parts.len() || a.is_directory();
        let right_directory = index + 1 < b_parts.len() || b.is_directory();
        return right_directory
            .cmp(&left_directory)
            .then_with(|| left.to_lowercase().cmp(&right.to_lowercase()))
            .then_with(|| left.cmp(right));
    }
    a_parts.len().cmp(&b_parts.len())
}

impl ProjectStore {
    /// Lists the project tree. A transaction in progress, or one committed
    /// during the scan, returns `Busy`; the caller retries on its next poll.
    pub fn scan_tree(&self) -> Result<TreeScan> {
        let before = self.head()?;
        if before.active_transaction_id.is_some() {
            // The scan does not wait for the writer lock. While this owner's
            // worker holds it, the active transaction is a commit in progress;
            // only one left behind with the lock free needs recovery. A
            // reader beside a living writer in another process sees that
            // writer's commit in progress (MCP-08a, planning decision 2).
            // The worker is only asked, never held while the probe waits.
            let own_commit = matches!(
                self.owner.worker.try_lock(),
                Err(std::sync::TryLockError::WouldBlock)
            );
            return Err(
                if own_commit
                    || self.head()?.active_transaction_id.is_none()
                    || (self.is_read_only() && self.has_live_writer()?)
                {
                    ProjectError::Busy
                } else {
                    ProjectError::RecoveryRequired
                },
            );
        }
        let mut entries = Vec::new();
        let mut truncated = false;
        self.scan_directory("", 0, &mut entries, &mut truncated)?;
        if self.head()? != before {
            return Err(ProjectError::Busy);
        }
        entries.sort_by(tree_order);
        Ok(TreeScan {
            generation: before.generation,
            entries,
            truncated,
        })
    }

    fn scan_directory(
        &self,
        relative: &str,
        depth: usize,
        entries: &mut Vec<TreeEntry>,
        truncated: &mut bool,
    ) -> Result<()> {
        let directory = if relative.is_empty() {
            path::check_boundary(self.root())?;
            self.root().to_owned()
        } else {
            self.root().join(relative)
        };
        let listing = match fs::read_dir(&directory) {
            Ok(listing) => listing,
            // Removed between listing its parent and reading it.
            Err(error) if error.kind() == io::ErrorKind::NotFound && !relative.is_empty() => {
                return Ok(());
            }
            Err(error) => return Err(ProjectError::io(&directory, error)),
        };
        for item in listing {
            let item = item.map_err(|error| ProjectError::io(&directory, error))?;
            let Some(name) = item.file_name().to_str().map(str::to_owned) else {
                // Not representable as a portable path; a scan cannot address it.
                continue;
            };
            if relative.is_empty() && is_hidden(&name) {
                continue;
            }
            if entries.len() >= MAX_TREE_ENTRIES {
                *truncated = true;
                return Ok(());
            }
            let path = if relative.is_empty() {
                name.clone()
            } else {
                format!("{relative}/{name}")
            };
            let metadata = match fs::symlink_metadata(item.path()) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(ProjectError::io(&item.path(), error)),
            };
            let portable = path::validate_relative(&path).is_ok()
                && !name.eq_ignore_ascii_case(".git")
                && !name.eq_ignore_ascii_case(".apiforge");
            let kind = if is_link(&metadata) {
                TreeEntryKind::Link
            } else if metadata.is_dir() {
                TreeEntryKind::Directory
            } else {
                if path.ends_with(".afs") || path == MANIFEST_PATH {
                    match hash_file(&item.path()) {
                        Ok((bytes, sha256)) => TreeEntryKind::File {
                            bytes,
                            sha256: Some(sha256),
                        },
                        Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                        Err(error) => return Err(ProjectError::io(&item.path(), error)),
                    }
                } else {
                    TreeEntryKind::File {
                        bytes: metadata.len(),
                        sha256: None,
                    }
                }
            };
            let descend = kind == TreeEntryKind::Directory && depth + 1 < MAX_TREE_DEPTH;
            entries.push(TreeEntry {
                path: path.clone(),
                kind,
                portable,
            });
            if descend {
                self.scan_directory(&path, depth + 1, entries, truncated)?;
            } else if depth + 1 >= MAX_TREE_DEPTH {
                *truncated = true;
            }
        }
        Ok(())
    }

    /// The current state of one path: a file with its hash, a directory or
    /// absence. A file larger than the payload limit is reported as `Limit`.
    pub fn file_state(&self, path: &ProjectPath) -> Result<FileState> {
        self.state(path.as_str())
    }

    /// Every file and every directory below `directory`, each list sorted. Links
    /// and non-portable names refuse the listing: an operation on the directory
    /// could not account for them.
    pub fn list_directory(
        &self,
        directory: &ProjectPath,
    ) -> Result<(Vec<ProjectPath>, Vec<ProjectPath>)> {
        let mut files = Vec::new();
        let mut directories = Vec::new();
        let mut pending = vec![directory.clone()];
        while let Some(current) = pending.pop() {
            let absolute = path::resolve(self.root(), current.as_str())?;
            let listing =
                fs::read_dir(&absolute).map_err(|error| ProjectError::io(&absolute, error))?;
            for item in listing {
                let item = item.map_err(|error| ProjectError::io(&absolute, error))?;
                let name = item
                    .file_name()
                    .to_str()
                    .map(str::to_owned)
                    .ok_or_else(|| ProjectError::UnsafePath { path: item.path() })?;
                let child = ProjectPath::new(format!("{}/{name}", current.as_str()))
                    .map_err(|_| ProjectError::UnsafePath { path: item.path() })?;
                let metadata = fs::symlink_metadata(item.path())
                    .map_err(|error| ProjectError::io(&item.path(), error))?;
                if is_link(&metadata) {
                    return Err(ProjectError::UnsafePath { path: item.path() });
                }
                if files.len() + directories.len() >= MAX_TRANSACTION_OPERATIONS {
                    return Err(ProjectError::Limit);
                }
                if metadata.is_dir() {
                    directories.push(child.clone());
                    pending.push(child);
                } else {
                    files.push(child);
                }
            }
        }
        files.sort();
        directories.sort();
        Ok((files, directories))
    }
}

/// The project's own control directories are never listed.
pub(super) fn is_hidden(name: &str) -> bool {
    name.eq_ignore_ascii_case(".git") || name.eq_ignore_ascii_case(".apiforge")
}

/// Streams the file, so that sources over the size limit also have a stable
/// state for change detection without being held in memory.
fn hash_file(path: &std::path::Path) -> io::Result<(u64, String)> {
    use std::io::Read;
    let mut file = fs::File::open(path)?;
    let mut context = ring::digest::Context::new(&ring::digest::SHA256);
    let mut buffer = vec![0; 64 * 1024];
    let mut total = 0u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total += read as u64;
        context.update(&buffer[..read]);
    }
    let digest = context.finish();
    Ok((
        total,
        digest
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    ))
}

fn is_link(metadata: &fs::Metadata) -> bool {
    let link = metadata.file_type().is_symlink();
    #[cfg(windows)]
    let link = {
        use std::os::windows::fs::MetadataExt;
        link || metadata.file_attributes() & 0x400 != 0
    };
    link
}

#[cfg(test)]
#[path = "explorer_tests.rs"]
mod tests;
