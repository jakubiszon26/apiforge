//! Development tool that converts version-one documents to the current format.
//!
//! It is not shipped and not part of the import path ([ADR 0004]). Usage:
//!
//! ```text
//! apiforge-convert-v1 <input.json> <output.json>   convert one workspace or project
//! apiforge-convert-v1 --fixtures [--check]         regenerate or verify fixtures/
//! ```
//!
//! Fixtures are generated from the frozen version-one sources in
//! `tools/convert-v1/sources/`: the projects and workspace exported by the
//! TypeScript prototype and the run reports it recorded.
//!
//! [ADR 0004]: ../../docs/decisions/0004-document-format-v2.md

mod convert;
mod fixtures;
mod v1;

use std::{path::Path, process::ExitCode};

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let result = match arguments.as_slice() {
        [flag] if flag == "--fixtures" => fixtures::write(&fixtures::repository()),
        [flag, check] if flag == "--fixtures" && check == "--check" => {
            fixtures::check(&fixtures::repository())
        }
        [input, output] if !input.starts_with("--") => convert_file(input, output),
        _ => {
            eprintln!(
                "Usage: apiforge-convert-v1 <input.json> <output.json>\n       apiforge-convert-v1 --fixtures [--check]"
            );
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn convert_file(input: &str, output: &str) -> Result<(), String> {
    let bytes = std::fs::read(input).map_err(|error| format!("{input}: {error}"))?;
    let text = fixtures::convert_document(&bytes).map_err(|error| format!("{input}: {error}"))?;
    std::fs::write(Path::new(output), text).map_err(|error| format!("{output}: {error}"))
}
