//! A small local HTTP API the demo projects run against.
//!
//! It serves a demo shop (products and carts) and a support ticket under `/demo`,
//! with the routes, status codes and JSON shapes the demo projects expect.
//! State lives in memory and is bounded. The server only listens on loopback
//! addresses and rejects requests addressed to other hosts or sent by foreign web
//! pages, so a browser cannot be used to reach it through DNS rebinding.

mod store;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Request, State, rejection::BytesRejection},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::{Map, Value, json};
use std::{
    io,
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex, PoisonError},
    thread::JoinHandle,
    time::Instant,
};
use store::{CartError, Line, Store};
use tokio::{net::TcpListener, sync::oneshot};

pub use store::{CART_TTL, MAX_CARTS, MAX_ITEMS};

/// Port the demo projects expect the API on.
pub const DEFAULT_PORT: u16 = 4318;
/// Largest accepted request body.
pub const BODY_LIMIT: usize = 4 * 1024 * 1024;

/// The default listening address, `127.0.0.1:4318`.
pub fn default_addr() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, DEFAULT_PORT))
}

type Shared = Arc<Mutex<Store>>;

/// All demo routes under `/demo`, with fresh in-memory state.
pub fn router() -> Router {
    let state: Shared = Arc::default();
    let demo = Router::new()
        .route("/shop/products/{id}", get(get_product))
        .route("/shop/carts", post(create_cart))
        .route("/shop/carts/{id}/items", post(add_item))
        .route("/shop/carts/{id}", get(get_cart).delete(delete_cart))
        .route("/support/tickets/{id}", get(get_ticket))
        .with_state(state);
    Router::new()
        .nest("/demo", demo)
        .fallback(|| async { error(StatusCode::NOT_FOUND, "Endpoint not found") })
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .layer(middleware::from_fn(guard))
}

/// Serves the demo API on a loopback listener until the future is dropped.
pub async fn serve(listener: TcpListener) -> io::Result<()> {
    ensure_loopback(listener.local_addr()?)?;
    axum::serve(listener, router()).await
}

/// A demo server running on a background thread. Dropping the handle stops it.
#[derive(Debug)]
pub struct DemoServerHandle {
    local_addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<io::Result<()>>>,
}

impl DemoServerHandle {
    /// The bound address, with the actual port when port 0 was requested.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Base URL of the API, e.g. `http://127.0.0.1:4318/demo`.
    pub fn base_url(&self) -> String {
        format!("http://{}/demo", self.local_addr)
    }

    /// Stops the server, waits for open requests to finish and reports how it ended.
    pub fn stop(mut self) -> io::Result<()> {
        self.shut_down()
    }

    fn shut_down(&mut self) -> io::Result<()> {
        if let Some(shutdown) = self.shutdown.take() {
            // The server may already have stopped on its own; its result is below.
            let _ = shutdown.send(());
        }
        match self.thread.take().map(JoinHandle::join) {
            Some(Ok(result)) => result,
            Some(Err(_)) => Err(io::Error::other("demo server thread panicked")),
            None => Ok(()),
        }
    }
}

impl Drop for DemoServerHandle {
    fn drop(&mut self) {
        // Errors cannot be reported from drop; `stop` returns them.
        let _ = self.shut_down();
    }
}

/// Starts the server on a background thread with its own runtime; dropping the handle
/// stops it. The address must be a loopback address; port 0 picks a free port.
pub fn spawn(addr: SocketAddr) -> io::Result<DemoServerHandle> {
    ensure_loopback(addr)?;
    let listener = std::net::TcpListener::bind(addr)?;
    listener.set_nonblocking(true)?;
    let local_addr = listener.local_addr()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()?;
    let (shutdown, stopped) = oneshot::channel::<()>();
    let thread = std::thread::Builder::new()
        .name("apiforge-demo-api".into())
        .spawn(move || {
            runtime.block_on(async move {
                let listener = TcpListener::from_std(listener)?;
                axum::serve(listener, router())
                    .with_graceful_shutdown(async {
                        // A dropped sender also means stop.
                        let _ = stopped.await;
                    })
                    .await
            })
        })?;
    Ok(DemoServerHandle {
        local_addr,
        shutdown: Some(shutdown),
        thread: Some(thread),
    })
}

fn ensure_loopback(addr: SocketAddr) -> io::Result<()> {
    if addr.ip().is_loopback() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("the demo API only listens on loopback addresses, not {addr}"),
        ))
    }
}

fn error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

/// Rejects requests for other hosts and from foreign origins, answers preflight
/// requests and requires JSON for request bodies.
async fn guard(request: Request, next: Next) -> Response {
    let headers = request.headers();
    if !headers
        .get(header::HOST)
        .and_then(|host| host.to_str().ok())
        .is_some_and(is_local_host)
    {
        return error(StatusCode::FORBIDDEN, "Only a local host is allowed.");
    }
    let origin = headers.get(header::ORIGIN).cloned();
    if let Some(origin) = &origin
        && !origin.to_str().is_ok_and(is_local_origin)
    {
        return error(StatusCode::FORBIDDEN, "Request origin is not allowed.");
    }
    let cross_site = headers
        .get("sec-fetch-site")
        .is_some_and(|site| site == "cross-site");
    if cross_site && origin.is_none() {
        return error(StatusCode::FORBIDDEN, "Request origin is not allowed.");
    }
    let mut response = if request.method() == Method::OPTIONS {
        StatusCode::NO_CONTENT.into_response()
    } else if mutates(request.method()) && has_body(headers) && !is_json(headers) {
        error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Requests with a body require application/json.",
        )
    } else {
        next.run(request).await
    };
    let response_headers = response.headers_mut();
    if let Some(origin) = origin {
        response_headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        response_headers.insert(header::VARY, HeaderValue::from_static("Origin"));
        response_headers.insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static("GET,POST,DELETE,OPTIONS"),
        );
        response_headers.insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static("Content-Type"),
        );
    }
    response_headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// `localhost`, `127.0.0.1` or `[::1]`, with an optional port.
fn is_local_host(host: &str) -> bool {
    let name = match host.rsplit_once(':') {
        Some((name, port)) if !name.ends_with(':') && port.bytes().all(|b| b.is_ascii_digit()) => {
            name
        }
        _ => host,
    };
    matches!(name, "localhost" | "127.0.0.1" | "[::1]")
}

fn is_local_origin(origin: &str) -> bool {
    origin.strip_prefix("http://").is_some_and(is_local_host)
}

fn mutates(method: &Method) -> bool {
    method != Method::GET && method != Method::HEAD
}

fn has_body(headers: &HeaderMap) -> bool {
    let length = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|length| length.to_str().ok())
        .and_then(|length| length.parse::<u64>().ok());
    length.is_some_and(|length| length > 0) || headers.contains_key(header::TRANSFER_ENCODING)
}

fn is_json(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"))
}

/// Why a request body was refused.
#[derive(Clone, Copy, Debug)]
enum BodyError {
    TooLarge,
    Invalid,
}

impl IntoResponse for BodyError {
    fn into_response(self) -> Response {
        match self {
            Self::TooLarge => error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "Request exceeds the 4 MiB limit.",
            ),
            Self::Invalid => error(StatusCode::BAD_REQUEST, "Invalid JSON request."),
        }
    }
}

/// The fields of a JSON request body; an empty body has none. Like the reference,
/// only an object or array is a valid body, and an array carries no fields.
fn body_fields(body: Result<Bytes, BytesRejection>) -> Result<Map<String, Value>, BodyError> {
    let body = body.map_err(|rejection| {
        if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
            BodyError::TooLarge
        } else {
            BodyError::Invalid
        }
    })?;
    if body.is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_slice(&body) {
        Ok(Value::Object(fields)) => Ok(fields),
        Ok(Value::Array(_)) => Ok(Map::new()),
        _ => Err(BodyError::Invalid),
    }
}

fn lock(state: &Shared) -> std::sync::MutexGuard<'_, Store> {
    // The store stays consistent even if a handler panicked mid-request.
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

fn cart_error(error_kind: CartError) -> Response {
    match error_kind {
        CartError::InvalidLine => error(StatusCode::BAD_REQUEST, "Invalid product or quantity"),
        CartError::TooManyCarts => error(StatusCode::TOO_MANY_REQUESTS, "Too many demo carts"),
        CartError::NotFound => error(StatusCode::NOT_FOUND, "Create a cart first"),
        CartError::ItemsNotSupported => error(
            StatusCode::BAD_REQUEST,
            "This cart does not accept more items",
        ),
    }
}

async fn get_product(Path(id): Path<String>) -> Response {
    let id = store::js_number_from_str(&id);
    [123, 124]
        .into_iter()
        .find(|&known| f64::from(known) == id)
        .and_then(store::product)
        .map_or_else(
            || error(StatusCode::NOT_FOUND, "Product not found"),
            |product| Json(product).into_response(),
        )
}

async fn create_cart(State(state): State<Shared>, body: Result<Bytes, BytesRejection>) -> Response {
    let fields = match body_fields(body) {
        Ok(fields) => fields,
        Err(refused) => return refused.into_response(),
    };
    let empty = !fields.contains_key("productId") && !fields.contains_key("quantity");
    let line = match (empty, Line::parse(&fields)) {
        (true, _) => None,
        (false, Some(line)) => Some(line),
        (false, None) => return cart_error(CartError::InvalidLine),
    };
    match lock(&state).create(line, Instant::now()) {
        Ok(cart) => (StatusCode::CREATED, Json(cart)).into_response(),
        Err(error_kind) => cart_error(error_kind),
    }
}

/// Creating a cart and adding items separately makes the ordered workflow observable.
async fn add_item(
    State(state): State<Shared>,
    Path(id): Path<String>,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let fields = match body_fields(body) {
        Ok(fields) => fields,
        Err(refused) => return refused.into_response(),
    };
    let now = Instant::now();
    if lock(&state).get(&id, now).is_none() {
        return cart_error(CartError::NotFound);
    }
    let Some(line) = Line::parse(&fields) else {
        return cart_error(CartError::InvalidLine);
    };
    match lock(&state).add_item(&id, line, now) {
        Ok(cart) => Json(cart).into_response(),
        Err(error_kind) => cart_error(error_kind),
    }
}

async fn get_cart(State(state): State<Shared>, Path(id): Path<String>) -> Response {
    match lock(&state).get(&id, Instant::now()) {
        Some(cart) => Json(cart).into_response(),
        None => error(StatusCode::NOT_FOUND, "Cart not found"),
    }
}

async fn delete_cart(State(state): State<Shared>, Path(id): Path<String>) -> Response {
    let deleted = lock(&state).delete(&id, Instant::now());
    let status = if deleted {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    };
    (status, Json(json!({ "deleted": deleted }))).into_response()
}

async fn get_ticket(Path(id): Path<String>) -> Response {
    store::ticket(store::js_number_from_str(&id)).map_or_else(
        || error(StatusCode::NOT_FOUND, "Ticket not found"),
        |ticket| Json(ticket).into_response(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_loopback_hosts_are_local() {
        for host in [
            "localhost",
            "127.0.0.1:4318",
            "localhost:5173",
            "[::1]:80",
            "[::1]",
        ] {
            assert!(is_local_host(host), "{host}");
        }
        for host in [
            "",
            "example.com",
            "127.0.0.1.example.com",
            "localhost:x",
            "10.0.0.1",
        ] {
            assert!(!is_local_host(host), "{host}");
        }
        assert!(is_local_origin("http://localhost:5173"));
        assert!(!is_local_origin("https://localhost:5173"));
        assert!(!is_local_origin("http://evil.example"));
    }

    #[test]
    fn spawn_refuses_other_interfaces() {
        let error = spawn(SocketAddr::from(([0, 0, 0, 0], 0))).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
}
