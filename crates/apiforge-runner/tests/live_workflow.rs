//! End-to-end checks against a real loopback HTTP server, without Express or a
//! simulated transport. Reuse the shipped workflow so its control and data edges
//! are exercised together with request serialization and cleanup.

use apiforge_core::{
    format::Project,
    run::{Phase, RunMode, RunResult, RunStatus, StepStatus},
};
use apiforge_runner::{HttpTransport, RunOptions, RunRequest, run_scenario};
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

#[derive(Clone, Copy)]
enum Behavior {
    Success,
    MissingProduct,
    RedirectOnAdd,
    CancelOnAdd,
}

#[derive(Clone, Debug, PartialEq)]
struct Received {
    method: String,
    path: String,
    body: Option<Value>,
}

struct Shop {
    base_url: String,
    received: Arc<Mutex<Vec<Received>>>,
    task: JoinHandle<()>,
}

impl Shop {
    async fn start(behavior: Behavior, cancel: CancellationToken) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let received = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&received);
        let task = tokio::spawn(async move {
            // Separate connections let cleanup reach the server while a cancelled
            // main request is still awaiting a response. Dropping the set aborts
            // all handlers, including the deliberately stalled cancellation case.
            let mut handlers = JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let (stream, _) = accepted.unwrap();
                        let log = Arc::clone(&log);
                        let cancel = cancel.clone();
                        handlers.spawn(async move {
                            serve(stream, behavior, log, cancel).await;
                        });
                    }
                    Some(result) = handlers.join_next(), if !handlers.is_empty() => {
                        result.expect("local HTTP handler must finish successfully");
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

    fn requests(&self) -> Vec<Received> {
        self.received.lock().unwrap().clone()
    }
}

impl Drop for Shop {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Minimal HTTP/1.1 fixture parser. Read complete headers and Content-Length body
/// rather than assuming a TCP read corresponds to an entire request.
async fn read_request(stream: &mut TcpStream) -> Received {
    let mut bytes = Vec::new();
    let header_end = loop {
        if let Some(index) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            break index + 4;
        }
        assert!(bytes.len() < 65_536, "fixture request headers too large");
        let mut chunk = [0; 1024];
        let count = stream.read(&mut chunk).await.unwrap();
        assert_ne!(count, 0, "request closed before its headers");
        bytes.extend_from_slice(&chunk[..count]);
    };
    let headers = std::str::from_utf8(&bytes[..header_end]).unwrap();
    let mut first = headers.lines().next().unwrap().split_whitespace();
    let method = first.next().unwrap().to_owned();
    let path = first.next().unwrap().to_owned();
    let content_length = headers
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .map(|(_, length)| length.trim().parse::<usize>().unwrap())
        .unwrap_or(0);
    assert!(content_length <= 65_536, "fixture request body too large");
    while bytes.len() < header_end + content_length {
        let mut chunk = [0; 1024];
        let count = stream.read(&mut chunk).await.unwrap();
        assert_ne!(count, 0, "request closed before its body");
        bytes.extend_from_slice(&chunk[..count]);
    }
    let body = (content_length > 0)
        .then(|| serde_json::from_slice(&bytes[header_end..header_end + content_length]).unwrap());
    Received { method, path, body }
}

async fn serve(
    mut stream: TcpStream,
    behavior: Behavior,
    log: Arc<Mutex<Vec<Received>>>,
    cancel: CancellationToken,
) {
    let request = timeout(Duration::from_secs(3), read_request(&mut stream))
        .await
        .expect("local HTTP request timed out");
    log.lock().unwrap().push(request.clone());
    let (status, body) = match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/products/123") if matches!(behavior, Behavior::MissingProduct) => {
            (404, json!({"error": "product missing"}))
        }
        ("GET", "/products/123") => (200, json!({"id": 987, "promotion": {"price": 13}})),
        ("POST", "/carts") => (201, json!({"id": "live-cart-7"})),
        ("POST", "/carts/live-cart-7/items") => match behavior {
            // Redirects are transport errors. The engine must neither follow the
            // redirect nor retry a non-idempotent operation before cleanup.
            Behavior::RedirectOnAdd => (302, json!({"error": "redirect"})),
            Behavior::CancelOnAdd => {
                cancel.cancel();
                // No response can race cancellation; the run must abandon this
                // request and use its independent cleanup cancellation token.
                std::future::pending::<()>().await;
                unreachable!();
            }
            _ => (200, json!({"total": 26, "quantity": 2})),
        },
        ("DELETE", "/carts/live-cart-7") => (200, json!({"deleted": true})),
        _ => (400, json!({"error": "unexpected request"})),
    };
    let body = body.to_string();
    let response = format!(
        "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nLocation: /must-not-follow\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await.unwrap();
    stream.shutdown().await.unwrap();
}

async fn run(shop: &Shop, cancel: CancellationToken) -> RunResult {
    let mut project: Project =
        serde_json::from_slice(include_bytes!("../../../fixtures/projects/workflow.json")).unwrap();
    project.environment.base_url.clone_from(&shop.base_url);
    let request = RunRequest::new(project, "workflow-cart", RunMode::Live);
    timeout(
        Duration::from_secs(5),
        run_scenario(
            request,
            RunOptions::new(HttpTransport::new().unwrap()),
            cancel,
            |_| {},
        ),
    )
    .await
    .expect("workflow and cleanup must finish within the test budget")
    .expect("shipped workflow is valid")
}

fn assert_request_sequence(shop: &Shop, expected: &[(&str, &str)]) {
    let actual = shop.requests();
    let sequence: Vec<_> = actual
        .iter()
        .map(|request| (request.method.as_str(), request.path.as_str()))
        .collect();
    assert_eq!(sequence, expected, "unexpected requests, order or retries");
}

#[tokio::test]
async fn prices_then_cart_then_items_use_live_response_values_and_cleanup() {
    let cancel = CancellationToken::new();
    let shop = Shop::start(Behavior::Success, cancel.clone()).await;
    let report = run(&shop, cancel).await;
    assert_eq!(report.status, RunStatus::Passed);
    assert_eq!(
        report.step("flow-else").unwrap().status,
        StepStatus::Skipped
    );
    assert_eq!(
        report.step("flow-check").unwrap().status,
        StepStatus::Passed
    );
    assert_eq!(report.step("flow-cleanup").unwrap().phase, Phase::Cleanup);
    assert_request_sequence(
        &shop,
        &[
            ("GET", "/products/123"),
            ("POST", "/carts"),
            ("POST", "/carts/live-cart-7/items"),
            ("DELETE", "/carts/live-cart-7"),
        ],
    );
    assert_eq!(shop.requests()[1].body, Some(json!({})));
    assert_eq!(
        shop.requests()[2].body,
        Some(json!({"productId": 987, "quantity": 2})),
        "the payload must come from HTTP responses, not simulation samples"
    );
    assert_eq!(
        report.step("flow-check").unwrap().inputs["expected"],
        json!(26)
    );
}

#[tokio::test]
async fn a_non_200_response_takes_else_without_creating_or_deleting_a_cart() {
    let cancel = CancellationToken::new();
    let shop = Shop::start(Behavior::MissingProduct, cancel.clone()).await;
    let report = run(&shop, cancel).await;
    assert_eq!(report.status, RunStatus::Failed);
    assert_eq!(
        report.step("flow-if").unwrap().outputs["passed"],
        json!(false)
    );
    assert_eq!(report.step("flow-else").unwrap().status, StepStatus::Failed);
    for id in ["flow-cart", "flow-add", "flow-check", "flow-cleanup"] {
        assert_eq!(report.step(id).unwrap().status, StepStatus::Skipped, "{id}");
    }
    assert_request_sequence(&shop, &[("GET", "/products/123")]);
}

#[tokio::test]
async fn an_add_error_or_cancellation_still_deletes_the_created_cart_once() {
    for (behavior, status, step_status) in [
        (
            Behavior::RedirectOnAdd,
            RunStatus::Failed,
            StepStatus::Error,
        ),
        (
            Behavior::CancelOnAdd,
            RunStatus::Cancelled,
            StepStatus::Cancelled,
        ),
    ] {
        let cancel = CancellationToken::new();
        let shop = Shop::start(behavior, cancel.clone()).await;
        let report = run(&shop, cancel).await;
        assert_eq!(report.status, status);
        assert_eq!(report.step("flow-add").unwrap().status, step_status);
        let cleanup = report.step("flow-cleanup").unwrap();
        assert_eq!(cleanup.status, StepStatus::Passed);
        assert_eq!(cleanup.phase, Phase::Cleanup);
        assert_request_sequence(
            &shop,
            &[
                ("GET", "/products/123"),
                ("POST", "/carts"),
                ("POST", "/carts/live-cart-7/items"),
                ("DELETE", "/carts/live-cart-7"),
            ],
        );
    }
}
