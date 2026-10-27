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
    match apiforge_mcp::serve_stdio_with(folder, settings()) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("apiforge mcp: {error}");
            1
        }
    }
}

fn settings() -> RunSettings {
    let milliseconds = |name| {
        std::env::var(name)
            .ok()
            .and_then(|value| value.parse().ok())
            .map(Duration::from_millis)
    };
    let defaults = RunSettings::default();
    RunSettings {
        deadline: milliseconds(DEADLINE_SEAM).unwrap_or(defaults.deadline),
        step_delay: milliseconds(STEP_DELAY_SEAM),
    }
}
