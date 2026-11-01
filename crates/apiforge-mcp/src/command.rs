//! `apiforge run --folder … --live`: a live run of a saved scenario from the
//! command line (MCP-15). The run's body is a library function so that the
//! `apiforge` binary prints its report and a desktop test runs the same path
//! as a process of its own.
//!
//! A live run is a write (it journals what it sends), so it needs the
//! project's writer lock. When a window holds the project and names its
//! control channel, the command asks it for the project as the MCP server
//! does (ADR 0014 decision 4): a clean window hands it over at once, a
//! window with unsaved work asks the person, and the command waits as long
//! as the person takes. The window observes while the command runs, hears
//! when the run starts and ends, and takes the project back when the
//! command says `released` — at every end of the command — or, when the
//! command is killed, by its own check. Any other holder is named in the
//! refusal and nothing is sent. The person's own `--live` is the consent:
//! the window's setting for the agent does not govern the command.
use crate::{
    agent::{GRANTED_LOCK, agent_name, command_name, could_try, on_windows_drive},
    executor::{self, CancellationToken, DEADLINE, EndedBy, RunOutcome},
    handover::{self, Reply},
};
use apiforge_channel::{Asker, Connection, Endpoint, Message, Received, RunMode, RunState, Writer};
use apiforge_core::{journal::JournalRun, run::RunStatus};
use apiforge_project::{DocumentKind, ProjectSession};
use apiforge_storage::project::{Holder, HolderRole, IdentityBase, ProjectError, ProjectStore};
use std::{
    io,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// The command's name as the person typed it, in the holder file and in the
/// request to a window.
pub const CLIENT: &str = "run --live";
/// Said once the window has handed the project over.
pub const HANDED_OVER: &str = "The open API Forge window handed the project over for this run";
/// Said once when the window has not answered within [`NOTICE`].
pub const WAITING: &str =
    "The API Forge window has unsaved work and asks you what to do; answer there";
/// The reason a window gives when the person keeps editing (MCP-08c).
const KEEP_EDITING: &str = "the person chose to keep editing";
/// How long the command waits for the window's answer before it says so.
const NOTICE: Duration = Duration::from_secs(2);
/// The prefix of the command's channel log lines.
const PROGRAM: &str = "apiforge run";

/// Runs the saved scenario `scenario` (a registered path or an entity ID) of
/// the folder project at `folder` live, journalled, and returns what
/// happened. `before` hears what an earlier run left unfinished and whether
/// the journal was unreadable, before this run adds to it.
pub fn run_live(
    folder: &Path,
    scenario: &str,
    secrets: Option<&Path>,
    before: &mut dyn FnMut(&[JournalRun], bool),
) -> Result<RunOutcome, String> {
    // A scenario that is not there, or secrets that cannot be read, ask no
    // window for anything.
    select(
        &ProjectSession::open_saved_read_only(folder).map_err(|error| error.to_string())?,
        scenario,
    )?;
    let secrets = executor::secrets(secrets)?;
    let mut hold = Hold::take(folder)?;
    let mut session = ProjectSession::from_store_saved(hold.store().clone())
        .map_err(|error| error.to_string())?;
    let (id, path) = select(&session, scenario)?;
    let snapshot = session
        .current_snapshot(&id)
        .map_err(|error| error.to_string())?;
    let IdentityBase::Scenario(chosen) = snapshot.document() else {
        return Err("Expected a scenario".into());
    };
    let mut project = snapshot.context().clone();
    project.scenarios.push(chosen.clone());
    session.store().set_known_secrets(secrets.values().cloned());
    let journal = apiforge_storage::JournalStore::open_project(session.store().clone())
        .map_err(|error| error.to_string())?;
    drop(session);
    before(&journal.interrupted, journal.unreadable);
    let request = apiforge_runner::RunRequest {
        project,
        scenario_id: chosen.id.clone(),
        target: apiforge_runner::RunTarget::Scenario,
        mode: apiforge_core::run::RunMode::Live,
        target_node_id: None,
        single_node_id: None,
        previous_run: None,
        secrets,
    };
    hold.tell_run(&path, RunState::Running);
    let outcome = executor::execute(request, Some(journal), CancellationToken::new(), DEADLINE);
    hold.tell_run(
        &path,
        match &outcome {
            Ok(outcome) if outcome.ended_by != EndedBy::Completed => RunState::Cancelled,
            Ok(outcome) => match outcome.report.status {
                RunStatus::Passed => RunState::Passed,
                RunStatus::Cancelled => RunState::Cancelled,
                RunStatus::Failed | RunStatus::Running => RunState::Failed,
            },
            Err(_) => RunState::Failed,
        },
    );
    hold.release();
    outcome
}

/// The registered scenario named by its path or entity ID: its id and path.
fn select(session: &ProjectSession, scenario: &str) -> Result<(String, String), String> {
    session
        .documents()
        .iter()
        .find(|document| {
            document.entry().kind == DocumentKind::Scenario
                && (document.entry().entity_id == scenario
                    || document.entry().path.as_str() == scenario)
        })
        .map(|document| {
            (
                document.id().to_owned(),
                document.entry().path.as_str().to_owned(),
            )
        })
        .ok_or_else(|| "No scenario has this registered path or ID".to_owned())
}

/// The command's hold on the project: the writer lock, and the connection
/// to the window that handed it over, when one did. Dropping it releases
/// the lock first and then tells the window.
struct Hold {
    store: Option<ProjectStore>,
    link: Option<Link>,
}

/// The connection to the window that handed the project over.
struct Link {
    runtime: tokio::runtime::Runtime,
    writer: Writer,
    reader: tokio::task::JoinHandle<()>,
    /// The connection was lost; said once on stderr.
    lost: Arc<AtomicBool>,
}

impl Hold {
    /// The writer lock, asked of the window that holds it when that window
    /// names its channel; any other holder is named in the refusal.
    fn take(folder: &Path) -> Result<Self, String> {
        let holder = Holder::command(CLIENT);
        let probe = ProjectStore::open_read_only(folder).map_err(|error| error.to_string())?;
        // A holder that leaves between the refusal and the reading of its
        // description gets one more attempt.
        for _ in 0..2 {
            match ProjectStore::open_exclusive_as(folder, &holder) {
                Ok(store) => {
                    return Ok(Self {
                        store: Some(store),
                        link: None,
                    });
                }
                Err(ProjectError::Locked) => match probe.holder() {
                    Ok(Some(found)) => {
                        if found.role() == Some(HolderRole::Window)
                            && let Some(endpoint) = found.endpoint()
                        {
                            return Self::ask(folder, &holder, &probe, &found, endpoint);
                        }
                        return Err(held(&found, None));
                    }
                    Ok(None) => continue,
                    Err(error) => return Err(error.to_string()),
                },
                Err(error) => return Err(error.to_string()),
            }
        }
        Err(held(&Holder::unknown(), None))
    }

    /// Asks the window on `endpoint` for the project and waits for its
    /// answer, however long the person takes. On `granted` the window has
    /// let the lock go; a third process that took it meanwhile is named, and
    /// the window is told `released` so it takes the project back.
    fn ask(
        folder: &Path,
        holder: &Holder,
        probe: &ProjectStore,
        window: &Holder,
        endpoint: &str,
    ) -> Result<Self, String> {
        let not_reached = || held(window, could_try(window, endpoint).then_some(probe.root()));
        let Some(endpoint) = Endpoint::parse(endpoint) else {
            return Err(not_reached());
        };
        // One worker, so the connection is read while the run holds this
        // thread.
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|error| format!("Cannot ask the API Forge window: {error}"))?;
        let request = Message::Request {
            client: Some(CLIENT.to_owned()),
            pid: std::process::id(),
            role: Some(Asker::Command),
        };
        let owner_uid = window.owner_uid();
        let asked = runtime.block_on(async {
            let mut connection = apiforge_channel::connect(&endpoint, owner_uid).await?;
            // The window says the person's consent to the agent's live
            // requests; the command hears it and does not obey it.
            let mut consent = None;
            let reply = {
                let asking =
                    handover::request_project(&mut connection, &request, &mut consent, PROGRAM);
                tokio::pin!(asking);
                tokio::select! {
                    reply = &mut asking => reply,
                    () = tokio::time::sleep(NOTICE) => {
                        eprintln!("{WAITING}");
                        asking.await
                    }
                }
            }?;
            Ok::<_, io::Error>((reply, connection))
        });
        let connection = match asked {
            Ok((Reply::Granted, connection)) => connection,
            Ok((Reply::Refused(reason), _)) => return Err(kept(&reason)),
            Err(error) => {
                eprintln!(
                    "{PROGRAM}: channel: the window could not be asked: {}",
                    error.kind()
                );
                return Err(not_reached());
            }
        };
        let deadline = Instant::now() + GRANTED_LOCK;
        loop {
            match ProjectStore::open_exclusive_as(folder, holder) {
                Ok(store) => {
                    eprintln!("{HANDED_OVER}");
                    return Ok(Self {
                        store: Some(store),
                        link: Some(Link::start(runtime, connection)),
                    });
                }
                Err(ProjectError::Locked) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(error) => {
                    // The window let go, but the project is not the
                    // command's: the window takes it back once it is free.
                    Link::start(runtime, connection).close(true);
                    return Err(match error {
                        ProjectError::Locked => match probe.holder() {
                            Ok(Some(other)) if other.role() != Some(HolderRole::Window) => {
                                held(&other, None)
                            }
                            _ => held(&Holder::unknown(), None),
                        },
                        error => error.to_string(),
                    });
                }
            }
        }
    }

    fn store(&self) -> &ProjectStore {
        self.store.as_ref().expect("the command holds the project")
    }

    /// Tells the window the run's `state`.
    fn tell_run(&mut self, path: &str, state: RunState) {
        if let Some(link) = &mut self.link {
            link.send(&Message::Run {
                path: path.to_owned(),
                mode: RunMode::Live,
                state,
            });
        }
    }

    /// Lets the project go, then tells the window, which takes it back.
    fn release(&mut self) {
        drop(self.store.take());
        if let Some(link) = self.link.take() {
            link.close(true);
        }
    }
}

impl Drop for Hold {
    /// Every end of the command gives the project back the same way.
    fn drop(&mut self) {
        self.release();
    }
}

impl Link {
    fn start(runtime: tokio::runtime::Runtime, connection: Connection) -> Self {
        let (mut reader, writer) = connection.split();
        let lost = Arc::new(AtomicBool::new(false));
        let gone = lost.clone();
        let reader = runtime.spawn(async move {
            loop {
                match reader.next().await {
                    // The person's consent to the agent's live requests,
                    // said on every change: not the command's (decision 3).
                    Ok(Some(Received::Message(Message::Permission { .. }))) => {}
                    Ok(Some(Received::Message(message))) => {
                        handover::ignored_by(PROGRAM, handover::kind(&message));
                    }
                    Ok(Some(Received::Ignored(why))) => handover::ignored_by(PROGRAM, why),
                    Ok(None) | Err(_) => {
                        say_lost(&gone);
                        break;
                    }
                }
            }
        });
        Self {
            runtime,
            writer,
            reader,
            lost,
        }
    }

    /// Sends `message`; a window that cannot be told is lost, and the run
    /// goes on.
    fn send(&mut self, message: &Message) {
        if self.lost.load(Ordering::Acquire) {
            return;
        }
        let writer = &mut self.writer;
        let sent = self
            .runtime
            .block_on(async { tokio::time::timeout(handover::SEND, writer.send(message)).await });
        if !matches!(sent, Ok(Ok(()))) {
            say_lost(&self.lost);
        }
    }

    /// Says `released` when `released`, then ends the connection. From
    /// here on the end of the connection is the command's own, never a lost
    /// window: a window that closes as soon as it hears `released` is not
    /// reported.
    fn close(mut self, released: bool) {
        let lost = self.lost.swap(true, Ordering::AcqRel);
        if released && !lost {
            let writer = &mut self.writer;
            let _ = self.runtime.block_on(async {
                tokio::time::timeout(handover::SEND, writer.send(&Message::Released)).await
            });
        }
        self.reader.abort();
        let reader = self.reader;
        let _ = self.runtime.block_on(reader);
        drop(self.writer);
        // Nothing is left on the runtime: the reader has ended.
        self.runtime.shutdown_background();
    }
}

/// The one line a lost connection leaves on stderr.
fn say_lost(lost: &AtomicBool) {
    if !lost.swap(true, Ordering::AcqRel) {
        eprintln!("{PROGRAM}: channel: the window disconnected; the run goes on");
    }
}

/// The window kept the project and said why.
fn kept(reason: &str) -> String {
    let reason = if reason == KEEP_EDITING {
        "you chose to keep editing"
    } else {
        reason
    };
    format!("The window kept the project ({reason}); nothing was sent")
}

/// The refusal while another process holds the project: who and since
/// when, on this computer's clock. `tried` is the project's folder when a
/// window was actually tried over its channel and could not be reached, for
/// MCP-12's Windows/WSL sentence.
fn held(holder: &Holder, tried: Option<&Path>) -> String {
    let since = holder
        .since()
        .map(wall_clock)
        .unwrap_or_else(|| "an unknown time".to_owned());
    match holder.role() {
        Some(HolderRole::Window) => {
            let mut text = format!(
                "The API Forge window has held this project since {since} and could not be asked; nothing was sent. Close the project there or update API Forge, then run again"
            );
            if tried.is_some_and(on_windows_drive) {
                text.push_str(". This project is on a Windows drive; if the window and this command run on different sides of Windows/WSL, run both on the same side");
            }
            text
        }
        Some(HolderRole::Command) => format!(
            "{} has held this project since {since}; nothing was sent. Run again when it has finished",
            command_name(holder.client())
        ),
        Some(HolderRole::Agent) => format!(
            "{} has held this project since {since}; nothing was sent. Run again when it has finished editing",
            agent_name(holder.client())
        ),
        None => {
            "Another process holds this project; nothing was sent. Run again when it has let go"
                .to_owned()
        }
    }
}

/// `HH:MM` on this computer's clock for an RFC 3339 timestamp, or the text
/// as it is when it is not one.
fn wall_clock(since: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(since)
        .map(|at| at.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_else(|_| since.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(since: &str) -> String {
        wall_clock(since)
    }

    /// Criterion 6: each holder in the words of the ticket.
    #[test]
    fn every_holder_is_named_with_nothing_sent() {
        let since = "2026-11-01T09:42:11Z";
        let time = at(since);
        assert_eq!(time.len(), 5, "{time}");
        let agent = Holder::new(HolderRole::Agent, Some("Claude Code".into())).with_since(since);
        assert_eq!(
            held(&agent, None),
            format!(
                "Claude Code via apiforge mcp has held this project since {time}; nothing was sent. Run again when it has finished editing"
            )
        );
        assert_eq!(
            held(&Holder::command("apply").with_since(since), None),
            format!(
                "apiforge apply has held this project since {time}; nothing was sent. Run again when it has finished"
            )
        );
        assert_eq!(
            held(&Holder::window().with_since(since), None),
            format!(
                "The API Forge window has held this project since {time} and could not be asked; nothing was sent. Close the project there or update API Forge, then run again"
            )
        );
        assert_eq!(
            held(&Holder::unknown(), None),
            "Another process holds this project; nothing was sent. Run again when it has let go"
        );
        assert_eq!(
            held(&Holder::window().with_since("not a time"), None),
            "The API Forge window has held this project since not a time and could not be asked; nothing was sent. Close the project there or update API Forge, then run again"
        );
    }

    #[test]
    fn the_person_keeping_the_project_is_said_in_their_words() {
        assert_eq!(
            kept(KEEP_EDITING),
            "The window kept the project (you chose to keep editing); nothing was sent"
        );
        assert_eq!(
            kept("the window has unsaved work"),
            "The window kept the project (the window has unsaved work); nothing was sent"
        );
    }

    /// Criterion 6: the Windows/WSL sentence only for a window that was
    /// tried and not reached, on a Windows drive mounted in Linux.
    #[cfg(target_os = "linux")]
    #[test]
    fn only_an_unreachable_window_on_a_windows_drive_gets_the_wsl_sentence() {
        let window = Holder::window().with_since("2026-11-01T09:42:11Z");
        for (path, hint) in [
            ("/mnt/c/project", true),
            ("/mnt/C/project", true),
            ("/mnt/cat/project", false),
            ("/mnt/c", false),
            ("/home/project", false),
        ] {
            let text = held(&window, Some(Path::new(path)));
            assert_eq!(text.contains("Windows/WSL"), hint, "{path}: {text}");
            assert!(text.contains("could not be asked; nothing was sent"));
        }
        assert!(!held(&window, None).contains("Windows/WSL"), "not tried");
        let agent = Holder::new(HolderRole::Agent, None);
        assert!(!held(&agent, Some(Path::new("/mnt/c/project"))).contains("Windows/WSL"));
    }
}
