//! Measures source analysis against the budget the [roadmap](../../../docs/roadmap.md)
//! gives it — parse, lower and validate 1000 lines in 5 ms — and reports the
//! cost of a source at the 1 MiB limit, which has no budget of its own.
//!
//! Run it in release mode; a debug build says nothing about the budget:
//!
//! ```text
//! cargo run -p apiforge-project --example analysis --release --locked [-- --strict]
//! ```
//!
//! Without `--strict` it prints the measurements and exits successfully, which is
//! what a developer wants on an unknown machine. With `--strict` it fails when a
//! budget is exceeded, for a machine whose speed is known.

use apiforge_project::{AnalysisInput, DocumentKind, Origin, ProjectSession, Restored};
use apiforge_script::{LowerContext, lower, parse};
use std::time::{Duration, Instant};

/// The size limit the editor and the store accept.
const LIMIT: usize = 1024 * 1024;

/// One step: a request with headers, query, a body and a response sample of
/// `rows` rows, then an assertion wired to it — two blocks, so a scenario under
/// the 200-block limit can still reach either size. Large samples are how a
/// real source grows towards the limit: a pasted response.
fn step(index: usize, rows: usize) -> String {
    let after = if index == 0 {
        "begin".to_owned()
    } else {
        format!("ok_{}", index - 1)
    };
    let mut text = format!(
        "  // Step {index}.\n  @after({after})\n  item_{index} = POST \"/items/{index}\" {{\n    \
         headers {{ Accept: \"application/json\" }}\n    query {{ limit: 10 }}\n    body {{\n      \
         name: \"item {index}\",\n      price: 12.5\n    }}\n    timeout 5s\n    sample 200 {{\n      \
         rows: [\n"
    );
    for row in 0..rows {
        let comma = if row + 1 < rows { "," } else { "" };
        text.push_str(&format!(
            "        {{ id: \"r-{index}-{row}\", price: {row}.5 }}{comma}\n"
        ));
    }
    text.push_str(&format!(
        "      ]\n    }}\n  }}\n  ok_{index} = assert item_{index}.status == 200\n"
    ));
    text
}

/// A valid scenario of at most `lines` lines and `bytes` bytes, made of steps
/// with `rows` sample rows each.
fn scenario(lines: usize, bytes: usize, rows: usize) -> String {
    let head = "scenario \"Bench\" {\n  begin = start\n";
    let tail = "}\n";
    let mut text = head.to_owned();
    let mut index = 0;
    loop {
        let next = step(index, rows);
        let line_count = text.lines().count() + 1;
        if line_count + next.lines().count() > lines || text.len() + next.len() + tail.len() > bytes
        {
            break;
        }
        text.push_str(&next);
        index += 1;
    }
    text.push_str(tail);
    text
}

/// The median of `runs` runs, after one warm-up.
fn median(runs: usize, mut work: impl FnMut()) -> Duration {
    work();
    let mut times: Vec<Duration> = (0..runs)
        .map(|_| {
            let start = Instant::now();
            work();
            start.elapsed()
        })
        .collect();
    times.sort();
    times[runs / 2]
}

/// The pending analysis of a document edited to hold `text`, as the desktop's
/// worker receives it. With `edited` the document was already valid, so the
/// analysis reconciles against the session's model; otherwise it starts from
/// an empty source and lays every block out from scratch.
fn analysis_input(text: &str, edited: bool) -> (tempfile::TempDir, AnalysisInput) {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Bench").unwrap();
    // Creating a document analyses it at once.
    let initial = if edited { text } else { "" };
    let id = session
        .create_document(DocumentKind::Scenario, "bench.afs", initial)
        .unwrap();
    let changed = text.replacen("limit: 10", "limit: 11", 1);
    session.edit_source(&id, &changed, Origin(1)).unwrap();
    let input = session.pending_analyses().remove(0);
    (directory, input)
}

struct Check {
    name: &'static str,
    measured: Duration,
    budget: Option<Duration>,
}

fn main() {
    let strict = std::env::args().any(|argument| argument == "--strict");
    let mut checks = Vec::new();

    let thousand = scenario(1000, LIMIT, 2);
    let megabyte = scenario(usize::MAX, LIMIT, 300);
    println!(
        "sources: {} lines / {} bytes and {} lines / {} bytes",
        thousand.lines().count(),
        thousand.len(),
        megabyte.lines().count(),
        megabyte.len()
    );

    // An empty project's context, exactly as the session lowers with it.
    let directory = tempfile::tempdir().unwrap();
    let project = ProjectSession::create(directory.path(), "Bench")
        .unwrap()
        .context()
        .clone();
    let context = LowerContext::folder(&project);
    for (name, text, budget) in [
        (
            "parse + lower + validate, 1000 lines",
            &thousand,
            Some(Duration::from_millis(5)),
        ),
        ("parse + lower + validate, 1 MiB", &megabyte, None),
    ] {
        let lowered = lower(&parse(text), &context);
        assert!(!lowered.has_errors(), "{:?}", lowered.diagnostics);
        checks.push(Check {
            name,
            measured: median(11, || {
                let _ = lower(&parse(text), &context);
            }),
            budget,
        });
    }

    for (name, text, edited, runs) in [
        ("analysis after an edit, 1000 lines", &thousand, true, 11),
        (
            "first analysis with layout, 1000 lines",
            &thousand,
            false,
            11,
        ),
        ("analysis after an edit, 1 MiB", &megabyte, true, 3),
    ] {
        let (_directory, input) = analysis_input(text, edited);
        let result = input.run();
        assert!(result.is_valid(), "{:?}", result.diagnostics);
        let expected = if edited {
            Restored::Reconciled
        } else {
            Restored::Fresh
        };
        assert_eq!(result.restored(), Some(expected));
        checks.push(Check {
            name,
            measured: median(runs, || {
                let _ = input.run();
            }),
            budget: None,
        });
    }

    let mut failed = false;
    for check in &checks {
        let measured = check.measured.as_secs_f64() * 1000.0;
        match check.budget {
            Some(budget) => {
                let over = check.measured > budget;
                failed |= over;
                println!(
                    "{:<42} {measured:>9.2} ms  (budget {} ms){}",
                    check.name,
                    budget.as_millis(),
                    if over { "  OVER" } else { "" }
                );
            }
            None => println!("{:<42} {measured:>9.2} ms", check.name),
        }
    }
    if strict && failed {
        std::process::exit(1);
    }
}
