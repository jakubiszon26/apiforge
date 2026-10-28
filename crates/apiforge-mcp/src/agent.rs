//! The agent session (MCP-04, ADR 0014 decision 1): the first write takes the
//! project's writer lock and the server keeps it across calls until the model
//! calls `finish_editing`, stdin ends or the process exits. Each write builds
//! a fresh session over the saved project from that one store, checks the
//! version the model names, validates before the first byte, and commits.
use crate::{
    error::{ErrorCode, ToolError, Wrote},
    folder,
    secrets::Secrets,
    tools::{self, Diagnostic, Kind, MAX_TEXT_BYTES},
};
use apiforge_project::{
    DocumentKind, NewDocumentCheck, Origin, ProjectSession, RegistrationProblem, Unavailable,
};
use apiforge_storage::project::{
    FileState, Holder, HolderRole, MANIFEST_PATH, ProjectError, ProjectPath, ProjectSnapshot,
    ProjectStore,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    ffi::OsString,
    path::PathBuf,
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
};

const DIAGRAM_PATH: &str = ".apiforge/diagram.json";

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct WriteSourceArguments {
    /// The document's registered path, as `project_overview` lists it.
    pub path: String,
    /// The complete new `.afs` text, at most 1 MiB, saved exactly as given.
    pub text: String,
    /// The `source_sha256` of the saved text this one was based on, as
    /// `read_source` returned it. A different hash on disk is `stale_source`.
    pub expected_source_sha256: String,
    /// The project `generation` the text was checked at; when given and
    /// different, `stale_project`.
    #[serde(default)]
    pub expected_generation: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct CreateDocumentArguments {
    /// `scenario` or `component`; the header must declare the same.
    pub kind: Kind,
    /// The new document's path, for example `components/new-product.afs`.
    pub path: String,
    /// The complete `.afs` text, at most 1 MiB, saved exactly as given.
    pub text: String,
    /// The project `generation` the text was checked at; when given and
    /// different, `stale_project`.
    #[serde(default)]
    pub expected_generation: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct WriteResult {
    pub path: String,
    /// The commit changed the project; `false` when the disk already held
    /// exactly this text and its checkpoint.
    pub wrote: bool,
    /// The project paths the commit wrote.
    pub changed: Vec<String>,
    /// SHA-256 of the saved bytes, which equal the `text` argument.
    pub source_sha256: String,
    /// The project's head generation after the write.
    pub generation: u64,
    /// The saved text has a valid definition (warnings allowed).
    pub valid: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub omitted: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct CreateResult {
    pub path: String,
    pub kind: Kind,
    /// The component handle the header declares; absent for a scenario.
    pub handle: Option<String>,
    pub wrote: bool,
    /// The project paths the commit wrote: the source, the registry and the
    /// checkpoint.
    pub changed: Vec<String>,
    pub source_sha256: String,
    /// The project's head generation after the write.
    pub generation: u64,
    pub diagnostics: Vec<Diagnostic>,
    pub omitted: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Finished {
    /// An agent session was open and has ended: the writer lock is released.
    pub finished: bool,
}

/// One server's agent session. Writes are serialised; the read tools never
/// wait for it.
pub struct AgentSession {
    folder: PathBuf,
    /// Where the known secret values are read from, at each call.
    secrets: Secrets,
    state: Mutex<State>,
    /// Whether the session holds the lock, readable while a write holds
    /// `state`: the overview never waits for a write.
    active: AtomicBool,
}

#[derive(Default)]
struct State {
    /// The writer lock, held from the first write until the session ends.
    store: Option<ProjectStore>,
    client: Option<String>,
    /// A commit of this session failed after its activation: every later
    /// write is refused until the session ends and a writer recovers.
    recovery_required: bool,
}

impl std::fmt::Debug for AgentSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentSession")
            .field("folder", &self.folder)
            .field("active", &self.is_active())
            .finish_non_exhaustive()
    }
}

impl AgentSession {
    pub fn new(folder: PathBuf) -> Self {
        Self::with_secrets(folder, Secrets::default())
    }

    /// A session that reads `APIFORGE_SECRET_<NAME>` from `environment`
    /// instead of the process environment: a test seam.
    pub fn with_environment(folder: PathBuf, environment: Vec<(OsString, OsString)>) -> Self {
        Self::with_secrets(folder, Secrets::with_environment(None, environment))
    }

    /// A session whose known secret values come from `secrets`: the
    /// server's environment and its `--secrets` file.
    pub fn with_secrets(folder: PathBuf, secrets: Secrets) -> Self {
        Self {
            folder,
            secrets,
            state: Mutex::default(),
            active: AtomicBool::new(false),
        }
    }

    /// The client's name from `initialize` or the request metadata, written
    /// in the holder description when the session takes the lock. The first
    /// name given is kept.
    pub fn set_client(&self, client: Option<String>) {
        let mut state = self.state();
        if state.client.is_none() {
            state.client = client.filter(|name| !name.trim().is_empty());
        }
    }

    /// The session holds the writer lock.
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    /// Replaces the text of a registered document, exactly, when it is the
    /// version the model read and has no error.
    pub fn write_source(&self, arguments: &WriteSourceArguments) -> Result<WriteResult, ToolError> {
        limit(&arguments.text)?;
        let path = folder::validate(&arguments.path)?;
        self.refuse_secrets(&arguments.text)?;
        let mut state = self.state();
        let store = self.store(&mut state)?;
        let watched = watched(&path)?;
        let before = read(&store, &watched)?;
        let mut session = ProjectSession::from_store_saved(store.clone()).map_err(load_error)?;
        expect_generation(&before, arguments.expected_generation)?;
        let id = folder::select(&session, &arguments.path)?;
        let document = session.document(&id).ok_or_else(unknown_document)?;
        match document.unavailable() {
            None => {}
            Some(Unavailable::TooLarge) => {
                return Err(ToolError::new(
                    ErrorCode::TooLarge,
                    "The saved document exceeds the 1 MiB limit and cannot be replaced from here",
                ));
            }
            Some(_) => {
                return Err(ToolError::new(
                    ErrorCode::ProjectUnreadable,
                    "The saved document cannot be read as source; read_source says why",
                ));
            }
        }
        let saved = document.saved_sha256().map(str::to_owned);
        let proposed = sha256(arguments.text.as_bytes());
        // The disk already holding exactly this text is the same write
        // repeated, not a stale one.
        if saved.as_deref() != Some(arguments.expected_source_sha256.as_str())
            && saved.as_deref() != Some(proposed.as_str())
        {
            return Err(ToolError::new(
                ErrorCode::StaleSource,
                "The document changed on disk since the version you read; nothing was written. Read it again, merge your change and write with the new source_sha256",
            )
            .with_details(json!({ "source_sha256": saved })));
        }
        session
            .edit_source(&id, &arguments.text, Origin::SESSION)
            .map_err(tools::storage_error)?;
        session.settle(&id).map_err(tools::storage_error)?;
        let (document, valid) = folder::check_document(&session, &id)?;
        let (diagnostics, omitted) = tools::diagnostics(document);
        if !valid {
            return Err(ToolError::new(
                ErrorCode::InvalidText,
                "The text has errors and was not saved; fix the diagnostics and write again",
            )
            .with_details(json!({ "diagnostics": diagnostics, "omitted": omitted })));
        }
        if let Err(error) = session.save(&id) {
            return Err(commit_failed(&mut state, &store, error, &path, false));
        }
        let after = read(&store, &watched)?;
        let changed = changed(&watched, &before, &after);
        Ok(WriteResult {
            path: path.as_str().to_owned(),
            wrote: !changed.is_empty(),
            changed,
            source_sha256: proposed,
            generation: after.generation,
            valid,
            diagnostics,
            omitted,
        })
    }

    /// Registers a new document when it passes the check of a new document;
    /// its source, registry entry and checkpoint commit in one transaction.
    pub fn create_document(
        &self,
        arguments: &CreateDocumentArguments,
    ) -> Result<CreateResult, ToolError> {
        limit(&arguments.text)?;
        let path = folder::validate(&arguments.path)?;
        self.refuse_secrets(&arguments.text)?;
        let mut state = self.state();
        let store = self.store(&mut state)?;
        // The new path itself is not read: one that collides with another in
        // letter case is the check's `path_exists`, not a storage failure.
        let watched = &watched(&path)?[1..];
        let before = read(&store, watched)?;
        let mut session = ProjectSession::from_store_saved(store.clone()).map_err(load_error)?;
        expect_generation(&before, arguments.expected_generation)?;
        let kind = match arguments.kind {
            Kind::Scenario => DocumentKind::Scenario,
            Kind::Component => DocumentKind::Component,
        };
        let id = match session.create_document_sealed(kind, path.as_str(), &arguments.text) {
            Ok(Ok(id)) => id,
            Ok(Err(check)) => return Err(refusal(&check, &arguments.text, &store, &path)),
            Err(error) => return Err(commit_failed(&mut state, &store, error, &path, true)),
        };
        let after = read(&store, watched)?;
        let document = session.document(&id).ok_or_else(unknown_document)?;
        let (diagnostics, omitted) = tools::diagnostics(document);
        // The source did not exist before: the transaction expected it missing.
        let mut changed = vec![path.as_str().to_owned()];
        changed.extend(self::changed(watched, &before, &after));
        Ok(CreateResult {
            path: path.as_str().to_owned(),
            kind: arguments.kind,
            handle: match kind {
                DocumentKind::Scenario => None,
                DocumentKind::Component => {
                    apiforge_project::declaration(&arguments.text).and_then(|d| d.handle)
                }
            },
            wrote: !changed.is_empty(),
            changed,
            source_sha256: sha256(arguments.text.as_bytes()),
            generation: after.generation,
            diagnostics,
            omitted,
        })
    }

    /// Ends the session and releases the writer lock.
    pub fn finish_editing(&self) -> Finished {
        let mut state = self.state();
        state.recovery_required = false;
        let finished = state.store.take().is_some();
        self.active.store(false, Ordering::Release);
        Finished { finished }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The writer lock of the session, taken at its first write.
    fn store(&self, state: &mut State) -> Result<ProjectStore, ToolError> {
        if state.recovery_required {
            return Err(ToolError::new(
                ErrorCode::RecoveryRequired,
                "An earlier write of this session stopped after its commit began, so the project needs recovery; nothing was written. Call finish_editing; the API Forge window finishes the operation when it opens the project",
            ));
        }
        let store = match &state.store {
            Some(store) => store.clone(),
            None => {
                let store = self.take_lock(state.client.clone())?;
                state.store = Some(store.clone());
                self.active.store(true, Ordering::Release);
                store
            }
        };
        store.set_known_secrets(self.secrets.values()?);
        Ok(store)
    }

    fn take_lock(&self, client: Option<String>) -> Result<ProjectStore, ToolError> {
        // A folder without a project gets no `.apiforge/local` from a write.
        let probe = ProjectStore::open_read_only(&self.folder).map_err(storage)?;
        let manifest = ProjectPath::new(MANIFEST_PATH).map_err(storage)?;
        if read(&probe, std::slice::from_ref(&manifest))?.files[&manifest].is_none() {
            return Err(tools::storage_error(apiforge_project::Error::NotAProject));
        }
        let holder = Holder::new(HolderRole::Agent, client);
        // A holder that leaves between the refusal and the reading of its
        // description gets one more attempt.
        for _ in 0..2 {
            match ProjectStore::open_exclusive_as(&self.folder, &holder) {
                Ok(store) => return Ok(store),
                Err(ProjectError::Locked) => match probe.holder().map_err(storage)? {
                    Some(holder) => return Err(held(&holder)),
                    None => continue,
                },
                Err(error) => return Err(storage(error)),
            }
        }
        Err(held(&Holder::unknown()))
    }

    /// A text containing a known secret value is refused before anything
    /// else touches the project; the value is never repeated.
    fn refuse_secrets(&self, text: &str) -> Result<(), ToolError> {
        let probe = ProjectStore::open_read_only(&self.folder).map_err(storage)?;
        probe.set_known_secrets(self.secrets.values()?);
        probe
            .validate_managed_payload(text.as_bytes())
            .map_err(|_| secret_in_text())
    }
}

fn limit(text: &str) -> Result<(), ToolError> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(
            ToolError::new(ErrorCode::TooLarge, "The text exceeds the 1 MiB limit")
                .with_details(json!({ "bytes": text.len(), "limit": MAX_TEXT_BYTES })),
        );
    }
    Ok(())
}

fn secret_in_text() -> ToolError {
    ToolError::new(
        ErrorCode::SecretInText,
        "The text contains the value of a known secret; nothing was written and the value is not repeated. Name the secret instead: env.name",
    )
}

fn unknown_document() -> ToolError {
    ToolError::new(ErrorCode::UnknownDocument, "Unknown document")
}

fn storage(error: ProjectError) -> ToolError {
    tools::storage_error(apiforge_project::Error::Storage(error))
}

fn load_error(error: apiforge_project::Error) -> ToolError {
    match error {
        apiforge_project::Error::Storage(ProjectError::InvalidCheckpoint) => ToolError::new(
            ErrorCode::InvalidCheckpoint,
            "The project's identity and layout checkpoint is unreadable; open the project in the window to repair it",
        ),
        error => tools::storage_error(error),
    }
}

/// The paths a write can change: the document, the checkpoint, the registry.
fn watched(path: &ProjectPath) -> Result<[ProjectPath; 3], ToolError> {
    Ok([
        path.clone(),
        ProjectPath::new(DIAGRAM_PATH).map_err(storage)?,
        ProjectPath::new(MANIFEST_PATH).map_err(storage)?,
    ])
}

fn read(store: &ProjectStore, paths: &[ProjectPath]) -> Result<ProjectSnapshot, ToolError> {
    store.read(paths).map_err(storage)
}

fn changed(
    paths: &[ProjectPath],
    before: &ProjectSnapshot,
    after: &ProjectSnapshot,
) -> Vec<String> {
    paths
        .iter()
        .filter(|path| before.state(path) != after.state(path))
        .map(|path| path.as_str().to_owned())
        .collect()
}

fn expect_generation(current: &ProjectSnapshot, expected: Option<u64>) -> Result<(), ToolError> {
    match expected {
        Some(expected) if expected != current.generation => Err(ToolError::new(
            ErrorCode::StaleProject,
            "The project changed since the generation you checked at; nothing was written. Check again and write with the new generation",
        )
        .with_details(json!({ "generation": current.generation }))),
        _ => Ok(()),
    }
}

fn sha256(bytes: &[u8]) -> String {
    match FileState::of(bytes) {
        FileState::File { sha256, .. } => sha256,
        _ => String::new(),
    }
}

/// A commit that failed. One that failed after its activation left intent the
/// next writer finishes: `wrote` is unknown and the session refuses every
/// later write. Before activation nothing was installed.
fn commit_failed(
    state: &mut State,
    store: &ProjectStore,
    error: apiforge_project::Error,
    source: &ProjectPath,
    creating: bool,
) -> ToolError {
    if matches!(store.read(&[]), Err(ProjectError::RecoveryRequired)) {
        state.recovery_required = true;
        return ToolError::new(
            ErrorCode::RecoveryRequired,
            "The write stopped after its commit began; whether the files changed is unknown and the project needs recovery. Call finish_editing; the API Forge window finishes the operation when it opens the project",
        )
        .with_wrote(Wrote::Unknown);
    }
    match error {
        apiforge_project::Error::Storage(ProjectError::Secret) => secret_in_text(),
        // The storage's own expected-state check found a file changed on disk
        // after the session read it: the document itself, or the checkpoint
        // or the registry, which another program edited without the lock.
        apiforge_project::Error::Storage(ProjectError::Conflict { path })
            if path == store.root().join(source.as_str()) =>
        {
            if creating {
                ToolError::new(
                    ErrorCode::PathExists,
                    "A file appeared at this path while the document was prepared; nothing was written",
                )
            } else {
                ToolError::new(
                    ErrorCode::StaleSource,
                    "The document changed on disk while the write was prepared; nothing was written. Read it again and write with the new source_sha256",
                )
            }
        }
        apiforge_project::Error::Storage(ProjectError::Conflict { .. }) => ToolError::new(
            ErrorCode::StaleProject,
            "The project's checkpoint or registry changed on disk while the write was prepared; nothing was written. Check again and write with the new generation",
        ),
        error => load_error(error),
    }
}

/// Why a new document was refused: its first registration problem, or the
/// errors of its text.
fn refusal(
    check: &NewDocumentCheck,
    text: &str,
    store: &ProjectStore,
    path: &ProjectPath,
) -> ToolError {
    let (diagnostics, omitted) =
        tools::positioned(text, &check.diagnostics, check.omitted_diagnostics);
    let first = check
        .problems
        .iter()
        .copied()
        .min_by_key(|problem| match problem {
            RegistrationProblem::PathRejected => 0,
            RegistrationProblem::PathRegistered | RegistrationProblem::PathExists => 1,
            RegistrationProblem::HandleExists => 2,
            RegistrationProblem::NoKind | RegistrationProblem::KindMismatch => 3,
        });
    let (code, message) = match first {
        Some(problem) => (tools::problem(problem).code, problem.message()),
        None => (
            ErrorCode::InvalidText,
            "The text has errors and was not saved; fix the diagnostics and create it again",
        ),
    };
    // A retry after a lost response meets the file it created, with its hash.
    // A path the storage refuses to read (a case collision, a link) has none.
    let existing = store
        .read(std::slice::from_ref(path))
        .ok()
        .and_then(|snapshot| match snapshot.state(path) {
            Some(FileState::File { sha256, .. }) => Some(sha256),
            _ => None,
        });
    ToolError::new(code, message).with_details(json!({
        "problems": check.problems.iter().map(|p| tools::problem(*p)).collect::<Vec<_>>(),
        "kind": check.kind.map(tools::kind),
        "handle": check.handle,
        "source_sha256": existing,
        "diagnostics": diagnostics,
        "omitted": omitted,
    }))
}

/// The refusal of a write while another process holds the project: who and
/// since when. Never advice to edit a file directly.
fn held(holder: &Holder) -> ToolError {
    let since = holder.since().unwrap_or("an unknown time");
    let (code, message) = match (holder.role(), holder.client()) {
        (Some(HolderRole::Window), _) => (
            ErrorCode::HeldByWindow,
            format!(
                "Another API Forge window has held this project since {since} and saves it there; nothing was written. Ask the person to close the project in that window, then write again; reading works meanwhile"
            ),
        ),
        (Some(HolderRole::Command), client) => (
            ErrorCode::HeldByOther,
            format!(
                "apiforge {} has held this project since {since}; nothing was written. Write again when it has finished",
                client.unwrap_or("command")
            ),
        ),
        (Some(HolderRole::Agent), client) => (
            ErrorCode::HeldByOther,
            format!(
                "{} via apiforge mcp has held this project since {since}; nothing was written. Write again when it has finished editing",
                client.unwrap_or("An agent")
            ),
        ),
        (None, _) => (
            ErrorCode::HeldByOther,
            "Another process holds this project; nothing was written. Write again when it has let go".to_owned(),
        ),
    };
    ToolError::new(code, message).with_details(json!({
        "holder": {
            "role": holder.role(),
            "client": holder.client(),
            "since": holder.since(),
        }
    }))
}
