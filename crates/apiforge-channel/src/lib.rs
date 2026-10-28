//! The control channel between the holder of a folder project and a process
//! that wants it or watches it (ADR 0014 decision 4, MCP-08b).
//!
//! The holder listens on a same-user local endpoint and names it in its holder
//! file; the other side connects. An endpoint is a random name and nothing
//! else: on Unix a socket in a private directory under the runtime directory,
//! on Windows the named pipe `\\.\pipe\<name>`. The name carries no path and
//! no secret, and a reader accepts only a name of that exact shape, so a
//! holder file can never point a client anywhere else.
//!
//! Messages are newline-delimited JSON objects with a `version` and a `type`.
//! A line that is not a message of this version is [`Received::Ignored`] with
//! the reason, never its content, for the caller to log.
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::path::PathBuf;
use std::{io, pin::Pin};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

/// The protocol version both sides write and accept.
pub const VERSION: u32 = 1;
/// A message is one short line; a longer one ends the connection.
pub const MAX_LINE: usize = 64 * 1024;
const PREFIX: &str = "apiforge-";

/// What one side tells the other.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    /// The server asks the window that holds the project to hand it over.
    Request {
        /// The agent's client, as it named itself.
        client: Option<String>,
        pid: u32,
    },
    /// The window has let the project go; the server takes it.
    Granted,
    /// The window keeps the project, and says why.
    Refused { reason: String },
    /// The holder committed: the window rescans at once.
    Saved {
        generation: u64,
        /// The project paths the commit wrote.
        paths: Vec<String>,
    },
    /// The holder's run started or ended.
    Run {
        path: String,
        mode: RunMode,
        state: RunState,
    },
    /// The holder let the project go; the window may take it back.
    Released,
    /// The person's consent to live requests (MCP-09; defined, not acted on).
    Permission { live: bool },
    /// The person wants the project back.
    TakeBack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    Simulation,
    Live,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Running,
    Passed,
    Failed,
    Cancelled,
}

/// One line read from the other side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Received {
    Message(Message),
    /// Not a message this side reads: why, never what it said.
    Ignored(&'static str),
}

#[derive(Serialize)]
struct Envelope<'a> {
    version: u32,
    #[serde(flatten)]
    message: &'a Message,
}

/// `message` as one line, newline included.
pub fn encode(message: &Message) -> Vec<u8> {
    let mut line = serde_json::to_vec(&Envelope {
        version: VERSION,
        message,
    })
    .expect("a message always serialises");
    line.push(b'\n');
    line
}

/// Reads one line, without its line break.
pub fn decode(line: &[u8]) -> Received {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(line) else {
        return Received::Ignored("a line that is not JSON");
    };
    let Some(object) = value.as_object() else {
        return Received::Ignored("a line that is not a JSON object");
    };
    match object.get("version").and_then(serde_json::Value::as_u64) {
        Some(version) if version == u64::from(VERSION) => {}
        Some(_) => return Received::Ignored("a message of another version"),
        None => return Received::Ignored("a message without a version"),
    }
    match serde_json::from_value::<Message>(value) {
        Ok(message) => Received::Message(message),
        Err(_) => Received::Ignored("an unknown or malformed message"),
    }
}

/// The name of a listening endpoint: `apiforge-` and 32 lowercase hex digits.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Endpoint(String);

impl Endpoint {
    /// A new random name, unique to one listener.
    pub fn random() -> Self {
        Self(format!("{PREFIX}{}", uuid::Uuid::new_v4().simple()))
    }

    /// `name` when it has exactly the shape of an endpoint, so a holder file
    /// can name nothing but an endpoint.
    pub fn parse(name: &str) -> Option<Self> {
        let random = name.strip_prefix(PREFIX)?;
        (random.len() == 32
            && random
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
        .then(|| Self(name.to_owned()))
    }

    pub fn name(&self) -> &str {
        &self.0
    }

    /// Where the socket is: `<runtime directory>/<name>/socket`, the
    /// directory private to this user. The runtime directory is
    /// `$XDG_RUNTIME_DIR` when it is an absolute path, otherwise the temporary
    /// directory; a socket path is limited to about a hundred bytes, which
    /// both keep well under.
    #[cfg(unix)]
    pub fn socket_path(&self) -> PathBuf {
        runtime_directory().join(&self.0).join("socket")
    }

    /// `\\.\pipe\<name>`.
    #[cfg(windows)]
    pub fn pipe_name(&self) -> String {
        format!(r"\\.\pipe\{}", self.0)
    }
}

impl std::fmt::Display for Endpoint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(unix)]
fn runtime_directory() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|directory| directory.is_absolute())
        .unwrap_or_else(std::env::temp_dir)
}

trait Stream: AsyncRead + AsyncWrite + Send {}
impl<T: AsyncRead + AsyncWrite + Send> Stream for T {}

/// The reading half of a connection.
pub struct Reader {
    inner: BufReader<Pin<Box<dyn AsyncRead + Send>>>,
}

/// The writing half of a connection.
pub struct Writer {
    inner: Pin<Box<dyn AsyncWrite + Send>>,
}

/// One connection, either end.
pub struct Connection {
    pub reader: Reader,
    pub writer: Writer,
}

impl Connection {
    fn new(stream: impl Stream + 'static) -> Self {
        let (reader, writer) = tokio::io::split(stream);
        Self {
            reader: Reader {
                inner: BufReader::new(Box::pin(reader)),
            },
            writer: Writer {
                inner: Box::pin(writer),
            },
        }
    }

    /// Splits the connection so that its halves can live in different tasks.
    pub fn split(self) -> (Reader, Writer) {
        (self.reader, self.writer)
    }
}

impl Reader {
    /// The next line, or `None` when the other side has gone. A line over
    /// [`MAX_LINE`] is an error: the caller drops the connection.
    pub async fn next(&mut self) -> io::Result<Option<Received>> {
        let mut line = Vec::new();
        let read = (&mut self.inner)
            .take(MAX_LINE as u64 + 1)
            .read_until(b'\n', &mut line)
            .await?;
        if read == 0 {
            return Ok(None);
        }
        if line.last() == Some(&b'\n') {
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
        } else if line.len() > MAX_LINE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "a line longer than a message can be",
            ));
        }
        Ok(Some(decode(&line)))
    }
}

impl Writer {
    pub async fn send(&mut self, message: &Message) -> io::Result<()> {
        self.inner.write_all(&encode(message)).await?;
        self.inner.flush().await
    }
}

/// Connects to `endpoint`. Must be called inside a Tokio runtime.
pub async fn connect(endpoint: &Endpoint) -> io::Result<Connection> {
    platform::connect(endpoint).await.map(Connection::new)
}

/// A listening endpoint. On Unix the socket and its directory are removed
/// when it is dropped.
pub struct Listener {
    endpoint: Endpoint,
    inner: platform::Listener,
}

impl Listener {
    /// Listens on a new random endpoint. Must be called inside a Tokio
    /// runtime.
    pub fn bind() -> io::Result<Self> {
        let endpoint = Endpoint::random();
        let inner = platform::Listener::bind(&endpoint)?;
        Ok(Self { endpoint, inner })
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    /// The next connection.
    pub async fn accept(&mut self) -> io::Result<Connection> {
        self.inner.accept().await
    }
}

#[cfg(unix)]
mod platform {
    use super::{Connection, Endpoint};
    use std::{
        fs::{self, DirBuilder, Permissions},
        io,
        os::unix::fs::{DirBuilderExt, PermissionsExt},
        path::PathBuf,
    };
    use tokio::net::{UnixListener, UnixStream};

    pub(super) struct Listener {
        listener: UnixListener,
        socket: PathBuf,
    }

    impl Listener {
        pub(super) fn bind(endpoint: &Endpoint) -> io::Result<Self> {
            let socket = endpoint.socket_path();
            let directory = socket
                .parent()
                .ok_or_else(|| io::Error::other("the socket has no directory"))?;
            // A new directory only this user may enter; an existing one is
            // never reused, whoever made it.
            DirBuilder::new().mode(0o700).create(directory)?;
            fs::set_permissions(directory, Permissions::from_mode(0o700))?;
            let listener = match UnixListener::bind(&socket) {
                Ok(listener) => listener,
                Err(error) => {
                    let _ = fs::remove_dir(directory);
                    return Err(error);
                }
            };
            let listener = Self { listener, socket };
            fs::set_permissions(&listener.socket, Permissions::from_mode(0o600))?;
            Ok(listener)
        }

        pub(super) async fn accept(&mut self) -> io::Result<Connection> {
            let (stream, _) = self.listener.accept().await?;
            Ok(Connection::new(stream))
        }
    }

    impl Drop for Listener {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.socket);
            if let Some(directory) = self.socket.parent() {
                let _ = fs::remove_dir(directory);
            }
        }
    }

    pub(super) async fn connect(endpoint: &Endpoint) -> io::Result<UnixStream> {
        UnixStream::connect(endpoint.socket_path()).await
    }
}

#[cfg(windows)]
mod platform {
    use super::{Connection, Endpoint};
    use std::{io, time::Duration};
    use tokio::net::windows::named_pipe::{
        ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
    };

    /// `ERROR_PIPE_BUSY`: every instance is connected; another comes soon.
    const PIPE_BUSY: i32 = 231;

    pub(super) struct Listener {
        name: String,
        next: NamedPipeServer,
    }

    impl Listener {
        pub(super) fn bind(endpoint: &Endpoint) -> io::Result<Self> {
            let name = endpoint.pipe_name();
            // The first instance claims the name: a pipe of that name made by
            // anyone else first is an error, never joined.
            let next = ServerOptions::new()
                .first_pipe_instance(true)
                .reject_remote_clients(true)
                .create(&name)?;
            Ok(Self { name, next })
        }

        pub(super) async fn accept(&mut self) -> io::Result<Connection> {
            self.next.connect().await?;
            let following = ServerOptions::new()
                .reject_remote_clients(true)
                .create(&self.name)?;
            let connected = std::mem::replace(&mut self.next, following);
            Ok(Connection::new(connected))
        }
    }

    pub(super) async fn connect(endpoint: &Endpoint) -> io::Result<NamedPipeClient> {
        let name = endpoint.pipe_name();
        for _ in 0..40 {
            match ClientOptions::new().open(&name) {
                Err(error) if error.raw_os_error() == Some(PIPE_BUSY) => {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                result => return result,
            }
        }
        ClientOptions::new().open(&name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_round_trips_with_its_version() {
        let messages = [
            Message::Request {
                client: Some("Claude Code".into()),
                pid: 42,
            },
            Message::Granted,
            Message::Refused {
                reason: "the window has unsaved work".into(),
            },
            Message::Saved {
                generation: 7,
                paths: vec!["scenarios/main.afs".into()],
            },
            Message::Run {
                path: "scenarios/main.afs".into(),
                mode: RunMode::Simulation,
                state: RunState::Running,
            },
            Message::Released,
            Message::Permission { live: false },
            Message::TakeBack,
        ];
        for message in messages {
            let line = encode(&message);
            assert_eq!(line.last(), Some(&b'\n'));
            assert_eq!(line.iter().filter(|byte| **byte == b'\n').count(), 1);
            let value: serde_json::Value = serde_json::from_slice(&line).unwrap();
            assert_eq!(value["version"], 1);
            assert_eq!(decode(&line[..line.len() - 1]), Received::Message(message));
        }
        assert_eq!(
            String::from_utf8(encode(&Message::TakeBack)).unwrap(),
            "{\"version\":1,\"type\":\"take_back\"}\n"
        );
    }

    #[test]
    fn what_is_not_a_message_of_this_version_is_ignored_with_a_reason() {
        for line in [
            &b"not json"[..],
            b"[1,2]",
            b"{\"type\":\"granted\"}",
            b"{\"version\":2,\"type\":\"granted\"}",
            b"{\"version\":1,\"type\":\"bogus\"}",
            b"{\"version\":1,\"type\":\"refused\"}",
        ] {
            assert!(
                matches!(decode(line), Received::Ignored(_)),
                "{}",
                String::from_utf8_lossy(line)
            );
        }
        // A field this version does not know is no reason to ignore one.
        assert_eq!(
            decode(b"{\"version\":1,\"type\":\"released\",\"later\":true}"),
            Received::Message(Message::Released)
        );
    }

    #[test]
    fn an_endpoint_is_a_random_name_and_only_such_a_name_parses() {
        let endpoint = Endpoint::random();
        assert_eq!(Endpoint::parse(endpoint.name()), Some(endpoint.clone()));
        assert_ne!(Endpoint::random(), endpoint);
        for name in [
            "",
            "apiforge-",
            "apiforge-0123456789abcdef0123456789abcde",
            "apiforge-0123456789ABCDEF0123456789abcdef",
            "apiforge-0123456789abcdef0123456789abcdef0",
            "apiforge-../../../../tmp/x0123456789abcdef",
            "/tmp/apiforge-0123456789abcdef0123456789abcdef",
            r"\\.\pipe\apiforge-0123456789abcdef0123456789abcdef",
            "other-0123456789abcdef0123456789abcdef",
        ] {
            assert_eq!(Endpoint::parse(name), None, "{name:?}");
        }
        assert!(Endpoint::parse("apiforge-0123456789abcdef0123456789abcdef").is_some());
    }

    #[test]
    fn a_listener_answers_and_a_client_reads_what_it_sends() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let mut listener = Listener::bind().unwrap();
            let endpoint = listener.endpoint().clone();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let socket = endpoint.socket_path();
                let mode = std::fs::metadata(&socket).unwrap().permissions().mode();
                assert_eq!(mode & 0o077, 0, "only this user");
            }
            let server = tokio::spawn(async move {
                let (mut reader, mut writer) = listener.accept().await.unwrap().split();
                let first = reader.next().await.unwrap();
                let second = reader.next().await.unwrap();
                writer.send(&Message::Granted).await.unwrap();
                let end = reader.next().await.unwrap();
                (first, second, end, listener)
            });
            let (mut reader, mut writer) = connect(&endpoint).await.unwrap().split();
            writer
                .inner
                .write_all(b"{\"version\":1,\"type\":\"nonsense\"}\r\n")
                .await
                .unwrap();
            writer
                .send(&Message::Request {
                    client: None,
                    pid: 1,
                })
                .await
                .unwrap();
            assert_eq!(
                reader.next().await.unwrap(),
                Some(Received::Message(Message::Granted))
            );
            drop(writer);
            drop(reader);
            let (first, second, end, listener) = server.await.unwrap();
            assert!(matches!(first, Some(Received::Ignored(_))));
            assert_eq!(
                second,
                Some(Received::Message(Message::Request {
                    client: None,
                    pid: 1
                }))
            );
            assert_eq!(end, None, "the end of the connection");
            #[cfg(unix)]
            {
                let socket = endpoint.socket_path();
                drop(listener);
                assert!(!socket.exists());
                assert!(!socket.parent().unwrap().exists());
            }
            #[cfg(windows)]
            drop(listener);
        });
    }

    #[test]
    fn a_line_longer_than_a_message_ends_the_connection() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let mut listener = Listener::bind().unwrap();
            let endpoint = listener.endpoint().clone();
            let server = tokio::spawn(async move {
                let (mut reader, _writer) = listener.accept().await.unwrap().split();
                reader.next().await
            });
            let (_reader, mut writer) = connect(&endpoint).await.unwrap().split();
            writer
                .inner
                .write_all(&vec![b'x'; MAX_LINE + 10])
                .await
                .unwrap();
            let read = server.await.unwrap();
            assert_eq!(read.unwrap_err().kind(), io::ErrorKind::InvalidData);
        });
    }
}
