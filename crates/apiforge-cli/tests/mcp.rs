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
/// with their `destructiveHint` and `idempotentHint`.
const WRITE_TOOLS: [(&str, bool, bool); 4] = [
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
