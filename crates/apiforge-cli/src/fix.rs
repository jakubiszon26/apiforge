//! `apiforge fix --folder`: converts a project written for 0.1.0 to the
//! build's language whole, in one commit, or writes nothing and says what to
//! change by hand (LANG-20). `--check` prints the same report and never
//! writes.
use apiforge_project::language::{self, FixReport, Planned, Problem, SourceOutcome};
use std::{ffi::OsString, path::PathBuf};

pub const USAGE: &str = "apiforge fix --folder <directory> [--check]";

struct Args {
    folder: PathBuf,
    check: bool,
}

fn arguments(args: &[OsString]) -> Option<Args> {
    if args.first()?.to_str()? != "fix" {
        return None;
    }
    let (mut folder, mut check) = (None, false);
    let mut rest = args[1..].iter();
    while let Some(arg) = rest.next() {
        match arg.to_str() {
            Some("--folder") if folder.is_none() => folder = Some(PathBuf::from(rest.next()?)),
            Some("--check") if !check => check = true,
            _ => return None,
        }
    }
    Some(Args {
        folder: folder?,
        check,
    })
}

pub fn run(args: &[OsString]) -> u8 {
    let Some(args) = arguments(args) else {
        eprintln!("{USAGE}");
        return 2;
    };
    match execute(&args) {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

/// Whether the project is (or, with `--check`, can be) in the build's
/// language.
fn execute(args: &Args) -> Result<bool, String> {
    let plan = match language::plan_fix(&args.folder, args.check).map_err(|e| e.to_string())? {
        Planned::Current => {
            println!(
                "This project is already written in language {}; nothing to convert.",
                language::LANGUAGE_VERSION
            );
            return Ok(true);
        }
        Planned::Plan(plan) => plan,
    };
    if !plan.report().convertible() {
        blocked(plan.report());
        return Ok(false);
    }
    if args.check {
        converted(plan.report(), false);
        println!("Every source can be converted; nothing was written.");
        return Ok(true);
    }
    let report = plan.commit().map_err(|error| error.to_string())?;
    converted(&report, true);
    let changed = report
        .sources
        .iter()
        .filter(|source| matches!(source.outcome, SourceOutcome::Converted(_)))
        .count();
    println!(
        "The project is written in language {} now: {changed} of {} sources changed.",
        language::LANGUAGE_VERSION,
        report.sources.len()
    );
    Ok(true)
}

/// Each change of each converted source, and the manifest's.
fn converted(report: &FixReport, written: bool) {
    let (remove, write) = if written {
        ("removed", "wrote")
    } else {
        ("would remove", "would write")
    };
    for source in &report.sources {
        let SourceOutcome::Converted(changes) = &source.outcome else {
            continue;
        };
        for change in changes {
            if !change.removed.is_empty() {
                println!(
                    "{}:{}: {remove} `{}`",
                    source.path, change.line, change.removed
                );
            }
            if !change.written.is_empty() {
                println!(
                    "{}:{}: {write} `{}`",
                    source.path, change.line, change.written
                );
            }
        }
    }
    println!(
        "{}: {} language {}",
        apiforge_storage::project::MANIFEST_PATH,
        if written {
            "raised to"
        } else {
            "would be raised to"
        },
        language::LANGUAGE_VERSION
    );
}

/// Every statement in the way and every source that cannot be read; nothing
/// was written.
fn blocked(report: &FixReport) {
    let statements: Vec<(&str, &Problem)> = report
        .sources
        .iter()
        .flat_map(|source| match &source.outcome {
            SourceOutcome::Blocked(problems) => problems
                .iter()
                .map(|problem| (source.path.as_str(), problem))
                .collect(),
            _ => vec![],
        })
        .collect();
    let unreadable: Vec<(&str, &Problem)> = report
        .sources
        .iter()
        .filter_map(|source| match &source.outcome {
            SourceOutcome::Unreadable(problem) => Some((source.path.as_str(), problem)),
            _ => None,
        })
        .collect();
    if !statements.is_empty() {
        eprintln!("Statements to change by hand:");
        for (path, problem) in statements {
            let statement = if problem.statement.is_empty() {
                String::new()
            } else {
                format!("`{}`: ", problem.statement)
            };
            if problem.line == 0 {
                eprintln!("{path}: {} {}", problem.reason, problem.todo);
            } else {
                eprintln!(
                    "{path}:{}:{}: {statement}{} {}",
                    problem.line, problem.column, problem.reason, problem.todo
                );
            }
        }
    }
    if !unreadable.is_empty() {
        eprintln!("Sources that cannot be read:");
        for (path, problem) in unreadable {
            if problem.line == 0 {
                eprintln!("{path}: {} {}", problem.reason, problem.todo);
            } else {
                eprintln!(
                    "{path}:{}:{}: {} {}",
                    problem.line, problem.column, problem.reason, problem.todo
                );
            }
        }
    }
    eprintln!("{}", language::NOTHING_WRITTEN);
}
