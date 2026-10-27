//! Who holds a folder project (ADR 0014 decision 3, MCP-07). The writer
//! describes itself in one JSON line beside the lock, never in the lock: on
//! Windows the lock is mandatory and its file cannot be read while it is held.
//! A reader trusts the description only while the probe finds a live writer,
//! so one left behind by a holder that is gone describes nobody.
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::Read,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) const HOLDER_PATH: &str = ".apiforge/local/holder.json";
const PROGRAM: &str = "apiforge";
const VERSION: u32 = 1;
/// A description is one short line; anything longer is not one.
const MAX_BYTES: u64 = 4096;

/// What kind of process holds the project.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HolderRole {
    /// An API Forge window.
    Window,
    /// `apiforge mcp`, writing for an agent.
    Agent,
    /// A command-line subcommand such as `apply` or `run --live`.
    Command,
}

/// The holder's description: its role, the client or subcommand it names,
/// when it took the project, and the endpoint of its control channel. Never
/// a path and never a secret. [`Holder::unknown`] is a live writer that left
/// no readable description, such as an older build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Holder {
    role: Option<HolderRole>,
    client: Option<String>,
    since: Option<String>,
    endpoint: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Line {
    version: u32,
    program: String,
    role: HolderRole,
    client: Option<String>,
    since: String,
    endpoint: Option<String>,
}

impl Holder {
    /// A holder taking the project now. `client` is the agent's client name,
    /// or the subcommand's name for a command.
    pub fn new(role: HolderRole, client: Option<String>) -> Self {
        Self {
            role: Some(role),
            client,
            since: Some(rfc3339(SystemTime::now())),
            endpoint: None,
        }
    }

    pub fn window() -> Self {
        Self::new(HolderRole::Window, None)
    }

    /// A command-line subcommand, named as the user typed it: `apply`, `run`.
    pub fn command(name: &str) -> Self {
        Self::new(HolderRole::Command, Some(name.to_owned()))
    }

    /// A live writer that described itself in no way this build can read.
    pub fn unknown() -> Self {
        Self {
            role: None,
            client: None,
            since: None,
            endpoint: None,
        }
    }

    /// The same holder with another start time, an RFC 3339 UTC timestamp.
    pub fn with_since(mut self, since: impl Into<String>) -> Self {
        self.since = Some(since.into());
        self
    }

    pub fn role(&self) -> Option<HolderRole> {
        self.role
    }
    pub fn client(&self) -> Option<&str> {
        self.client.as_deref()
    }
    /// When the holder took the project, as RFC 3339 in UTC.
    pub fn since(&self) -> Option<&str> {
        self.since.as_deref()
    }
    /// Reserved for the control channel (MCP-08b); always `None` until then.
    pub fn endpoint(&self) -> Option<&str> {
        self.endpoint.as_deref()
    }
    pub fn is_unknown(&self) -> bool {
        self.role.is_none()
    }

    /// One JSON line, or `None` for an unknown holder, which is never written.
    pub(super) fn to_line(&self) -> Option<Vec<u8>> {
        let line = Line {
            version: VERSION,
            program: PROGRAM.into(),
            role: self.role?,
            client: self.client.clone(),
            since: self.since.clone()?,
            endpoint: self.endpoint.clone(),
        };
        let mut bytes = serde_json::to_vec(&line).ok()?;
        bytes.push(b'\n');
        Some(bytes)
    }

    fn parse(bytes: &[u8]) -> Self {
        match serde_json::from_slice::<Line>(bytes) {
            Ok(line) if line.version == VERSION && line.program == PROGRAM => Self {
                role: Some(line.role),
                client: line.client,
                since: Some(line.since),
                endpoint: line.endpoint,
            },
            _ => Self::unknown(),
        }
    }

    /// The description at `path`. Anything that is not a readable
    /// description, whether it is missing, unreadable (permissions, another
    /// user's file), oversized or malformed, is an unknown holder: the writer
    /// is live, only its name is lost. Only called while a writer is live.
    pub(super) fn read(path: &Path) -> Self {
        let Ok(file) = File::open(path) else {
            return Self::unknown();
        };
        let mut bytes = Vec::new();
        match file.take(MAX_BYTES + 1).read_to_end(&mut bytes) {
            Ok(_) if bytes.len() as u64 <= MAX_BYTES => Self::parse(&bytes),
            _ => Self::unknown(),
        }
    }
}

/// `2026-10-27T10:42:11Z`: whole seconds in UTC.
fn rfc3339(time: SystemTime) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest / 60 % 60,
        rest % 60
    )
}

/// Howard Hinnant's days-to-civil algorithm for days since 1970-01-01.
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn timestamps_are_rfc3339_in_utc() {
        assert_eq!(rfc3339(UNIX_EPOCH), "1970-01-01T00:00:00Z");
        let at = UNIX_EPOCH + Duration::from_secs(1_793_097_731);
        assert_eq!(rfc3339(at), "2026-10-27T10:42:11Z");
        let leap = UNIX_EPOCH + Duration::from_secs(951_782_400);
        assert_eq!(rfc3339(leap), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn a_line_round_trips_and_an_unknown_holder_writes_nothing() {
        let holder = Holder::new(HolderRole::Agent, Some("Claude Code".into()))
            .with_since("2026-10-27T10:42:11Z");
        assert_eq!(
            String::from_utf8(holder.to_line().unwrap()).unwrap(),
            "{\"version\":1,\"program\":\"apiforge\",\"role\":\"agent\",\
             \"client\":\"Claude Code\",\"since\":\"2026-10-27T10:42:11Z\",\"endpoint\":null}\n"
        );
        assert_eq!(Holder::parse(&holder.to_line().unwrap()), holder);
        assert_eq!(Holder::unknown().to_line(), None);
    }
}
