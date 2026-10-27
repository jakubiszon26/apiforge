//! `apiforge mcp [--folder <dir>]`: the project as an MCP server over stdio.
//! Stdout belongs to the protocol; this command writes only to stderr.
use std::{ffi::OsString, path::PathBuf};

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
    match apiforge_mcp::serve_stdio(folder) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("apiforge mcp: {error}");
            1
        }
    }
}
