//! The request log: one JSON line per request the server received, for counting what
//! a client really sent.

use crate::is_json;
use axum::{
    body::{Body, Bytes, HttpBody},
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::Response,
};
use chrono::{SecondsFormat, Utc};
use http_body::Frame;
use serde::Serialize;
use serde_json::Value;
use std::{
    fmt,
    fs::OpenOptions,
    future::poll_fn,
    io::{self, Write},
    path::Path,
    pin::Pin,
    sync::{Arc, Mutex, PoisonError},
    task::{Context, Poll},
};

/// Longest request body the log keeps.
const BODY_LOG_LIMIT: usize = 64 * 1024;

/// A file that receives one JSON line per request: its time, method, path with query,
/// status, JSON body of at most 64 KiB, the names of its headers and the value of
/// `X-Client`. No other header value is written.
#[derive(Clone)]
pub struct RequestLog(Arc<Mutex<Sink>>);

struct Sink {
    out: Box<dyn Write + Send>,
    /// The last write failed and may have left a part of its line.
    torn: bool,
    reported: bool,
}

impl fmt::Debug for RequestLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RequestLog").finish_non_exhaustive()
    }
}

impl RequestLog {
    /// Opens `path` for appending and creates it when it does not exist.
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new().append(true).create(true).open(path)?;
        Ok(Self::to(file))
    }

    fn to(out: impl Write + Send + 'static) -> Self {
        Self(Arc::new(Mutex::new(Sink {
            out: Box::new(out),
            torn: false,
            reported: false,
        })))
    }

    /// Appends one line. A failure is said once on stderr, because the count is then
    /// incomplete, and is never the request's.
    fn write(&self, entry: &Entry) {
        let mut sink = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let mut line = Vec::new();
        if sink.torn {
            line.push(b'\n');
        }
        let written = serde_json::to_writer(&mut line, entry)
            .map_err(io::Error::from)
            .and_then(|()| {
                line.push(b'\n');
                sink.out.write_all(&line)
            });
        sink.torn = written.is_err();
        if let Err(error) = written
            && !sink.reported
        {
            sink.reported = true;
            let _ = writeln!(
                io::stderr(),
                "The request log cannot be written and misses requests: {error}"
            );
        }
    }
}

#[derive(Serialize)]
struct Entry {
    time: String,
    method: String,
    path: String,
    status: u16,
    /// Names only, sorted.
    headers: Vec<String>,
    /// The value of `X-Client`.
    #[serde(skip_serializing_if = "Option::is_none")]
    client: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<Value>,
}

/// Writes each request and the status of its response to the log.
pub(crate) async fn record(
    State(log): State<RequestLog>,
    request: Request,
    next: Next,
) -> Response {
    let time = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let (parts, body) = request.into_parts();
    let (kept, body) = if is_json(&parts.headers) {
        read_ahead(body).await
    } else {
        (None, body)
    };
    let mut entry = Entry {
        time,
        method: parts.method.to_string(),
        path: parts
            .uri
            .path_and_query()
            .map_or_else(|| parts.uri.to_string(), ToString::to_string),
        status: 0,
        headers: header_names(&parts.headers),
        client: client(&parts.headers),
        body: kept.and_then(|body| serde_json::from_slice(&body).ok()),
    };
    let response = next.run(Request::from_parts(parts, body)).await;
    entry.status = response.status().as_u16();
    log.write(&entry);
    response
}

fn header_names(headers: &HeaderMap) -> Vec<String> {
    let mut names: Vec<_> = headers.keys().map(ToString::to_string).collect();
    names.sort_unstable();
    names
}

fn client(headers: &HeaderMap) -> Option<String> {
    let value = headers.get("x-client")?;
    Some(String::from_utf8_lossy(value.as_bytes()).into_owned())
}

/// The whole body when it ends within [`BODY_LOG_LIMIT`], and a body for the handler
/// that yields the same bytes, the same error and the same end as the original.
async fn read_ahead(mut body: Body) -> (Option<Bytes>, Body) {
    let limit = BODY_LOG_LIMIT;
    if HttpBody::size_hint(&body).lower() > limit as u64 {
        return (None, body);
    }
    let mut read = Vec::new();
    while read.len() <= limit {
        match poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await {
            Some(Ok(frame)) => {
                if let Ok(data) = frame.into_data() {
                    read.extend_from_slice(&data);
                }
            }
            Some(Err(error)) => return (None, Body::new(Resumed::new(read, Rest::Failed(error)))),
            None => {
                let read = Bytes::from(read);
                return (Some(read.clone()), Body::from(read));
            }
        }
    }
    (None, Body::new(Resumed::new(read, Rest::Body(body))))
}

/// A body whose first bytes were already read.
struct Resumed {
    read: Option<Bytes>,
    rest: Rest,
}

enum Rest {
    Body(Body),
    Failed(axum::Error),
    Ended,
}

impl Resumed {
    fn new(read: Vec<u8>, rest: Rest) -> Self {
        Self {
            read: Some(Bytes::from(read)).filter(|read| !read.is_empty()),
            rest,
        }
    }
}

impl HttpBody for Resumed {
    type Data = Bytes;
    type Error = axum::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, axum::Error>>> {
        if let Some(read) = self.read.take() {
            return Poll::Ready(Some(Ok(Frame::data(read))));
        }
        match &mut self.rest {
            Rest::Body(body) => Pin::new(body).poll_frame(cx),
            Rest::Failed(_) | Rest::Ended => {
                let rest = std::mem::replace(&mut self.rest, Rest::Ended);
                Poll::Ready(match rest {
                    Rest::Failed(error) => Some(Err(error)),
                    Rest::Body(_) | Rest::Ended => None,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Takes the first three bytes of a write and refuses the rest, once.
    struct FailsOnce {
        calls: usize,
        out: Arc<Mutex<Vec<u8>>>,
    }

    impl Write for FailsOnce {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.calls += 1;
            let taken = match self.calls {
                1 => &bytes[..3],
                2 => return Err(io::Error::other("full")),
                _ => bytes,
            };
            self.out.lock().unwrap().extend_from_slice(taken);
            Ok(taken.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// A body that breaks off must not reach a handler as a shorter, complete one.
    #[tokio::test]
    async fn a_body_that_breaks_off_is_handed_on_as_broken() {
        let error = axum::Error::new(io::Error::other("broken off"));
        let source = Resumed::new(b"{}".to_vec(), Rest::Failed(error));
        let (kept, mut body) = read_ahead(Body::new(source)).await;
        assert_eq!(kept, None);
        let mut next = async || poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await;
        let data = next().await.unwrap().unwrap().into_data().unwrap();
        assert_eq!(data, "{}");
        assert!(next().await.unwrap().is_err());
        assert!(next().await.is_none());
    }

    #[test]
    fn a_torn_line_does_not_take_the_next_one() {
        let out = Arc::default();
        let log = RequestLog::to(FailsOnce {
            calls: 0,
            out: Arc::clone(&out),
        });
        for status in [200, 201, 202] {
            log.write(&Entry {
                time: String::new(),
                method: String::new(),
                path: String::new(),
                status,
                headers: Vec::new(),
                client: None,
                body: None,
            });
        }
        let text = String::from_utf8(out.lock().unwrap().clone()).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "{\"t");
        for (line, status) in lines[1..].iter().zip([201, 202]) {
            let entry: Value = serde_json::from_str(line).unwrap();
            assert_eq!(entry["status"], status);
        }
    }
}
