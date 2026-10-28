//! MCP-01 (#60), criteria 1, 2 and 8 over real stdio: the `apiforge mcp`
//! binary driven with both protocol revisions, a malformed line, an unknown
//! method, a tool error and EOF. Criterion 6's "byte for byte what
//! `apiforge print` prints" is checked against the command here as well.
//!
//! MCP-03 (#62) over real stdio: `check_text`, `run`, `language_reference` and
//! the resources; a run cancelled with `notifications/cancelled` while reads
//! are answered and a second run is refused; the deadline; EOF during a run.
//! A slow simulation comes from the server's test seam
//! `APIFORGE_MCP_TEST_STEP_DELAY_MS`, a short deadline from
//! `APIFORGE_MCP_TEST_DEADLINE_MS`.
//!
//! MCP-04 (#63) over real stdio: `write_source`, `create_document` and
//! `finish_editing`; the agent session holding the writer lock until
//! `finish_editing` or EOF, with the client's name in the holder file; a
//! write based on a version `apiforge apply` replaced; a secret from
//! `APIFORGE_SECRET_<NAME>` in the text.
//!
//! MCP-06 (#65) over real stdio: `--allow-live` and `--secrets`; EOF during
//! a live run, with its cleanup finishing and blocked
//! (`APIFORGE_MCP_TEST_CLEANUP_DELAY_MS`); a server killed mid-run and the
//! next server reporting the journal's entry; a whole session grepped for
//! two secret values. Live requests go to the demo API in this process or a
//! loopback listener that never answers.
//!
//! MCP-05 (#64) over real stdio: `publish` in the tool list with its
//! annotations; the comment-loss refusal and the consent; a repeated
//! `request_id` answered from the record; a window holding the project.
use apiforge_project::{ProjectSession, prepare_project};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, ChildStdin, Command, ExitStatus, Stdio},
    sync::mpsc::{Receiver, channel},
    thread,
    time::{Duration, Instant},
};

const PRODUCT: &str = "components/product.afs";
const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const TOOLS: [&str; 11] = [
    "project_overview",
    "language_reference",
    "read_source",
    "check",
    "check_text",
    "print",
    "run",
    "write_source",
    "create_document",
    "publish",
    "finish_editing",
];
/// The tools that change the project or its writer lock (MCP-04, MCP-05),
/// or may send live requests (`run`, MCP-06), with their `destructiveHint`
/// and `idempotentHint`.
const WRITE_TOOLS: [(&str, bool, bool); 5] = [
    ("run", true, false),
    ("write_source", true, true),
    ("create_document", false, false),
    ("publish", false, false),
    ("finish_editing", false, true),
];

fn demo() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &apiforge_core::demos::shop_project()).unwrap();
    std::fs::remove_dir_all(directory.path().join(".apiforge/local")).unwrap();
    directory
}

struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    /// Every line written to stdout, in order.
    stdout: Vec<String>,
    stderr: thread::JoinHandle<String>,
}

impl Server {
    fn spawn(folder: Option<&Path>, current_dir: &Path) -> Self {
        Self::spawn_with(folder, current_dir, &[])
    }

    fn spawn_with(folder: Option<&Path>, current_dir: &Path, envs: &[(&str, &str)]) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_apiforge"));
        command.arg("mcp").envs(envs.iter().copied());
        if let Some(folder) = folder {
            command.arg("--folder").arg(folder);
        }
        Self::spawn_command(command, current_dir)
    }

    fn spawn_command(mut command: Command, current_dir: &Path) -> Self {
        let mut child = command
            .current_dir(current_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, lines) = channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        let mut stderr = child.stderr.take().unwrap();
        let stderr = thread::spawn(move || {
            let mut text = String::new();
            let _ = stderr.read_to_string(&mut text);
            text
        });
        Self {
            stdin: child.stdin.take(),
            child,
            lines,
            stdout: Vec::new(),
            stderr,
        }
    }

    fn send(&mut self, message: &str) {
        let stdin = self.stdin.as_mut().unwrap();
        stdin.write_all(message.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin.flush().unwrap();
    }

    fn request(&mut self, id: u64, method: &str, params: Value) -> Value {
        self.send(
            &json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string(),
        );
        self.response(|message| message["id"] == json!(id))
    }

    /// The next stdout message that matches, every line read on the way kept.
    fn response(&mut self, matches: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let line = self
                .lines
                .recv_timeout(left)
                .unwrap_or_else(|_| panic!("no answer; stdout so far: {:#?}", self.stdout));
            self.stdout.push(line.clone());
            let message: Value = serde_json::from_str(&line)
                .unwrap_or_else(|_| panic!("stdout carried a non-JSON line: {line:?}"));
            if matches(&message) {
                return message;
            }
        }
    }

    /// Closes stdin and waits for the process to end.
    fn close(mut self) -> (ExitStatus, Duration, Vec<String>, String) {
        drop(self.stdin.take());
        let closed = Instant::now();
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            if closed.elapsed() > Duration::from_secs(10) {
                let _ = self.child.kill();
                panic!("the server did not end after EOF");
            }
            thread::sleep(Duration::from_millis(10));
        };
        let elapsed = closed.elapsed();
        while let Ok(line) = self.lines.recv_timeout(Duration::from_secs(2)) {
            self.stdout.push(line);
        }
        let stderr = self.stderr.join().unwrap();
        (status, elapsed, self.stdout, stderr)
    }
}

fn meta() -> Value {
    json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientInfo": { "name": "acceptance", "version": "1.0.0" },
        "io.modelcontextprotocol/clientCapabilities": {}
    })
}

fn server_info(result: &Value) -> &Value {
    if result["serverInfo"].is_object() {
        &result["serverInfo"]
    } else {
        &result["_meta"]["io.modelcontextprotocol/serverInfo"]
    }
}

fn assert_tools(list: &Value) {
    let tools = list["result"]["tools"].as_array().unwrap();
    let names: Vec<_> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, TOOLS);
    for tool in tools {
        assert!(!tool["description"].as_str().unwrap().is_empty(), "{tool}");
        assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
        assert_eq!(tool["outputSchema"]["type"], "object", "{tool}");
        let name = tool["name"].as_str().unwrap();
        match WRITE_TOOLS.iter().find(|(write, _, _)| *write == name) {
            None => assert_eq!(tool["annotations"]["readOnlyHint"], true, "{tool}"),
            Some((_, destructive, idempotent)) => {
                assert_eq!(tool["annotations"]["readOnlyHint"], false, "{tool}");
                assert_eq!(
                    tool["annotations"]["destructiveHint"], *destructive,
                    "{tool}"
                );
                assert_eq!(tool["annotations"]["idempotentHint"], *idempotent, "{tool}");
            }
        }
    }
}

/// A successful tool result: `structuredContent` plus its readable text.
fn structured(response: &Value) -> &Value {
    let result = &response["result"];
    assert_ne!(result["isError"], true, "{response}");
    let text = result["content"][0]["text"].as_str().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(text).unwrap(),
        result["structuredContent"]
    );
    &result["structuredContent"]
}

fn tool_error(response: &Value, code: &str) {
    let result = &response["result"];
    assert_eq!(result["isError"], true, "{response}");
    let error = &result["structuredContent"];
    assert_eq!(error["code"], code, "{response}");
    assert_eq!(error["wrote"], false);
    assert!(error["message"].is_string());
    assert!(error["details"].is_object());
}

fn assert_clean_end(server: Server) {
    let (status, elapsed, stdout, stderr) = server.close();
    assert_eq!(status.code(), Some(0), "stderr: {stderr}");
    assert!(elapsed < Duration::from_secs(2), "EOF took {elapsed:?}");
    for line in stdout {
        let message: Value = serde_json::from_str(&line)
            .unwrap_or_else(|_| panic!("stdout carried a non-protocol line: {line:?}"));
        assert_eq!(message["jsonrpc"], "2.0", "{line}");
    }
}

#[test]
fn criteria_1_and_2_a_discover_session_over_stdio() {
    let directory = demo();
    let root = directory.path();
    let mut server = Server::spawn(Some(root), root);

    let discover = server.request(1, "server/discover", json!({ "_meta": meta() }));
    let result = &discover["result"];
    let versions = result["supportedVersions"].as_array().unwrap();
    assert!(versions.contains(&json!("2026-07-28")), "{discover}");
    assert!(versions.contains(&json!("2025-11-25")), "{discover}");
    assert!(result["capabilities"]["tools"].is_object(), "{discover}");
    assert!(result["capabilities"]["resources"].is_object(), "MCP-03");
    assert_eq!(server_info(result)["name"], "apiforge", "{discover}");
    assert_eq!(server_info(result)["version"], env!("CARGO_PKG_VERSION"));

    assert_tools(&server.request(2, "tools/list", json!({ "_meta": meta() })));

    let read = server.request(
        3,
        "tools/call",
        json!({ "_meta": meta(), "name": "read_source", "arguments": { "path": PRODUCT } }),
    );
    assert_eq!(
        structured(&read)["text"],
        std::fs::read_to_string(root.join(PRODUCT)).unwrap()
    );

    server.send("{ this is not JSON");
    server.send(&json!({ "jsonrpc": "2.0", "id": 4, "method": "no/such/method", "params": { "_meta": meta() } }).to_string());
    // MCP-03 carry-over 2: the error answers the request's id, never null.
    let unknown = server.response(|m| m["id"] == json!(4));
    assert!(unknown["error"].is_object(), "{unknown}");

    let rejected = server.request(
        5,
        "tools/call",
        json!({ "_meta": meta(), "name": "read_source", "arguments": { "path": "../x.afs" } }),
    );
    tool_error(&rejected, "path_rejected");

    // Still answering after the malformed line, the unknown method and the error.
    let check = server.request(
        6,
        "tools/call",
        json!({ "_meta": meta(), "name": "check", "arguments": {} }),
    );
    assert_eq!(structured(&check)["valid"], true);
    assert_clean_end(server);
    assert!(!root.join(".apiforge/local").exists());
}

#[test]
fn criteria_1_and_2_an_initialize_session_over_stdio() {
    let directory = demo();
    let root = directory.path();
    let mut server = Server::spawn(Some(root), root);

    let initialize = server.request(
        1,
        "initialize",
        json!({
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": { "name": "acceptance", "version": "1.0.0" }
        }),
    );
    let result = &initialize["result"];
    assert_eq!(result["protocolVersion"], "2025-11-25", "{initialize}");
    assert_eq!(result["serverInfo"]["name"], "apiforge");
    assert_eq!(result["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(result["capabilities"]["tools"].is_object());
    assert!(result["capabilities"]["resources"].is_object(), "MCP-03");
    server.send(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);

    assert_tools(&server.request(2, "tools/list", json!({})));
    let overview = server.request(
        3,
        "tools/call",
        json!({ "name": "project_overview", "arguments": {} }),
    );
    assert_eq!(structured(&overview)["name"], "Demo shop");

    server.send("not json at all");
    server.send(r#"{"jsonrpc":"2.0","id":4,"method":"no/such/method"}"#);
    // MCP-03 carry-over 2: the error answers the request's id, never null.
    let unknown = server.response(|m| m["id"] == json!(4));
    assert!(unknown["error"].is_object(), "{unknown}");

    let missing = server.request(
        5,
        "tools/call",
        json!({ "name": "print", "arguments": { "path": "components/missing.afs" } }),
    );
    tool_error(&missing, "unknown_document");

    let printed = server.request(
        6,
        "tools/call",
        json!({ "name": "print", "arguments": { "path": PRODUCT } }),
    );
    assert!(
        structured(&printed)["text"]
            .as_str()
            .unwrap()
            .starts_with("component product")
    );
    assert_clean_end(server);
}

#[test]
fn criterion_8_without_folder_the_server_uses_the_current_directory() {
    let directory = demo();
    let mut server = Server::spawn(None, directory.path());
    server.request(
        1,
        "initialize",
        json!({ "protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": { "name": "acceptance", "version": "1.0.0" } }),
    );
    server.send(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    let overview = server.request(
        2,
        "tools/call",
        json!({ "name": "project_overview", "arguments": {} }),
    );
    assert_eq!(structured(&overview)["name"], "Demo shop");
    assert_clean_end(server);

    let empty = tempfile::tempdir().unwrap();
    let mut server = Server::spawn(None, empty.path());
    server.request(
        1,
        "initialize",
        json!({ "protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": { "name": "acceptance", "version": "1.0.0" } }),
    );
    server.send(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    for (id, name, arguments) in [
        (2, "project_overview", json!({})),
        (3, "read_source", json!({ "path": PRODUCT })),
        (4, "check", json!({})),
        (5, "print", json!({ "path": PRODUCT })),
    ] {
        let response = server.request(
            id,
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        );
        tool_error(&response, "not_a_project");
    }
    assert_tools(&server.request(6, "tools/list", json!({})));
    assert_clean_end(server);
    assert_eq!(std::fs::read_dir(empty.path()).unwrap().count(), 0);
}

#[test]
fn criterion_6_print_matches_the_print_command_byte_for_byte() {
    let directory = demo();
    let root = directory.path();
    let session = ProjectSession::open_saved_read_only(root).unwrap();
    for document in session.documents() {
        let path = document.entry().path.as_str();
        let output = Command::new(env!("CARGO_BIN_EXE_apiforge"))
            .args(["print", "--folder"])
            .arg(root)
            .args(["--file", path])
            .output()
            .unwrap();
        assert!(output.status.success());
        let printed = apiforge_mcp::tools::print(root, path).unwrap();
        assert_eq!(printed.text.as_bytes(), output.stdout.as_slice(), "{path}");
    }
}

// MCP-03 (#62) ---------------------------------------------------------------

fn initialize(server: &mut Server) {
    server.request(
        1,
        "initialize",
        json!({ "protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": { "name": "acceptance", "version": "1.0.0" } }),
    );
    server.send(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
}

fn call(server: &mut Server, id: u64, name: &str, arguments: Value) -> Value {
    server.request(
        id,
        "tools/call",
        json!({ "name": name, "arguments": arguments }),
    )
}

fn send_call(server: &mut Server, id: u64, name: &str, arguments: Value) {
    server.send(
        &json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": name, "arguments": arguments } })
            .to_string(),
    );
}

fn answered(server: &Server, id: u64) -> bool {
    server.stdout.iter().any(|line| {
        serde_json::from_str::<Value>(line).is_ok_and(|message| message["id"] == json!(id))
    })
}

#[test]
fn mcp03_check_text_run_the_reference_and_resources_over_stdio() {
    let directory = demo();
    let root = directory.path();
    let mut server = Server::spawn(Some(root), root);
    initialize(&mut server);
    assert_tools(&server.request(2, "tools/list", json!({})));

    let product = std::fs::read_to_string(root.join(PRODUCT)).unwrap();
    let checked = call(
        &mut server,
        3,
        "check_text",
        json!({ "path": PRODUCT, "text": product.replacen("export product", "export ???", 1) }),
    );
    let checked = structured(&checked);
    assert_eq!(checked["registered"], true);
    assert_eq!(checked["valid"], false);
    assert_eq!(checked["diagnostics"][0]["severity"], "error");

    let report = call(&mut server, 4, "run", json!({ "path": SCENARIO }));
    let report = structured(&report);
    assert_eq!(report["status"], "passed");
    assert_eq!(report["ended_by"], "completed");
    assert_eq!(report["step_count"], 18);
    assert_eq!(report["redacted"], true);

    let live = call(
        &mut server,
        5,
        "run",
        json!({ "path": SCENARIO, "live": true }),
    );
    tool_error(&live, "live_disabled");

    let reference = call(&mut server, 6, "language_reference", json!({}));
    let reference = structured(&reference);
    assert_eq!(reference["version"], "1");
    let text = reference["text"].as_str().unwrap().to_owned();

    let listed = server.request(7, "resources/list", json!({}));
    let uris: Vec<_> = listed["result"]["resources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|resource| resource["uri"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(uris.len(), 5, "{listed}");
    assert_eq!(uris[0], "apiforge://reference");
    assert!(uris.contains(&format!("apiforge://source/{PRODUCT}")));

    let read = server.request(
        8,
        "resources/read",
        json!({ "uri": "apiforge://reference" }),
    );
    let contents = &read["result"]["contents"][0];
    assert_eq!(contents["mimeType"], "text/plain", "{read}");
    assert_eq!(contents["text"], text);
    let read = server.request(
        9,
        "resources/read",
        json!({ "uri": format!("apiforge://source/{PRODUCT}") }),
    );
    assert_eq!(read["result"]["contents"][0]["text"], product);
    assert_eq!(read["result"]["contents"][0]["mimeType"], "text/plain");

    let refused = server.request(
        10,
        "resources/read",
        json!({ "uri": "apiforge://source/%2e%2e/x.afs" }),
    );
    assert!(refused["error"].is_object(), "{refused}");
    assert_eq!(
        refused["error"]["data"]["code"], "path_rejected",
        "{refused}"
    );
    assert_clean_end(server);
    assert!(!root.join(".apiforge/local").exists());
}

#[test]
fn mcp03_criterion_10_a_cancelled_run_over_stdio() {
    let directory = demo();
    let root = directory.path();
    let mut server = Server::spawn_with(
        Some(root),
        root,
        &[("APIFORGE_MCP_TEST_STEP_DELAY_MS", "300")],
    );
    initialize(&mut server);

    send_call(&mut server, 2, "run", json!({ "path": SCENARIO }));
    thread::sleep(Duration::from_millis(400));
    // Answered while the run is in progress.
    let overview = call(&mut server, 3, "project_overview", json!({}));
    assert_eq!(structured(&overview)["name"], "Demo shop");
    let second = call(&mut server, 4, "run", json!({ "path": PRODUCT }));
    tool_error(&second, "run_in_progress");
    assert!(!answered(&server, 2), "the first run is still going");

    server.send(
        &json!({ "jsonrpc": "2.0", "method": "notifications/cancelled", "params": { "requestId": 2, "reason": "acceptance" } })
            .to_string(),
    );
    // The slot frees once the cancelled run has run its cleanup.
    let started = Instant::now();
    let mut id = 5;
    let report = loop {
        let response = call(&mut server, id, "run", json!({ "path": PRODUCT }));
        if response["result"]["structuredContent"]["code"] != "run_in_progress" {
            break response;
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the cancelled run did not end"
        );
        thread::sleep(Duration::from_millis(100));
        id += 1;
    };
    assert_eq!(structured(&report)["status"], "passed");

    let (status, _, stdout, stderr) = server.close();
    assert_eq!(status.code(), Some(0), "stderr: {stderr}");
    assert!(
        !stdout
            .iter()
            .any(|line| { serde_json::from_str::<Value>(line).unwrap()["id"] == json!(2) }),
        "a cancelled request gets no response"
    );
    assert!(stderr.contains("apiforge mcp: run: cancelled"), "{stderr}");
}

#[test]
fn mcp03_criterion_11_a_run_past_the_deadline_over_stdio() {
    let directory = demo();
    let root = directory.path();
    let mut server = Server::spawn_with(
        Some(root),
        root,
        &[
            ("APIFORGE_MCP_TEST_STEP_DELAY_MS", "200"),
            ("APIFORGE_MCP_TEST_DEADLINE_MS", "300"),
        ],
    );
    initialize(&mut server);
    let report = call(&mut server, 2, "run", json!({ "path": SCENARIO }));
    let report = structured(&report);
    assert_eq!(report["status"], "cancelled");
    assert_eq!(report["ended_by"], "timed_out");
    // The server stays up.
    let overview = call(&mut server, 3, "project_overview", json!({}));
    assert_eq!(structured(&overview)["name"], "Demo shop");
    assert_clean_end(server);
}

/// At EOF rmcp gives in-flight requests five seconds to finish. A run that
/// takes longer (eight steps of two seconds here) is then cancelled: it gets
/// no response, its cleanup runs, and the process ends.
#[test]
fn mcp03_eof_during_a_run_cancels_it_and_ends_the_process() {
    let directory = demo();
    let root = directory.path();
    let mut server = Server::spawn_with(
        Some(root),
        root,
        &[("APIFORGE_MCP_TEST_STEP_DELAY_MS", "2000")],
    );
    initialize(&mut server);
    send_call(&mut server, 2, "run", json!({ "path": SCENARIO }));
    thread::sleep(Duration::from_millis(400));
    let (status, elapsed, stdout, stderr) = server.close();
    assert_eq!(status.code(), Some(0), "stderr: {stderr}");
    assert!(
        stderr.contains("apiforge mcp: run: cancelled"),
        "the run was cancelled: {stderr}"
    );
    assert!(
        !stdout
            .iter()
            .any(|line| serde_json::from_str::<Value>(line).unwrap()["id"] == json!(2)),
        "a run cancelled at EOF gets no response"
    );
    assert!(elapsed < Duration::from_secs(15), "EOF took {elapsed:?}");
    assert!(!root.join(".apiforge/local").exists());
}

#[test]
fn mcp03_a_test_seam_that_is_not_a_number_stops_the_server() {
    let directory = demo();
    let root = directory.path();
    for name in [
        "APIFORGE_MCP_TEST_DEADLINE_MS",
        "APIFORGE_MCP_TEST_STEP_DELAY_MS",
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_apiforge"))
            .args(["mcp", "--folder"])
            .arg(root)
            .env(name, "300ms")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{name}");
        assert!(output.stdout.is_empty(), "{name}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains(&format!("{name} must be a whole number of milliseconds")),
            "{stderr}"
        );
    }
}

// MCP-04 (#63) ---------------------------------------------------------------

const DRAFT_TEXT: &str = "scenario \"Draft\" {\n}\n";

/// Another writer's attempt to take the project: `true` while the lock is
/// held. A successful attempt releases the lock at once.
fn locked(root: &Path) -> bool {
    use apiforge_storage::project::{ProjectError, ProjectStore};
    match ProjectStore::open_exclusive(root) {
        Err(ProjectError::Locked) => true,
        Ok(_) => false,
        Err(error) => panic!("{error}"),
    }
}

/// Waits up to a second after `since` for the lock to be free.
fn released_within_a_second(root: &Path, since: Instant) {
    while locked(root) {
        assert!(
            since.elapsed() < Duration::from_secs(1),
            "the lock is still held a second later"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn commented(text: &str) -> String {
    text.replacen(" {\n", " {\n  // Checked by the model.\n", 1)
}

fn holder(root: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(root.join(".apiforge/local/holder.json")).unwrap())
        .unwrap()
}

/// The saved text and its hash, over stdio.
fn read(server: &mut Server, id: u64, path: &str) -> (String, String) {
    let read = call(server, id, "read_source", json!({ "path": path }));
    let read = structured(&read);
    (
        read["text"].as_str().unwrap().to_owned(),
        read["source_sha256"].as_str().unwrap().to_owned(),
    )
}

#[test]
fn mcp04_criterion_7_the_session_keeps_the_lock_until_eof_and_names_the_client() {
    let directory = demo();
    let root = directory.path();
    let mut server = Server::spawn(Some(root), root);
    initialize(&mut server);
    let (text, hash) = read(&mut server, 2, SCENARIO);
    let written = call(
        &mut server,
        3,
        "write_source",
        json!({ "path": SCENARIO, "text": commented(&text), "expected_source_sha256": hash }),
    );
    assert_eq!(structured(&written)["wrote"], true);
    assert_eq!(
        std::fs::read_to_string(root.join(SCENARIO)).unwrap(),
        commented(&text)
    );
    assert!(locked(root), "the lock is kept after the write returns");
    let described = holder(root);
    assert_eq!(described["program"], "apiforge");
    assert_eq!(described["role"], "agent");
    assert_eq!(
        described["client"], "acceptance",
        "the client from initialize"
    );
    let overview = call(&mut server, 4, "project_overview", json!({}));
    assert_eq!(structured(&overview)["holder"], "agent_session");

    // A second write in the same session, after a pause.
    thread::sleep(Duration::from_millis(300));
    assert!(locked(root));
    let created = call(
        &mut server,
        5,
        "create_document",
        json!({ "kind": "scenario", "path": "scenarios/draft.afs", "text": DRAFT_TEXT }),
    );
    assert_eq!(structured(&created)["wrote"], true);
    assert!(locked(root));

    drop(server.stdin.take());
    released_within_a_second(root, Instant::now());
    assert_clean_end(server);
}

#[test]
fn mcp04_criterion_7_finish_editing_releases_the_lock_over_stdio() {
    let directory = demo();
    let root = directory.path();
    let mut server = Server::spawn(Some(root), root);
    let discover = server.request(1, "server/discover", json!({ "_meta": meta() }));
    assert!(discover["result"].is_object(), "{discover}");
    let read = server.request(
        2,
        "tools/call",
        json!({ "_meta": meta(), "name": "read_source", "arguments": { "path": SCENARIO } }),
    );
    let read = structured(&read).clone();
    let written = server.request(
        3,
        "tools/call",
        json!({ "_meta": meta(), "name": "write_source", "arguments": {
            "path": SCENARIO,
            "text": commented(read["text"].as_str().unwrap()),
            "expected_source_sha256": read["source_sha256"],
            "expected_generation": read["generation"],
        } }),
    );
    assert_eq!(structured(&written)["wrote"], true);
    assert_eq!(
        holder(root)["client"],
        "acceptance",
        "the client from the request metadata"
    );
    assert!(locked(root));
    let finished = server.request(
        4,
        "tools/call",
        json!({ "_meta": meta(), "name": "finish_editing", "arguments": {} }),
    );
    let answered = Instant::now();
    assert_eq!(structured(&finished)["finished"], true);
    released_within_a_second(root, answered);
    let overview = server.request(
        5,
        "tools/call",
        json!({ "_meta": meta(), "name": "project_overview", "arguments": {} }),
    );
    assert_eq!(structured(&overview)["holder"], "none");
    assert_clean_end(server);
}

#[test]
fn mcp04_criterion_3_a_write_based_on_a_version_apply_replaced_is_stale() {
    let directory = demo();
    let root = directory.path();
    let mut server = Server::spawn(Some(root), root);
    initialize(&mut server);
    let (text, hash) = read(&mut server, 2, SCENARIO);

    let incoming = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        incoming.path(),
        text.replace("on_error continue", "on_error stop"),
    )
    .unwrap();
    let applied = Command::new(env!("CARGO_BIN_EXE_apiforge"))
        .args(["apply", "--folder"])
        .arg(root)
        .args(["--file", SCENARIO])
        .arg(incoming.path())
        .output()
        .unwrap();
    assert!(applied.status.success(), "{applied:?}");
    let current = std::fs::read(root.join(SCENARIO)).unwrap();

    let stale = call(
        &mut server,
        3,
        "write_source",
        json!({ "path": SCENARIO, "text": commented(&text), "expected_source_sha256": hash }),
    );
    tool_error(&stale, "stale_source");
    let (_, current_hash) = read(&mut server, 4, SCENARIO);
    assert_eq!(
        stale["result"]["structuredContent"]["details"]["source_sha256"],
        current_hash.as_str()
    );
    assert_eq!(std::fs::read(root.join(SCENARIO)).unwrap(), current);
    assert_clean_end(server);
}

#[test]
fn mcp04_criterion_9_a_secret_from_the_environment_is_refused_and_never_echoed() {
    const SECRET: &str = "s3cr3t-token-over-stdio";
    let directory = demo();
    let root = directory.path();
    let mut server = Server::spawn_with(Some(root), root, &[("APIFORGE_SECRET_TOKEN", SECRET)]);
    initialize(&mut server);
    let (text, hash) = read(&mut server, 2, SCENARIO);
    let leaked = text.replacen(" {\n", &format!(" {{\n  // token {SECRET}\n"), 1);
    let refused = call(
        &mut server,
        4,
        "write_source",
        json!({ "path": SCENARIO, "text": leaked, "expected_source_sha256": hash }),
    );
    tool_error(&refused, "secret_in_text");
    let created = call(
        &mut server,
        5,
        "create_document",
        json!({
            "kind": "scenario",
            "path": "scenarios/leak.afs",
            "text": format!("scenario \"Leak\" {{\n  // {SECRET}\n}}\n"),
        }),
    );
    tool_error(&created, "secret_in_text");
    assert_eq!(std::fs::read_to_string(root.join(SCENARIO)).unwrap(), text);
    assert!(!root.join("scenarios/leak.afs").exists());
    let (status, _, stdout, stderr) = server.close();
    assert_eq!(status.code(), Some(0));
    for line in &stdout {
        assert!(!line.contains(SECRET), "stdout echoed the secret: {line}");
    }
    assert!(!stderr.contains(SECRET), "stderr echoed the secret");
}

#[test]
fn mcp04_criterion_8_a_window_holding_the_project_refuses_the_write_over_stdio() {
    use apiforge_storage::project::ProjectStore;
    let directory = demo();
    let root = directory.path();
    let window = ProjectStore::open_exclusive(root).unwrap();
    let mut server = Server::spawn(Some(root), root);
    initialize(&mut server);
    let (text, hash) = read(&mut server, 2, SCENARIO);
    let refused = call(
        &mut server,
        3,
        "write_source",
        json!({ "path": SCENARIO, "text": commented(&text), "expected_source_sha256": hash }),
    );
    tool_error(&refused, "held_by_window");
    assert!(
        refused["result"]["structuredContent"]["message"]
            .as_str()
            .unwrap()
            .contains("API Forge window"),
        "{refused}"
    );
    // Reads in the same session still work.
    assert_eq!(read(&mut server, 4, SCENARIO).0, text);
    drop(window);
    assert_clean_end(server);
    assert_eq!(std::fs::read_to_string(root.join(SCENARIO)).unwrap(), text);
}

// MCP-06 (#65) ---------------------------------------------------------------

/// `APIFORGE_SECRET_TOKEN` and the `--secrets` file's `FILE` in these tests.
const TOKEN: &str = "tok-mcp06-stdio-5d2e8a";
const FILE_TOKEN: &str = "file-tok-mcp06-77ab31";
const JOURNAL: &str = ".apiforge/local/journal.json";
const MANIFEST: &str = "apiforge.project.json";
const NOT_ENABLED: &str = "live requests are not enabled on this server";

/// A listener that counts the connections it accepts and answers none: a
/// request sent to it stays on its way.
struct Listener {
    address: std::net::SocketAddr,
    accepted: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Listener {
    fn start() -> Self {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        };
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let accepted = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (counter, stopped) = (accepted.clone(), stop.clone());
        thread::spawn(move || {
            let mut open = Vec::new();
            while !stopped.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((socket, _)) => {
                        counter.fetch_add(1, Ordering::AcqRel);
                        open.push(socket);
                    }
                    Err(_) => thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        Self {
            address,
            accepted,
            stop,
        }
    }

    fn accepted(&self) -> usize {
        self.accepted.load(std::sync::atomic::Ordering::Acquire)
    }

    fn base_url(&self) -> String {
        format!("http://{}/demo/shop", self.address)
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
    }
}

/// Points the project's base URL at `base_url`, the shop's root.
fn point_at(root: &Path, base_url: &str) {
    let path = root.join(MANIFEST);
    let manifest = std::fs::read_to_string(&path).unwrap();
    let current: Value = serde_json::from_str(&manifest).unwrap();
    let current = current["environment"]["baseUrl"].as_str().unwrap();
    let changed = manifest.replace(current, base_url);
    assert_ne!(changed, manifest, "the base URL was not replaced");
    std::fs::write(&path, changed).unwrap();
}

/// `text` with `from` replaced once, which must be there.
fn replaced(text: &str, from: &str, to: &str) -> String {
    let changed = text.replacen(from, to, 1);
    assert_ne!(changed, text, "the fixture lacks {from:?}");
    changed
}

/// The demo scenario with two declared secrets, `TOKEN` and `FILE`, that
/// one more request sends in its query and a header.
fn with_secrets(root: &Path) {
    let path = root.join(MANIFEST);
    let manifest = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        replaced(
            &manifest,
            "\"secretNames\": []",
            "\"secretNames\": [\"TOKEN\", \"FILE\"]",
        ),
    )
    .unwrap();
    let path = root.join(SCENARIO);
    let scenario = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        replaced(
            &scenario,
            "  cleanup {\n",
            "  @label(\"Echo\")\n  echo = GET \"/products/124?q={{env.TOKEN}}\" {\n    headers { \"X-Echo\": \"{{env.FILE}}\" }\n    sample 200 { id: 124 }\n  }\n  cleanup {\n",
        ),
    )
    .unwrap();
}

fn journal(root: &Path) -> Option<String> {
    std::fs::read_to_string(root.join(JOURNAL)).ok()
}

/// Waits up to ten seconds for a request to reach `api`, then for the run
/// journal to show it. A request is journalled before it is sent, so the
/// file is read only once the request has arrived: polling it meanwhile
/// would race the runner's atomic replace, which Windows refuses while the
/// file is open.
fn wait_for_journal(root: &Path, api: &Listener) -> String {
    let started = Instant::now();
    while api.accepted() == 0 {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "no request was sent"
        );
        thread::sleep(Duration::from_millis(5));
    }
    loop {
        if let Some(text) = journal(root)
            && serde_json::from_str::<Value>(&text).is_ok_and(|journal| {
                journal["runs"][0]["entries"]
                    .as_array()
                    .is_some_and(|entries| !entries.is_empty())
            })
        {
            return text;
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "no request was journalled"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

/// How `apiforge run` prints a journalled request's state.
fn state_label(state: &Value) -> &'static str {
    match state.as_str().unwrap() {
        "inFlight" => "Sending",
        "completed" => "Performed",
        "notSent" => "Not sent",
        "uncertain" => "Possibly performed",
        other => panic!("unknown state {other}"),
    }
}

impl Server {
    fn spawn_live(root: &Path, args: &[&str], envs: &[(&str, &str)]) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_apiforge"));
        command
            .arg("mcp")
            .arg("--folder")
            .arg(root)
            .args(args)
            .envs(envs.iter().copied());
        Self::spawn_command(command, root)
    }

    /// Closes stdin and waits up to `limit` for the process to end.
    fn close_within(mut self, limit: Duration) -> (ExitStatus, Duration, Vec<String>, String) {
        drop(self.stdin.take());
        let closed = Instant::now();
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            if closed.elapsed() > limit {
                let _ = self.child.kill();
                panic!("the server did not end within {limit:?} of EOF");
            }
            thread::sleep(Duration::from_millis(10));
        };
        let elapsed = closed.elapsed();
        while let Ok(line) = self.lines.recv_timeout(Duration::from_secs(2)) {
            self.stdout.push(line);
        }
        let stderr = self.stderr.join().unwrap();
        (status, elapsed, self.stdout, stderr)
    }
}

/// The interrupted run a journalled request belongs to, as the next server
/// reports it: method, URL and state as `apiforge run` prints them.
fn assert_reported(reported: &Value, journalled: &str) {
    let journalled: Value = serde_json::from_str(journalled).unwrap();
    let entry = &journalled["runs"][0]["entries"][0];
    let runs = reported.as_array().unwrap();
    assert_eq!(runs.len(), 1, "{reported}");
    assert_eq!(runs[0]["scenario"], journalled["runs"][0]["scenarioName"]);
    let request = &runs[0]["requests"][0];
    assert_eq!(request["method"], entry["method"]);
    assert_eq!(request["url"], entry["url"]);
    assert_eq!(request["state"], state_label(&entry["state"]));
}

/// Criterion 1 over stdio: without `--allow-live` the run is refused and
/// nothing is sent; the overview says why; with the flag it says so.
#[test]
fn mcp06_criterion_1_without_allow_live_over_stdio() {
    let directory = demo();
    let root = directory.path();
    let api = Listener::start();
    point_at(root, &api.base_url());
    let mut server = Server::spawn(Some(root), root);
    initialize(&mut server);
    let overview = call(&mut server, 2, "project_overview", json!({}));
    assert_eq!(
        structured(&overview)["live_requests"],
        json!({ "allowed": false, "source": "no --allow-live" })
    );
    let refused = call(
        &mut server,
        3,
        "run",
        json!({ "path": SCENARIO, "live": true }),
    );
    tool_error(&refused, "live_disabled");
    assert_eq!(
        refused["result"]["structuredContent"]["message"],
        NOT_ENABLED
    );
    assert_clean_end(server);
    thread::sleep(Duration::from_millis(200));
    assert_eq!(api.accepted(), 0, "nothing was sent");
    assert!(!root.join(".apiforge/local").exists(), "nothing was taken");

    let mut server = Server::spawn_live(root, &["--allow-live"], &[]);
    initialize(&mut server);
    let overview = call(&mut server, 2, "project_overview", json!({}));
    assert_eq!(
        structured(&overview)["live_requests"],
        json!({ "allowed": true, "source": "--allow-live" })
    );
    let tools = server.request(3, "tools/list", json!({}));
    let run = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "run")
        .unwrap()
        .clone();
    assert_eq!(run["annotations"]["openWorldHint"], true, "{run}");
    let first = run["description"]
        .as_str()
        .unwrap()
        .split(". ")
        .next()
        .unwrap()
        .to_owned();
    assert!(first.contains("real requests"), "{first}");
    assert_clean_end(server);
}

/// Criterion 7: EOF during a live run cancels it at once; its cleanup ends
/// in time, the journal is empty, the lock is released and the process
/// exits.
#[test]
fn mcp06_criterion_7_eof_during_a_live_run_empties_the_journal() {
    let directory = demo();
    let root = directory.path();
    let api = Listener::start();
    point_at(root, &api.base_url());
    let mut server = Server::spawn_live(root, &["--allow-live"], &[]);
    initialize(&mut server);
    send_call(
        &mut server,
        2,
        "run",
        json!({ "path": SCENARIO, "live": true }),
    );
    wait_for_journal(root, &api);
    let started = Instant::now();
    while api.accepted() == 0 {
        assert!(started.elapsed() < Duration::from_secs(10), "no connection");
        thread::sleep(Duration::from_millis(5));
    }
    assert!(locked(root), "the live run holds the agent session");
    let (status, elapsed, _, stderr) = server.close_within(Duration::from_secs(15));
    assert_eq!(status.code(), Some(0), "stderr: {stderr}");
    assert!(elapsed < Duration::from_secs(10), "EOF took {elapsed:?}");
    assert!(stderr.contains("apiforge mcp: run: cancelled"), "{stderr}");
    assert!(api.accepted() >= 1);
    assert_eq!(journal(root), None, "the cleanup ended and removed it");
    assert!(!locked(root), "the lock is released");
}

/// Criterion 7 with the cleanup blocked (the test seam): the server waits
/// ten seconds for it, then exits; the entry stays and the next server's
/// `run` reports it under `interrupted`.
#[test]
fn mcp06_criterion_7_with_the_cleanup_blocked_the_entry_stays() {
    let directory = demo();
    let root = directory.path();
    let api = Listener::start();
    point_at(root, &api.base_url());
    let mut server = Server::spawn_live(
        root,
        &["--allow-live"],
        &[("APIFORGE_MCP_TEST_CLEANUP_DELAY_MS", "600000")],
    );
    initialize(&mut server);
    send_call(
        &mut server,
        2,
        "run",
        json!({ "path": SCENARIO, "live": true }),
    );
    wait_for_journal(root, &api);
    let (status, elapsed, _, stderr) = server.close_within(Duration::from_secs(20));
    assert_eq!(status.code(), Some(0), "stderr: {stderr}");
    assert!(
        elapsed >= Duration::from_secs(9) && elapsed < Duration::from_secs(14),
        "the cleanup is waited for ten seconds: {elapsed:?}"
    );
    let journalled = journal(root).expect("the entry stays");
    assert!(!locked(root), "the lock is released");

    let mut next = Server::spawn(Some(root), root);
    initialize(&mut next);
    let overview = call(&mut next, 2, "project_overview", json!({}));
    assert_eq!(structured(&overview)["interrupted_runs"], 1);
    let report = call(&mut next, 3, "run", json!({ "path": SCENARIO }));
    let report = structured(&report);
    assert_eq!(report["status"], "passed");
    assert_reported(&report["interrupted"], &journalled);
    assert_clean_end(next);
}

/// Criterion 8: a server killed with a live request on its way leaves the
/// journal entry; a new server's next `run`, simulated or live, returns it
/// under `interrupted`, and its overview counts it.
#[test]
fn mcp06_criterion_8_a_killed_server_leaves_the_journal_entry() {
    let directory = demo();
    let root = directory.path();
    let api = Listener::start();
    point_at(root, &api.base_url());
    let mut server = Server::spawn_live(root, &["--allow-live"], &[]);
    initialize(&mut server);
    send_call(
        &mut server,
        2,
        "run",
        json!({ "path": SCENARIO, "live": true, "request_id": "killed-run" }),
    );
    let journalled = wait_for_journal(root, &api);
    server.child.kill().unwrap();
    server.child.wait().unwrap();
    assert_eq!(
        journal(root).as_deref(),
        Some(&*journalled),
        "the entry stays"
    );
    let entry = &serde_json::from_str::<Value>(&journalled).unwrap()["runs"][0]["entries"][0];
    assert_eq!(entry["method"], "GET");
    assert_eq!(entry["url"], format!("{}/products/123", api.base_url()));
    assert_eq!(entry["state"], "inFlight");
    assert!(!locked(root), "the operating system released the lock");

    let mut next = Server::spawn(Some(root), root);
    initialize(&mut next);
    let overview = call(&mut next, 2, "project_overview", json!({}));
    assert_eq!(structured(&overview)["interrupted_runs"], 1);
    let report = call(&mut next, 3, "run", json!({ "path": SCENARIO }));
    let report = structured(&report);
    assert_eq!(report["live"], false);
    assert_reported(&report["interrupted"], &journalled);
    assert_eq!(report["interrupted"][0]["requests"][0]["state"], "Sending");
    assert_clean_end(next);

    let demo_api = apiforge_demo_api::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    point_at(root, &format!("{}/shop", demo_api.base_url()));
    let mut live = Server::spawn_live(root, &["--allow-live"], &[]);
    initialize(&mut live);
    let report = call(
        &mut live,
        2,
        "run",
        json!({ "path": SCENARIO, "live": true }),
    );
    let report = structured(&report);
    assert_eq!(report["status"], "passed", "{}", report["error"]);
    assert_eq!(report["live"], true);
    assert_reported(&report["interrupted"], &journalled);
    // Criterion 9: the killed server's request_id is unknown_outcome here.
    let retried = call(
        &mut live,
        3,
        "run",
        json!({ "path": SCENARIO, "live": true, "request_id": "killed-run" }),
    );
    tool_error(&retried, "unknown_outcome");
    assert_reported(
        &retried["result"]["structuredContent"]["details"]["interrupted"],
        &journalled,
    );
    assert_clean_end(live);
}

/// Criterion 10: with a secret in the environment and another in the
/// `--secrets` file, a session of every tool — reads, checks, runs, refused
/// writes with the value inside `text` — never returns a value: not in a
/// result, not on stderr, not in the journal written mid-run.
#[test]
fn mcp06_criterion_10_no_secret_value_leaves_the_server() {
    let directory = demo();
    let root = directory.path();
    let demo_api = apiforge_demo_api::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    point_at(root, &format!("{}/shop", demo_api.base_url()));
    with_secrets(root);
    let secrets = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(secrets.path(), json!({ "FILE": FILE_TOKEN }).to_string()).unwrap();
    let mut server = Server::spawn_live(
        root,
        &[
            "--allow-live",
            "--secrets",
            secrets.path().to_str().unwrap(),
        ],
        &[("APIFORGE_SECRET_TOKEN", TOKEN)],
    );
    initialize(&mut server);
    server.request(2, "tools/list", json!({}));
    let text = std::fs::read_to_string(root.join(SCENARIO)).unwrap();
    let hash = read(&mut server, 3, SCENARIO).1;
    let leaked = |value: &str| text.replacen(" {\n", &format!(" {{\n  // {value}\n"), 1);
    let mut id = 4;
    let mut next = || {
        id += 1;
        id
    };
    for (name, arguments) in [
        ("project_overview", json!({})),
        ("language_reference", json!({})),
        ("check", json!({})),
        ("check_text", json!({ "path": SCENARIO, "text": text })),
        ("print", json!({ "path": SCENARIO })),
        ("run", json!({ "path": SCENARIO })),
    ] {
        let response = call(&mut server, next(), name, arguments);
        structured(&response);
    }
    let report = call(
        &mut server,
        next(),
        "run",
        json!({ "path": SCENARIO, "live": true }),
    );
    assert_eq!(
        structured(&report)["status"],
        "passed",
        "{}",
        report["result"]["structuredContent"]["error"]
    );
    let full = call(
        &mut server,
        next(),
        "run",
        json!({ "path": SCENARIO, "live": true, "full": true }),
    );
    assert!(
        structured(&full).to_string().contains("[REDACTED]"),
        "the values are masked in the report"
    );
    for (name, arguments) in [
        (
            "check_text",
            json!({ "path": SCENARIO, "text": leaked(TOKEN) }),
        ),
        (
            "write_source",
            json!({ "path": SCENARIO, "text": leaked(TOKEN), "expected_source_sha256": hash }),
        ),
        (
            "write_source",
            json!({ "path": SCENARIO, "text": leaked(FILE_TOKEN), "expected_source_sha256": hash }),
        ),
        (
            "create_document",
            json!({ "kind": "scenario", "path": "scenarios/leak.afs", "text": format!("scenario \"Leak\" {{\n  // {FILE_TOKEN}\n}}\n") }),
        ),
        ("run", json!({ "path": SCENARIO, "request_id": TOKEN })),
    ] {
        let response = call(&mut server, next(), name, arguments);
        tool_error(&response, "secret_in_text");
    }
    let listed = server.request(next(), "resources/list", json!({}));
    assert!(listed["result"]["resources"].is_array());
    let read = server.request(
        next(),
        "resources/read",
        json!({ "uri": format!("apiforge://source/{SCENARIO}") }),
    );
    assert!(read["result"]["contents"][0]["text"].is_string());

    // The journal while a live request is on its way.
    let api = Listener::start();
    point_at(root, &api.base_url());
    let run_id = next();
    send_call(
        &mut server,
        run_id,
        "run",
        json!({ "path": SCENARIO, "live": true }),
    );
    let journalled = wait_for_journal(root, &api);
    server.send(
        &json!({ "jsonrpc": "2.0", "method": "notifications/cancelled", "params": { "requestId": run_id, "reason": "acceptance" } })
            .to_string(),
    );
    // The run's slot frees once its cleanup has run; the journal file is
    // not polled while the runner rewrites it.
    let started = Instant::now();
    loop {
        let response = call(&mut server, next(), "run", json!({ "path": PRODUCT }));
        if response["result"]["structuredContent"]["code"] != "run_in_progress" {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the run did not end"
        );
        thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(journal(root), None, "the cancelled run removed its entry");
    let finished = call(&mut server, next(), "finish_editing", json!({}));
    assert_eq!(structured(&finished)["finished"], true);
    let (status, _, stdout, stderr) = server.close();
    assert_eq!(status.code(), Some(0), "stderr: {stderr}");
    for value in [TOKEN, FILE_TOKEN] {
        for line in &stdout {
            assert!(!line.contains(value), "a result carried a secret value");
        }
        assert!(!stderr.contains(value), "stderr carried a secret value");
        assert!(
            !journalled.contains(value),
            "the journal holds a secret value"
        );
    }
    assert!(!root.join("scenarios/leak.afs").exists());
    assert_eq!(std::fs::read_to_string(root.join(SCENARIO)).unwrap(), text);
}

/// The `--secrets` file is read at start; one that cannot be used stops the
/// server with exit code 2 and never quotes its content.
#[test]
fn mcp06_a_secrets_file_that_cannot_be_read_stops_the_server() {
    let directory = demo();
    let root = directory.path();
    let secrets = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(secrets.path(), format!("{{ \"TOKEN\": {TOKEN} }}")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_apiforge"))
        .args(["mcp", "--folder"])
        .arg(root)
        .arg("--secrets")
        .arg(secrets.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("The secrets file is not JSON"), "{stderr}");
    assert!(!stderr.contains(TOKEN));
}

// MCP-05 (#64) ---------------------------------------------------------------

/// Criteria 1, 4 and 6 over stdio: the refusal that lists the comments, the
/// consent bound to the hash, a repeated `request_id` answered from the
/// record, and the overview listing the revision with its hash.
#[test]
fn mcp05_publish_over_stdio() {
    const LOOSE: &str = "// A loose note from the model.";
    let directory = demo();
    let root = directory.path();
    let mut server = Server::spawn(Some(root), root);
    initialize(&mut server);
    let (text, hash) = read(&mut server, 2, PRODUCT);
    let written = call(
        &mut server,
        3,
        "write_source",
        json!({ "path": PRODUCT, "text": format!("{LOOSE}\n\n{text}"), "expected_source_sha256": hash }),
    );
    let hash = structured(&written)["source_sha256"]
        .as_str()
        .unwrap()
        .to_owned();

    let refused = call(
        &mut server,
        4,
        "publish",
        json!({ "path": PRODUCT, "expected_source_sha256": hash, "request_id": "qa-1" }),
    );
    tool_error(&refused, "comments_would_be_lost");
    assert_eq!(
        refused["result"]["structuredContent"]["details"]["comments"][0]["text"],
        LOOSE
    );

    let arguments = json!({
        "path": PRODUCT,
        "expected_source_sha256": hash,
        "accept_comment_loss": true,
        "request_id": "qa-1",
    });
    let published = call(&mut server, 5, "publish", arguments.clone());
    let published = structured(&published).clone();
    assert_eq!(published["wrote"], true);
    assert_eq!(published["replayed"], false);
    assert_eq!(published["handle"], "product");
    assert_eq!(published["revision"], 2);
    let revision_path = published["revision_path"].as_str().unwrap();
    assert!(root.join(revision_path).is_file());

    let again = call(&mut server, 6, "publish", arguments);
    let again = structured(&again).clone();
    assert_eq!(again["replayed"], true);
    assert_eq!(again["revision"], 2);
    assert_eq!(again["source_sha256"], published["source_sha256"]);
    assert!(
        !root
            .join(revision_path.replace("/2.afs", "/3.afs"))
            .exists()
    );

    let overview = call(&mut server, 7, "project_overview", json!({}));
    let overview = structured(&overview);
    assert!(
        overview["revisions"].as_array().unwrap().iter().any(|r| {
            r["handle"] == "product"
                && r["revision"] == 2
                && r["source_sha256"] == published["source_sha256"]
        }),
        "{overview}"
    );
    assert_eq!(overview["holder"], "agent_session");
    assert!(locked(root), "the lock stays with the session");
    assert_clean_end(server);
}

/// Criterion 7 over stdio: a window holding the project refuses `publish`.
#[test]
fn mcp05_criterion_7_a_window_holding_the_project_refuses_publish_over_stdio() {
    use apiforge_storage::project::ProjectStore;
    let directory = demo();
    let root = directory.path();
    let manifest = std::fs::read(root.join("apiforge.project.json")).unwrap();
    let window = ProjectStore::open_exclusive(root).unwrap();
    let mut server = Server::spawn(Some(root), root);
    initialize(&mut server);
    let (_, hash) = read(&mut server, 2, PRODUCT);
    let refused = call(
        &mut server,
        3,
        "publish",
        json!({ "path": PRODUCT, "expected_source_sha256": hash }),
    );
    tool_error(&refused, "held_by_window");
    drop(window);
    assert_clean_end(server);
    assert_eq!(
        std::fs::read(root.join("apiforge.project.json")).unwrap(),
        manifest,
        "no revision was registered"
    );
}

#[test]
fn mcp12_stdio_exposes_holder_contract_and_handover_guidance() {
    let directory = demo();
    let root = directory.path();
    let mut server = Server::spawn(Some(root), root);
    let init = server.request(
        1,
        "initialize",
        json!({
            "protocolVersion": "2025-11-25", "capabilities": {},
            "clientInfo": {"name": "MCP-12 client", "version": "1"}
        }),
    );
    assert!(
        init["result"]["instructions"]
            .as_str()
            .unwrap()
            .contains("hand")
    );
    server.send(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    let listed = server.request(2, "tools/list", json!({}));
    let tools = listed["result"]["tools"].as_array().unwrap();
    let overview = tools
        .iter()
        .find(|tool| tool["name"] == "project_overview")
        .unwrap();
    let schema = &overview["outputSchema"];
    let holder_schema = &schema["properties"]["holder"];
    assert_eq!(holder_schema["type"], "string");
    let mut values: Vec<_> = holder_schema["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    values.sort();
    assert_eq!(
        values,
        ["agent_session", "another_process", "none", "window"]
    );
    for field in ["holder_details", "hands_over_on_write"] {
        assert!(schema["properties"].get(field).is_some(), "{schema}");
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .contains(&json!(field))
        );
    }
    assert!(
        overview["description"]
            .as_str()
            .unwrap()
            .contains("hands_over_on_write")
    );
    let run = tools.iter().find(|tool| tool["name"] == "run").unwrap();
    let guidance = run["description"].as_str().unwrap();
    assert!(guidance.contains("hand"));
    assert!(guidance.contains("unsaved"));
    assert!(guidance.contains("held_by_other"));
    assert!(guidance.contains("live_disabled"));
    let before = call(&mut server, 3, "project_overview", json!({}));
    assert_eq!(structured(&before)["holder"], "none");
    assert_eq!(structured(&before)["holder_details"], Value::Null);
    assert_eq!(structured(&before)["hands_over_on_write"], false);
    let created = call(
        &mut server,
        4,
        "create_document",
        json!({
            "kind": "scenario", "path": "scenarios/mcp12.afs", "text": "scenario \"MCP-12\" {\n}\n"
        }),
    );
    assert_eq!(structured(&created)["wrote"], true);
    let active = call(&mut server, 5, "project_overview", json!({}));
    assert_eq!(structured(&active)["holder"], "agent_session");
    assert_eq!(structured(&active)["holder_details"]["role"], "agent");
    assert_eq!(
        structured(&active)["holder_details"]["client"],
        "MCP-12 client"
    );
    assert_eq!(structured(&active)["hands_over_on_write"], false);
    assert_clean_end(server);
}

#[test]
fn mcp12_actual_cli_connects_with_a_filtered_environment() {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args([
        "--ignored",
        "--exact",
        "mcp12_cli_window_process",
        "--nocapture",
    ]);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        let temp = tempfile::tempdir().unwrap();
        let runtime = format!(
            "/run/user/{}",
            std::fs::metadata(temp.path()).unwrap().uid()
        );
        assert!(
            Path::new(&runtime).is_dir(),
            "MCP-12 requires provisioned {runtime}"
        );
        command.env("XDG_RUNTIME_DIR", runtime);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "launched by MCP-12 with the window's isolated runtime environment"]
fn mcp12_cli_window_process() {
    use apiforge_channel::{Listener, Message, Received};
    use apiforge_storage::project::{Holder, ProjectStore};
    for eof in [false, true] {
        let directory = demo();
        let root = directory.path();
        #[cfg(unix)]
        let temp = tempfile::tempdir().unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let mut listener = {
            let _context = runtime.enter();
            Listener::bind().unwrap()
        };
        let owner = ProjectStore::open_exclusive_as(
            root,
            &Holder::window().with_endpoint(Some(listener.endpoint().name().to_owned())),
        )
        .unwrap();
        let window = thread::spawn(move || {
            runtime.block_on(async move {
                let mut connection = listener.accept().await.unwrap();
                assert!(matches!(
                    connection.reader.next().await.unwrap(),
                    Some(Received::Message(Message::Request { .. }))
                ));
                drop(owner);
                connection.writer.send(&Message::Granted).await.unwrap();
                let mut saved = false;
                loop {
                    match connection.reader.next().await.unwrap() {
                        Some(Received::Message(Message::Saved { .. })) => saved = true,
                        Some(Received::Message(Message::Released)) | None => break,
                        _ => {}
                    }
                }
                assert!(saved, "the CLI sent its saved event");
            })
        });
        let mut command = Command::new(env!("CARGO_BIN_EXE_apiforge"));
        command.arg("mcp").arg("--folder").arg(root);
        #[cfg(unix)]
        command
            .env_remove("XDG_RUNTIME_DIR")
            .env("TMPDIR", temp.path());
        #[cfg(windows)]
        {
            command.env_clear();
            for name in ["PATH", "SystemRoot"] {
                if let Some(value) = std::env::var_os(name) {
                    command.env(name, value);
                }
            }
        }
        let mut server = Server::spawn_command(command, root);
        initialize(&mut server);
        let overview = call(&mut server, 2, "project_overview", json!({}));
        assert_eq!(structured(&overview)["holder"], "window");
        assert_eq!(structured(&overview)["hands_over_on_write"], true);
        let created = call(
            &mut server,
            3,
            "create_document",
            json!({
                "kind": "scenario", "path": "scenarios/mcp12.afs", "text": "scenario \"MCP-12\" {\n}\n"
            }),
        );
        assert_eq!(structured(&created)["wrote"], true, "{created}");
        if !eof {
            let finished = call(&mut server, 4, "finish_editing", json!({}));
            assert_eq!(structured(&finished)["finished"], true);
        }
        assert_clean_end(server);
        window.join().unwrap();
        assert!(
            ProjectStore::open_exclusive(root).is_ok(),
            "ownership returns on finish and EOF"
        );
    }
}
