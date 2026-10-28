//! The window's end of the control channel (MCP-08b, ADR 0014 decision 4).
//!
//! While the window holds the project it listens on an endpoint named in its
//! holder file. An agent's server that wants to write connects and asks; a
//! clean window lets the project go at once and observes over the same
//! connection (decision 11 A), a window with unsaved work refuses and says
//! so in the bar. While the window observes an agent it keeps one
//! connection to it: a `saved` makes it look at the folder at once rather
//! than at the next check, `released` lets it take the project back, and
//! *Take back* asks for it. The once-a-second check stays the fallback, and
//! the window never takes the project while its holder lives.
//!
//! The listener and the connections live on one background thread with a
//! current-thread runtime; the frame reads what they report, as it reads the
//! watcher's results.
use super::*;
use apiforge_channel::{Connection, Endpoint, Listener, Message, Received, RunState, Writer};
use apiforge_storage::project::{Holder, HolderRole};
use std::{
    collections::HashMap,
    sync::mpsc::{self, Sender},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

/// The reason the window gives when it keeps the project for unsaved work;
/// the server words its refusal from it.
const UNSAVED_WORK: &str = "the window has unsaved work";
/// How long after a failed connection to the holder the window tries again.
const RECONNECT: Duration = Duration::from_secs(1);
/// How long one message may take to reach the other side.
const SEND: Duration = Duration::from_secs(2);
/// How long the window waits for an agent it granted the project to take
/// the lock or say it could not, before a free lock is free again.
const GRANT: Duration = Duration::from_secs(10);

/// What the channel's thread reports to the frame.
enum Event {
    /// A server asks for the project on connection `link`.
    Request {
        link: u64,
        client: Option<String>,
    },
    Message {
        link: u64,
        message: Message,
    },
    /// A connection this window opened is open.
    Connected {
        link: u64,
    },
    /// A connection closed, or could not be opened.
    Closed {
        link: u64,
    },
}

enum Command {
    Send { link: u64, message: Message },
    Connect { link: u64, endpoint: Endpoint },
    Close { link: u64 },
}

/// The listener and the connections, on their own thread.
pub(super) struct Channel {
    endpoint: Option<String>,
    commands: UnboundedSender<Command>,
    events: Receiver<Event>,
    next: u64,
}

impl Channel {
    /// Listens on a new endpoint. A window that cannot listen still works:
    /// it names no endpoint, and an agent is refused as before.
    pub(super) fn start(repaint: Option<egui::Context>) -> Self {
        let (commands, receiver) = unbounded_channel();
        let (sender, events) = mpsc::channel();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        let mut endpoint = None;
        match runtime {
            Ok(runtime) => {
                let listener = {
                    let _context = runtime.enter();
                    Listener::bind()
                };
                let listener = match listener {
                    Ok(listener) => {
                        endpoint = Some(listener.endpoint().name().to_owned());
                        Some(listener)
                    }
                    Err(error) => {
                        eprintln!("apiforge: channel: cannot listen: {}", error.kind());
                        None
                    }
                };
                let spawned = std::thread::Builder::new()
                    .name("apiforge-channel".into())
                    .spawn(move || {
                        runtime.block_on(serve(listener, receiver, sender, repaint));
                    });
                if spawned.is_err() {
                    eprintln!("apiforge: channel: cannot start its thread");
                    endpoint = None;
                }
            }
            Err(error) => eprintln!("apiforge: channel: cannot start: {}", error.kind()),
        }
        Self {
            endpoint,
            commands,
            events,
            // Connections this window opens; accepted ones count from the top.
            next: 1,
        }
    }

    /// The endpoint's name, for the holder file.
    pub(super) fn endpoint(&self) -> Option<&str> {
        self.endpoint.as_deref()
    }

    fn send(&self, link: u64, message: Message) {
        let _ = self.commands.send(Command::Send { link, message });
    }

    fn connect(&mut self, endpoint: Endpoint) -> u64 {
        let link = self.next;
        self.next += 1;
        let _ = self.commands.send(Command::Connect { link, endpoint });
        link
    }

    fn close(&self, link: u64) {
        let _ = self.commands.send(Command::Close { link });
    }
}

struct Open {
    writer: Writer,
    reader: tokio::task::JoinHandle<()>,
}

/// The channel's thread: accepts, connects, reads and writes until the
/// window drops its end.
async fn serve(
    mut listener: Option<Listener>,
    mut commands: UnboundedReceiver<Command>,
    events: Sender<Event>,
    repaint: Option<egui::Context>,
) {
    let tell = move |event: Event| {
        let _ = events.send(event);
        if let Some(context) = &repaint {
            context.request_repaint();
        }
    };
    let mut open: HashMap<u64, Open> = HashMap::new();
    let (closed, mut gone) = unbounded_channel::<u64>();
    let mut accepted = u64::MAX / 2;
    loop {
        tokio::select! {
            connection = accept(&mut listener) => match connection {
                Ok(connection) => {
                    accepted += 1;
                    let reading = read(accepted, connection, &tell, &closed);
                    open.insert(accepted, reading);
                }
                Err(error) => {
                    eprintln!("apiforge: channel: a connection failed: {}", error.kind());
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            },
            command = commands.recv() => match command {
                // The window has gone: so have its connections.
                None => break,
                Some(Command::Send { link, message }) => {
                    let sent = match open.get_mut(&link) {
                        Some(connection) => matches!(
                            tokio::time::timeout(SEND, connection.writer.send(&message)).await,
                            Ok(Ok(()))
                        ),
                        None => true,
                    };
                    if !sent && let Some(connection) = open.remove(&link) {
                        connection.reader.abort();
                        tell(Event::Closed { link });
                    }
                }
                Some(Command::Connect { link, endpoint }) => {
                    match tokio::time::timeout(SEND, apiforge_channel::connect(&endpoint)).await {
                        Ok(Ok(connection)) => {
                            let reading = read(link, connection, &tell, &closed);
                            open.insert(link, reading);
                            tell(Event::Connected { link });
                        }
                        _ => {
                            eprintln!("apiforge: channel: the holder could not be reached");
                            tell(Event::Closed { link });
                        }
                    }
                }
                Some(Command::Close { link }) => {
                    if let Some(connection) = open.remove(&link) {
                        connection.reader.abort();
                    }
                }
            },
            Some(link) = gone.recv() => {
                if open.remove(&link).is_some() {
                    tell(Event::Closed { link });
                }
            }
        }
    }
    for (_, connection) in open {
        connection.reader.abort();
    }
}

async fn accept(listener: &mut Option<Listener>) -> std::io::Result<Connection> {
    match listener {
        Some(listener) => listener.accept().await,
        None => std::future::pending().await,
    }
}

/// Reads one connection until it closes, reporting what it says.
fn read(
    link: u64,
    connection: Connection,
    tell: &(impl Fn(Event) + Clone + Send + 'static),
    closed: &UnboundedSender<u64>,
) -> Open {
    let (mut reader, writer) = connection.split();
    let tell = tell.clone();
    let closed = closed.clone();
    let reader = tokio::spawn(async move {
        loop {
            match reader.next().await {
                Ok(Some(Received::Message(Message::Request { client, .. }))) => {
                    tell(Event::Request { link, client });
                }
                Ok(Some(Received::Message(message))) => tell(Event::Message { link, message }),
                Ok(Some(Received::Ignored(why))) => {
                    eprintln!("apiforge: channel: ignored {why}");
                }
                Ok(None) | Err(_) => break,
            }
        }
        let _ = closed.send(link);
    });
    Open { writer, reader }
}

/// Where the hand-over stands, for the frame.
pub(super) struct Handover {
    pub(super) channel: Channel,
    /// A request not answered yet: its connection and the agent's name.
    pending: Option<(u64, Option<String>)>,
    /// The request whose release is on the storage worker, and whether its
    /// connection closed meanwhile: then nobody waits for the project.
    releasing: Option<(u64, bool)>,
    /// The connection to the agent this window observes.
    link: Option<u64>,
    /// A connection being opened to the holder's endpoint.
    connecting: Option<u64>,
    /// When the last connection to the holder failed.
    failed_at: Option<Instant>,
    /// The holder said `released`, or went: the take-back may go ahead.
    released: bool,
    /// When the window granted the project, until a check finds the agent
    /// holding it: a lock that looks free meanwhile is the agent between the
    /// grant and its own lock, not a lock let go.
    granted_at: Option<Instant>,
    /// No check has named a holder since the last connection ended, so the
    /// holder last named may be gone: no new connection to it.
    stale: bool,
    /// The person asked for the project back and waits for `released`.
    pub(super) taking_back: bool,
    /// The holder's run is in progress, as its last `run` message said.
    agent_running: bool,
    /// A request was refused for unsaved work: the bar says so until the
    /// work is saved.
    pub(super) refused: bool,
}

impl Handover {
    pub(super) fn new(repaint: Option<egui::Context>) -> Self {
        Self {
            channel: Channel::start(repaint),
            pending: None,
            releasing: None,
            link: None,
            connecting: None,
            failed_at: None,
            released: false,
            granted_at: None,
            stale: false,
            taking_back: false,
            agent_running: false,
            refused: false,
        }
    }

    /// The holder this window observes over a connection, so *Take back*
    /// can be offered.
    pub(super) fn is_linked(&self) -> bool {
        self.link.is_some() && !self.released
    }

    /// Whether the take-back must wait: the agent was granted the project
    /// and has neither been found holding it nor said it let go.
    pub(super) fn holds_back(&self) -> bool {
        !self.released
            && self
                .granted_at
                .is_some_and(|granted| granted.elapsed() < GRANT)
    }

    /// A check named the holder: an agent that was granted the project has
    /// taken it, and a connection to the holder may be opened again.
    pub(super) fn holder_seen(&mut self, holder: &Holder) {
        if holder.role() == Some(HolderRole::Agent) {
            self.granted_at = None;
        }
        self.stale = false;
    }

    /// The agent let go, or its connection ended.
    fn let_go(&mut self) {
        self.released = true;
        self.granted_at = None;
        self.stale = true;
        self.agent_running = false;
    }

    /// The person waits for the agent's run to end before the take-back.
    pub(super) fn waits_for_run(&self) -> bool {
        self.taking_back && self.agent_running
    }

    /// The window is the writer again: what belonged to the agent is gone.
    pub(super) fn reset_observing(&mut self) {
        if let Some(link) = self.link.take() {
            self.channel.close(link);
        }
        if let Some(link) = self.connecting.take() {
            self.channel.close(link);
        }
        self.released = false;
        self.granted_at = None;
        self.stale = false;
        self.taking_back = false;
        self.agent_running = false;
        self.failed_at = None;
    }
}

impl FolderApp {
    /// Follows what the channel reported since the last frame, answers a
    /// request when the window can, and connects to an agent it observes.
    pub(super) fn follow_channel(&mut self) {
        let events: Vec<Event> = self.handover.channel.events.try_iter().collect();
        for event in events {
            self.follow_channel_event(event);
        }
        self.answer_request();
        self.connect_to_holder();
        if self.handover.refused
            && self
                .session
                .as_ref()
                .is_none_or(|session| session.is_read_only() || !session.has_unsaved_work())
        {
            self.handover.refused = false;
        }
    }

    fn follow_channel_event(&mut self, event: Event) {
        let handover = &mut self.handover;
        match event {
            Event::Request { link, client } => {
                // The newest request is the one an agent waits for.
                if let Some((older, _)) = handover.pending.replace((link, client))
                    && older != link
                {
                    handover.channel.close(older);
                }
            }
            Event::Message { link, message } if Some(link) == handover.link => match message {
                Message::Saved { .. } => self.watcher.rescan(),
                Message::Released => {
                    handover.let_go();
                    self.watcher.rescan();
                }
                Message::Run { state, .. } => {
                    handover.agent_running = state == RunState::Running;
                }
                Message::Permission { .. } => {}
                Message::Request { .. }
                | Message::Granted
                | Message::Refused { .. }
                | Message::TakeBack => {
                    eprintln!("apiforge: channel: ignored a message only a window sends");
                }
            },
            Event::Message { .. } => {
                eprintln!(
                    "apiforge: channel: ignored a message on a connection that observes nothing"
                );
            }
            Event::Connected { link } => {
                if handover.connecting == Some(link) {
                    handover.connecting = None;
                    handover.link = Some(link);
                    handover.released = false;
                    handover.failed_at = None;
                } else {
                    handover.channel.close(link);
                }
            }
            Event::Closed { link } => {
                if handover.connecting == Some(link) {
                    handover.connecting = None;
                    handover.failed_at = Some(Instant::now());
                }
                if handover
                    .pending
                    .as_ref()
                    .is_some_and(|(pending, _)| *pending == link)
                {
                    handover.pending = None;
                }
                if let Some((releasing, lost)) = &mut handover.releasing
                    && *releasing == link
                {
                    *lost = true;
                }
                if handover.link == Some(link) {
                    // The agent finished, or its process ended: the lock is
                    // free or about to be, and the next check finds out.
                    handover.link = None;
                    handover.let_go();
                    self.watcher.rescan();
                }
            }
        }
    }

    /// Answers the waiting request once the window can: a clean window
    /// hands the project over, a window with unsaved work refuses. While
    /// storage work, a run, a dialog or a switch is in progress the request
    /// waits; the agent hears `handover_pending` after thirty seconds and
    /// the next request is answered at once.
    fn answer_request(&mut self) {
        let Some((link, client)) = self.handover.pending.clone() else {
            return;
        };
        if self.handover.releasing.is_some() {
            return;
        }
        let Some(session) = self.session.as_ref() else {
            if !self.opening {
                self.refuse(link, "the window has no project open");
            }
            return;
        };
        if session.is_read_only() {
            self.refuse(link, "the window does not hold the project");
            return;
        }
        if self.persistence.closing {
            self.refuse(link, "the window is closing");
            return;
        }
        if session.has_unsaved_work() {
            self.refuse(link, UNSAVED_WORK);
            self.handover.refused = true;
            return;
        }
        if self.storage.is_busy()
            || self.runs.handle.is_some()
            || self.prompt.is_some()
            || self.watcher.recovery_required
            || self.switching()
            || self.opening
        {
            return;
        }
        self.release(link, client);
    }

    fn refuse(&mut self, link: u64, reason: &str) {
        self.handover.pending = None;
        self.handover.channel.send(
            link,
            Message::Refused {
                reason: reason.to_owned(),
            },
        );
        self.handover.channel.close(link);
    }

    /// Lets the project go to the agent that asked on `link`: the saved
    /// project is reread read-only with the same navigation, and only then
    /// is the agent told `granted`, so the lock is free when it tries.
    fn release(&mut self, link: u64, client: Option<String>) {
        self.handover.pending = None;
        self.handover.releasing = Some((link, false));
        self.close_run_journal();
        self.storage_project(
            "Handing the project over…",
            apiforge_project::StorageMode::Exclusive,
            true,
            |session| session.release_writer(),
            move |app, result| {
                let lost = app.handover.releasing.take().is_some_and(|(_, lost)| lost);
                match result {
                    Ok(true) => {
                        app.handed_over(link, client);
                        if lost {
                            // The agent stopped waiting: the window takes the
                            // project back at its next check.
                            app.handover.link = None;
                            app.handover.let_go();
                        }
                    }
                    // Unsaved work appeared, or the window changed while the
                    // project was read: decide again.
                    Ok(false) | Err(apiforge_project::Error::StaleStorage) => {
                        app.handover.pending = Some((link, client));
                        app.open_run_journal();
                    }
                    Err(error) => {
                        eprintln!("apiforge: channel: the project could not be handed over");
                        app.refuse(link, "the window could not hand the project over");
                        app.open_run_journal();
                        app.notify(
                            format!("This window could not hand the project over: {error}"),
                            true,
                        );
                    }
                }
            },
        );
    }

    /// The window observes the agent it handed the project to, over the
    /// connection the agent asked on.
    fn handed_over(&mut self, link: u64, client: Option<String>) {
        self.handover.reset_observing();
        self.handover.link = Some(link);
        self.handover.granted_at = Some(Instant::now());
        self.handover.refused = false;
        self.holder = Some(holder::Observed::new(Holder::new(
            HolderRole::Agent,
            client,
        )));
        self.take_back = holder::TakeBack::default();
        self.last_change = None;
        self.follow_external(ExternalOutcome {
            project_reloaded: true,
            ..Default::default()
        });
        self.handover.channel.send(link, Message::Granted);
        self.watcher.rescan();
    }

    /// While observing an agent that names an endpoint, keeps a connection
    /// to it.
    fn connect_to_holder(&mut self) {
        if !self.observing()
            || self.handover.link.is_some()
            || self.handover.connecting.is_some()
            || self.handover.stale
            || self
                .handover
                .failed_at
                .is_some_and(|failed| failed.elapsed() < RECONNECT)
        {
            return;
        }
        let Some(endpoint) = self
            .holder
            .as_ref()
            .map(holder::Observed::holder)
            .filter(|holder| holder.role() == Some(HolderRole::Agent))
            .and_then(Holder::endpoint)
            .and_then(Endpoint::parse)
        else {
            return;
        };
        let link = self.handover.channel.connect(endpoint);
        self.handover.connecting = Some(link);
        self.handover.released = false;
    }

    /// *Take back*: asks the agent to finish what is in flight and let go.
    pub(super) fn request_take_back(&mut self) {
        let Some(link) = self.handover.link else {
            return;
        };
        if self.handover.taking_back {
            return;
        }
        self.handover.taking_back = true;
        self.handover.channel.send(link, Message::TakeBack);
    }

    /// The window holds the project: its holder file names its endpoint.
    pub(super) fn announce_endpoint(&mut self) {
        let Some(endpoint) = self.handover.channel.endpoint().map(str::to_owned) else {
            return;
        };
        let Some(session) = self
            .session
            .as_ref()
            .filter(|session| !session.is_read_only())
        else {
            return;
        };
        let store = session.store().clone();
        let identity = session.storage_identity();
        self.storage_io(
            "Naming this window's channel…",
            move || {
                let since = store
                    .holder()
                    .ok()
                    .flatten()
                    .and_then(|holder| holder.since().map(str::to_owned));
                let mut holder = Holder::window().with_endpoint(Some(endpoint));
                if let Some(since) = since {
                    holder = holder.with_since(since);
                }
                store.describe(&holder)
            },
            move |app, result| {
                if !app.session_matches(&identity) {
                    return;
                }
                if !matches!(result, Ok(Ok(()))) {
                    eprintln!(
                        "apiforge: channel: the holder file could not name this window's channel"
                    );
                }
            },
        );
    }

    /// The writer's description, with this window's endpoint.
    pub(super) fn window_holder(&self) -> Holder {
        Holder::window().with_endpoint(self.handover.channel.endpoint().map(str::to_owned))
    }
}
