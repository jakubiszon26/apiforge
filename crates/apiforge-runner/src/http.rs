//! HTTP transport of live runs.
//!
//! Every request is sent exactly once: redirects are reported as errors, there are
//! no automatic retries and connections are not reused, so a request that creates
//! a resource can never be repeated behind the user's back. Proxies from the
//! environment are ignored and responses are not decompressed implicitly.

use apiforge_core::{js, model::HttpMethod};
use indexmap::IndexMap;
use reqwest::{
    Method, StatusCode,
    header::{CONTENT_LENGTH, HeaderMap, HeaderName, HeaderValue},
};
use serde_json::Value;
use thiserror::Error;
use url::Url;

use crate::support::byte_size;

/// A shareable HTTP client configured for test execution.
#[derive(Clone, Debug)]
pub struct HttpTransport {
    client: reqwest::Client,
}

impl HttpTransport {
    pub fn new() -> Result<Self, TransportError> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            // A pooled connection that turns out to be closed may be retried by the
            // connection layer; fresh connections rule that out.
            .pool_max_idle_per_host(0)
            .no_proxy()
            .http1_only()
            .user_agent(concat!("apiforge-runner/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| TransportError(error_chain(&error)))?;
        Ok(Self { client })
    }

    /// Sends a request and returns the response head; the body is read separately.
    pub(crate) async fn send(
        &self,
        method: HttpMethod,
        url: Url,
        headers: &IndexMap<String, String>,
        body: Option<String>,
    ) -> Result<reqwest::Response, HttpError> {
        let mut map = HeaderMap::new();
        for (name, value) in headers {
            let header = HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| HttpError::InvalidHeader(name.clone()))?;
            let value =
                header_value(value).ok_or_else(|| HttpError::InvalidHeader(name.clone()))?;
            map.append(header, value);
        }
        let method = Method::from_bytes(method_name(method).as_bytes())
            .map_err(|_| HttpError::InvalidHeader(String::new()))?;
        let secure = url.scheme() == "https";
        let mut builder = self.client.request(method, url).headers(map);
        if let Some(body) = body {
            builder = builder.body(body);
        }
        let response = builder
            .send()
            .await
            .map_err(|error| HttpError::Transport(transport_failure(error, secure)))?;
        if matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308) {
            return Err(HttpError::Redirect(response.status()));
        }
        Ok(response)
    }
}

/// The HTTP client could not be created, e.g. because TLS roots failed to load.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("cannot create the HTTP client: {0}")]
pub struct TransportError(String);

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub(crate) enum HttpError {
    #[error("Invalid request header: {0}.")]
    InvalidHeader(String),
    #[error("{0}")]
    Transport(TransportFailure),
    #[error("Redirects are not followed (HTTP {0}).")]
    Redirect(StatusCode),
    #[error("The response exceeds the {} limit.", byte_size(*.0))]
    TooLarge(usize),
    #[error("The response JSON exceeds the nesting limit of {0} levels.")]
    TooDeep(usize),
    #[error("The response JSON contains a number outside the finite range.")]
    NonFinite,
}

impl HttpError {
    /// The response status, where the error came with one.
    pub(crate) fn response_status(&self) -> Option<u16> {
        match self {
            Self::Redirect(status) => Some(status.as_u16()),
            // A body or decoding limit is reported without the status that carried
            // it; the entry still records that the request was performed.
            _ => None,
        }
    }

    /// Whether the tested API may have performed the request despite the error.
    pub(crate) fn reached_the_api(&self) -> Reached {
        match self {
            // A header the client rejects is never put on the wire.
            Self::InvalidHeader(_) => Reached::No,
            Self::Transport(failure) => failure.kind.reached_the_api(),
            // A status or a body means the request was answered, so it happened.
            Self::Redirect(_) | Self::TooLarge(_) | Self::TooDeep(_) | Self::NonFinite => {
                Reached::Yes
            }
        }
    }
}

/// How far a request got before it failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reached {
    /// The request never left this process; the API cannot have performed it.
    No,
    /// Bytes were sent and the outcome is unknown; the API may have performed it.
    Maybe,
    /// The API answered, so it performed the request.
    Yes,
}

/// Why a live request failed, in the terms a report needs: a category the reader can
/// act on, and whether the tested API may already have performed the request.
///
/// `reqwest` groups name resolution, TLS and TCP into one connect error, so the
/// category is taken from the cause chain. The chain's wording belongs to the HTTP
/// and TLS stacks and is matched case-insensitively against the markers below; an
/// unrecognised connect failure stays [`FailureKind::Connect`], which is the
/// conservative answer rather than a guess.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("Request failed: {kind} ({detail}).", kind = self.kind.label())]
pub(crate) struct TransportFailure {
    pub(crate) kind: FailureKind,
    /// The error with its causes, without the URL.
    pub(crate) detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FailureKind {
    /// The host name could not be resolved.
    Dns,
    /// The TLS handshake or certificate validation failed.
    Tls,
    /// The connection could not be established, e.g. refused or unreachable.
    Connect,
    /// No answer within the request's timeout.
    Timeout,
    /// The request body could not be sent, or the response body not read.
    Body,
    /// The response could not be decoded.
    Decode,
    /// The request could not be built or started.
    Request,
    /// None of the above; the transport reported no category.
    Other,
}

impl FailureKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Dns => "the host name could not be resolved",
            Self::Tls => "the TLS connection failed",
            Self::Connect => "the connection failed",
            Self::Timeout => "no response in time",
            Self::Body => "the body could not be transferred",
            Self::Decode => "the response could not be decoded",
            Self::Request => "the request could not be sent",
            Self::Other => "transport error",
        }
    }

    fn reached_the_api(self) -> Reached {
        match self {
            // Resolution, TLS and connection setup all precede the request itself.
            Self::Dns | Self::Tls | Self::Connect | Self::Request => Reached::No,
            // The request was on its way; only the API knows whether it ran.
            Self::Timeout | Self::Body | Self::Decode | Self::Other => Reached::Maybe,
        }
    }
}

pub(crate) fn method_name(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Head => "HEAD",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
        HttpMethod::Options => "OPTIONS",
    }
}

/// A header value as `fetch` sends it: surrounding HTTP whitespace removed and
/// characters up to U+00FF sent as single bytes. Anything else is invalid.
fn header_value(text: &str) -> Option<HeaderValue> {
    let trimmed = text.trim_matches([' ', '\t', '\n', '\r']);
    let bytes = trimmed
        .chars()
        .map(|character| u8::try_from(u32::from(character)).ok())
        .collect::<Option<Vec<u8>>>()?;
    HeaderValue::from_bytes(&bytes).ok()
}

/// Response headers as a `fetch` `Headers` object enumerates them: lowercase names in
/// sorted order, repeated fields joined with `, ` (only the last `set-cookie` kept),
/// bytes decoded as Latin-1.
pub(crate) fn response_headers(response: &reqwest::Response) -> IndexMap<String, String> {
    let mut names: Vec<&HeaderName> = response.headers().keys().collect();
    names.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    let latin1 = |value: &HeaderValue| {
        value
            .as_bytes()
            .iter()
            .map(|&byte| char::from(byte))
            .collect::<String>()
    };
    let headers = names.into_iter().filter_map(|name| {
        let values = response.headers().get_all(name);
        let text = if name.as_str() == "set-cookie" {
            values.iter().next_back().map(latin1)?
        } else {
            values.iter().map(latin1).collect::<Vec<_>>().join(", ")
        };
        Some((name.as_str().to_owned(), text))
    });
    js::js_key_order(headers.collect::<Vec<_>>())
        .into_iter()
        .collect()
}

/// Reads the body, failing as soon as it exceeds `limit` bytes, whether announced by
/// `Content-Length` or observed while streaming.
pub(crate) async fn read_body(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, HttpError> {
    let announced = response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<f64>().ok());
    if announced.is_some_and(|length| length > limit as f64) {
        return Err(HttpError::TooLarge(limit));
    }
    let secure = response.url().scheme() == "https";
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| HttpError::Transport(transport_failure(error, secure)))?
    {
        if body.len() + chunk.len() > limit {
            return Err(HttpError::TooLarge(limit));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Decodes a body as JSON when it parses, otherwise keeps it as text.
///
/// Numbers are normalised to JavaScript doubles and object keys to JavaScript
/// enumeration order, as `JSON.parse` would produce them.
pub(crate) fn decode_body(bytes: &[u8], max_depth: usize) -> Result<Value, HttpError> {
    let text = String::from_utf8_lossy(bytes);
    match serde_json::from_str::<Value>(&text) {
        Ok(value) => normalize(value, 0, max_depth),
        // JSON.parse turns these into Infinity, which the reference then rejects.
        Err(error) if error.to_string().starts_with("number out of range") => {
            Err(HttpError::NonFinite)
        }
        // serde_json's own recursion limit (128) is above the response limit.
        Err(error) if error.to_string().starts_with("recursion limit exceeded") => {
            Err(HttpError::TooDeep(max_depth))
        }
        Err(_) => Ok(Value::String(text.into_owned())),
    }
}

fn normalize(value: Value, depth: usize, max_depth: usize) -> Result<Value, HttpError> {
    if depth > max_depth {
        return Err(HttpError::TooDeep(max_depth));
    }
    Ok(match value {
        Value::Number(number) => {
            js::number_value(js::number_as_f64(&number)).ok_or(HttpError::NonFinite)?
        }
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| normalize(item, depth + 1, max_depth))
                .collect::<Result<_, _>>()?,
        ),
        Value::Object(fields) => Value::Object(
            js::js_key_order(fields)
                .into_iter()
                .map(|(key, item)| Ok((key, normalize(item, depth + 1, max_depth)?)))
                .collect::<Result<_, HttpError>>()?,
        ),
        other => other,
    })
}

/// Classifies a `reqwest::Error` and keeps its full cause chain as the detail.
///
/// The URL is stripped before anything else, so neither the detail nor the category
/// can be decided by a word that happens to appear in the address; `secure` carries
/// the one thing the address does decide, namely whether TLS was involved at all.
fn transport_failure(error: reqwest::Error, secure: bool) -> TransportFailure {
    let error = error.without_url();
    let detail = error_chain(&error);
    // A connect error covers resolution, TLS and TCP; the chain tells them apart.
    // The timeout check comes first because a connect timeout is reported as both
    // and the stage it stopped at is then unknown.
    let kind = if error.is_timeout() {
        FailureKind::Timeout
    } else if error.is_connect() {
        connect_kind(&detail, secure)
    } else if error.is_body() {
        FailureKind::Body
    } else if error.is_decode() {
        FailureKind::Decode
    } else if error.is_request() {
        FailureKind::Request
    } else {
        FailureKind::Other
    };
    TransportFailure { kind, detail }
}

/// Markers of a failed name resolution in the cause chain, lowercase.
const DNS_MARKERS: [&str; 7] = [
    "dns error",
    "failed to lookup address",
    "name or service not known",
    "nodename nor servname",
    "temporary failure in name resolution",
    "no such host",
    "name resolution",
];

/// Markers of a connection that was never established, lowercase.
const TCP_MARKERS: [&str; 8] = [
    "tcp connect error",
    "connection refused",
    "connection reset",
    "connection timed out",
    "network is unreachable",
    "no route to host",
    "host is unreachable",
    "address not available",
];

/// Markers of a failed TLS handshake or certificate check, lowercase.
const TLS_MARKERS: [&str; 6] = [
    "tls",
    "certificate",
    "handshake",
    "webpki",
    "ssl",
    "unknown issuer",
];

/// Splits a connect failure into resolution, TLS and connection setup.
///
/// Resolution is checked first: a name that cannot be resolved never reaches TLS,
/// while a TLS error about a host name can mention both. A connection that was
/// never established comes next. What remains, on an `https` address, happened
/// while the secure channel was being set up, whatever the TLS stack called it —
/// its wordings (`received corrupt message`, `peer misbehaved`, a numbered alert)
/// share no reliable keyword. On an `http` address the same remainder stays a
/// connection failure rather than becoming a guess.
fn connect_kind(chain: &str, secure: bool) -> FailureKind {
    let chain = chain.to_ascii_lowercase();
    let contains = |markers: &[&str]| markers.iter().any(|marker| chain.contains(marker));
    if contains(&DNS_MARKERS) {
        FailureKind::Dns
    } else if contains(&TCP_MARKERS) {
        FailureKind::Connect
    } else if contains(&TLS_MARKERS) || secure {
        FailureKind::Tls
    } else {
        FailureKind::Connect
    }
}

/// An error with its causes, e.g. `error sending request: connection refused`.
fn error_chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        let cause_text = cause.to_string();
        if !text.contains(&cause_text) {
            text.push_str(": ");
            text.push_str(&cause_text);
        }
        source = cause.source();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bodies_decode_like_json_parse() {
        assert_eq!(
            decode_body(br#"{"b": 1.0, "2": [3]}"#, 100),
            Ok(json!({"2": [3], "b": 1}))
        );
        assert_eq!(
            serde_json::to_string(&decode_body(br#"{"b": 1.0, "2": [3]}"#, 100).unwrap()).unwrap(),
            r#"{"2":[3],"b":1}"#
        );
        assert_eq!(decode_body(b"plain text", 100), Ok(json!("plain text")));
        assert_eq!(decode_body(b"", 100), Ok(json!("")));
        assert_eq!(
            decode_body(b"{\"a\":1e999}", 100),
            Err(HttpError::NonFinite)
        );
        assert_eq!(
            decode_body(b"18446744073709551617", 100),
            Ok(json!(18_446_744_073_709_552_000.0))
        );
    }

    #[test]
    fn nesting_is_limited() {
        let nested = |levels: usize| format!("{}1{}", "[".repeat(levels), "]".repeat(levels));
        assert!(decode_body(nested(100).as_bytes(), 100).is_ok());
        assert_eq!(
            decode_body(nested(101).as_bytes(), 100),
            Err(HttpError::TooDeep(100))
        );
        assert_eq!(
            decode_body(nested(500).as_bytes(), 100),
            Err(HttpError::TooDeep(100))
        );
    }

    #[test]
    fn connect_failures_are_split_by_their_cause_chain() {
        // Wordings observed from hyper-util, rustls and the operating system.
        for secure in [false, true] {
            assert_eq!(
                connect_kind(
                    "error trying to connect: dns error: failed to lookup address information: Name or service not known",
                    secure
                ),
                FailureKind::Dns
            );
            assert_eq!(
                connect_kind(
                    "error trying to connect: Temporary failure in name resolution",
                    secure
                ),
                FailureKind::Dns
            );
            assert_eq!(
                connect_kind(
                    "error trying to connect: tcp connect error: Connection refused (os error 111)",
                    secure
                ),
                FailureKind::Connect
            );
            assert_eq!(
                connect_kind(
                    "error trying to connect: invalid peer certificate: UnknownIssuer",
                    secure
                ),
                FailureKind::Tls
            );
        }
        // Wordings with no keyword at all: TLS only where TLS was in use.
        let corrupt = "error sending request: client error (Connect): received corrupt message";
        assert_eq!(connect_kind(corrupt, true), FailureKind::Tls);
        assert_eq!(connect_kind(corrupt, false), FailureKind::Connect);
    }

    #[test]
    fn a_failure_decides_whether_the_api_may_have_performed_the_request() {
        let failure = |kind| {
            HttpError::Transport(TransportFailure {
                kind,
                detail: String::new(),
            })
        };
        for kind in [
            FailureKind::Dns,
            FailureKind::Tls,
            FailureKind::Connect,
            FailureKind::Request,
        ] {
            assert_eq!(failure(kind).reached_the_api(), Reached::No, "{kind:?}");
        }
        for kind in [
            FailureKind::Timeout,
            FailureKind::Body,
            FailureKind::Decode,
            FailureKind::Other,
        ] {
            assert_eq!(failure(kind).reached_the_api(), Reached::Maybe, "{kind:?}");
        }
        assert_eq!(
            HttpError::InvalidHeader("a b".into()).reached_the_api(),
            Reached::No
        );
        for error in [
            HttpError::Redirect(StatusCode::FOUND),
            HttpError::TooLarge(1),
            HttpError::TooDeep(1),
            HttpError::NonFinite,
        ] {
            assert_eq!(error.reached_the_api(), Reached::Yes, "{error:?}");
        }
    }

    /// The categories of real failures, not of strings written in this test.
    #[tokio::test]
    async fn real_transport_failures_carry_their_category() {
        // Exercise reqwest's DNS failure path without depending on the host's
        // DNS setup: captive resolvers can answer even reserved .invalid names.
        struct MissingHost;
        impl reqwest::dns::Resolve for MissingHost {
            fn resolve(&self, _: reqwest::dns::Name) -> reqwest::dns::Resolving {
                Box::pin(async {
                    Err(std::io::Error::new(std::io::ErrorKind::NotFound, "no such host").into())
                })
            }
        }
        let transport = HttpTransport {
            client: reqwest::Client::builder()
                .no_proxy()
                .dns_resolver(std::sync::Arc::new(MissingHost))
                .build()
                .unwrap(),
        };
        let send = async |url: &str| {
            let error = transport
                .send(
                    HttpMethod::Get,
                    Url::parse(url).unwrap(),
                    &IndexMap::new(),
                    None,
                )
                .await
                .expect_err("the request cannot succeed");
            match error {
                HttpError::Transport(failure) => failure,
                other => panic!("expected a transport failure, got {other:?}"),
            }
        };

        // The injected resolver produces a real connector DNS error.
        let dns = send("http://host.invalid/orders").await;
        assert_eq!(dns.kind, FailureKind::Dns, "{}", dns.detail);
        assert_eq!(dns.kind.reached_the_api(), Reached::No);

        // A closed port on loopback: the connection itself fails.
        let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = closed.local_addr().unwrap();
        drop(closed);
        let refused = send(&format!("http://{address}/orders")).await;
        assert_eq!(refused.kind, FailureKind::Connect, "{}", refused.detail);
        assert_eq!(refused.kind.reached_the_api(), Reached::No);

        // A plain TCP server answering an HTTPS request fails the handshake.
        let plain = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = plain.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = plain.accept().await {
                use tokio::io::AsyncWriteExt;
                let _ = socket.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
                let _ = socket.shutdown().await;
            }
        });
        let tls = send(&format!("https://{address}/orders")).await;
        assert_eq!(tls.kind, FailureKind::Tls, "{}", tls.detail);
        assert_eq!(tls.kind.reached_the_api(), Reached::No);
    }

    #[test]
    fn header_values_follow_fetch_byte_strings() {
        assert_eq!(
            header_value("  text\t"),
            Some(HeaderValue::from_static("text"))
        );
        assert_eq!(
            header_value("café").map(|v| v.as_bytes().to_vec()),
            Some(b"caf\xe9".to_vec())
        );
        assert_eq!(header_value("żółw"), None);
        assert_eq!(header_value("a\nb"), None);
    }
}
