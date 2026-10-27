//! Project/bootstrap reads run before installation; the UI receives owned data.
use super::*;

pub(super) struct Destination {
    pub root: PathBuf,
    pub session: Option<ProjectSession>,
    pub state: Option<WorkspaceState>,
    pub survey: Vec<SurveyRow>,
    /// Who holds the project when it opened read-only (MCP-07).
    pub holder: Option<apiforge_storage::project::Holder>,
}

pub(super) fn load(root: PathBuf) -> apiforge_project::Result<Destination> {
    let root = std::path::absolute(root).map_err(|error| apiforge_project::ProjectError::Io {
        path: PathBuf::from("."),
        source: error,
    })?;
    let holder = apiforge_storage::project::Holder::window();
    match ProjectSession::open_with_as(&root, Arc::new(CardPlacer), &holder) {
        Ok(mut session) => {
            // A read-only window says who holds the project from its first
            // frame; the watcher keeps the answer current.
            let holder = if session.is_read_only() {
                session.store().holder().ok().flatten()
            } else {
                None
            };
            let state = session
                .store()
                .load_workspace_state(&session.manifest().id)
                .ok()
                .flatten();
            let mut texts: Vec<_> = state
                .iter()
                .flat_map(|state| state.tabs.iter())
                .filter(|tab| session.document(&tab.document_id).is_none())
                .filter_map(|tab| tab.document_id.strip_prefix(TEXT_TAB).map(str::to_owned))
                .collect();
            texts.extend(
                session
                    .recovered_text_drafts()
                    .into_iter()
                    .map(str::to_owned),
            );
            texts.sort();
            texts.dedup();
            for path in texts {
                let _ = session.open_text_file(&path);
            }
            Ok(Destination {
                root,
                session: Some(session),
                state,
                survey: Vec::new(),
                holder,
            })
        }
        Err(apiforge_project::Error::NotAProject) => {
            let survey = ProjectSession::survey(&root)?
                .into_iter()
                .map(|source| SurveyRow {
                    path: source.path,
                    include: true,
                    kind: source.declaration.map(|declaration| declaration.kind),
                })
                .collect();
            Ok(Destination {
                root,
                session: None,
                state: None,
                survey,
                holder: None,
            })
        }
        Err(error) => Err(error),
    }
}

impl FolderApp {
    pub(super) fn begin_open(&mut self) {
        self.open_generation += 1;
        self.opening = true;
        let generation = self.open_generation;
        let root = self.root.clone();
        self.storage_io(
            "Opening project…",
            move || {
                (
                    Preferences::load(),
                    navigation::RecentProjects::default(),
                    load(root),
                )
            },
            move |app, result| {
                if app.open_generation != generation {
                    return;
                }
                if app.persistence.closing {
                    app.opening = false;
                    return;
                }
                app.opening = false;
                match result {
                    Ok((preferences, recent, result)) => {
                        app.preferences = preferences;
                        app.navigation.recent = recent;
                        match result {
                            Ok(destination) => app.install_destination(destination),
                            Err(error) => {
                                app.open_error = Some(error.to_string());
                                if matches!(
                                    error,
                                    apiforge_project::Error::Storage(
                                        apiforge_project::ProjectError::RecoveryRequired
                                    )
                                ) {
                                    app.review_transaction(app.root.clone());
                                }
                            }
                        }
                    }
                    Err(error) => app.open_error = Some(error),
                }
            },
        );
    }

    pub(super) fn install_destination(&mut self, destination: Destination) {
        self.root = destination.root;
        self.open_error = None;
        if let Some(session) = destination.session {
            self.install(session, destination.state);
            self.holder = destination.holder.map(holder::Observed::new);
        } else {
            self.uninitialised = true;
            self.survey = destination.survey;
            self.new_name = self
                .root
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
        }
    }
}
