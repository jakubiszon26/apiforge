//! All folder persistence boundaries pass here. Test-only, thread-local probes
//! inject errors or stop a child process at the actual filesystem operation,
//! including before/after file sync, replacement and directory sync. Other
//! crates' tests reach them through the `fault-injection` feature
//! (`project::faults`), never enabled in a release build.
use super::{ProjectError, Result, path};
use std::{
    ffi::OsString,
    fs::{self, File},
    io::Write,
    path::Path,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boundary {
    TemporaryCreate,
    Write,
    FileSync,
    Replace,
    InstallNew,
    DirectorySync,
    CreateDirectory,
    RemoveFile,
    RemoveDirectory,
    Cleanup,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Before,
    After,
}

pub(super) fn atomic(path: &Path, bytes: &[u8], new_only: bool) -> Result<()> {
    path::check_boundary(path)?;
    let parent = path
        .parent()
        .filter(|parent| parent.is_dir())
        .ok_or(ProjectError::InvalidPath)?;
    probe(Boundary::TemporaryCreate, Phase::Before, path)?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|error| ProjectError::io(parent, error))?;
    probe(Boundary::TemporaryCreate, Phase::After, path)?;
    // Chunk boundaries also cover a partially-written temporary payload failing
    // with ENOSPC. It can never replace the original before the final sync.
    for chunk in bytes.chunks(16 * 1024) {
        probe(Boundary::Write, Phase::Before, path)?;
        temporary
            .write_all(chunk)
            .map_err(|error| ProjectError::io(path, error))?;
        probe(Boundary::Write, Phase::After, path)?;
    }
    probe(Boundary::FileSync, Phase::Before, path)?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| ProjectError::io(path, error))?;
    probe(Boundary::FileSync, Phase::After, path)?;
    let kind = if new_only {
        Boundary::InstallNew
    } else {
        Boundary::Replace
    };
    probe(kind, Phase::Before, path)?;
    // Recheck immediately before installing, rather than only at planning time.
    path::check_boundary(path)?;
    if new_only {
        temporary
            .persist_noclobber(path)
            .map(drop)
            .map_err(|error| error.error)
    } else {
        // A reader holding the target never fails the replace (MCP-19).
        crate::file::install(temporary, path)
    }
    .map_err(|error| ProjectError::io(path, error))?;
    probe(kind, Phase::After, path)?;
    sync_directory(parent)
}

pub(super) fn sync_directory(path: &Path) -> Result<()> {
    path::check_boundary(path)?;
    probe(Boundary::DirectorySync, Phase::Before, path)?;
    crate::file::sync_directory(path).map_err(|error| ProjectError::io(path, error))?;
    probe(Boundary::DirectorySync, Phase::After, path)
}

pub(super) fn sync_file(path: &Path) -> Result<()> {
    path::check_boundary(path)?;
    probe(Boundary::FileSync, Phase::Before, path)?;
    File::options()
        .read(true)
        .write(true)
        .open(path)
        .and_then(|file| file.sync_all())
        .map_err(|error| ProjectError::io(path, error))?;
    probe(Boundary::FileSync, Phase::After, path)
}

pub(super) fn sync_parent(path: &Path) -> Result<()> {
    sync_directory(path.parent().ok_or(ProjectError::InvalidPath)?)
}
pub(super) fn sync_surviving_parent(path: &Path) -> Result<()> {
    let mut parent = path.parent().ok_or(ProjectError::InvalidPath)?;
    while !parent.is_dir() {
        parent = parent.parent().ok_or(ProjectError::InvalidPath)?;
    }
    sync_directory(parent)
}

pub(super) fn create_directory(path: &Path) -> Result<()> {
    path::check_boundary(path)?;
    probe(Boundary::CreateDirectory, Phase::Before, path)?;
    fs::create_dir(path).map_err(|error| ProjectError::io(path, error))?;
    probe(Boundary::CreateDirectory, Phase::After, path)?;
    sync_directory(path)?;
    sync_parent(path)
}

/// Removes the file or empty directory `path`. `removed` names the entries
/// of a directory that its transaction removed before it: on Windows a
/// removed file that another reader still holds keeps its name until that
/// handle closes, and the directory's removal waits for it, at most
/// [`crate::file::HELD_BOUND`] (MCP-19).
pub(super) fn remove(path: &Path, directory: bool, removed: &[OsString]) -> Result<()> {
    path::check_boundary(path)?;
    let kind = if directory {
        Boundary::RemoveDirectory
    } else {
        Boundary::RemoveFile
    };
    probe(kind, Phase::Before, path)?;
    if directory {
        crate::file::while_held(
            || fs::remove_dir(path),
            |error| holds_only(error, path, removed),
        )
    } else {
        fs::remove_file(path)
    }
    .map_err(|error| ProjectError::io(path, error))?;
    probe(kind, Phase::After, path)?;
    sync_parent(path)
}

/// Whether removing the directory `path` failed on Windows only because it
/// still lists names in `removed`, which are delete-pending.
fn holds_only(error: &std::io::Error, path: &Path, removed: &[OsString]) -> bool {
    const ERROR_DIR_NOT_EMPTY: i32 = 145;
    cfg!(windows)
        && error.raw_os_error() == Some(ERROR_DIR_NOT_EMPTY)
        && fs::read_dir(path).is_ok_and(|mut entries| {
            entries.all(|entry| entry.is_ok_and(|entry| removed.contains(&entry.file_name())))
        })
}

pub(super) fn cleanup_directory(path: &Path) -> Result<()> {
    path::check_boundary(path)?;
    probe(Boundary::Cleanup, Phase::Before, path)?;
    fs::remove_dir_all(path).map_err(|error| ProjectError::io(path, error))?;
    probe(Boundary::Cleanup, Phase::After, path)?;
    sync_parent(path)
}

#[cfg(not(any(test, feature = "fault-injection")))]
fn probe(_: Boundary, _: Phase, _: &Path) -> Result<()> {
    Ok(())
}
#[cfg(any(test, feature = "fault-injection"))]
fn probe(boundary: Boundary, phase: Phase, path: &Path) -> Result<()> {
    testing::visit(boundary, phase, path)
}

#[cfg(any(test, feature = "fault-injection"))]
pub(super) mod testing {
    use super::*;
    use std::{cell::RefCell, path::PathBuf};

    /// One filesystem operation at one of its boundaries.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Event {
        pub boundary: Boundary,
        pub phase: Phase,
        pub path: PathBuf,
    }
    type Hook = Box<dyn FnMut(&Event) -> Result<()>>;
    thread_local! { static HOOK: RefCell<Option<Hook>> = RefCell::new(None); }
    pub(in crate::project) fn visit(boundary: Boundary, phase: Phase, path: &Path) -> Result<()> {
        HOOK.with(|hook| match hook.borrow_mut().as_mut() {
            Some(hook) => hook(&Event {
                boundary,
                phase,
                path: path.into(),
            }),
            None => Ok(()),
        })
    }
    /// Runs `run` with `hook` called at every persistence boundary this
    /// thread passes; an error from the hook is the operation's error.
    pub fn with_hook<T>(
        hook: impl FnMut(&Event) -> Result<()> + 'static,
        run: impl FnOnce() -> T,
    ) -> T {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                HOOK.with(|hook| *hook.borrow_mut() = None);
            }
        }
        HOOK.with(|slot| {
            assert!(slot.borrow().is_none(), "nested storage I/O hook");
            *slot.borrow_mut() = Some(Box::new(hook));
        });
        let _reset = Reset;
        run()
    }
}
