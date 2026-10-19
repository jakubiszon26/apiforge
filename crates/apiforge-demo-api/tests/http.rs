//! Every demo route over real HTTP on an ephemeral loopback port.

use apiforge_demo_api::{DemoServerHandle, MAX_ITEMS, spawn};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    time::Duration,
};

struct Reply {
    status: u16,
    headers: String,
    body: Value,
}

fn server() -> DemoServerHandle {
    spawn(SocketAddr::from(([127, 0, 0, 1], 0))).expect("start demo server")
}

/// One HTTP/1.1 exchange on a fresh connection; `extra` holds additional header lines.
fn exchange(
    addr: SocketAddr,
    method: &str,
    path: &str,
    body: Option<&str>,
    extra: &[&str],
) -> Reply {
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout");
    let mut request = format!("{method} {path} HTTP/1.1\r\nConnection: close\r\n");
    if !extra.iter().any(|line| line.starts_with("Host:")) {
        request.push_str(&format!("Host: {addr}\r\n"));
    }
    for line in extra {
        request.push_str(&format!("{line}\r\n"));
    }
    let body = body.unwrap_or_default();
    if !body.is_empty() {
        if !extra.iter().any(|line| line.starts_with("Content-Type:")) {
            request.push_str("Content-Type: application/json\r\n");
        }
        request.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    request.push_str("\r\n");
    request.push_str(body);
    stream.write_all(request.as_bytes()).expect("send");

    let mut raw = String::new();
    stream.read_to_string(&mut raw).expect("receive");
    let (head, payload) = raw.split_once("\r\n\r\n").expect("response head");
    let status = head
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .expect("status code");
    let body = if payload.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(payload).expect("JSON body")
    };
    Reply {
        status,
        headers: head.to_ascii_lowercase(),
        body,
    }
}

fn call(server: &DemoServerHandle, method: &str, path: &str, body: Option<Value>) -> Reply {
    let body = body.map(|body| body.to_string());
    exchange(server.local_addr(), method, path, body.as_deref(), &[])
}

fn expect_error(reply: &Reply, status: u16, message: &str) {
    assert_eq!(reply.status, status);
    assert_eq!(reply.body, json!({ "error": message }));
}

/// A cart response without its volatile id and timestamp.
fn stable(cart: &Value) -> Value {
    let mut cart = cart.clone();
    let fields = cart.as_object_mut().expect("cart object");
    assert!(fields.remove("id").is_some_and(|id| id.is_string()));
    assert!(fields.remove("createdAt").is_some_and(|at| at.is_u64()));
    cart
}

#[test]
fn products_are_served_with_their_promotion() {
    let server = server();
    let reply = call(&server, "GET", "/demo/shop/products/123", None);
    assert_eq!(reply.status, 200);
    assert_eq!(
        reply.body,
        json!({
            "id": 123,
            "name": "Whole bean coffee",
            "price": 100,
            "promotion": {
                "type": "fixed_price",
                "active": true,
                "price": 80,
                "inna_nazwa": "Autumn offer",
            },
        })
    );
    assert!(reply.headers.contains("cache-control: no-store"));
    let other = call(&server, "GET", "/demo/shop/products/124", None);
    assert_eq!(
        (
            other.body["id"].clone(),
            other.body["promotion"]["price"].clone()
        ),
        (json!(124), json!(90))
    );
    for missing in ["999", "abc", "12.5"] {
        let reply = call(
            &server,
            "GET",
            &format!("/demo/shop/products/{missing}"),
            None,
        );
        expect_error(&reply, 404, "Product not found");
    }
}

#[test]
fn carts_with_a_product_are_priced_and_deleted() {
    let server = server();
    let created = call(
        &server,
        "POST",
        "/demo/shop/carts",
        Some(json!({ "productId": 123, "quantity": 2 })),
    );
    assert_eq!(created.status, 201);
    assert_eq!(
        stable(&created.body),
        json!({ "productId": 123, "quantity": 2, "total": 160 })
    );
    let id = created.body["id"].as_str().expect("cart id").to_owned();
    let path = format!("/demo/shop/carts/{id}");

    let read = call(&server, "GET", &path, None);
    assert_eq!(read.status, 200);
    assert_eq!(read.body, created.body);

    let numeric_text = call(
        &server,
        "POST",
        "/demo/shop/carts",
        Some(json!({ "productId": "124", "quantity": 3 })),
    );
    assert_eq!(numeric_text.status, 201);
    assert_eq!(numeric_text.body["total"], json!(270));

    let deleted = call(&server, "DELETE", &path, None);
    assert_eq!(
        (deleted.status, deleted.body),
        (200, json!({ "deleted": true }))
    );
    let again = call(&server, "DELETE", &path, None);
    assert_eq!(
        (again.status, again.body),
        (404, json!({ "deleted": false }))
    );
    expect_error(&call(&server, "GET", &path, None), 404, "Cart not found");
}

#[test]
fn invalid_cart_lines_are_rejected() {
    let server = server();
    for body in [
        json!({ "productId": 999, "quantity": 1 }),
        json!({ "productId": 123, "quantity": 0 }),
        json!({ "productId": 123, "quantity": 101 }),
        json!({ "productId": 123, "quantity": 1.5 }),
        json!({ "productId": 123 }),
        json!({ "quantity": 1 }),
    ] {
        let reply = call(&server, "POST", "/demo/shop/carts", Some(body));
        expect_error(&reply, 400, "Invalid product or quantity");
    }
}

#[test]
fn empty_carts_accept_items_in_order() {
    let server = server();
    for body in [None, Some(json!({})), Some(json!([]))] {
        let created = call(&server, "POST", "/demo/shop/carts", body);
        assert_eq!(created.status, 201);
        assert_eq!(
            stable(&created.body),
            json!({ "items": [], "quantity": 0, "total": 0 })
        );
    }

    let created = call(&server, "POST", "/demo/shop/carts", Some(json!({})));
    let id = created.body["id"].as_str().expect("cart id").to_owned();
    let items = format!("/demo/shop/carts/{id}/items");
    let first = call(
        &server,
        "POST",
        &items,
        Some(json!({ "productId": 123, "quantity": 2 })),
    );
    assert_eq!(first.status, 200);
    assert_eq!(
        stable(&first.body),
        json!({
            "items": [{ "productId": 123, "quantity": 2, "price": 80 }],
            "quantity": 2,
            "total": 160,
        })
    );
    let second = call(
        &server,
        "POST",
        &items,
        Some(json!({ "productId": 124, "quantity": 1 })),
    );
    assert_eq!(
        (
            second.body["quantity"].clone(),
            second.body["total"].clone()
        ),
        (json!(3), json!(250))
    );
    let invalid = call(&server, "POST", &items, Some(json!({ "productId": 123 })));
    expect_error(&invalid, 400, "Invalid product or quantity");

    for _ in 2..MAX_ITEMS {
        let added = call(
            &server,
            "POST",
            &items,
            Some(json!({ "productId": 123, "quantity": 1 })),
        );
        assert_eq!(added.status, 200);
    }
    let full = call(
        &server,
        "POST",
        &items,
        Some(json!({ "productId": 123, "quantity": 1 })),
    );
    expect_error(&full, 400, "This cart does not accept more items");

    let missing = call(
        &server,
        "POST",
        "/demo/shop/carts/unknown/items",
        Some(json!({ "productId": 123, "quantity": 1 })),
    );
    expect_error(&missing, 404, "Create a cart first");
}

#[test]
fn carts_created_with_a_product_take_no_items() {
    let server = server();
    let created = call(
        &server,
        "POST",
        "/demo/shop/carts",
        Some(json!({ "productId": 123, "quantity": 1 })),
    );
    let id = created.body["id"].as_str().expect("cart id");
    let reply = call(
        &server,
        "POST",
        &format!("/demo/shop/carts/{id}/items"),
        Some(json!({ "productId": 123, "quantity": 1 })),
    );
    expect_error(&reply, 400, "This cart does not accept more items");
}

#[test]
fn the_support_ticket_is_served() {
    let server = server();
    let reply = call(&server, "GET", "/demo/support/tickets/7", None);
    assert_eq!(reply.status, 200);
    assert_eq!(
        reply.body,
        json!({ "id": 7, "title": "Cannot sign in", "status": "open", "priority": "high" })
    );
    let missing = call(&server, "GET", "/demo/support/tickets/8", None);
    expect_error(&missing, 404, "Ticket not found");
}

#[test]
fn malformed_and_foreign_requests_are_refused() {
    let server = server();
    let carts = "/demo/shop/carts";
    let invalid = exchange(
        server.local_addr(),
        "POST",
        carts,
        Some("{\"productId\":"),
        &[],
    );
    expect_error(&invalid, 400, "Invalid JSON request.");
    let scalar = exchange(server.local_addr(), "POST", carts, Some("5"), &[]);
    expect_error(&scalar, 400, "Invalid JSON request.");
    let text = exchange(
        server.local_addr(),
        "POST",
        carts,
        Some("productId=123"),
        &["Content-Type: application/x-www-form-urlencoded"],
    );
    expect_error(&text, 415, "Requests with a body require application/json.");

    let path = "/demo/support/tickets/7";
    let rebound = exchange(
        server.local_addr(),
        "GET",
        path,
        None,
        &["Host: attacker.example"],
    );
    expect_error(&rebound, 403, "Only a local host is allowed.");
    let foreign = exchange(
        server.local_addr(),
        "GET",
        path,
        None,
        &["Origin: http://attacker.example"],
    );
    expect_error(&foreign, 403, "Request origin is not allowed.");
    let cross_site = exchange(
        server.local_addr(),
        "GET",
        path,
        None,
        &["Sec-Fetch-Site: cross-site"],
    );
    expect_error(&cross_site, 403, "Request origin is not allowed.");

    let local = exchange(
        server.local_addr(),
        "GET",
        path,
        None,
        &["Origin: http://localhost:5173"],
    );
    assert_eq!(local.status, 200);
    assert!(
        local
            .headers
            .contains("access-control-allow-origin: http://localhost:5173")
    );
    let preflight = exchange(
        server.local_addr(),
        "OPTIONS",
        carts,
        None,
        &["Origin: http://127.0.0.1:5173"],
    );
    assert_eq!(preflight.status, 204);

    let unknown = call(&server, "GET", "/demo/shop/unknown", None);
    expect_error(&unknown, 404, "Endpoint not found");
}

#[test]
fn dropping_the_handle_stops_the_server() {
    let server = server();
    let addr = server.local_addr();
    assert!(addr.ip().is_loopback() && addr.port() != 0);
    assert_eq!(server.base_url(), format!("http://{addr}/demo"));
    assert_eq!(
        call(&server, "GET", "/demo/support/tickets/7", None).status,
        200
    );
    drop(server);
    assert!(TcpStream::connect_timeout(&addr, Duration::from_secs(1)).is_err());
}

#[test]
fn serve_runs_on_a_provided_listener() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .expect("runtime");
    let reply = runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("address");
        tokio::spawn(apiforge_demo_api::serve(listener));
        tokio::task::spawn_blocking(move || {
            exchange(addr, "GET", "/demo/support/tickets/7", None, &[])
        })
        .await
        .expect("client")
    });
    assert_eq!(reply.status, 200);
}

#[test]
fn stop_reports_a_clean_shutdown() {
    server().stop().expect("clean shutdown");
}
