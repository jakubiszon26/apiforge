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
        let mut builder = self.client.request(method, url).headers(map);
        if let Some(body) = body {
            builder = builder.body(body);
        }
        let response = builder
            .send()
            .await
            .map_err(|error| HttpError::Transport(transport_error_detail(error)))?;
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
    #[error("Request failed: {0}.")]
    Transport(String),
    #[error("Redirects are not followed (HTTP {0}).")]
    Redirect(StatusCode),
    #[error("The response exceeds the {} limit.", byte_size(*.0))]
    TooLarge(usize),
    #[error("The response JSON exceeds the nesting limit of {0} levels.")]
    TooDeep(usize),
    #[error("The response JSON contains a number outside the finite range.")]
    NonFinite,
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
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| HttpError::Transport(transport_error_detail(error)))?
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

/// Formats a `reqwest::Error` with an explicit failure category prefix (`timeout`,
/// `connect`, `body`, `decode`, or `request`) followed by its full cause chain.
fn transport_error_detail(error: reqwest::Error) -> String {
    let category = if error.is_timeout() {
        Some("timeout")
    } else if error.is_connect() {
        Some("connect")
    } else if error.is_body() {
        Some("body")
    } else if error.is_decode() {
        Some("decode")
    } else if error.is_request() {
        Some("request")
    } else {
        None
    };
    let chain = error_chain(&error.without_url());
    let lower = chain.to_ascii_lowercase();
    match category {
        Some(prefix) if !lower.starts_with(prefix) => format!("{prefix}: {chain}"),
        _ => chain,
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
