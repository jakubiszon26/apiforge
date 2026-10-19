//! Import and export through native file dialogs, off the UI thread.
//!
//! One operation runs at a time. Reading is bounded by the import limit and the
//! document is validated before the editor sees it, so a failed import never
//! changes the open workspace.

use apiforge_core::{ImportLimits, ValidatedDocument, format::Project, import_document};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, TryRecvError},
};

pub enum Outcome {
    Imported(ValidatedDocument),
    Exported(PathBuf),
    Failed(String),
    Cancelled,
}

#[derive(Default)]
pub struct Files {
    pending: Option<Receiver<Outcome>>,
}

impl Files {
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }

    /// The result of a finished operation, if any.
    pub fn poll(&mut self) -> Option<Outcome> {
        let receiver = self.pending.as_ref()?;
        let outcome = match receiver.try_recv() {
            Ok(outcome) => outcome,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => {
                Outcome::Failed("The file operation was interrupted.".into())
            }
        };
        self.pending = None;
        Some(outcome)
    }

    pub fn import(&mut self) {
        self.spawn(|| {
            let Some(path) = rfd::FileDialog::new()
                .set_title("Import a project or workspace")
                .add_filter("JSON project", &["json"])
                .pick_file()
            else {
                return Outcome::Cancelled;
            };
            match read_document(&path) {
                Ok(document) => Outcome::Imported(document),
                Err(message) => Outcome::Failed(format!("Import failed: {message}")),
            }
        });
    }

    /// Exports a project that must already have its secret values removed.
    pub fn export(&mut self, project: Project) {
        self.spawn(move || {
            let name: String = project
                .name
                .chars()
                .map(|character| {
                    if character.is_alphanumeric() || character == '-' {
                        character
                    } else {
                        '-'
                    }
                })
                .collect();
            let Some(path) = rfd::FileDialog::new()
                .set_title("Export the project")
                .set_file_name(format!("{name}.json"))
                .add_filter("JSON project", &["json"])
                .save_file()
            else {
                return Outcome::Cancelled;
            };
            let result = serde_json::to_vec_pretty(&project)
                .map_err(|error| error.to_string())
                .and_then(|bytes| {
                    apiforge_storage::write_atomic(&path, &bytes).map_err(|error| error.to_string())
                });
            match result {
                Ok(()) => Outcome::Exported(path),
                Err(message) => Outcome::Failed(format!("Export failed: {message}")),
            }
        });
    }

    fn spawn(&mut self, work: impl FnOnce() -> Outcome + Send + 'static) {
        if self.pending.is_some() {
            return;
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let started = std::thread::Builder::new()
            .name("file-dialog".into())
            .spawn(move || {
                let _ = sender.send(work());
            });
        self.pending = match started {
            Ok(_) => Some(receiver),
            Err(error) => {
                let (sender, receiver) = mpsc::sync_channel(1);
                let _ = sender.send(Outcome::Failed(format!(
                    "Cannot open the file dialog: {error}"
                )));
                Some(receiver)
            }
        };
    }
}

/// Reads and validates a document; the message never contains file contents.
pub fn read_document(path: &Path) -> Result<ValidatedDocument, String> {
    let limits = ImportLimits::default();
    let file = File::open(path).map_err(|error| format!("cannot open the file ({error})"))?;
    let mut bytes = Vec::new();
    // The file may grow between a size check and the read, so bound the read itself.
    file.take(limits.max_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read the file ({error})"))?;
    import_document(&bytes, &limits).map_err(|error| error.to_string())
}
