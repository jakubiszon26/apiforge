//! A spawned MCP server driven over real stdio, for the tests of other
//! crates (MCP-08b): the desktop's hand-over tests start a server beside a
//! window and call its tools while the window runs. Only with the
//! `test-support` feature, which no release enables.
//!
//! The process may be any command that serves MCP on its stdio: the
//! `apiforge mcp` binary, or a test executable that runs one test which
//! serves. A test executable prints its harness's own lines (`running 1
//! test`) on the same stdout; a line that is not a JSON object is kept apart
//! as noise rather than taken for a protocol message.
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Child, ChildStdin, Command, ExitStatus, Stdio},
    sync::mpsc::{Receiver, channel},
    thread,
    time::{Duration, Instant},
};

/// How long a blocking request waits for its answer.
const ANSWER: Duration = Duration::from_secs(30);

pub struct StdioServer {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    /// Every protocol message read so far, in order.
    pub stdout: Vec<Value>,
    /// Lines on stdout that were not JSON objects.
    pub noise: Vec<String>,
    stderr: Option<thread::JoinHandle<String>>,
    next_id: u64,
}

/// What a server left when it ended.
pub struct Ended {
    pub status: ExitStatus,
    /// From the end of stdin, or the kill, to the end of the process.
    pub elapsed: Duration,
    pub stdout: Vec<Value>,
    pub noise: Vec<String>,
    pub stderr: String,
}

impl StdioServer {
    /// Starts `command` with piped stdio.
    pub fn spawn(mut command: Command) -> Self {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the server starts");
        let stdout = child.stdout.take().expect("piped stdout");
        let (sender, lines) = channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        let mut stderr = child.stderr.take().expect("piped stderr");
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
            noise: Vec::new(),
            stderr: Some(stderr),
            next_id: 1,
        }
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Writes one line to the server's stdin.
    pub fn send(&mut self, message: &str) {
        let stdin = self.stdin.as_mut().expect("stdin is open");
        stdin
            .write_all(message.as_bytes())
            .expect("the server reads");
        stdin.write_all(b"\n").expect("the server reads");
        stdin.flush().expect("the server reads");
    }

    /// Sends a request and returns its id without waiting for the answer.
    pub fn start(&mut self, method: &str, params: Value) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.send(
            &json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string(),
        );
        id
    }

    /// Starts a `tools/call` without waiting for its answer.
    pub fn start_call(&mut self, tool: &str, arguments: Value) -> u64 {
        self.start(
            "tools/call",
            json!({ "name": tool, "arguments": arguments }),
        )
    }

    /// The answer to request `id` if it has arrived; reads what is waiting
    /// on stdout and never blocks.
    pub fn poll(&mut self, id: u64) -> Option<Value> {
        while let Ok(line) = self.lines.try_recv() {
            self.keep(line);
        }
        self.answer(id)
    }

    /// Waits up to thirty seconds for the answer to request `id`.
    pub fn wait(&mut self, id: u64) -> Value {
        let deadline = Instant::now() + ANSWER;
        loop {
            if let Some(answer) = self.answer(id) {
                return answer;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(line) => self.keep(line),
                Err(_) => panic!(
                    "no answer to request {id}; stdout so far: {:#?}",
                    self.stdout
                ),
            }
        }
    }

    /// Sends a request and waits for its answer.
    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.start(method, params);
        self.wait(id)
    }

    /// Calls a tool and waits for its answer.
    pub fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let id = self.start_call(tool, arguments);
        self.wait(id)
    }

    /// `initialize` with protocol revision 2025-11-25 as the client
    /// `client`, then `notifications/initialized`.
    pub fn initialize(&mut self, client: &str) -> Value {
        let answer = self.request(
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": client, "version": "1.0.0" }
            }),
        );
        self.send(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
        answer
    }

    /// Closes stdin and waits up to ten seconds for the process to end.
    pub fn close(mut self) -> Ended {
        drop(self.stdin.take());
        self.end(false)
    }

    /// Kills the process and waits for it.
    pub fn kill(mut self) -> Ended {
        self.end(true)
    }

    /// Whether the process has ended.
    pub fn has_exited(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_some()
    }

    fn end(&mut self, kill: bool) -> Ended {
        let started = Instant::now();
        if kill {
            let _ = self.child.kill();
        }
        let status = loop {
            if let Some(status) = self.child.try_wait().expect("the process can be waited on") {
                break status;
            }
            if started.elapsed() > Duration::from_secs(10) {
                let _ = self.child.kill();
                panic!("the server did not end");
            }
            thread::sleep(Duration::from_millis(10));
        };
        let elapsed = started.elapsed();
        while let Ok(line) = self.lines.recv_timeout(Duration::from_secs(2)) {
            self.keep(line);
        }
        let stderr = self
            .stderr
            .take()
            .map(|reader| reader.join().unwrap_or_default())
            .unwrap_or_default();
        Ended {
            status,
            elapsed,
            stdout: std::mem::take(&mut self.stdout),
            noise: std::mem::take(&mut self.noise),
            stderr,
        }
    }

    fn keep(&mut self, line: String) {
        match serde_json::from_str::<Value>(&line) {
            Ok(message) if message.is_object() => self.stdout.push(message),
            _ => self.noise.push(line),
        }
    }

    fn answer(&self, id: u64) -> Option<Value> {
        self.stdout
            .iter()
            .find(|message| message["id"] == json!(id) && message.get("method").is_none())
            .cloned()
    }
}

impl Drop for StdioServer {
    /// A test that fails half-way leaves no server holding a project.
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// A tool's `structuredContent` when it succeeded.
pub fn structured(answer: &Value) -> &Value {
    let result = &answer["result"];
    assert_ne!(result["isError"], true, "{answer}");
    &result["structuredContent"]
}

/// A tool's `structuredContent` when it answered an error.
pub fn tool_error(answer: &Value) -> &Value {
    let result = &answer["result"];
    assert_eq!(result["isError"], true, "{answer}");
    &result["structuredContent"]
}
