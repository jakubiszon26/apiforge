//! `apiforge mcp [--folder <dir>]`: the project as an MCP server over stdio.
//! Stdout belongs to the protocol; this command writes only to stderr.
use apiforge_mcp::tools::RunSettings;
use std::{ffi::OsString, path::PathBuf, time::Duration};

/// Test seams: a shorter run deadline, and a pause after every run step so a
/// simulation lasts long enough to be cancelled. Milliseconds; unset in use.
const DEADLINE_SEAM: &str = "APIFORGE_MCP_TEST_DEADLINE_MS";
const STEP_DELAY_SEAM: &str = "APIFORGE_MCP_TEST_STEP_DELAY_MS";

pub const USAGE: &str = "apiforge mcp [--folder <directory>]";

fn arguments(args: &[OsString]) -> Option<Option<PathBuf>> {
    match args {
        [command] if command == "mcp" => Some(None),
        [command, flag, folder] if command == "mcp" && flag == "--folder" => {
            Some(Some(PathBuf::from(folder)))
        }
        _ => None,
    }
}

pub fn run(args: &[OsString]) -> u8 {
    let Some(folder) = arguments(args) else {
        eprintln!("{USAGE}");
        return 2;
    };
    // Without --folder the server serves the directory it was started in.
    let folder = match folder.map_or_else(std::env::current_dir, Ok) {
        Ok(folder) => folder,
        Err(error) => {
            eprintln!("Cannot read the current directory: {error}");
            return 1;
        }
    };
    let settings = match settings() {
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

fn settings() -> Result<RunSettings, String> {
    let defaults = RunSettings::default();
    let seam = |name| milliseconds(name, std::env::var_os(name), defaults.deadline);
    Ok(RunSettings {
        deadline: seam(DEADLINE_SEAM)?.unwrap_or(defaults.deadline),
        step_delay: seam(STEP_DELAY_SEAM)?,
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
