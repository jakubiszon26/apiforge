//! Managed startup: deciding which project the application data directory
//! should open, and preparing one when it holds none.
//!
//! Three states are possible there. A completed migration left a project
//! catalogue, and folder projects are read from it. A legacy workspace that has
//! never been migrated is migrated now, which is safe to repeat: the originals
//! stay intact, an interrupted attempt resumes, and a failure leaves the
//! catalogue inactive rather than rolling anything back. A data directory with
//! neither starts one managed project instead of writing another legacy
//! workspace.
//!
//! The catalogue itself is evidence that one migration completed, carrying its
//! identifier and the hashes of the files it read. A project created later was
//! not part of that migration, so it is remembered the way an opened folder is
//! remembered, and the catalogue is never rewritten after activation.
use super::*;
use apiforge_project::{MigrationError, ProjectCatalogue, migrate_workspace, prepare_project};
use apiforge_storage::{StorageError, WorkspaceStore};
use std::path::Path;

/// The legacy workspace file, beside which a migration writes its catalogue.
const WORKSPACE_FILE: &str = "workspace.json";

/// What managed startup did, reported once the window is up.
pub(super) enum Outcome {
    /// A project that was already managed.
    Opened,
    /// A migration ran now, resumed an interrupted one, or found one complete.
    Migrated {
        original: PathBuf,
        projects: PathBuf,
        unmigrated_drafts: usize,
    },
    /// The data directory held nothing, so one project was created.
    Created,
}

pub(super) struct Activated {
    pub destination: opening::Destination,
    pub outcome: Outcome,
    /// Every project the catalogue names, so the ones this run did not open are
    /// offered as recent projects rather than silently forgotten.
    pub known: Vec<navigation::Recent>,
}

/// Whether managed startup can proceed, and the migration it needs first.
///
/// Opening a project is the window's own asynchronous work. Migrating a legacy
/// workspace is not: it happens once in the life of an installation. A window
/// cannot change which application it is running once it exists, so whether it
/// can has to be known before one is built. A migration that refuses is its own
/// start, naming what it refused, and ends in a window that says so and offers
/// the previous workspace (KTLO-03); the causes only the workspace window can
/// resolve still open it.
pub enum Start {
    /// Managed startup; the slow work runs in the folder window.
    Managed,
    /// Open the legacy workspace window instead, for the stated reason.
    Legacy(Option<String>),
    /// Migration refused the workspace: report it and stop (KTLO-03).
    Refused(Refusal),
}

/// Why migration refused the legacy workspace, for the *Migration stopped*
/// window: the project and scenario it names, when it names them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub project: Option<String>,
    pub scenario: Option<String>,
    pub reason: String,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(project) = &self.project {
            write!(formatter, "project \u{201c}{project}\u{201d}: ")?;
        }
        write!(formatter, "{}", self.reason)?;
        if let Some(scenario) = &self.scenario {
            write!(formatter, " (scenario \u{201c}{scenario}\u{201d})")?;
        }
        Ok(())
    }
}

impl Refusal {
    fn of(error: &apiforge_project::Error) -> Self {
        match error {
            apiforge_project::Error::Migration(MigrationError::Verification {
                project_name,
                scenario,
                reason,
                ..
            }) => Self {
                project: Some(project_name.clone()),
                scenario: scenario.clone(),
                reason: (*reason).to_owned(),
            },
            other => Self {
                project: None,
                scenario: None,
                reason: other.to_string(),
            },
        }
    }
}

pub fn decide(data: &Path) -> Start {
    if data.join(apiforge_project::CATALOGUE_FILE).is_file() {
        return Start::Managed;
    }
    let workspace = data.join(WORKSPACE_FILE);
    if !workspace.is_file() {
        return Start::Managed;
    }
    let store = match WorkspaceStore::open_exclusive(&workspace) {
        Ok(store) => store,
        // A second window: the first one owns the workspace, and migration
        // needs that ownership. Start exactly as a second window starts today.
        Err(StorageError::Locked { .. }) => return Start::Legacy(None),
        // Anything else is the legacy window's own error to explain.
        Err(error) => return Start::Legacy(Some(error.to_string())),
    };
    let ready = match store.load() {
        Ok(Some(loaded)) if loaded.recovered.is_some() => {
            Err(MigrationError::WorkspaceRecoveryRequired.to_string())
        }
        Ok(_) => match store.load_drafts() {
            Ok(None) => Ok(()),
            Ok(Some(drafts)) if !drafts.recovered => Ok(()),
            _ => Err(MigrationError::DraftsUnreadable.to_string()),
        },
        Err(error) => Err(error.to_string()),
    };
    let decision = match ready {
        Err(reason) => Start::Legacy(Some(reason)),
        // Migrating here rather than on the worker, so that a workspace which
        // cannot be migrated is known before any window exists.
        // A migration that succeeded leaves a catalogue, so every later start
        // takes the fast path and reads it instead.
        Ok(()) => match migrate_workspace(&store) {
            Ok(_) => Start::Managed,
            Err(apiforge_project::Error::Migration(MigrationError::InputsChanged)) => {
                match migrate_workspace(&store) {
                    Ok(_) => Start::Managed,
                    Err(error) => Start::Refused(Refusal::of(&error)),
                }
            }
            Err(error) => Start::Refused(Refusal::of(&error)),
        },
    };
    // The worker takes the lock again for the work itself; holding it here
    // across window creation would keep it for the whole session.
    drop(store);
    decision
}

/// Resolves the data directory to one open project, migrating or creating one
/// if that is what it holds. Runs on the storage worker.
pub(super) fn activate(data: &Path) -> apiforge_project::Result<Activated> {
    if let Some(catalogue) = ProjectCatalogue::load(data)? {
        if let Some(root) = preferred_root(&catalogue, data) {
            return Ok(Activated {
                destination: opening::load(root)?,
                outcome: Outcome::Opened,
                known: known_projects(&catalogue),
            });
        }
        // A catalogue without a usable project is not a reason to refuse to
        // start; the data directory still needs one project to open.
        return create(data).map(|destination| Activated {
            destination,
            outcome: Outcome::Created,
            known: known_projects(&catalogue),
        });
    }
    let workspace = data.join(WORKSPACE_FILE);
    if !workspace.is_file() {
        // No migration has happened and none is owed. A project created by an
        // earlier start is not in any catalogue, because it was not part of a
        // migration, so this is where it is picked up again; creating is the
        // last resort, not the answer to every start.
        if let Some(root) = last_project(data) {
            return Ok(Activated {
                destination: opening::load(root)?,
                outcome: Outcome::Opened,
                known: Vec::new(),
            });
        }
        return create(data).map(|destination| Activated {
            destination,
            outcome: Outcome::Created,
            known: Vec::new(),
        });
    }
    let store = WorkspaceStore::open_exclusive(&workspace)
        .map_err(|error| apiforge_project::Error::Migration(MigrationError::Workspace(error)))?;
    let outcome = match migrate_workspace(&store) {
        // The workspace changed between reading its hashes and activating.
        // One retry covers a write that finished during the attempt; a second
        // failure is reported rather than retried indefinitely.
        Err(apiforge_project::Error::Migration(MigrationError::InputsChanged)) => {
            migrate_workspace(&store)?
        }
        result => result?,
    };
    let original = outcome.original.clone();
    let unmigrated_drafts = outcome.unmigrated_drafts.len();
    let catalogue = outcome.catalogue;
    let root = preferred_root(&catalogue, data).ok_or(apiforge_project::Error::NotAProject)?;
    Ok(Activated {
        destination: opening::load(root)?,
        outcome: Outcome::Migrated {
            original,
            projects: data.join(apiforge_project::PROJECTS_DIRECTORY),
            unmigrated_drafts,
        },
        known: known_projects(&catalogue),
    })
}

/// Each catalogue project as a recent entry. The manifest is read directly:
/// naming a project in a menu must not open a session or take its lock, and a
/// project whose manifest cannot be read is left out rather than guessed at.
fn known_projects(catalogue: &ProjectCatalogue) -> Vec<navigation::Recent> {
    catalogue
        .projects
        .iter()
        .filter_map(|entry| {
            let bytes =
                std::fs::read(entry.path.join(apiforge_storage::project::MANIFEST_PATH)).ok()?;
            let manifest: apiforge_storage::project::ProjectManifest =
                serde_json::from_slice(&bytes).ok()?;
            Some(navigation::Recent {
                id: manifest.id,
                name: manifest.name,
                path: entry.path.clone(),
            })
        })
        .collect()
}

/// The project this computer was last in, when it is still a project. Used
/// when no migration is involved, so a created project is reopened rather than
/// a second one being made beside it.
fn last_project(data: &Path) -> Option<PathBuf> {
    navigation::RecentProjects::at(data).active.filter(|path| {
        path.join(apiforge_storage::project::MANIFEST_PATH)
            .is_file()
    })
}

/// The project to open from a catalogue: the one this computer was last in, if
/// it is still one of them, then the one that was active when the workspace was
/// migrated, then the first recorded.
fn preferred_root(catalogue: &ProjectCatalogue, data: &Path) -> Option<PathBuf> {
    let known = |path: &Path| catalogue.projects.iter().any(|entry| entry.path == path);
    let path_of = |id: &str| {
        catalogue
            .projects
            .iter()
            .find(|entry| entry.project_id == id)
            .map(|entry| entry.path.clone())
    };
    navigation::RecentProjects::at(data)
        .active
        .filter(|path| known(path) && path.is_dir())
        .or_else(|| {
            catalogue
                .active_project_id
                .as_deref()
                .and_then(path_of)
                .filter(|path| path.is_dir())
        })
        .or_else(|| {
            catalogue
                .projects
                .iter()
                .map(|entry| entry.path.clone())
                .find(|path| path.is_dir())
        })
}

/// Writes one managed project and opens it. A new user has an application to
/// look at rather than an empty board, so it holds the demo the legacy window
/// shows on a first run; a managed project is a folder project like any other
/// and can be renamed, edited or replaced.
fn create(data: &Path) -> apiforge_project::Result<opening::Destination> {
    let project = apiforge_core::demos::shop_project();
    let projects = data.join(apiforge_project::PROJECTS_DIRECTORY);
    // Migration names a destination after its project, so creation does too.
    // Nothing should occupy the name here, but a directory left by something
    // else is stepped over rather than written into.
    let root = (0..)
        .map(|suffix| match suffix {
            0 => projects.join(&project.id),
            n => projects.join(format!("{}-{n}", project.id)),
        })
        .find(|path| !path.exists())
        .expect("a free project directory");
    std::fs::create_dir_all(&root).map_err(|source| apiforge_project::ProjectError::Io {
        path: root.clone(),
        source,
    })?;
    prepare_project(&root, &project)?;
    opening::load(root)
}

impl FolderApp {
    /// Opens whichever project the application data directory holds, preparing
    /// one if it holds none. The directory stands in as the root until the
    /// project is known, so nothing is drawn against a folder that was never
    /// chosen.
    pub fn start_managed(context: &egui::Context, data: PathBuf) -> Self {
        let mut app = Self::shell(context, data.clone());
        app.begin_activation(data);
        app
    }

    fn begin_activation(&mut self, data: PathBuf) {
        self.open_generation += 1;
        self.opening = true;
        let generation = self.open_generation;
        self.storage_io(
            "Starting…",
            move || {
                (
                    Preferences::load(),
                    navigation::RecentProjects::at(&data),
                    activate(&data),
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
                    Ok((preferences, recent, activated)) => {
                        app.preferences = preferences;
                        app.navigation.recent = recent;
                        match activated {
                            Ok(activated) => app.install_activation(activated),
                            Err(error) => app.open_error = Some(error.to_string()),
                        }
                    }
                    Err(error) => app.open_error = Some(error),
                }
            },
        );
    }

    fn install_activation(&mut self, activated: Activated) {
        let Activated {
            destination,
            outcome,
            known,
        } = activated;
        // Merged before installation, because installing records the opened
        // project and saves the file: every migrated project is reachable from
        // the start, not only the one that opened.
        self.navigation.recent.remember_all(known);
        self.install_destination(destination);
        self.notice = match outcome {
            Outcome::Opened => None,
            Outcome::Created => Some(Notice {
                text: format!("Created a project in {}.", self.root.display()),
                error: false,
            }),
            Outcome::Migrated {
                original,
                projects,
                unmigrated_drafts,
            } => {
                let drafts = match unmigrated_drafts {
                    0 => String::new(),
                    1 => " One draft had no project left to belong to and stays in the original."
                        .into(),
                    many => format!(
                        " {many} drafts had no project left to belong to and stay in the original."
                    ),
                };
                Some(Notice {
                    text: format!(
                        "Projects are now folders in {}. The original workspace is unchanged at {}.{drafts}",
                        projects.display(),
                        original.display()
                    ),
                    error: false,
                })
            }
        };
    }
}
