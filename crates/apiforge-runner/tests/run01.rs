//! RUN-01 (#126): under `on_error continue` the chain goes on after a step
//! that failed; only a value that does not exist holds a step back.
//!
//! A step waits until the one before it has finished, whatever its result.
//! A step that reads an output which does not exist is *blocked* and names
//! the statement that should have made it; an `if` that cannot decide blocks
//! what stands in its braces. In the cleanup a step skipped for a missing
//! value no longer takes the steps after it along (ADR 0015).
//!
//! Every scenario here runs twice, simulated and live against a loopback
//! server that answers what the samples say, and is judged the same way both
//! times. A request "without an answer" goes to a port nothing listens on;
//! the simulation has no sample for it. The texts are lowered in the demo
//! shop project.

use apiforge_core::{
    demos::shop_project,
    format::{Project, Scenario},
    run::{NotRunCause, Phase, RunMode, RunResult, RunStatus, StepResult, StepStatus},
};
use apiforge_runner::{HttpTransport, RunOptions, RunRequest, run_scenario};
use apiforge_script::{Document, LowerContext, lower, parse};
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::{JoinHandle, JoinSet},
    time::timeout,
};
use tokio_util::sync::CancellationToken;

// Lowering ---------------------------------------------------------------------

fn lowered(project: &Project, text: &str) -> Document {
    let result = lower(&parse(text), &LowerContext::folder(project));
    assert!(!result.has_errors(), "{:#?}", result.diagnostics);
    result.document.expect("a document")
}

/// The shop project with `components` published at revision 1, and the
/// scenario `text` lowered in it.
fn scenario(components: &[&str], text: &str) -> (Project, Scenario) {
    let mut project = shop_project();
    for component in components {
        let Document::Component(mut component) = lowered(&project, component) else {
            panic!("not a component");
        };
        component.revision = 1;
        project.components.push(component);
    }
    let Document::Scenario(scenario) = lowered(&project, text) else {
        panic!("not a scenario");
    };
    project.scenarios.push(scenario.clone());
    (project, scenario)
}

/// An address nothing listens on, so a request to it gets no answer.
fn refused() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a free port");
    format!("http://{}", listener.local_addr().expect("an address"))
}

// The loopback server -------------------------------------------------------------

/// What the server answers: method, path, status and JSON body. Anything else
/// is a 404.
type Route = (&'static str, &'static str, u16, Value);

struct Server {
    base_url: String,
    received: Arc<Mutex<Vec<String>>>,
    task: JoinHandle<()>,
}

impl Server {
    async fn start(routes: Vec<Route>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
        let base_url = format!("http://{}", listener.local_addr().expect("an address"));
        let received = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&received);
        let routes = Arc::new(routes);
        let task = tokio::spawn(async move {
            let mut handlers = JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let (stream, _) = accepted.expect("a connection");
                        let log = Arc::clone(&log);
                        let routes = Arc::clone(&routes);
                        handlers.spawn(async move { serve(stream, &routes, &log).await });
                    }
                    Some(result) = handlers.join_next(), if !handlers.is_empty() => {
                        result.expect("the handler finishes");
                    }
                }
            }
        });
        Self {
            base_url,
            received,
            task,
        }
    }

    /// `METHOD /path` of every request received, in order.
    fn requests(&self) -> Vec<String> {
        self.received.lock().expect("the log").clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn serve(mut stream: TcpStream, routes: &[Route], log: &Mutex<Vec<String>>) {
    let mut bytes = Vec::new();
    let header_end = loop {
        if let Some(index) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            break index + 4;
        }
        let mut chunk = [0; 1024];
        let count = timeout(Duration::from_secs(3), stream.read(&mut chunk))
            .await
            .expect("the request arrives")
            .expect("readable");
        assert_ne!(count, 0, "request closed before its headers");
        bytes.extend_from_slice(&chunk[..count]);
    };
    let head = String::from_utf8_lossy(&bytes[..header_end]).into_owned();
    let mut first = head.lines().next().unwrap_or_default().split_whitespace();
    let (method, path) = (
        first.next().unwrap_or_default().to_owned(),
        first.next().unwrap_or_default().to_owned(),
    );
    let length = head
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .map_or(0, |(_, length)| length.trim().parse::<usize>().unwrap_or(0));
    while bytes.len() < header_end + length {
        let mut chunk = [0; 1024];
        let count = stream.read(&mut chunk).await.expect("readable");
        assert_ne!(count, 0, "request closed before its body");
        bytes.extend_from_slice(&chunk[..count]);
    }
    log.lock()
        .expect("the log")
        .push(format!("{method} {path}"));
    let (status, body) = routes
        .iter()
        .find(|(m, p, _, _)| *m == method && *p == path)
        .map_or((404, json!({ "error": "unexpected request" })), |route| {
            (route.2, route.3.clone())
        });
    let body = body.to_string();
    let response = format!(
        "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .await
        .expect("writable");
    stream.shutdown().await.expect("closed");
}

// Running ------------------------------------------------------------------------

async fn execute(project: Project, scenario: &Scenario, mode: RunMode) -> RunResult {
    timeout(
        Duration::from_secs(20),
        run_scenario(
            RunRequest::new(project, scenario.id.clone(), mode),
            RunOptions::new(HttpTransport::new().expect("client builds")),
            CancellationToken::new(),
            |_| {},
        ),
    )
    .await
    .expect("the run ends")
    .expect("the run starts")
}

/// The scenario simulated, then live against a server with `routes`. Returns
/// both reports and what the server received.
async fn both(
    components: &[&str],
    text: &str,
    routes: Vec<Route>,
) -> ([(RunMode, RunResult); 2], Vec<String>) {
    let (project, scenario) = scenario(components, text);
    let simulated = execute(project.clone(), &scenario, RunMode::Simulation).await;
    let server = Server::start(routes).await;
    let mut project = project;
    project.environment.base_url.clone_from(&server.base_url);
    let live = execute(project, &scenario, RunMode::Live).await;
    (
        [(RunMode::Simulation, simulated), (RunMode::Live, live)],
        server.requests(),
    )
}

fn step<'a>(steps: &'a [StepResult], handle: &str) -> &'a StepResult {
    steps
        .iter()
        .find(|step| step.handle == handle)
        .unwrap_or_else(|| panic!("no result for `{handle}`: {steps:#?}"))
}

fn status(run: &RunResult, handle: &str) -> StepStatus {
    step(&run.steps, handle).status
}

/// `handle` ran and ended with `expected`.
fn assert_ran(mode: RunMode, run: &RunResult, handle: &str, expected: StepStatus) {
    let found = step(&run.steps, handle);
    assert_eq!(found.status, expected, "{mode:?} `{handle}`: {found:#?}");
    assert_eq!(found.cause, None, "{mode:?} `{handle}`");
}

/// `handle` is blocked and names `by`, the statement whose value is missing
/// (criterion 8).
fn assert_blocked_by(mode: RunMode, run: &RunResult, handle: &str, by: &str) {
    let found = step(&run.steps, handle);
    assert_eq!(
        found.status,
        StepStatus::Blocked,
        "{mode:?} `{handle}`: {found:#?}"
    );
    assert_eq!(
        found.cause,
        Some(NotRunCause::BlockedBy {
            step: step(&run.steps, by).node_id.clone()
        }),
        "{mode:?} `{handle}` names `{by}`"
    );
}

/// The run is failed (criterion 8).
fn assert_failed(mode: RunMode, run: &RunResult) {
    assert_eq!(run.status, RunStatus::Failed, "{mode:?}: {run:#?}");
}

// Criterion 1 ------------------------------------------------------------------

const CRITERION_1: &str = r#"scenario "RUN-01 criterion 1" {
  on_error continue
  begin = start
  @after(begin)
  a = GET "{down}/a"
  @after(a)
  b = GET "/b/{{a.body.id}}" {
    sample 200 { ok: true }
  }
  @after(b)
  c = GET "/c" {
    sample 200 { ok: true }
  }
  seen = assert c.status == 200
}
"#;

/// Criterion 1: *blocked* travels through values, not along the chain. `b`
/// reads `a`, which errored without an answer, and is blocked by it; `c`
/// only follows `b` and runs.
#[tokio::test]
async fn run01_criterion_1_blocked_travels_through_values_only() {
    let text = CRITERION_1.replace("{down}", &refused());
    let (runs, received) = both(&[], &text, vec![("GET", "/c", 200, json!({ "ok": true }))]).await;
    for (mode, run) in &runs {
        let mode = *mode;
        assert_failed(mode, run);
        assert_ran(mode, run, "a", StepStatus::Error);
        assert_blocked_by(mode, run, "b", "a");
        assert_ran(mode, run, "c", StepStatus::Passed);
        assert_ran(mode, run, "seen", StepStatus::Passed);
    }
    assert_eq!(received, ["GET /c"]);
}

// Criterion 2 ------------------------------------------------------------------

const CRITERION_2: &str = r#"scenario "RUN-01 criterion 2" {
  on_error continue
  begin = start
  @after(begin)
  a = GET "/a" {
    sample 500 { error: "boom", requestId: "r-1" }
  }
  @after(a)
  b = GET "/orders/{{a.body.id}}" {
    sample 200 { ok: true }
  }
  @after(b)
  trace = GET "/trace/{{a.body.requestId}}" {
    sample 200 { ok: true }
  }
  @after(trace)
  c = GET "/c" {
    sample 200 { ok: true }
  }
}
"#;

/// Criterion 2: an answer has its outputs at any status. At 500, a step that
/// reads a field the body lacks errors with *Missing value*; one that reads a
/// field the body has runs; the step after either runs.
#[tokio::test]
async fn run01_criterion_2_an_answer_has_its_outputs_at_any_status() {
    let (runs, received) = both(
        &[],
        CRITERION_2,
        vec![
            (
                "GET",
                "/a",
                500,
                json!({ "error": "boom", "requestId": "r-1" }),
            ),
            ("GET", "/trace/r-1", 200, json!({ "ok": true })),
            ("GET", "/c", 200, json!({ "ok": true })),
        ],
    )
    .await;
    for (mode, run) in &runs {
        let mode = *mode;
        assert_failed(mode, run);
        assert_ran(mode, run, "a", StepStatus::Passed);
        assert_eq!(step(&run.steps, "a").outputs["status"], json!(500));
        assert_ran(mode, run, "b", StepStatus::Error);
        assert_eq!(
            step(&run.steps, "b").message.as_deref(),
            Some("Missing value for template value 1."),
            "{mode:?}"
        );
        assert_ran(mode, run, "trace", StepStatus::Passed);
        assert_ran(mode, run, "c", StepStatus::Passed);
    }
    assert_eq!(received, ["GET /a", "GET /trace/r-1", "GET /c"]);
}

// Criterion 3 ------------------------------------------------------------------

const CRITERION_3: &str = r#"scenario "RUN-01 criterion 3" {
  on_error continue
  begin = start
  @after(begin)
  a = GET "{down}/a"
  a_answered = assert a.status == 200
  @after(a)
  b = GET "/b" {
    sample 200 { count: 1 }
  }
  b_answered = assert b.status == 200
  b_counted = assert b.body.count == 2
}
"#;

/// Criterion 3: a check that reads a value which does not exist is blocked;
/// the checks of a later step that ran pass or fail on their own.
#[tokio::test]
async fn run01_criterion_3_checks_follow_their_values() {
    let text = CRITERION_3.replace("{down}", &refused());
    let (runs, received) = both(&[], &text, vec![("GET", "/b", 200, json!({ "count": 1 }))]).await;
    for (mode, run) in &runs {
        let mode = *mode;
        assert_failed(mode, run);
        assert_ran(mode, run, "a", StepStatus::Error);
        assert_blocked_by(mode, run, "a_answered", "a");
        assert_ran(mode, run, "b", StepStatus::Passed);
        assert_ran(mode, run, "b_answered", StepStatus::Passed);
        assert_ran(mode, run, "b_counted", StepStatus::Failed);
    }
    assert_eq!(received, ["GET /b"]);
}

// Criterion 4 ------------------------------------------------------------------

const CRITERION_4_ERROR: &str = r#"scenario "RUN-01 criterion 4, an error in a branch" {
  on_error continue
  begin = start
  @after(begin)
  probe = GET "/probe" {
    sample 200 { go: true }
  }
  decide = if probe.body.go == true {
    first = GET "{down}/first"
    @after(first)
    second = GET "/second" {
      sample 200 { id: 2 }
    }
    @after(second)
    reads_first = GET "/reads/{{first.body.id}}" {
      sample 200 { ok: true }
    }
  } else {
    other = GET "/other" {
      sample 200 { ok: true }
    }
  }
  @after(decide)
  after_if = GET "/after" {
    sample 200 { ok: true }
  }
  later = if after_if.status == 200 {
    inner = GET "/inner" {
      sample 200 { ok: true }
    }
  }
  @after(later)
  last = GET "/last" {
    sample 200 { ok: true }
  }
}
"#;

/// Criterion 4: after an error inside a branch, the next step of the branch,
/// the step after the `if`, a second `if` and the last step each run; only
/// the step that reads the failed one is blocked.
#[tokio::test]
async fn run01_criterion_4_an_error_inside_a_branch() {
    let text = CRITERION_4_ERROR.replace("{down}", &refused());
    let (runs, received) = both(
        &[],
        &text,
        vec![
            ("GET", "/probe", 200, json!({ "go": true })),
            ("GET", "/second", 200, json!({ "id": 2 })),
            ("GET", "/after", 200, json!({ "ok": true })),
            ("GET", "/inner", 200, json!({ "ok": true })),
            ("GET", "/last", 200, json!({ "ok": true })),
        ],
    )
    .await;
    for (mode, run) in &runs {
        let mode = *mode;
        assert_failed(mode, run);
        assert_ran(mode, run, "decide", StepStatus::Passed);
        assert_ran(mode, run, "first", StepStatus::Error);
        assert_ran(mode, run, "second", StepStatus::Passed);
        assert_blocked_by(mode, run, "reads_first", "first");
        assert_eq!(status(run, "other"), StepStatus::Skipped, "{mode:?}");
        assert_eq!(
            step(&run.steps, "other").cause,
            Some(NotRunCause::OutsidePath)
        );
        assert_ran(mode, run, "after_if", StepStatus::Passed);
        assert_ran(mode, run, "later", StepStatus::Passed);
        assert_ran(mode, run, "inner", StepStatus::Passed);
        assert_ran(mode, run, "last", StepStatus::Passed);
    }
    assert_eq!(
        received,
        [
            "GET /probe",
            "GET /second",
            "GET /after",
            "GET /inner",
            "GET /last"
        ]
    );
}

const CRITERION_4_UNDECIDED: &str = r#"scenario "RUN-01 criterion 4, an if that cannot decide" {
  on_error continue
  begin = start
  @after(begin)
  probe = GET "/probe" {
    sample 200 { go: true }
  }
  @after(probe)
  a = GET "{down}/a"
  decide = if a.body.go == true {
    yes = GET "/yes" {
      sample 200 { ok: true }
    }
    @after(yes)
    nested = if probe.status == 200 {
      deep = GET "/deep" {
        sample 200 { ok: true }
      }
      deep_ok = assert probe.status == 200
    }
  } else {
    noted = assert probe.status == 200
  }
  @after(decide)
  after_if = GET "/after" {
    sample 200 { ok: true }
  }
  undecided = if a.status == 200 {
    only = GET "/only" {
      sample 200 { ok: true }
    }
  }
  @after(undecided)
  last = GET "/last" {
    sample 200 { ok: true }
  }
}
"#;

/// Criterion 4: an `if` that cannot decide is blocked; every statement in
/// both of its braces, at any depth, is blocked by it, also one that reads
/// only values that exist; the step after it runs, also when a branch holds
/// no step (`else` of `decide`, the missing `else` of `undecided`).
#[tokio::test]
async fn run01_criterion_4_an_if_that_cannot_decide() {
    let text = CRITERION_4_UNDECIDED.replace("{down}", &refused());
    let (runs, received) = both(
        &[],
        &text,
        vec![
            ("GET", "/probe", 200, json!({ "go": true })),
            ("GET", "/after", 200, json!({ "ok": true })),
            ("GET", "/last", 200, json!({ "ok": true })),
        ],
    )
    .await;
    for (mode, run) in &runs {
        let mode = *mode;
        assert_failed(mode, run);
        assert_ran(mode, run, "a", StepStatus::Error);
        assert_blocked_by(mode, run, "decide", "a");
        for handle in ["yes", "nested", "deep", "deep_ok", "noted"] {
            assert_blocked_by(mode, run, handle, "decide");
        }
        assert_ran(mode, run, "after_if", StepStatus::Passed);
        assert_blocked_by(mode, run, "undecided", "a");
        assert_blocked_by(mode, run, "only", "undecided");
        assert_ran(mode, run, "last", StepStatus::Passed);
    }
    assert_eq!(received, ["GET /probe", "GET /after", "GET /last"]);
}

// Criterion 5 ------------------------------------------------------------------

const MAKER: &str = r#"component run01_maker "RUN-01 maker" {
  made = POST "/made" {
    sample 201 { id: "m-1" }
  }
  export id: string = made.body.id
  @after(made)
  broken = GET "{down}/broken"
  export detail: string = broken.body.detail
}
"#;

const CRITERION_5: &str = r#"scenario "RUN-01 criterion 5" {
  on_error continue
  begin = start
  @after(begin)
  maker = run01_maker@1()
  uses_id = GET "/use/{{maker.id}}" {
    sample 200 { ok: true }
  }
  uses_detail = GET "/detail/{{maker.detail}}" {
    sample 200 { ok: true }
  }
}
"#;

/// Criterion 5: a component call that fails after computing an export: a
/// step reading that export runs, a step reading another export is blocked
/// by the call.
#[tokio::test]
async fn run01_criterion_5_exports_computed_before_the_failure() {
    let maker = MAKER.replace("{down}", &refused());
    let (runs, received) = both(
        &[&maker],
        CRITERION_5,
        vec![
            ("POST", "/made", 201, json!({ "id": "m-1" })),
            ("GET", "/use/m-1", 200, json!({ "ok": true })),
        ],
    )
    .await;
    for (mode, run) in &runs {
        let mode = *mode;
        assert_failed(mode, run);
        assert_ran(mode, run, "maker", StepStatus::Error);
        assert_eq!(
            step(&run.steps, "maker").outputs.get("id"),
            Some(&json!("m-1"))
        );
        assert_eq!(step(&run.steps, "maker").outputs.get("detail"), None);
        assert_ran(mode, run, "uses_id", StepStatus::Passed);
        assert_blocked_by(mode, run, "uses_detail", "maker");
    }
    assert_eq!(received, ["POST /made", "GET /use/m-1"]);
}

// Criterion 6 ------------------------------------------------------------------

const CRITERION_6: &str = r#"scenario "RUN-01 criterion 6" {
  on_error continue
  begin = start
  @after(begin)
  a = GET "{down}/a"
  cleanup {
    tidy = DELETE "/tidy" {
      sample 200 { ok: true }
    }
    @after(tidy)
    tidy2 = DELETE "/tidy/{{a.body.id}}" {
      sample 200 { ok: true }
    }
    @after(tidy2)
    tidy3 = DELETE "/tidy3" {
      sample 200 { ok: true }
    }
  }
}
"#;

/// Criterion 6: in a cleanup of three chained steps where the second lacks
/// its value, the first and third run and the second is skipped, naming the
/// statement whose value is missing.
#[tokio::test]
async fn run01_criterion_6_cleanup_goes_on_after_a_skipped_step() {
    let text = CRITERION_6.replace("{down}", &refused());
    let (runs, received) = both(
        &[],
        &text,
        vec![
            ("DELETE", "/tidy", 200, json!({ "ok": true })),
            ("DELETE", "/tidy3", 200, json!({ "ok": true })),
        ],
    )
    .await;
    for (mode, run) in &runs {
        let mode = *mode;
        assert_failed(mode, run);
        assert_ran(mode, run, "a", StepStatus::Error);
        for handle in ["tidy", "tidy2", "tidy3"] {
            assert_eq!(step(&run.steps, handle).phase, Phase::Cleanup, "{handle}");
        }
        assert_ran(mode, run, "tidy", StepStatus::Passed);
        // A cleanup step that does not run is skipped, not blocked.
        let tidy2 = step(&run.steps, "tidy2");
        assert_eq!(tidy2.status, StepStatus::Skipped, "{mode:?}: {tidy2:#?}");
        assert_eq!(
            tidy2.cause,
            Some(NotRunCause::BlockedBy {
                step: step(&run.steps, "a").node_id.clone()
            }),
            "{mode:?}"
        );
        assert_ran(mode, run, "tidy3", StepStatus::Passed);
    }
    assert_eq!(received, ["DELETE /tidy", "DELETE /tidy3"]);
}

const CLEANUP_AFTER_A_BRANCH_NOT_TAKEN: &str = r#"scenario "RUN-01 cleanup after a branch not taken" {
  on_error continue
  begin = start
  @after(begin)
  probe = GET "/probe" {
    sample 200 { go: false }
  }
  decide = if probe.body.go == true {
    made = POST "/made" {
      sample 201 { id: "m-1" }
    }
  }
  cleanup {
    tidy = DELETE "/made/{{made.body.id}}" {
      sample 200 { ok: true }
    }
    @after(tidy)
    tidy2 = DELETE "/tidy2" {
      sample 200 { ok: true }
    }
  }
}
"#;

/// Criterion 6 and rule 8 of ADR 0015: a first cleanup step that reads a
/// step on a branch not taken is skipped because its value does not exist,
/// not as outside the path, and the cleanup step chained after it runs.
#[tokio::test]
async fn run01_criterion_6_cleanup_reading_a_branch_not_taken_goes_on() {
    let (runs, received) = both(
        &[],
        CLEANUP_AFTER_A_BRANCH_NOT_TAKEN,
        vec![
            ("GET", "/probe", 200, json!({ "go": false })),
            ("DELETE", "/tidy2", 200, json!({ "ok": true })),
        ],
    )
    .await;
    for (mode, run) in &runs {
        let mode = *mode;
        let made = step(&run.steps, "made");
        assert_eq!(made.cause, Some(NotRunCause::OutsidePath), "{mode:?}");
        let tidy = step(&run.steps, "tidy");
        assert_eq!(tidy.phase, Phase::Cleanup);
        assert_eq!(tidy.status, StepStatus::Skipped, "{mode:?}: {tidy:#?}");
        assert_eq!(
            tidy.message.as_deref(),
            Some("Unavailable data or an unmet condition of an earlier step."),
            "{mode:?}"
        );
        assert_eq!(
            tidy.cause,
            Some(NotRunCause::BlockedBy {
                step: made.node_id.clone()
            }),
            "{mode:?}"
        );
        assert_ran(mode, run, "tidy2", StepStatus::Passed);
    }
    assert_eq!(received, ["GET /probe", "DELETE /tidy2"]);
}
