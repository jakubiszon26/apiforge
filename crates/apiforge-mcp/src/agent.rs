//! The agent session (MCP-04, ADR 0014 decision 1): the first write takes the
//! project's writer lock and the server keeps it across calls until the model
//! calls `finish_editing`, stdin ends or the process exits. Each write builds
//! a fresh session over the saved project from that one store, checks the
//! version the model names, validates before the first byte, and commits.
//!
//! `publish` (MCP-05) is the window's Publish on that session: the same
//! transaction, the comment-loss question answered only for the hash the
//! model names, and a `request_id` the server remembers so that a lost
//! response can be asked for again without a second revision.
use crate::{
    error::{ErrorCode, ToolError, Wrote},
    folder,
    handover::{Answer, Hub},
    requests,
    secrets::{self, Secrets},
    tools::{self, Diagnostic, Kind, MAX_TEXT_BYTES},
};
use apiforge_channel::Message;
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
    collections::VecDeque,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const DIAGRAM_PATH: &str = ".apiforge/diagram.json";
/// How long the server keeps trying for a lock the window has just granted:
/// a check of the window's may still hold it for a moment.
const GRANTED_LOCK: Duration = Duration::from_secs(2);
/// How long a write after *Take back* lets the window take the free lock
/// before it asks the window for the project again.
const YIELD: Duration = Duration::from_secs(3);
/// The reason a window gives when it keeps the project for unsaved work.
pub const UNSAVED_WORK: &str = "the window has unsaved work";

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

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct PublishArguments {
    /// The working component's registered path, as `project_overview` lists
    /// it.
    pub path: String,
    /// The `source_sha256` of the saved text you checked, as `read_source`
    /// or `write_source` returned it. A different hash on disk is
    /// `stale_source`.
    pub expected_source_sha256: String,
    /// Publish although canonical printing drops the comments that belong
    /// to no statement, which `comments_would_be_lost` listed. Holds for
    /// `expected_source_sha256` only, in this call only.
    #[serde(default)]
    pub accept_comment_loss: bool,
    /// The project `generation` the component was checked at; when given
    /// and different, `stale_project`.
    #[serde(default)]
    pub expected_generation: Option<u64>,
    /// An id of your choosing for this publication. Repeated after a lost
    /// response, this server returns the recorded result instead of
    /// publishing again; a restarted server does not know it.
    #[serde(default)]
    pub request_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct PublishResult {
    /// The working component's registered path.
    pub path: String,
    /// The handle a scenario calls the revision by: `handle@revision(…)`.
    pub handle: String,
    /// The new revision's number: always the next unused one.
    pub revision: u32,
    /// Where the immutable revision was written.
    pub revision_path: String,
    /// SHA-256 of the revision's bytes, as `project_overview` lists it.
    pub source_sha256: String,
    /// The project's head generation after the publication.
    pub generation: u64,
    pub wrote: bool,
    /// The project paths the commit wrote: the revision, the registry and
    /// the checkpoint.
    pub changed: Vec<String>,
    /// The result recorded for this `request_id` earlier: nothing was
    /// published by this call.
    pub replayed: bool,
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
    /// The control channel to windows, when the server has one (MCP-08b).
    hub: OnceLock<Arc<Hub>>,
    /// The session ended because the person asked for the project back: the
    /// next write lets the window take it first.
    yielded: AtomicBool,
    /// The session's store while it holds the lock, readable while a write
    /// holds `state`, for the journal reads of the overview and `run`.
    held: Mutex<Option<ProjectStore>>,
}

#[derive(Default)]
struct State {
    /// The writer lock, held from the first write until the session ends.
    store: Option<ProjectStore>,
    client: Option<String>,
    /// A commit of this session failed after its activation: every later
    /// write is refused until the session ends and a writer recovers.
    recovery_required: bool,
    /// The publications this server answered, by `request_id`. It outlives
    /// the session's lock and ends with the process.
    answered: Answered,
}

/// How many publications a server remembers by `request_id`.
const ANSWERED: usize = 64;
/// How many ids of dropped results a server remembers as answered. An id
/// older than that is unknown again: the overview's revisions tell.
const DROPPED: usize = 1024;

/// The arguments a `request_id` answered: the path and the expected hash.
type AnsweredCall = (String, String);

/// The answered publications, oldest first, with the arguments each
/// answered; and, oldest first, the ids whose result was dropped to keep the
/// list short. Both are bounded.
#[derive(Default)]
struct Answered {
    recent: VecDeque<(String, AnsweredCall, PublishResult)>,
    dropped: VecDeque<String>,
}

impl Answered {
    /// The recorded result of a publication with the same `request_id`. An
    /// id that answered another publication is refused; one whose result is
    /// no longer held is `unknown_outcome`, never published again.
    fn recorded(&self, arguments: &PublishArguments) -> Result<Option<PublishResult>, ToolError> {
        let Some(id) = arguments.request_id.as_ref() else {
            return Ok(None);
        };
        if self.dropped.iter().any(|dropped| dropped == id) {
            return Err(ToolError::new(
                ErrorCode::UnknownOutcome,
                "This server answered a publication with this request_id but no longer holds its result; nothing was written. project_overview lists every revision with its source_sha256",
            ));
        }
        let Some((_, call, result)) = self.recent.iter().find(|(answered, _, _)| answered == id)
        else {
            return Ok(None);
        };
        if *call != call_of(arguments) {
            return Err(ToolError::new(
                ErrorCode::InvalidInputs,
                "This request_id answered a publication of another document or another version; nothing was written. Use a new request_id for a new publication",
            )
            .with_details(json!({ "path": call.0 })));
        }
        Ok(Some(PublishResult {
            replayed: true,
            ..result.clone()
        }))
    }

    fn record(&mut self, id: &str, arguments: &PublishArguments, result: &PublishResult) {
        self.recent.retain(|(answered, _, _)| answered != id);
        self.dropped.retain(|dropped| dropped != id);
        if self.recent.len() == ANSWERED
            && let Some((dropped, _, _)) = self.recent.pop_front()
        {
            if self.dropped.len() == DROPPED {
                self.dropped.pop_front();
            }
            self.dropped.push_back(dropped);
        }
        self.recent
            .push_back((id.to_owned(), call_of(arguments), result.clone()));
    }
}

fn call_of(arguments: &PublishArguments) -> AnsweredCall {
    (
        arguments.path.clone(),
        arguments.expected_source_sha256.clone(),
    )
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
            hub: OnceLock::new(),
            yielded: AtomicBool::new(false),
            held: Mutex::default(),
        }
    }

    /// Connects the session to windows over the control channel (MCP-08b):
    /// it then names its endpoint as the holder, asks a window that holds
    /// the project to hand it over, and tells connected windows what it
    /// commits and when it lets go.
    pub fn set_hub(&self, hub: Arc<Hub>) {
        let _ = self.hub.set(hub);
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

    /// The writer lock for a live run (MCP-06): the session's, taken now
    /// when the project is free, refused with the holder otherwise. The
    /// store knows the session's secrets.
    pub(crate) fn writer(&self, client: Option<String>) -> Result<ProjectStore, ToolError> {
        self.set_client(client);
        let mut state = self.state();
        self.store(&mut state)
    }

    /// The session's store while it holds the lock. Never waits for a
    /// write.
    pub(crate) fn held(&self) -> Option<ProjectStore> {
        self.held
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn set_held(&self, store: Option<ProjectStore>) {
        *self
            .held
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = store;
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
        self.saved(after.generation, &changed);
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
        self.saved(after.generation, &changed);
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

    /// Publishes the saved working component as its next revision, in the
    /// window's Publish transaction, when it is the version the model names.
    pub fn publish(&self, arguments: &PublishArguments) -> Result<PublishResult, ToolError> {
        let path = folder::validate(&arguments.path)?;
        // The rules of a live run's id (MCP-06): bounded, and never a known
        // secret value.
        if let Some(id) = &arguments.request_id {
            requests::check_id(id)?;
            secrets::refuse_in_text(id, &self.secrets.values()?)?;
        }
        let mut state = self.state();
        if let Some(recorded) = state.answered.recorded(arguments)? {
            return Ok(recorded);
        }
        let store = self.store(&mut state)?;
        // The working source is not read: publishing never rewrites it, and a
        // source over the limit is the session's `too_large`, not a storage
        // failure. What a publication changes: the checkpoint and registry.
        let watched = &watched(&path)?[1..];
        let before = read(&store, watched)?;
        let mut session = ProjectSession::from_store_saved(store.clone()).map_err(load_error)?;
        expect_generation(&before, arguments.expected_generation)?;
        let id = folder::select(&session, &arguments.path)?;
        let document = session.document(&id).ok_or_else(unknown_document)?;
        if document.entry().kind != DocumentKind::Component {
            return Err(not_a_component());
        }
        match document.unavailable() {
            None => {}
            Some(Unavailable::TooLarge) => {
                return Err(ToolError::new(
                    ErrorCode::TooLarge,
                    "The saved component exceeds the 1 MiB limit and cannot be published",
                ));
            }
            Some(_) => {
                return Err(ToolError::new(
                    ErrorCode::ProjectUnreadable,
                    "The saved component cannot be read as source; read_source says why",
                ));
            }
        }
        let saved = document.saved_sha256().map(str::to_owned);
        if saved.as_deref() != Some(arguments.expected_source_sha256.as_str()) {
            return Err(ToolError::new(
                ErrorCode::StaleSource,
                "The component changed on disk since the version you checked; nothing was published. Read it again, check it and publish with the new source_sha256",
            )
            .with_details(json!({ "source_sha256": saved })));
        }
        // The comments a refusal lists come from the saved text.
        if store
            .validate_managed_payload(document.text().as_bytes())
            .is_err()
        {
            return Err(ToolError::new(
                ErrorCode::SecretInSource,
                "The saved component contains the value of a known secret; nothing was published and the value is not repeated. Name the secret instead: env.name",
            ));
        }
        session.settle(&id).map_err(tools::storage_error)?;
        let (document, valid) = folder::check_document(&session, &id)?;
        if !valid {
            let (diagnostics, omitted) = tools::diagnostics(document);
            return Err(ToolError::new(
                ErrorCode::InvalidText,
                "The saved component has errors and was not published; fix the diagnostics, write it and publish again",
            )
            .with_details(json!({ "diagnostics": diagnostics, "omitted": omitted })));
        }
        let comments = unattached(document);
        if !comments.is_empty() {
            if !arguments.accept_comment_loss {
                return Err(ToolError::new(
                    ErrorCode::CommentsWouldBeLost,
                    "Publishing prints the component canonically, which drops the comments that belong to no statement; nothing was published. Attach them to statements and write the component, or publish again with accept_comment_loss: true and the same expected_source_sha256",
                )
                .with_details(json!({ "comments": comments, "source_sha256": saved })));
            }
            // The decision binds to the source version just checked against
            // the hash the model named, as the window's binds to the text
            // the person reviewed.
            session
                .accept_comment_loss(&id)
                .map_err(tools::storage_error)?;
        }
        let published = match session.publish(&id, Origin::SESSION) {
            Ok(published) => published,
            Err(error) => return Err(publish_failed(&mut state, &store, error, &path)),
        };
        // From here the revision exists: nothing may fail before the result
        // is recorded, or a retry with the same request_id would publish a
        // second revision.
        let revision = session
            .manifest()
            .revisions
            .iter()
            .find(|entry| {
                entry.component_id == published.component_id && entry.revision == published.revision
            })
            .cloned()
            .expect("a publication registers the revision it wrote");
        // The revision did not exist before: the transaction expected it
        // missing. A publication always writes the registry and the
        // checkpoint, which is what `changed` says if they cannot be read.
        let mut changed = vec![revision.path.as_str().to_owned()];
        match read(&store, watched) {
            Ok(after) => changed.extend(self::changed(watched, &before, &after)),
            Err(_) => changed.extend(watched.iter().map(|p| p.as_str().to_owned())),
        }
        let result = PublishResult {
            path: path.as_str().to_owned(),
            handle: revision.handle,
            revision: published.revision,
            revision_path: revision.path.as_str().to_owned(),
            source_sha256: revision.source_hash,
            generation: published.generation,
            wrote: true,
            changed,
            replayed: false,
        };
        if let Some(id) = &arguments.request_id {
            state.answered.record(id, arguments, &result);
        }
        // Connected windows hear of the revision before the tool answers,
        // as of a save (MCP-08b).
        self.saved(result.generation, &result.changed);
        Ok(result)
    }

    /// Ends the session and releases the writer lock; connected windows are
    /// told `released`. A write in flight finishes first: it holds the state.
    pub fn finish_editing(&self) -> Finished {
        self.end(false)
    }

    /// Ends the session because a window asked for the project back (MCP-08b,
    /// *Take back*): as [`Self::finish_editing`], and the next write, even
    /// one already waiting, lets the window take the free lock first and
    /// then asks it like any other write.
    pub fn give_back(&self) -> Finished {
        self.end(true)
    }

    fn end(&self, yielded: bool) -> Finished {
        let mut state = self.state();
        self.yielded.store(yielded, Ordering::Release);
        state.recovery_required = false;
        let finished = state.store.take().is_some();
        self.set_held(None);
        self.active.store(false, Ordering::Release);
        // Still under the state: a write that waits for it starts a new
        // session only after the windows were told this one ended.
        if let Some(hub) = self.hub.get() {
            hub.release();
        }
        drop(state);
        Finished { finished }
    }

    /// Tells connected windows about a commit, before the tool answers.
    fn saved(&self, generation: u64, changed: &[String]) {
        if let (Some(hub), false) = (self.hub.get(), changed.is_empty()) {
            hub.broadcast(&Message::Saved {
                generation,
                paths: changed.to_vec(),
            });
        }
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
                self.set_held(Some(store.clone()));
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
        if self.yielded.swap(false, Ordering::AcqRel) {
            // The person took the project back: the free lock is the
            // window's to take, and this write asks it afterwards. A window
            // that does not take it in time (it closed) is not waited for.
            let deadline = Instant::now() + YIELD;
            while Instant::now() < deadline
                && !matches!(probe.holder(), Ok(Some(holder)) if holder.role() == Some(HolderRole::Window))
            {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        let hub = self.hub.get();
        let holder = Holder::new(HolderRole::Agent, client.clone())
            .with_endpoint(hub.and_then(Hub::endpoint));
        // A holder that leaves between the refusal and the reading of its
        // description gets one more attempt.
        for _ in 0..2 {
            match ProjectStore::open_exclusive_as(&self.folder, &holder) {
                Ok(store) => return Ok(store),
                Err(ProjectError::Locked) => match probe.holder().map_err(storage)? {
                    Some(found) => {
                        // A window with a channel is asked; anyone else is
                        // named in the refusal.
                        if found.role() == Some(HolderRole::Window)
                            && let (Some(hub), Some(endpoint)) = (hub, found.endpoint())
                        {
                            return self.ask(
                                hub,
                                endpoint,
                                client.clone(),
                                &holder,
                                &probe,
                                &found,
                            );
                        }
                        return Err(held(&found));
                    }
                    None => continue,
                },
                Err(error) => return Err(storage(error)),
            }
        }
        Err(held(&Holder::unknown()))
    }

    /// Asks the window that holds the project to hand it over (MCP-08b,
    /// decision 11: a clean window does so at once). On `granted` the window
    /// has let the lock go and keeps the connection to observe; a third
    /// process that took the lock meanwhile is `held_by_other`.
    fn ask(
        &self,
        hub: &Arc<Hub>,
        endpoint: &str,
        client: Option<String>,
        holder: &Holder,
        probe: &ProjectStore,
        window: &Holder,
    ) -> Result<ProjectStore, ToolError> {
        let connection = match hub.ask(endpoint, window.owner_uid(), client) {
            Answer::Granted(connection) => connection,
            Answer::Refused(reason) => return Err(refused(&reason, window)),
            Answer::Pending => return Err(pending(window)),
            Answer::Unreachable => return Err(unreachable(window, probe.root())),
        };
        let deadline = Instant::now() + GRANTED_LOCK;
        loop {
            match ProjectStore::open_exclusive_as(&self.folder, holder) {
                Ok(store) => {
                    hub.adopt(connection);
                    return Ok(store);
                }
                Err(ProjectError::Locked) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(ProjectError::Locked) => {
                    // The window let go, but another process took the
                    // project: the window takes it back once it is free.
                    hub.decline(connection);
                    return Err(match probe.holder().map_err(storage)? {
                        Some(other) if other.role() != Some(HolderRole::Window) => held(&other),
                        _ => held(&Holder::unknown()),
                    });
                }
                Err(error) => {
                    hub.decline(connection);
                    return Err(storage(error));
                }
            }
        }
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

fn not_a_component() -> ToolError {
    ToolError::new(
        ErrorCode::NotAComponent,
        "Only a component can be published; this document is a scenario",
    )
}

/// A publication the session refused or a commit that failed. The session's
/// own refusals keep their meaning; storage failures are a write's.
fn publish_failed(
    state: &mut State,
    store: &ProjectStore,
    error: apiforge_project::Error,
    source: &ProjectPath,
) -> ToolError {
    use apiforge_project::Error;
    match error {
        Error::NotAComponent => not_a_component(),
        Error::HandleChanged { published } => ToolError::new(
            ErrorCode::InvalidText,
            format!(
                "This component is published as {published}; a published component cannot be renamed, so nothing was published. Keep its handle"
            ),
        ),
        Error::RevisionExists { revision, .. } => ToolError::new(
            ErrorCode::PathExists,
            format!(
                "A file for revision {revision} already exists and is never overwritten; nothing was published"
            ),
        ),
        Error::CommentsWouldBeLost { .. } => ToolError::new(
            ErrorCode::CommentsWouldBeLost,
            "Publishing would drop comments that belong to no statement; nothing was published",
        ),
        Error::InvalidModel | Error::InvalidRevision { .. } | Error::Unprintable => ToolError::new(
            ErrorCode::InvalidText,
            "The component cannot be printed as a valid revision; nothing was published. Check it and publish again",
        ),
        Error::RegistryChanged | Error::ExternalConflict => ToolError::new(
            ErrorCode::StaleProject,
            "The project changed on disk while the publication was prepared; nothing was published. Check again and publish with the new generation",
        ),
        error => commit_failed(state, store, error, source, false),
    }
}

/// The comments of a settled document's text that canonical printing would
/// drop, with their positions, in source order.
fn unattached(document: &apiforge_project::DocumentSession) -> Vec<serde_json::Value> {
    let found: Vec<_> = document
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.message == apiforge_script::UNATTACHED_COMMENT)
        .cloned()
        .collect();
    let (positioned, _) = tools::positioned(document.text(), &found, 0);
    found
        .iter()
        .zip(positioned)
        .map(|(diagnostic, position)| {
            json!({
                "line": position.line,
                "column": position.column,
                "text": document.text().get(diagnostic.range.clone()).unwrap_or_default(),
            })
        })
        .collect()
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

/// The window kept the project and said why.
fn refused(reason: &str, window: &Holder) -> ToolError {
    let message = if reason == UNSAVED_WORK {
        "The API Forge window that holds this project has unsaved work, so it kept the project; nothing was written. Ask the person to save their work there, then write again; reading works meanwhile"
    } else {
        "The API Forge window that holds this project did not hand it over; nothing was written. Write again later; reading works meanwhile"
    };
    ToolError::new(ErrorCode::HandoverRefused, message).with_details(json!({
        "reason": reason,
        "holder": description(window),
    }))
}

/// Nobody answered in the window within thirty seconds.
fn pending(window: &Holder) -> ToolError {
    ToolError::new(
        ErrorCode::HandoverPending,
        "The API Forge window that holds this project has not answered yet; nothing was written. Write again in a moment; the window answers the next request at once",
    )
    .with_details(json!({ "holder": description(window) }))
}

fn description(holder: &Holder) -> serde_json::Value {
    json!({
        "role": holder.role(),
        "client": holder.client(),
        "since": holder.since(),
    })
}

/// A failed connection is evidence for a possible same-side mismatch only
/// on a Linux Windows-drive mount, never proof of a Windows writer.
fn unreachable(window: &Holder, root: &Path) -> ToolError {
    let error = held(window);
    #[cfg(target_os = "linux")]
    {
        let mut parts = root.components();
        let mounted_drive = parts.next() == Some(std::path::Component::RootDir)
            && parts.next().is_some_and(|part| part.as_os_str() == "mnt")
            && parts.next().is_some_and(|part| {
                let bytes = part.as_os_str().as_encoded_bytes();
                bytes.len() == 1 && bytes[0].is_ascii_alphabetic()
            })
            && parts.next().is_some();
        if mounted_drive {
            let mut error = error;
            error.message.push_str(". This project is on a Windows drive; if the window and MCP run on different sides of Windows/WSL, run both on the same side");
            return error;
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = root;
    error
}

/// The refusal of a write while another process holds the project: who and
/// since when. Never advice to edit a file directly.
fn held(holder: &Holder) -> ToolError {
    let since = holder.since().unwrap_or("an unknown time");
    let (code, message) = match (holder.role(), holder.client()) {
        (Some(HolderRole::Window), _) => (
            ErrorCode::HeldByWindow,
            format!(
                "Another API Forge window has held this project since {since} and saves it there; nothing was written. The window could not be reached for hand-over; reading remains available"
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
    ToolError::new(code, message).with_details(json!({ "holder": description(holder) }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(path: &str, hash: &str, id: &str) -> PublishArguments {
        PublishArguments {
            path: path.into(),
            expected_source_sha256: hash.into(),
            accept_comment_loss: false,
            expected_generation: None,
            request_id: Some(id.into()),
        }
    }

    fn result(revision: u32) -> PublishResult {
        PublishResult {
            path: "components/product.afs".into(),
            handle: "product".into(),
            revision,
            revision_path: format!(".apiforge/revisions/product/{revision}.afs"),
            source_sha256: "a".repeat(64),
            generation: u64::from(revision),
            wrote: true,
            changed: Vec::new(),
            replayed: false,
        }
    }

    /// A `request_id` names one publication: the same id for another
    /// document or another version is refused, never answered with a result
    /// that does not belong to it.
    #[test]
    fn a_request_id_answers_only_the_publication_it_named() {
        let mut answered = Answered::default();
        let first = arguments("components/product.afs", "h1", "p-1");
        answered.record("p-1", &first, &result(2));
        let replayed = answered.recorded(&first).unwrap().unwrap();
        assert!(replayed.replayed);
        assert_eq!(replayed.revision, 2);
        for other in [
            arguments("components/other.afs", "h1", "p-1"),
            arguments("components/product.afs", "h2", "p-1"),
        ] {
            let error = answered.recorded(&other).unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidInputs);
        }
        assert_eq!(
            answered
                .recorded(&arguments("components/product.afs", "h1", "p-2"))
                .unwrap(),
            None,
            "an unknown id publishes"
        );
    }

    /// The record is bounded; an id whose result was dropped is
    /// `unknown_outcome`, never published a second time.
    #[test]
    fn a_dropped_request_id_is_unknown_outcome() {
        let mut answered = Answered::default();
        for index in 0..=ANSWERED {
            let id = format!("p-{index}");
            answered.record(&id, &arguments("c.afs", "h", &id), &result(2));
        }
        assert_eq!(answered.recent.len(), ANSWERED);
        let error = answered
            .recorded(&arguments("c.afs", "h", "p-0"))
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::UnknownOutcome);
        assert!(
            answered
                .recorded(&arguments("c.afs", "h", "p-1"))
                .unwrap()
                .is_some()
        );
    }

    /// The ids of dropped results are bounded too (review of #117): many
    /// unique ids never grow the record past `ANSWERED + DROPPED`.
    #[test]
    fn the_ids_of_dropped_results_are_bounded() {
        let mut answered = Answered::default();
        let total = ANSWERED + DROPPED + 10;
        for index in 0..total {
            let id = format!("p-{index}");
            answered.record(&id, &arguments("c.afs", "h", &id), &result(2));
        }
        assert_eq!(answered.recent.len(), ANSWERED);
        assert_eq!(answered.dropped.len(), DROPPED);
        // The oldest are forgotten; the newest dropped are still answered.
        assert_eq!(
            answered.recorded(&arguments("c.afs", "h", "p-0")).unwrap(),
            None
        );
        let newest_dropped = format!("p-{}", total - ANSWERED - 1);
        assert_eq!(
            answered
                .recorded(&arguments("c.afs", "h", &newest_dropped))
                .unwrap_err()
                .code,
            ErrorCode::UnknownOutcome
        );
    }
}

#[cfg(test)]
mod mcp12_diagnostics {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn only_unreachable_live_window_endpoints_on_windows_mounts_get_the_qualified_hint() {
        let window = Holder::window()
            .with_endpoint(Some("apiforge-0123456789abcdef0123456789abcdef".into()));
        for (path, hint) in [
            ("/mnt/c/project", true),
            ("/mnt/C/project", true),
            ("/mnt/cat/project", false),
            ("/mnt/1/project", false),
            ("/mnt/c", false),
            ("/home/project", false),
        ] {
            let error = unreachable(&window, Path::new(path));
            assert_eq!(error.code, ErrorCode::HeldByWindow);
            assert_eq!(error.wrote, Wrote::No);
            assert_eq!(
                error.message.contains("Windows/WSL"),
                hint,
                "{path}: {}",
                error.message
            );
            if hint {
                assert!(error.message.contains("if"));
                assert!(error.message.contains("same side"));
            }
            assert!(error.message.contains("reading"));
            assert!(
                !held(&window).message.contains("Windows/WSL"),
                "metadata alone is not a diagnosis"
            );
        }
    }
}
