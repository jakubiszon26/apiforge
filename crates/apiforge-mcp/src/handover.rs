//! The server's end of the control channel (ADR 0014 decision 4, MCP-08b).
//!
//! While the agent session holds the project the server listens on an
//! endpoint it names in its holder file; a window that opens meanwhile
//! connects there and observes. When a window holds the project, the
//! session's first write asks it over the window's own endpoint; a window
//! that grants keeps that connection to observe. Every window connected this
//! way is told each commit (`saved`) and the end of the session
//! (`released`), and may ask for the project back (`take_back`).
//!
//! The channel never decides anything on the agent's behalf: a refusal, a
//! request nobody answered and a window that cannot be reached become the
//! write's result. Lines that are not messages are ignored and logged on
//! stderr by kind, never quoted.
use apiforge_channel::{Connection, Endpoint, Listener, Message, Reader, Received, Writer};
use std::{
    io,
    sync::{
        Arc, Mutex, MutexGuard, OnceLock, Weak,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{runtime::Handle, task::JoinHandle};

/// How long a write waits for the window's answer before `handover_pending`.
const ANSWER: Duration = Duration::from_secs(30);
/// How long one message may take to reach a window before it is dropped.
const SEND: Duration = Duration::from_secs(2);

/// What a window answered when it was asked for the project.
pub(crate) enum Answer {
    /// The window let the project go; the connection stays to observe.
    Granted(Connection),
    Refused(String),
    /// Nobody answered in time; nothing was decided.
    Pending,
    /// The window could not be reached over the channel.
    Unreachable,
}

type TakeBack = Arc<dyn Fn() + Send + Sync>;

struct Link {
    id: u64,
    writer: Arc<tokio::sync::Mutex<Writer>>,
    reader: JoinHandle<()>,
}

/// Every window connected to this server, and the server's endpoint.
pub struct Hub {
    handle: Handle,
    endpoint: Mutex<Option<Endpoint>>,
    links: Mutex<Vec<Link>>,
    next: AtomicU64,
    take_back: OnceLock<TakeBack>,
}

impl std::fmt::Debug for Hub {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Hub")
            .field("endpoint", &*lock(&self.endpoint))
            .field("links", &lock(&self.links).len())
            .finish_non_exhaustive()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Hub {
    /// A hub whose connections run on `handle`. Its blocking calls are made
    /// from outside the runtime's async tasks: the tools' blocking pool, or
    /// the thread that waits for the server to end.
    pub fn new(handle: Handle) -> Arc<Self> {
        Arc::new(Self {
            handle,
            endpoint: Mutex::default(),
            links: Mutex::default(),
            next: AtomicU64::new(1),
            take_back: OnceLock::new(),
        })
    }

    /// What to do when a window asks for the project back. Called on the
    /// blocking pool, so it may wait for a run or a write to finish.
    pub fn on_take_back(&self, take_back: impl Fn() + Send + Sync + 'static) {
        let _ = self.take_back.set(Arc::new(take_back));
    }

    /// The endpoint to name in the holder file, listening from the first
    /// call on. `None` when the server cannot listen: it then works as
    /// before, without a window following it.
    pub fn endpoint(self: &Arc<Self>) -> Option<String> {
        let mut endpoint = lock(&self.endpoint);
        if let Some(endpoint) = &*endpoint {
            return Some(endpoint.name().to_owned());
        }
        let bound = {
            let _runtime = self.handle.enter();
            Listener::bind()
        };
        match bound {
            Ok(listener) => {
                let name = listener.endpoint().clone();
                self.handle.spawn(accept(Arc::downgrade(self), listener));
                *endpoint = Some(name.clone());
                Some(name.name().to_owned())
            }
            Err(error) => {
                eprintln!("apiforge mcp: channel: cannot listen: {}", error.kind());
                None
            }
        }
    }

    /// Asks the window listening on `endpoint` for the project and waits up
    /// to thirty seconds for its answer.
    pub(crate) fn ask(&self, endpoint: &str, client: Option<String>) -> Answer {
        let Some(endpoint) = Endpoint::parse(endpoint) else {
            eprintln!("apiforge mcp: channel: the holder names no valid endpoint");
            return Answer::Unreachable;
        };
        let request = Message::Request {
            client,
            pid: std::process::id(),
        };
        self.handle.block_on(async move {
            let asked = tokio::time::timeout(ANSWER, async {
                let mut connection = apiforge_channel::connect(&endpoint).await?;
                connection.writer.send(&request).await?;
                loop {
                    match connection.reader.next().await? {
                        None => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
                        Some(Received::Message(Message::Granted)) => {
                            return Ok(Answer::Granted(connection));
                        }
                        Some(Received::Message(Message::Refused { reason })) => {
                            return Ok(Answer::Refused(reason));
                        }
                        Some(Received::Message(message)) => ignored(kind(&message)),
                        Some(Received::Ignored(why)) => ignored(why),
                    }
                }
            })
            .await;
            match asked {
                Ok(Ok(answer)) => answer,
                Ok(Err(error)) => {
                    eprintln!(
                        "apiforge mcp: channel: the window could not be asked: {}",
                        error.kind()
                    );
                    Answer::Unreachable
                }
                Err(_) => {
                    eprintln!("apiforge mcp: channel: the window has not answered yet");
                    Answer::Pending
                }
            }
        })
    }

    /// Keeps the connection of a window that granted the project: it
    /// observes from now on.
    pub(crate) fn adopt(self: &Arc<Self>, connection: Connection) {
        self.add(connection);
    }

    /// Tells the window that granted the project the server could not take
    /// it after all, so the window takes it back.
    pub(crate) fn decline(&self, mut connection: Connection) {
        let _ = self.handle.block_on(async move {
            tokio::time::timeout(SEND, connection.writer.send(&Message::Released)).await
        });
    }

    /// Tells every connected window `message`; a window that cannot be told
    /// within two seconds is let go.
    pub(crate) fn broadcast(&self, message: &Message) {
        let writers: Vec<_> = lock(&self.links)
            .iter()
            .map(|link| (link.id, link.writer.clone()))
            .collect();
        let failed = self.send(writers, message);
        if !failed.is_empty() {
            eprintln!("apiforge mcp: channel: a window could not be told and was let go");
            lock(&self.links).retain(|link| {
                let keep = !failed.contains(&link.id);
                if !keep {
                    link.reader.abort();
                }
                keep
            });
        }
    }

    /// The session ended: the windows connected now are told `released` and
    /// let go; one that connects meanwhile stays, for the next session.
    pub(crate) fn release(&self) {
        let links = std::mem::take(&mut *lock(&self.links));
        let writers = links
            .iter()
            .map(|link| (link.id, link.writer.clone()))
            .collect();
        self.send(writers, &Message::Released);
        for link in links {
            link.reader.abort();
        }
    }

    /// Sends `message` to each writer; returns the ones that failed.
    fn send(
        &self,
        writers: Vec<(u64, Arc<tokio::sync::Mutex<Writer>>)>,
        message: &Message,
    ) -> Vec<u64> {
        if writers.is_empty() {
            return Vec::new();
        }
        self.handle.block_on(async {
            let mut failed = Vec::new();
            for (id, writer) in writers {
                let sent =
                    tokio::time::timeout(SEND, async { writer.lock().await.send(message).await })
                        .await;
                if !matches!(sent, Ok(Ok(()))) {
                    failed.push(id);
                }
            }
            failed
        })
    }

    fn add(self: &Arc<Self>, connection: Connection) {
        let (reader, writer) = connection.split();
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let writer = Arc::new(tokio::sync::Mutex::new(writer));
        let task = self
            .handle
            .spawn(read(Arc::downgrade(self), id, reader, writer.clone()));
        lock(&self.links).push(Link {
            id,
            writer,
            reader: task,
        });
    }

    fn take_back(&self) {
        if let Some(take_back) = self.take_back.get() {
            let take_back = take_back.clone();
            self.handle.spawn_blocking(move || take_back());
        }
    }
}

async fn accept(hub: Weak<Hub>, mut listener: Listener) {
    loop {
        match listener.accept().await {
            Ok(connection) => match hub.upgrade() {
                Some(hub) => hub.add(connection),
                None => break,
            },
            Err(error) => {
                if hub.strong_count() == 0 {
                    break;
                }
                eprintln!(
                    "apiforge mcp: channel: a connection failed: {}",
                    error.kind()
                );
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

/// Reads what one window says until it goes.
async fn read(
    hub: Weak<Hub>,
    id: u64,
    mut reader: Reader,
    writer: Arc<tokio::sync::Mutex<Writer>>,
) {
    loop {
        match reader.next().await {
            Ok(Some(Received::Message(Message::TakeBack))) => {
                eprintln!("apiforge mcp: channel: the window asked for the project back");
                if let Some(hub) = hub.upgrade() {
                    hub.take_back();
                }
            }
            // The person's live consent belongs to MCP-09.
            Ok(Some(Received::Message(Message::Permission { .. }))) => {}
            Ok(Some(Received::Message(Message::Request { .. }))) => {
                // No hand-over between agents (MCP-00).
                let refused = Message::Refused {
                    reason: "an agent does not hand the project to another agent".into(),
                };
                let _ = writer.lock().await.send(&refused).await;
            }
            Ok(Some(Received::Message(message))) => ignored(kind(&message)),
            Ok(Some(Received::Ignored(why))) => ignored(why),
            Ok(None) | Err(_) => {
                eprintln!("apiforge mcp: channel: a window disconnected");
                break;
            }
        }
    }
    if let Some(hub) = hub.upgrade() {
        lock(&hub.links).retain(|link| link.id != id);
    }
}

fn ignored(why: &str) {
    eprintln!("apiforge mcp: channel: ignored {why}");
}

fn kind(message: &Message) -> &'static str {
    match message {
        Message::Request { .. } => "a request",
        Message::Granted => "a granted message",
        Message::Refused { .. } => "a refused message",
        Message::Saved { .. } => "a saved message",
        Message::Run { .. } => "a run message",
        Message::Released => "a released message",
        Message::Permission { .. } => "a permission message",
        Message::TakeBack => "a take_back message",
    }
}
