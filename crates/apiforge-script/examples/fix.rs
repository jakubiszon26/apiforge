//! Applies the fix of the written language (LANG-13) to `.afs` files in
//! place, as a click on the error in the editor does: the `@after` lines of a
//! file written in the order it runs today are deleted and a missing
//! `begin = start` is written; every other byte stays. A file the fix refuses
//! is left as it is and the statement to place by hand is printed.
//!
//! The files are read in the demo shop project, with the published revisions
//! given as `--revision PATH` (read in their stored spelling) added to it.
//! `-` reads one text from standard input and writes the fixed text to
//! standard output.
//!
//! ```text
//! cargo run -p apiforge-script --example fix -- \
//!     --revision fixtures/scenarios/lay/rooms/.apiforge/revisions/sign-in/1.afs \
//!     fixtures/scenarios/lay/rooms/scenarios/*.afs
//! ```
use apiforge_script::{Document, LowerContext, fix_source, line_column, lower, parse};
use std::{
    io::{Read, Write},
    process::ExitCode,
};

fn main() -> ExitCode {
    let mut project = apiforge_core::demos::shop_project();
    let mut files = vec![];
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--revision" {
            let Some(path) = arguments.next() else {
                eprintln!("--revision needs a path");
                return ExitCode::FAILURE;
            };
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
            let lowered = lower(&parse(&text), &LowerContext::folder(&project).stored());
            match lowered.document {
                Some(Document::Component(component)) if !lowered.has_errors() => {
                    project.components.push(component)
                }
                _ => {
                    eprintln!("{path}: not a valid revision: {:?}", lowered.diagnostics);
                    return ExitCode::FAILURE;
                }
            }
        } else {
            files.push(argument);
        }
    }
    let context = LowerContext::folder(&project);
    let mut refused = false;
    for path in files {
        let text = if path == "-" {
            let mut text = String::new();
            std::io::stdin()
                .read_to_string(&mut text)
                .expect("standard input");
            text
        } else {
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
        };
        let fixed = fix_source(&text, &context);
        if let Some(reason) = &fixed.refused {
            refused = true;
            let (line, column) = line_column(&text, reason.range.start);
            eprintln!("{path}:{line}:{column}: {}", reason.message);
        }
        if path == "-" {
            std::io::stdout()
                .write_all(fixed.text.as_bytes())
                .expect("standard output");
        } else if fixed.changed(&text) {
            std::fs::write(&path, &fixed.text).unwrap_or_else(|e| panic!("{path}: {e}"));
            println!("{path}: fixed");
        }
    }
    if refused {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
