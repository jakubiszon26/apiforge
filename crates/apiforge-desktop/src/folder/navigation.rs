//! Global recent folders and recovery-protected project switching.
use super::*;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Read};

const RECENT_LIMIT: usize = 32;
const RECENT_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Recent {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecentFile {
    format_version: u32,
    active: Option<PathBuf>,
    projects: Vec<Recent>,
}

#[derive(Clone)]
pub(super) struct RecentProjects {
    pub entries: Vec<Recent>,
    pub active: Option<PathBuf>,
    path: Option<PathBuf>,
    writable: bool,
}

impl Default for RecentProjects {
    fn default() -> Self {
        let path = dirs::data_dir().map(|data| data.join("apiforge/recent-projects.json"));
        Self::load(path)
    }
}

impl RecentProjects {
    fn load(path: Option<PathBuf>) -> Self {
        let mut recent = Self {
            entries: Vec::new(),
            active: None,
            path,
            writable: true,
        };
        let Some(path) = &recent.path else {
            return recent;
        };
        let bytes = (|| {
            let file = match File::open(path) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(error),
            };
            let mut bytes = Vec::new();
            file.take(RECENT_BYTES as u64 + 1).read_to_end(&mut bytes)?;
            Ok(Some(bytes))
        })();
        match bytes {
            Ok(None) => {}
            Ok(Some(bytes)) => match serde_json::from_slice::<RecentFile>(&bytes) {
                Ok(file)
                    if bytes.len() <= RECENT_BYTES
                        && file.format_version == 1
                        && file.projects.len() <= RECENT_LIMIT
                        && file.projects.iter().all(|project| {
                            !project.id.is_empty() && project.path.is_absolute()
                        }) =>
                {
                    recent.entries = file.projects;
                    recent.active = file.active;
                }
                _ => recent.writable = false,
            },
            Err(_) => recent.writable = false,
        }
        recent
    }

    pub fn remember(&mut self, session: &ProjectSession) -> Result<(), String> {
        let entry = Recent {
            id: session.manifest().id.clone(),
            name: session.manifest().name.clone(),
            path: session.store().root().to_owned(),
        };
        self.entries
            .retain(|old| old.id != entry.id && old.path != entry.path);
        self.active = Some(entry.path.clone());
        self.entries.insert(0, entry);
        self.entries.truncate(RECENT_LIMIT);
        if !self.writable {
            return Err("Recent-project metadata is unreadable or unsupported; its existing bytes were preserved.".into());
        }
        let Some(path) = &self.path else {
            return Ok(());
        };
        let bytes = serde_json::to_vec_pretty(&RecentFile {
            format_version: 1,
            active: self.active.clone(),
            projects: self.entries.clone(),
        })
        .map_err(|error| error.to_string())?;
        if bytes.len() > RECENT_BYTES {
            return Err("Recent-project metadata exceeds its size limit.".into());
        }
        session
            .store()
            .validate_managed_payload(&bytes)
            .map_err(|error| error.to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        apiforge_storage::write_atomic(path, &bytes).map_err(|error| error.to_string())
    }
}

pub(super) struct Destination {
    root: PathBuf,
    session: Option<ProjectSession>,
    survey: Vec<SurveyRow>,
}

#[derive(Default)]
pub(super) struct Navigation {
    pub recent: RecentProjects,
    pending: Option<Destination>,
    pub failed: bool,
}

impl FolderApp {
    pub(super) fn navigation_menu(&mut self, ui: &mut egui::Ui) {
        let entries = self.navigation.recent.entries.clone();
        ui.menu_button("Project", |ui| {
            if ui.button("Open Folder…").clicked() {
                ui.close();
                self.pick_folder();
            }
            if self.session.is_some() {
                ui.separator();
                let writable = self
                    .session
                    .as_ref()
                    .is_some_and(|session| !session.is_read_only());
                if ui
                    .add_enabled(writable, egui::Button::new("Project settings…"))
                    .clicked()
                {
                    ui.close();
                    self.open_project_settings();
                }
                if ui.button("Test environment…").clicked() {
                    ui.close();
                    self.open_environment();
                }
            }
            if !entries.is_empty() {
                ui.separator();
                widgets::caption(ui, "RECENT PROJECTS");
                for entry in entries {
                    if ui
                        .button(&entry.name)
                        .on_hover_text(entry.path.display().to_string())
                        .clicked()
                    {
                        ui.close();
                        self.request_switch(entry.path);
                    }
                }
            }
        });
    }

    fn pick_folder(&mut self) {
        let (sender, receiver) = std::sync::mpsc::channel();
        let context = self.ui_context.clone();
        std::thread::spawn(move || {
            let path = rfd::FileDialog::new()
                .set_title("Open project folder")
                .pick_folder();
            let _ = sender.send(Picked::Folder(path));
            context.request_repaint();
        });
        self.picked.push(receiver);
    }

    pub(super) fn remember_project(&mut self) {
        let Some(session) = &self.session else { return };
        if let Err(error) = self.navigation.recent.remember(session) {
            self.notify(error, true);
        }
    }

    pub(super) fn request_switch(&mut self, root: PathBuf) {
        let root = match std::path::absolute(&root) {
            Ok(root) => root,
            Err(error) => {
                self.notify(error.to_string(), true);
                return;
            }
        };
        if self
            .session
            .as_ref()
            .is_some_and(|session| session.store().root() == root)
            || root == self.root
        {
            return;
        }
        let destination = match ProjectSession::open_with(&root, Arc::new(CardPlacer)) {
            Ok(session) => Destination {
                root,
                session: Some(session),
                survey: Vec::new(),
            },
            Err(apiforge_project::Error::NotAProject) => match ProjectSession::survey(&root) {
                Ok(survey) => Destination {
                    root,
                    session: None,
                    survey: survey
                        .into_iter()
                        .map(|source| SurveyRow {
                            path: source.path,
                            include: true,
                            kind: source.declaration.map(|declaration| declaration.kind),
                        })
                        .collect(),
                },
                Err(error) => {
                    self.notify(error.to_string(), true);
                    return;
                }
            },
            Err(apiforge_project::Error::Storage(
                apiforge_project::ProjectError::RecoveryRequired,
            )) => {
                self.review_transaction(root);
                return;
            }
            Err(error) => {
                self.notify(format!("Cannot open folder: {error}"), true);
                return;
            }
        };
        self.navigation.pending = Some(destination);
        self.navigation.failed = false;
        self.ui_context.request_repaint();
    }

    pub(super) fn advance_switch(&mut self) {
        if self.navigation.pending.is_none() || self.navigation.failed {
            return;
        }
        if self.prepare_close() {
            self.finish_departure();
        } else if let Some(error) = self.close_error() {
            self.navigation.failed = true;
            self.prompt = Some(Prompt::CloseFailed(error));
        }
    }

    pub(super) fn switching(&self) -> bool {
        self.navigation.pending.is_some()
    }

    pub(super) fn cancel_switch(&mut self) {
        self.navigation.pending = None;
        self.navigation.failed = false;
    }

    pub(super) fn finish_departure(&mut self) {
        let Some(destination) = self.navigation.pending.take() else {
            self.close_requested = true;
            return;
        };
        // A copied/moved project can retain document IDs. Retire widget history
        // and inspector fields only after the old recovery has succeeded.
        for tab in self.workbench.tabs() {
            code::clear_history(&self.ui_context, editor_id(&tab.document_id));
        }
        inspector::forget_field_text(&self.ui_context);
        inspector::drafts::restore_entries(&self.ui_context, Vec::new());
        let mut next = Self::shell(&self.ui_context, destination.root);
        next.navigation.recent = self.navigation.recent.clone();
        if let Some(session) = destination.session {
            next.install(session);
        } else {
            next.uninitialised = true;
            next.survey = destination.survey;
            next.new_name = next
                .root
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
        }
        *self = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_folders_retain_identity_order_and_active_path_across_restart() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("project");
        std::fs::create_dir(&root).unwrap();
        let session = ProjectSession::create(&root, "Project").unwrap();
        let path = directory.path().join("recent.json");
        let mut recent = RecentProjects::load(Some(path.clone()));
        recent.remember(&session).unwrap();
        recent.remember(&session).unwrap();
        let loaded = RecentProjects::load(Some(path));
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].id, session.manifest().id);
        assert_eq!(loaded.active.as_deref(), Some(root.as_path()));
    }

    #[test]
    fn future_recent_metadata_is_preserved() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("recent.json");
        let bytes = b"{\"formatVersion\":99}";
        std::fs::write(&path, bytes).unwrap();
        let mut recent = RecentProjects::load(Some(path.clone()));
        let session = ProjectSession::create(directory.path(), "Project").unwrap();
        assert!(recent.remember(&session).is_err());
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
}

#[cfg(test)]
mod secret_tests {
    use super::*;

    #[test]
    fn recent_metadata_cannot_persist_a_known_session_secret_in_a_project_name() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("recent.json");
        let mut session = ProjectSession::create(directory.path(), "Public name").unwrap();
        let mut recent = RecentProjects::load(Some(path.clone()));
        recent.remember(&session).unwrap();
        let before = std::fs::read(&path).unwrap();
        session
            .store()
            .set_known_secrets(["private-session-value".into()]);
        session
            .edit_configuration(
                |manifest| manifest.name = "private-session-value".into(),
                DIAGRAM,
            )
            .unwrap();
        assert!(recent.remember(&session).is_err());
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
}
