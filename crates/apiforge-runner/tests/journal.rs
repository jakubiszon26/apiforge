//! What the journal holds while a live run executes, and what a process that dies
//! mid-run leaves behind.
//!
//! The runs here are real HTTP exchanges against a loopback server, so the states an
//! entry passes through are the ones a live run actually produces.

use apiforge_core::{
    format::{Graph, Project},
    journal::{
        CleanupState, EffectState, Journal, JournalEntry, JournalRun, JournalSink, JournalSinkError,
    },
    run::{Phase, RunMode, RunStatus, StepStatus},
};
use apiforge_runner::{HttpTransport, RunOptions, RunRequest, run_scenario};
use serde_json::json;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

/// A journal in memory, with the calls it received and an optional failure.
#[derive(Default)]
struct Recorder {
    journal: Mutex<Journal>,
    /// One line per call, e.g. `record 1 inFlight POST`.
    calls: Mutex<Vec<String>>,
    /// Refuse every write, as a full or unwritable disk would.
    broken: bool,
}

impl Recorder {
    fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn broken() -> Arc<Self> {
        Arc::new(Self {
            broken: true,
            ..Self::default()
        })
    }

    fn snapshot(&self) -> Journal {
        self.journal.lock().unwrap().clone()
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn only_run(&self) -> JournalRun {
        let journal = self.snapshot();
        assert_eq!(journal.runs.len(), 1, "exactly one run is journalled");
        journal.runs[0].clone()
    }

    fn note(&self, line: String) -> Result<(), JournalSinkError> {
        self.calls.lock().unwrap().push(line);
        if self.broken {
            return Err(JournalSinkError::new("no space left on device"));
        }
        Ok(())
    }
}

impl JournalSink for Recorder {
    fn begin_run(&self, run: &JournalRun) -> Result<(), JournalSinkError> {
        self.note(format!("begin {}", run.scenario_name))?;
        self.journal.lock().unwrap().begin(run.clone());
        Ok(())
    }

    fn record(&self, run_id: &str, entry: &JournalEntry) -> Result<(), JournalSinkError> {
        self.note(format!(
            "record {} {} {:?} {}",
            entry.sequence,
            entry.method,
            entry.state,
            entry
                .status
                .map_or_else(|| "-".to_owned(), |status| status.to_string())
        ))?;
        let mut journal = self.journal.lock().unwrap();
        journal
            .run_mut(run_id)
            .expect("the run is journalled")
            .record(entry.clone());
        Ok(())
    }

    fn cleanup_started(&self, run_id: &str) -> Result<(), JournalSinkError> {
        self.note("cleanup".to_owned())?;
        self.journal
            .lock()
            .unwrap()
            .run_mut(run_id)
            .expect("the run is journalled")
            .cleanup = CleanupState::Started;
        Ok(())
    }

    fn finish_run(&self, run_id: &str) -> Result<(), JournalSinkError> {
        self.note("finish".to_owned())?;
        self.journal.lock().unwrap().finish(run_id);
        Ok(())
    }
}

/// What the loopback server does with the second request of the scenario.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Behavior {
    Answer,
    /// Accept the request and never answer, so the exchange stays in flight.
    Stall,
}

struct Server {
    base_url: String,
    received: Arc<AtomicUsize>,
    task: JoinHandle<()>,
}

impl Server {
    async fn start(behavior: Behavior) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let received = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&received);
        let task = tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let count = Arc::clone(&count);
                tokio::spawn(async move {
                    let mut buffer = [0; 4096];
                    let read = stream.read(&mut buffer).await.unwrap_or(0);
                    let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
                    count.fetch_add(1, Ordering::SeqCst);
                    if behavior == Behavior::Stall && request.starts_with("GET /orders/1") {
                        std::future::pending::<()>().await;
                    }
                    let body = json!({"id": 1}).to_string();
                    let _ = stream
                        .write_all(
                            format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                                body.len()
                            )
                            .as_bytes(),
                        )
                        .await;
                    let _ = stream.shutdown().await;
                });
            }
        });
        Self {
            base_url,
            received,
            task,
        }
    }

    fn requests(&self) -> usize {
        self.received.load(Ordering::SeqCst)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// A scenario that creates an order, reads it back and deletes it in cleanup.
fn project(base_url: &str) -> (Project, String) {
    let mut project: Project =
        serde_json::from_slice(include_bytes!("../../../fixtures/projects/shop.json")).unwrap();
    project.scenarios.truncate(1);
    project.environment.base_url = base_url.to_owned();
    let request = |id: &str, method: &str, path: &str| {
        json!({
            "id": id, "kind": "request", "handle": id,
            "position": {"x": 0, "y": 0},
            "inputs": {"url": {"kind": "literal", "value": path}},
            "config": {"method": method, "sampleResponse": {"id": 1}, "sampleStatus": 200}
        })
    };
    let scenario = &mut project.scenarios[0];
    scenario.name = "Orders".to_owned();
    scenario.graph = serde_json::from_value(json!({
        "nodes": [
            {"id": "start", "kind": "start", "handle": "begin",
             "position": {"x": 0, "y": 0}, "inputs": {}, "config": {}},
            request("create", "POST", "/orders"),
            request("read", "GET", "/orders/1"),
        ],
        "edges": [
            {"id": "e1", "source": "start", "target": "create"},
            {"id": "e2", "source": "create", "target": "read"},
        ]
    }))
    .unwrap();
    scenario.cleanup = serde_json::from_value(json!({
        "nodes": [request("remove", "DELETE", "/orders/1")],
        "edges": []
    }))
    .unwrap();
    let id = scenario.id.clone();
    (project, id)
}

fn options(journal: &Arc<Recorder>) -> RunOptions {
    RunOptions::new(HttpTransport::new().unwrap()).with_journal(Arc::clone(journal) as _)
}

#[tokio::test]
async fn a_run_that_reaches_its_end_resolves_every_entry_and_leaves_nothing() {
    let server = Server::start(Behavior::Answer).await;
    let (project, scenario) = project(&server.base_url);
    let journal = Recorder::shared();
    let report = run_scenario(
        RunRequest::new(project, scenario, RunMode::Live),
        options(&journal),
        CancellationToken::new(),
        |_| {},
    )
    .await
    .unwrap();

    assert_eq!(report.status, RunStatus::Passed);
    assert_eq!(server.requests(), 3);
    // Every request is recorded before it is sent and resolved after it answered.
    assert_eq!(
        journal.calls(),
        [
            "begin Orders",
            "record 1 POST InFlight -",
            "record 1 POST Completed 200",
            "record 2 GET InFlight -",
            "record 2 GET Completed 200",
            "cleanup",
            "record 3 DELETE InFlight -",
            "record 3 DELETE Completed 200",
            "finish",
        ]
    );
    // The report is the record of a finished run, so nothing stays behind.
    assert!(journal.snapshot().is_empty());
    assert!(report.error.is_none());
}

#[tokio::test]
async fn a_process_that_dies_mid_request_leaves_the_request_in_flight() {
    let server = Server::start(Behavior::Stall).await;
    let (project, scenario) = project(&server.base_url);
    let journal = Recorder::shared();
    // Abandoning the future is what a crash does to a run: nothing resolves the
    // entry, nothing finishes the run and cleanup never starts.
    let abandoned = tokio::time::timeout(
        Duration::from_millis(300),
        run_scenario(
            RunRequest::new(project, scenario, RunMode::Live),
            options(&journal),
            CancellationToken::new(),
            |_| {},
        ),
    )
    .await;
    assert!(abandoned.is_err(), "the stalled run cannot finish");

    let run = journal.only_run();
    assert_eq!(run.scenario_name, "Orders");
    assert_eq!(run.cleanup, CleanupState::NotStarted);
    assert_eq!(run.entries.len(), 2);
    assert_eq!(run.entries[0].state, EffectState::Completed);
    assert_eq!(run.entries[0].method, "POST");
    assert!(
        run.entries[0].url.ends_with("/orders"),
        "{}",
        run.entries[0].url
    );
    assert_eq!(run.entries[1].state, EffectState::InFlight);
    assert_eq!(run.entries[1].phase, Phase::Main);
    // Both may have changed the tested API: one certainly ran, one is unknown.
    assert_eq!(run.effects().count(), 2);
}

#[tokio::test]
async fn cancelling_a_request_in_flight_records_and_reports_the_uncertainty() {
    let server = Server::start(Behavior::Stall).await;
    let (project, scenario) = project(&server.base_url);
    let journal = Recorder::shared();
    let cancel = CancellationToken::new();
    let token = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        token.cancel();
    });
    let report = run_scenario(
        RunRequest::new(project, scenario, RunMode::Live),
        options(&journal),
        cancel,
        |_| {},
    )
    .await
    .unwrap();

    assert_eq!(report.status, RunStatus::Cancelled);
    let read = report.step("read").unwrap();
    assert_eq!(read.status, StepStatus::Cancelled);
    let message = read.message.clone().unwrap_or_default();
    assert!(
        message.contains("being sent") && message.contains("may have performed"),
        "{message}"
    );
    // Cleanup runs after cancellation, so the run still reaches its end and the
    // journal keeps nothing; the report carries the uncertainty from here on.
    assert!(journal.snapshot().is_empty());
    assert!(
        journal
            .calls()
            .contains(&"record 2 GET Uncertain -".to_owned()),
        "{:?}",
        journal.calls()
    );
}

#[tokio::test]
async fn a_request_that_never_left_the_process_is_not_an_effect() {
    // A closed loopback port: the connection fails before anything is sent.
    let closed = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = closed.local_addr().unwrap();
    drop(closed);
    let (project, scenario) = project(&format!("http://{address}"));
    let journal = Recorder::shared();
    let report = run_scenario(
        RunRequest::new(project, scenario, RunMode::Live),
        options(&journal),
        CancellationToken::new(),
        |_| {},
    )
    .await
    .unwrap();

    assert_eq!(report.status, RunStatus::Failed);
    let message = report.step("create").unwrap().message.clone().unwrap();
    assert!(
        message.contains("the connection failed") && message.contains("Nothing was sent"),
        "{message}"
    );
    assert!(
        journal
            .calls()
            .contains(&"record 1 POST NotSent -".to_owned()),
        "{:?}",
        journal.calls()
    );
}

#[tokio::test]
async fn a_journal_that_cannot_be_written_stops_the_run_before_any_request() {
    let server = Server::start(Behavior::Answer).await;
    let (project, scenario) = project(&server.base_url);
    let journal = Recorder::broken();
    let report = run_scenario(
        RunRequest::new(project, scenario, RunMode::Live),
        options(&journal),
        CancellationToken::new(),
        |_| {},
    )
    .await
    .unwrap();

    assert_eq!(report.status, RunStatus::Failed);
    assert!(report.steps.is_empty(), "nothing is executed");
    let error = report.error.clone().unwrap();
    assert!(
        error.contains("journal") && error.contains("no space left"),
        "{error}"
    );
    assert_eq!(
        server.requests(),
        0,
        "an effect that cannot be recorded is not performed"
    );
}

#[tokio::test]
async fn a_simulation_is_never_journalled() {
    let (project, scenario) = project("http://127.0.0.1:1");
    let journal = Recorder::shared();
    let report = run_scenario(
        RunRequest::new(project, scenario, RunMode::Simulation),
        options(&journal),
        CancellationToken::new(),
        |_| {},
    )
    .await
    .unwrap();

    assert_eq!(report.status, RunStatus::Passed);
    assert!(journal.calls().is_empty(), "{:?}", journal.calls());
}

#[tokio::test]
async fn the_journal_names_the_run_the_report_names() {
    let server = Server::start(Behavior::Stall).await;
    let (project, scenario) = project(&server.base_url);
    let journal = Recorder::shared();
    let identity = Arc::new(Mutex::new(None));
    let seen = Arc::clone(&identity);
    let abandoned = tokio::time::timeout(
        Duration::from_millis(300),
        run_scenario(
            RunRequest::new(project, scenario.clone(), RunMode::Live),
            options(&journal),
            CancellationToken::new(),
            move |report| {
                *seen.lock().unwrap() = Some((report.id.clone(), report.started_at.clone()));
            },
        ),
    )
    .await;
    assert!(abandoned.is_err());

    let (id, started_at) = identity
        .lock()
        .unwrap()
        .clone()
        .expect("a snapshot arrived");
    let run = journal.only_run();
    assert_eq!(
        run.run_id, id,
        "the journal and the report share an identifier"
    );
    assert_eq!(run.started_at, started_at);
    assert_eq!(run.scenario_id, scenario);
}

/// A graph with no requests still journals the run, so an interruption during an
/// assertion or a value is not reported as a run that never started.
#[tokio::test]
async fn a_run_without_requests_is_still_opened_and_closed() {
    let (mut project, scenario) = project("http://127.0.0.1:1");
    let target = &mut project.scenarios[0];
    target.graph = Graph {
        nodes: vec![],
        edges: vec![],
    };
    target.cleanup = Graph {
        nodes: vec![],
        edges: vec![],
    };
    let journal = Recorder::shared();
    let report = run_scenario(
        RunRequest::new(project, scenario, RunMode::Live),
        options(&journal),
        CancellationToken::new(),
        |_| {},
    )
    .await
    .unwrap();

    assert_eq!(report.status, RunStatus::Passed);
    assert_eq!(journal.calls(), ["begin Orders", "cleanup", "finish"]);
    assert!(journal.snapshot().is_empty());
}
