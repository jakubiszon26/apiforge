//! REQ-19 (#610): the demo API writes the requests it received to a file, when asked.

use apiforge_demo_api::BODY_LIMIT;
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpStream},
    path::Path,
    process::{Child, ChildStdout, Command, Output, Stdio},
    time::Duration,
};

const BODY_LOG_LIMIT: usize = 64 * 1024;

fn program() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_apiforge-demo-api"));
    command
        .args(["--port", "0"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

/// The program, listening on a free port; dropping it ends the process.
struct Running {
    child: Child,
    stdout: BufReader<ChildStdout>,
    first_line: String,
    addr: SocketAddr,
}

impl Running {
    fn start(command: &mut Command) -> Self {
        let mut child = command.spawn().expect("start the demo API");
        let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        let mut first_line = String::new();
        stdout.read_line(&mut first_line).expect("first line");
        let addr = first_line
            .trim_end()
            .strip_prefix("API Forge demo API: http://")
            .and_then(|rest| rest.strip_suffix("/demo"))
            .and_then(|addr| addr.parse().ok());
        let Some(addr) = addr else {
            let _ = child.kill();
            let mut message = String::new();
            let _ = child
                .stderr
                .take()
                .expect("piped stderr")
                .read_to_string(&mut message);
            let _ = child.wait();
            panic!("the demo API did not start: {first_line:?}\n{message}");
        };
        Self {
            child,
            stdout,
            first_line,
            addr,
        }
    }

    fn logging_to(log: &Path) -> Self {
        Self::start(program().arg("--request-log").arg(log))
    }

    /// Ends the program and returns all it printed to stdout and stderr.
    fn output(mut self) -> (String, String) {
        self.child.kill().expect("end the demo API");
        self.child.wait().expect("the ended demo API");
        let (mut out, mut err) = (self.first_line.clone(), String::new());
        self.stdout.read_to_string(&mut out).expect("stdout");
        self.child
            .stderr
            .take()
            .expect("piped stderr")
            .read_to_string(&mut err)
            .expect("stderr");
        (out, err)
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Sends `head` (a request line and headers, `Host` excluded) and the payload on a
/// fresh connection and returns the status of the reply.
fn send(addr: SocketAddr, head: &[&str], payload: &[u8]) -> u16 {
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout");
    let mut request = format!("{}\r\nConnection: close\r\n", head[0]);
    if !head.iter().any(|line| line.starts_with("Host:")) {
        request.push_str(&format!("Host: {addr}\r\n"));
    }
    for line in &head[1..] {
        request.push_str(&format!("{line}\r\n"));
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).expect("send head");
    stream.write_all(payload).expect("send payload");
    let mut reply = String::new();
    stream.read_to_string(&mut reply).expect("receive");
    reply
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .expect("status code")
}

fn get(addr: SocketAddr, path: &str, headers: &[&str]) -> u16 {
    let line = format!("GET {path} HTTP/1.1");
    send(addr, &[&[line.as_str()], headers].concat(), b"")
}

fn post(addr: SocketAddr, path: &str, content_type: &str, body: &str) -> u16 {
    let line = format!("POST {path} HTTP/1.1");
    let content_type = format!("Content-Type: {content_type}");
    let length = format!("Content-Length: {}", body.len());
    send(addr, &[&line, &content_type, &length], body.as_bytes())
}

/// Posts a JSON body in chunks, without a `Content-Length`.
fn post_chunked(addr: SocketAddr, path: &str, body: &str) -> u16 {
    let mut payload = Vec::new();
    for chunk in body.as_bytes().chunks(10_000) {
        payload.extend_from_slice(format!("{:x}\r\n", chunk.len()).as_bytes());
        payload.extend_from_slice(chunk);
        payload.extend_from_slice(b"\r\n");
    }
    payload.extend_from_slice(b"0\r\n\r\n");
    let line = format!("POST {path} HTTP/1.1");
    let head = [
        line.as_str(),
        "Content-Type: application/json",
        "Transfer-Encoding: chunked",
    ];
    send(addr, &head, &payload)
}

/// An empty cart, with a field the API ignores that makes the body `length` bytes long.
fn padded_body(length: usize) -> String {
    let empty = json!({ "note": "" }).to_string();
    json!({ "note": "x".repeat(length - empty.len()) }).to_string()
}

/// The log's entries; every line is one, and the last line is complete.
fn entries(log: &Path) -> Vec<Value> {
    let text = fs::read_to_string(log).expect("request log");
    assert!(text.is_empty() || text.ends_with('\n'), "{text:?}");
    text.lines()
        .map(|line| serde_json::from_str(line).unwrap_or_else(|error| panic!("{error}: {line}")))
        .collect()
}

#[test]
fn req19_criterion_1_one_json_line_per_request() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("requests.jsonl");
    let before = chrono::Utc::now();
    let api = Running::logging_to(&log);
    assert_eq!(entries(&log), Vec::<Value>::new());

    let product = "/demo/shop/products/123?lang=pl&q=a%20b";
    assert_eq!(get(api.addr, product, &["X-Client: round-1"]), 200);
    // The line is there when the reply is.
    assert_eq!(entries(&log).len(), 1);
    assert_eq!(
        post(
            api.addr,
            "/demo/shop/carts",
            "application/json",
            "{ \"productId\": 123,\n  \"quantity\": 2 }",
        ),
        201
    );
    assert_eq!(get(api.addr, "/demo/nothing", &[]), 404);
    // A request the API turns away was sent too.
    assert_eq!(
        get(
            api.addr,
            "/demo/shop/products/123",
            &["Host: shop.example.com"]
        ),
        403
    );
    let after = chrono::Utc::now();

    let lines = entries(&log);
    assert_eq!(lines.len(), 4);
    for line in &lines {
        let time = line["time"].as_str().expect("time");
        let time = chrono::DateTime::parse_from_rfc3339(time).expect("RFC 3339 time");
        // The log keeps milliseconds.
        assert!(
            (before.timestamp_millis()..=after.timestamp_millis())
                .contains(&time.timestamp_millis())
        );
    }
    let without_time: Vec<_> = lines
        .iter()
        .map(|line| {
            let mut line = line.as_object().expect("an object").clone();
            line.remove("time");
            Value::Object(line)
        })
        .collect();
    assert_eq!(
        without_time,
        [
            json!({
                "method": "GET",
                "path": product,
                "status": 200,
                "headers": ["connection", "host", "x-client"],
                "client": "round-1",
            }),
            json!({
                "method": "POST",
                "path": "/demo/shop/carts",
                "status": 201,
                "headers": ["connection", "content-length", "content-type", "host"],
                "body": { "productId": 123, "quantity": 2 },
            }),
            json!({
                "method": "GET",
                "path": "/demo/nothing",
                "status": 404,
                "headers": ["connection", "host"],
            }),
            json!({
                "method": "GET",
                "path": "/demo/shop/products/123",
                "status": 403,
                "headers": ["connection", "host"],
            }),
        ]
    );
}

#[test]
fn req19_criterion_1_the_body_is_kept_when_json_and_at_most_64_kib() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("requests.jsonl");
    let api = Running::logging_to(&log);
    let carts = "/demo/shop/carts";
    let json_type = "application/json; charset=utf-8";
    let at_limit = padded_body(BODY_LOG_LIMIT);
    let over_limit = padded_body(BODY_LOG_LIMIT + 1);
    assert_eq!(at_limit.len(), BODY_LOG_LIMIT);

    // Kept: at the limit, in chunks, and on a route that does not read it.
    assert_eq!(post(api.addr, carts, json_type, &at_limit), 201);
    assert_eq!(post_chunked(api.addr, carts, &at_limit), 201);
    assert_eq!(post(api.addr, "/demo/nothing", json_type, "null"), 404);
    let lines = entries(&log);
    let kept = Some(serde_json::from_str::<Value>(&at_limit).unwrap());
    assert_eq!(lines[0].get("body"), kept.as_ref());
    assert_eq!(lines[1].get("body"), kept.as_ref());
    assert_eq!(lines[2].get("body"), Some(&Value::Null));

    // Not kept, and answered as without the log: too long, not JSON, no body.
    assert_eq!(post(api.addr, carts, json_type, &over_limit), 201);
    assert_eq!(post_chunked(api.addr, carts, &over_limit), 201);
    assert_eq!(
        post_chunked(api.addr, carts, &padded_body(BODY_LIMIT + 1)),
        413
    );
    assert_eq!(post(api.addr, carts, json_type, "{\"productId\":"), 400);
    assert_eq!(post(api.addr, carts, "text/plain", "{}"), 415);
    assert_eq!(post(api.addr, carts, json_type, ""), 201);
    let lines = entries(&log);
    assert_eq!(lines.len(), 9);
    for line in &lines[3..] {
        assert_eq!(line.get("body"), None);
    }
}

#[test]
fn req19_criterion_1_the_log_is_appended_to() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("requests.jsonl");
    for count in 1..=2 {
        let mut option = std::ffi::OsString::from("--request-log=");
        option.push(&log);
        let api = Running::start(program().arg(option));
        assert_eq!(get(api.addr, "/demo/support/tickets/7", &[]), 200);
        drop(api);
        assert_eq!(entries(&log).len(), count);
    }
}

#[test]
fn req19_criterion_2_no_other_header_value_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("requests.jsonl");
    let api = Running::logging_to(&log);
    let headers = [
        "Authorization: Bearer value-one",
        "Cookie: session=value-two",
        "X-Api-Key: value-three",
        "X-Client-Secret: value-four",
        "X-Client: driver",
    ];
    assert_eq!(get(api.addr, "/demo/support/tickets/7", &headers), 200);

    let text = fs::read_to_string(&log).unwrap();
    for value in ["Bearer", "value-", "session=", &api.addr.to_string()] {
        assert!(!text.contains(value), "{value} in {text}");
    }
    let lines = entries(&log);
    assert_eq!(
        lines[0]["headers"],
        json!([
            "authorization",
            "connection",
            "cookie",
            "host",
            "x-api-key",
            "x-client",
            "x-client-secret",
        ])
    );
    assert_eq!(lines[0]["client"], "driver");
}

#[test]
fn req19_criterion_3_without_the_option_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let api = Running::start(program().current_dir(dir.path()));
    let expected = format!("API Forge demo API: http://{}/demo\n", api.addr);
    assert_eq!(get(api.addr, "/demo/support/tickets/7", &[]), 200);
    assert_eq!(
        post(api.addr, "/demo/shop/carts", "application/json", "{}"),
        201
    );
    assert_eq!(api.output(), (expected, String::new()));
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn req19_criterion_3_the_usage_names_the_option() {
    let run = |option: &str| {
        Command::new(env!("CARGO_BIN_EXE_apiforge-demo-api"))
            .arg(option)
            .output()
            .expect("run the demo API")
    };
    let help = run("--help");
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--request-log <file>"));

    let no_value = run("--request-log");
    assert_eq!(no_value.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&no_value.stderr).contains("--request-log needs a value"));
}

#[test]
fn req19_criterion_4_a_log_that_cannot_be_opened_stops_the_start() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing").join("requests.jsonl");
    // A missing folder, and a folder in place of the file.
    for log in [missing.as_path(), dir.path()] {
        let mut child = program()
            .arg("--request-log")
            .arg(log)
            .spawn()
            .expect("start the demo API");
        // A program that did start would never end: it is ended here instead.
        let mut line = String::new();
        BufReader::new(child.stdout.take().expect("piped stdout"))
            .read_line(&mut line)
            .expect("stdout");
        if !line.is_empty() {
            let _ = child.kill();
        }
        let Output { status, stderr, .. } = child.wait_with_output().expect("the ended demo API");
        assert_eq!(line, "");
        assert_eq!(status.code(), Some(1));
        let message = String::from_utf8_lossy(&stderr);
        assert!(
            message.contains("request log") && message.contains(&log.display().to_string()),
            "{message}"
        );
    }
}

/// `/dev/full` opens and refuses every write.
#[cfg(target_os = "linux")]
#[test]
fn req19_criterion_4_a_failed_write_never_fails_a_request() {
    let api = Running::logging_to(Path::new("/dev/full"));
    for _ in 0..3 {
        assert_eq!(get(api.addr, "/demo/support/tickets/7", &[]), 200);
        assert_eq!(
            post(api.addr, "/demo/shop/carts", "application/json", "{}"),
            201
        );
    }
    let (_, stderr) = api.output();
    // Said once, so that a round knows its count is incomplete.
    assert_eq!(stderr.matches("request log").count(), 1, "{stderr}");
}
