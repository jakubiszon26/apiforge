//! Import and export through native file dialogs, off the UI thread.
//!
//! One operation runs at a time. Reading is bounded by the import limit and the
//! document is validated before the editor sees it, so a failed import never
//! changes the open workspace.

use apiforge_core::{
    ImportLimits, ValidatedDocument, format::Project, import_document, run::RunResult,
};
use apiforge_runner::redact::redact_report;
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

pub struct Files {
    pending: Option<Receiver<Outcome>>,
    repaint: eframe::egui::Context,
}

impl Files {
    pub fn new(repaint: eframe::egui::Context) -> Self {
        Self {
            pending: None,
            repaint,
        }
    }

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
            let name = sanitize_filename(&project.name);
            let Some(path) = rfd::FileDialog::new()
                .set_title("Export the project")
                .set_file_name(format!("{name}.json"))
                .add_filter("JSON project", &["json"])
                .save_file()
            else {
                return Outcome::Cancelled;
            };
            write_json_bytes(&path, serde_json::to_vec_pretty(&project))
        });
    }

    /// Exports a redacted run report as pretty-printed JSON.
    pub fn export_report(&mut self, run: RunResult) {
        self.spawn(move || {
            let redacted = redact_report(&run);
            let id = sanitize_filename(&redacted.id);
            let Some(path) = rfd::FileDialog::new()
                .set_title("Export the run report")
                .set_file_name(format!("api-forge-run-{id}.json"))
                .add_filter("JSON report", &["json"])
                .save_file()
            else {
                return Outcome::Cancelled;
            };
            write_json_bytes(&path, serde_json::to_vec_pretty(&redacted))
        });
    }

    fn spawn(&mut self, work: impl FnOnce() -> Outcome + Send + 'static) {
        if self.pending.is_some() {
            return;
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let repaint = self.repaint.clone();
        let started = std::thread::Builder::new()
            .name("file-dialog".into())
            .spawn(move || {
                // Wake the event-driven UI even if the operation panics. Polling
                // then reports a disconnected worker instead of staying busy.
                struct WakeOnDrop {
                    context: eframe::egui::Context,
                    sender: Option<mpsc::SyncSender<Outcome>>,
                }
                impl Drop for WakeOnDrop {
                    fn drop(&mut self) {
                        // Close the channel before waking the UI on panic.
                        self.sender.take();
                        self.context.request_repaint();
                    }
                }
                let wake = WakeOnDrop {
                    context: repaint,
                    sender: Some(sender),
                };
                let _ = wake.sender.as_ref().unwrap().send(work());
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

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|character| {
            if character.is_alphanumeric() || character == '-' {
                character
            } else {
                '-'
            }
        })
        .collect()
}

fn write_json_bytes(path: &Path, serialized: serde_json::Result<Vec<u8>>) -> Outcome {
    let result = serialized
        .map_err(|error| error.to_string())
        .and_then(|bytes| {
            apiforge_storage::write_atomic(path, &bytes).map_err(|error| error.to_string())
        });
    match result {
        Ok(()) => Outcome::Exported(path.to_path_buf()),
        Err(message) => Outcome::Failed(format!("Export failed: {message}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn completed_operation_wakes_an_idle_ui_without_user_input() {
        let context = eframe::egui::Context::default();
        // Settle initial font/layout repaints so the context is idle.
        for _ in 0..4 {
            let mut output = context.run_ui(Default::default(), |_| {});
            output.textures_delta.clear();
        }
        let (sender, notified) = mpsc::channel();
        context.set_request_repaint_callback(move |_| {
            let _ = sender.send(());
        });
        let mut files = Files::new(context);
        files.spawn(|| Outcome::Cancelled);

        notified
            .recv_timeout(Duration::from_secs(2))
            .expect("the completed worker must request a UI frame");
        assert!(matches!(files.poll(), Some(Outcome::Cancelled)));
        assert!(!files.busy());
    }
}
