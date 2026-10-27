//! MCP-01 (#60), criteria 1, 2 and 8 over real stdio: the `apiforge mcp`
//! binary driven with both protocol revisions, a malformed line, an unknown
//! method, a tool error and EOF. Criterion 6's "byte for byte what
//! `apiforge print` prints" is checked against the command here as well.
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
const TOOLS: [&str; 4] = ["project_overview", "read_source", "check", "print"];

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
        let mut command = Command::new(env!("CARGO_BIN_EXE_apiforge"));
        command.arg("mcp");
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
        assert_eq!(tool["annotations"]["readOnlyHint"], true, "{tool}");
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
    let unknown =
        server.response(|m| m["id"] == json!(4) || (m["id"].is_null() && m["error"].is_object()));
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
    let unknown =
        server.response(|m| m["id"] == json!(4) || (m["id"].is_null() && m["error"].is_object()));
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
