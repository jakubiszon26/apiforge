mod document;
mod fix;
mod folder;
mod mcp;
mod run;
use std::{env, ffi::OsString, fs::File, io::Read, path::Path, process::ExitCode};

fn source(path: &Path) -> Result<String, String> {
    let file = File::open(path).map_err(|error| format!("Cannot read source: {error}"))?;
    let mut bytes = Vec::new();
    file.take(apiforge_script::MAX_SOURCE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Cannot read source: {error}"))?;
    if bytes.len() > apiforge_script::MAX_SOURCE_BYTES {
        return Err("Source exceeds the 1 MiB limit".into());
    }
    String::from_utf8(bytes).map_err(|_| "Source must be valid UTF-8".into())
}

fn arguments(args: &[OsString]) -> Option<(&Path, bool)> {
    match args {
        [command, path] if command == "fmt" && path != "--check" => Some((Path::new(path), false)),
        [command, path, flag] if command == "fmt" && flag == "--check" && path != "--check" => {
            Some((Path::new(path), true))
        }
        [command, flag, path] if command == "fmt" && flag == "--check" && path != "--check" => {
            Some((Path::new(path), true))
        }
        _ => None,
    }
}

fn main() -> ExitCode {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--version" {
        println!("apiforge {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    if args.len() == 1 && matches!(args[0].to_str(), Some("--help" | "-h")) {
        println!(
            "apiforge fmt <file.afs> [--check]\n{}\n{}\n{}\n{}\n{}\napiforge --version",
            document::USAGE,
            folder::USAGE,
            fix::USAGE,
            run::USAGE,
            mcp::USAGE
        );
        return ExitCode::SUCCESS;
    }
    if args
        .first()
        .and_then(|s| s.to_str())
        .is_some_and(|s| matches!(s, "check" | "print" | "apply"))
    {
        return ExitCode::from(if args.iter().any(|arg| arg == "--folder") {
            folder::run(&args)
        } else {
            document::run(&args)
        });
    }
    if args.first().and_then(|a| a.to_str()) == Some("fix") {
        return ExitCode::from(fix::run(&args));
    }
    if args.first().and_then(|a| a.to_str()) == Some("run") {
        return ExitCode::from(run::run(&args));
    }
    if args.first().and_then(|a| a.to_str()) == Some("mcp") {
        return ExitCode::from(mcp::run(&args));
    }
    let Some((path, check)) = arguments(&args) else {
        eprintln!("Usage: apiforge fmt <file.afs> [--check]");
        return ExitCode::from(2);
    };
    let text = match source(path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let tree = apiforge_script::parse(&text);
    let formatted = apiforge_script::format_tree(&tree);
    let diagnostics = match &formatted {
        Ok(_) => &tree.diagnostics,
        Err(diagnostics) => diagnostics,
    };
    self::diagnostics(Some(&path.display().to_string()), &text, diagnostics);
    match formatted {
        Ok(formatted) if check => {
            if text == formatted {
                ExitCode::SUCCESS
            } else {
                eprintln!("Source needs formatting");
                ExitCode::FAILURE
            }
        }
        Ok(formatted) if text == formatted => ExitCode::SUCCESS,
        Ok(formatted) => match apiforge_storage::write_atomic(path, formatted.as_bytes()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("Cannot save source: {error}");
                ExitCode::FAILURE
            }
        },
        Err(_) => ExitCode::FAILURE,
    }
}

/// Prints diagnostics of `text` to stderr as compilers do (#346):
/// `file:line:column: severity: message`, at the start of each range, the
/// line and column one-based and the column counted in characters. A text
/// with no file prints `line:column: severity: message`.
fn diagnostics(file: Option<&str>, text: &str, diagnostics: &[apiforge_script::Diagnostic]) {
    for diagnostic in diagnostics {
        let (line, column) = apiforge_script::line_column(text, diagnostic.range.start);
        let severity = match diagnostic.severity {
            apiforge_script::Severity::Error => "error",
            apiforge_script::Severity::Warning => "warning",
        };
        let file = file.map(|file| format!("{file}:")).unwrap_or_default();
        eprintln!("{file}{line}:{column}: {severity}: {}", diagnostic.message);
    }
}
