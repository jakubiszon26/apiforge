//! `apiforge mcp [--folder <dir>] [--allow-live] [--secrets <file>]`: the
//! project as an MCP server over stdio. Stdout belongs to the protocol; this
//! command writes only to stderr.
use apiforge_mcp::{ServerSettings, secrets::Secrets, tools::RunSettings};
use std::{ffi::OsString, path::PathBuf, time::Duration};

/// Test seams: a shorter run deadline, a pause after every run step so a
/// simulation lasts long enough to be cancelled, and a pause after every
/// step once a run is stopping, so its cleanup outlasts the end of stdin.
/// Milliseconds; unset in use.
const DEADLINE_SEAM: &str = "APIFORGE_MCP_TEST_DEADLINE_MS";
const STEP_DELAY_SEAM: &str = "APIFORGE_MCP_TEST_STEP_DELAY_MS";
const CLEANUP_DELAY_SEAM: &str = "APIFORGE_MCP_TEST_CLEANUP_DELAY_MS";

pub const USAGE: &str =
    "apiforge mcp [--folder <directory>] [--allow-live] [--secrets <file.json>]";

#[derive(Debug, Default, PartialEq, Eq)]
struct Arguments {
    folder: Option<PathBuf>,
    allow_live: bool,
    secrets: Option<PathBuf>,
}

/// The flags in any order, each at most once.
fn arguments(args: &[OsString]) -> Option<Arguments> {
    let (command, rest) = args.split_first()?;
    if command != "mcp" {
        return None;
    }
    let mut found = Arguments::default();
    let mut rest = rest.iter();
    while let Some(flag) = rest.next() {
        match flag.to_str()? {
            "--folder" if found.folder.is_none() => {
                found.folder = Some(PathBuf::from(rest.next()?));
            }
            "--allow-live" if !found.allow_live => found.allow_live = true,
            "--secrets" if found.secrets.is_none() => {
                found.secrets = Some(PathBuf::from(rest.next()?));
            }
            _ => return None,
        }
    }
    Some(found)
}

pub fn run(args: &[OsString]) -> u8 {
    let Some(arguments) = arguments(args) else {
        eprintln!("{USAGE}");
        return 2;
    };
    // Without --folder the server serves the directory it was started in.
    let folder = match arguments.folder.map_or_else(std::env::current_dir, Ok) {
        Ok(folder) => folder,
        Err(error) => {
            eprintln!("Cannot read the current directory: {error}");
            return 1;
        }
    };
    let settings = match settings(arguments.allow_live, arguments.secrets) {
        Ok(settings) => settings,
        Err(message) => {
            eprintln!("apiforge mcp: {message}");
            return 2;
        }
    };
    match apiforge_mcp::serve_stdio_with(folder, settings) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("apiforge mcp: {error}");
            1
        }
    }
}

/// The server's settings. The secrets are read once here, so a file that
/// cannot be used stops the server at start; the server reads them again at
/// each call.
fn settings(allow_live: bool, secrets: Option<PathBuf>) -> Result<ServerSettings, String> {
    let defaults = RunSettings::default();
    let seam = |name| milliseconds(name, std::env::var_os(name), defaults.deadline);
    let secrets = Secrets::new(secrets);
    secrets.read().map_err(|error| error.message)?;
    Ok(ServerSettings {
        run: RunSettings {
            deadline: seam(DEADLINE_SEAM)?.unwrap_or(defaults.deadline),
            step_delay: seam(STEP_DELAY_SEAM)?,
        },
        allow_live,
        secrets,
        cleanup_delay: seam(CLEANUP_DELAY_SEAM)?,
    })
}

/// A seam's value: whole milliseconds, at most `cap`, the ten-minute
/// deadline, so a seam can shorten a run but never lengthen it. A value that
/// is not a number stops the server instead of being ignored.
fn milliseconds(
    name: &str,
    value: Option<OsString>,
    cap: Duration,
) -> Result<Option<Duration>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let milliseconds: u64 = value
        .to_str()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| format!("{name} must be a whole number of milliseconds"))?;
    Ok(Some(Duration::from_millis(milliseconds).min(cap)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Option<Arguments> {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        arguments(&args)
    }

    #[test]
    fn the_flags_come_in_any_order_each_once() {
        assert_eq!(parse(&["mcp"]), Some(Arguments::default()));
        assert_eq!(
            parse(&[
                "mcp",
                "--secrets",
                "s.json",
                "--allow-live",
                "--folder",
                "p"
            ]),
            Some(Arguments {
                folder: Some("p".into()),
                allow_live: true,
                secrets: Some("s.json".into()),
            })
        );
        for refused in [
            &["mcp", "--allow-live", "--allow-live"][..],
            &["mcp", "--secrets"],
            &["mcp", "--folder", "a", "--folder", "b"],
            &["mcp", "--live"],
            &["mcp", "p"],
            &["run"],
        ] {
            assert_eq!(parse(refused), None, "{refused:?}");
        }
    }

    const CAP: Duration = Duration::from_secs(600);

    #[test]
    fn a_seam_is_whole_milliseconds_capped_at_the_deadline() {
        assert_eq!(milliseconds("X", None, CAP), Ok(None));
        assert_eq!(
            milliseconds("X", Some("300".into()), CAP),
            Ok(Some(Duration::from_millis(300)))
        );
        assert_eq!(
            milliseconds("X", Some("3600000".into()), CAP),
            Ok(Some(CAP)),
            "an hour is cut to ten minutes"
        );
        for bad in ["300ms", "", "-1", "1.5", " 300"] {
            assert_eq!(
                milliseconds("X", Some(bad.into()), CAP),
                Err("X must be a whole number of milliseconds".to_owned()),
                "{bad:?}"
            );
        }
    }
}
