//! LANG-05 (#142): a check that is one Boolean expression is written as the
//! author wrote it.
//!
//! The owner's decision A (2026-11-20): an expression that is a comparison or
//! a logical operation which, written alone, reads back as the same check
//! (`>=`, `<=`, `&&`, `||`, `!` at its top) is written without `== true`; a
//! bare value keeps the long form. The parser reads both spellings, which
//! lower to equal documents. `fmt` (the tree) and `print` (the document, as
//! the window and publication write it) agree.
use apiforge_core::format::Project;
use apiforge_script::{
    Document, LowerContext, Severity, ast::Form, format, lower, parse, print_folder_document,
};
use std::path::{Path, PathBuf};

fn project() -> Project {
    apiforge_core::demos::shop_project()
}

/// A scenario around one statement, in canonical form.
fn scenario(statement: &str) -> String {
    format!(
        "scenario \"Checks\" {{\n  on_error stop\n  begin = start\n  flag = true\n  a = GET \"/a\" {{\n    sample 200 {{ n: 1, m: 2, flag: true, items: [1] }}\n  }}\n{statement}}}\n"
    )
}

/// The three statement kinds that hold a check, around `check`.
fn assert_statement(check: &str) -> String {
    scenario(&format!("  ok = assert {check}\n"))
}
fn if_statement(check: &str) -> String {
    scenario(&format!(
        "  x = if {check} {{\n    b = GET \"/b\" {{\n      sample 200 {{}}\n    }}\n  }}\n"
    ))
}
fn condition_statement(check: &str) -> String {
    scenario(&format!(
        "  c = condition all {{\n    {check},\n    a.status == 200,\n  }}\n"
    ))
}

fn lowered(project: &Project, text: &str) -> Document {
    let lowered = lower(&parse(text), &LowerContext::folder(project));
    let errors: Vec<_> = lowered
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "{text}\n{errors:#?}");
    lowered.document.expect("a document")
}

/// What `fmt` writes for `text`.
fn formatted(text: &str) -> String {
    format(text).unwrap_or_else(|d| panic!("{text}\n{d:#?}"))
}

/// What the window and `print` write for the document `text` lowers to.
fn printed(project: &Project, text: &str) -> String {
    print_folder_document(project, &lowered(project, text))
        .unwrap_or_else(|d| panic!("{text}\n{d:#?}"))
}

/// The check as typed, the check as written by fmt and print.
const SHORTENED: &[(&str, &str)] = &[
    ("a.body.n >= a.body.m", "a.body.n >= a.body.m"),
    ("a.body.n <= a.body.m", "a.body.n <= a.body.m"),
    (
        "a.body.n == 1 && a.body.m != 2",
        "a.body.n == 1 && a.body.m != 2",
    ),
    ("a.body.n > 1 || a.body.flag", "a.body.n > 1 || a.body.flag"),
    ("!a.body.flag", "!a.body.flag"),
    ("!(a.body.n == 1)", "!(a.body.n == 1)"),
    // The 0.1.0 spelling becomes the short one.
    ("(a.body.n >= a.body.m) == true", "a.body.n >= a.body.m"),
    ("a.body.n >= a.body.m == true", "a.body.n >= a.body.m"),
    (
        "(a.body.n == 1 && a.body.m != 2) == true",
        "a.body.n == 1 && a.body.m != 2",
    ),
    (
        "(a.body.n > 1 || a.body.flag) == true",
        "a.body.n > 1 || a.body.flag",
    ),
    ("!a.body.flag == true", "!a.body.flag"),
];

/// A bare value keeps `== true`: alone it does not say what is expected.
const KEPT_LONG: &[(&str, &str)] = &[
    ("flag", "flag == true"),
    ("a.body.flag", "a.body.flag == true"),
    ("true", "true == true"),
    ("uuid()", "uuid() == true"),
    ("flag == true", "flag == true"),
    ("a.body.n + 1", "a.body.n + 1 == true"),
];

/// A predicate of its own is written as typed, and `(a == b) == true` is
/// another check than `a == b`, so it stays.
const OWN_PREDICATE: &[&str] = &[
    "a.body.n == 1",
    "a.body.n != 1",
    "a.body.n > 1",
    "a.body.n < 1",
    "a.body.items contains 1",
    "(a.body.n == 1) == true",
    "(a.body.n > 1) == true",
];

fn criterion_1(statement: fn(&str) -> String) {
    let project = project();
    let cases = SHORTENED
        .iter()
        .chain(KEPT_LONG)
        .copied()
        .chain(OWN_PREDICATE.iter().map(|check| (*check, *check)));
    for (typed, written) in cases {
        let text = statement(typed);
        let expected = statement(written);
        assert_eq!(formatted(&text), expected, "fmt of `{typed}`");
        assert_eq!(printed(&project, &text), expected, "print of `{typed}`");
    }
}

#[test]
fn lang05_criterion_1_assert() {
    criterion_1(assert_statement);
}

#[test]
fn lang05_criterion_1_if() {
    criterion_1(if_statement);
}

#[test]
fn lang05_criterion_1_condition() {
    criterion_1(condition_statement);
}

#[test]
fn lang05_criterion_2_round_trip() {
    let project = project();
    for statement in [assert_statement, if_statement, condition_statement] {
        for (typed, written) in SHORTENED.iter().chain(KEPT_LONG) {
            let text = statement(typed);
            let canonical = statement(written);
            // Text -> document -> text is a fixed point of the canonical form.
            assert_eq!(formatted(&canonical), canonical, "fmt of `{written}`");
            assert_eq!(
                printed(&project, &canonical),
                canonical,
                "print of `{written}`"
            );
            // Both spellings lower to equal documents.
            assert_eq!(
                lowered(&project, &text),
                lowered(&project, &canonical),
                "`{typed}` and `{written}`"
            );
        }
        // The explicit long form of a compound check lowers as the short one.
        for (short, long) in [
            ("a.body.n >= a.body.m", "(a.body.n >= a.body.m) == true"),
            ("!a.body.flag", "!a.body.flag == true"),
            (
                "a.body.n == 1 && a.body.flag",
                "(a.body.n == 1 && a.body.flag) == true",
            ),
        ] {
            assert_eq!(
                lowered(&project, &statement(short)),
                lowered(&project, &statement(long)),
                "`{short}` and `{long}`"
            );
        }
    }
}

#[test]
fn lang05_criterion_3_only_true_is_dropped() {
    let project = project();
    for statement in [assert_statement, if_statement, condition_statement] {
        for check in [
            "(a.body.n >= a.body.m) == false",
            "(a.body.n >= a.body.m) != true",
            "(a.body.n >= a.body.m) == flag",
            "(a.body.n >= a.body.m) == 1",
            "(a.body.n == 1 && a.body.flag) == false",
            "(a.body.n > 1 || a.body.flag) != true",
            "!a.body.flag == false",
        ] {
            let text = statement(check);
            assert_eq!(formatted(&text), text, "fmt of `{check}`");
            assert_eq!(printed(&project, &text), text, "print of `{check}`");
        }
    }
}

fn afs_files(directory: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            afs_files(&path, found);
        } else if path.extension().is_some_and(|e| e == "afs") {
            found.push(path);
        }
    }
}

/// Every check of `statements`, nested ones included.
fn checks<'a>(
    statements: &'a [apiforge_script::ast::Statement],
    out: &mut Vec<&'a str>,
    text: &'a str,
) {
    for statement in statements {
        match &statement.form {
            Form::Assert(check) => out.push(&text[check.range.clone()]),
            Form::Condition { checks: all, .. } => {
                out.extend(all.iter().map(|check| &text[check.range.clone()]))
            }
            Form::If {
                check,
                then,
                otherwise,
            } => {
                out.push(&text[check.range.clone()]);
                checks(then, out, text);
                if let Some(otherwise) = otherwise {
                    checks(otherwise, out, text);
                }
            }
            _ => {}
        }
    }
}

/// Every `.afs` under `fixtures/` writes each check in the canonical form:
/// formatting a file alone with that check gives the check as it is.
#[test]
fn lang05_criterion_4_every_fixture_check_is_canonical() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut files = Vec::new();
    afs_files(&root, &mut files);
    assert!(!files.is_empty());
    let mut failures = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path).unwrap();
        let tree = parse(&text);
        let Some(file) = &tree.file else { continue };
        let mut found = Vec::new();
        checks(&file.statements, &mut found, &text);
        if let Some(cleanup) = &file.cleanup {
            checks(cleanup, &mut found, &text);
        }
        for check in found {
            let alone = format!("scenario \"X\" {{\n  ok = assert {check}\n}}\n");
            let Ok(canonical) = format(&alone) else {
                failures.push(format!("{}: `{check}` does not format", path.display()));
                continue;
            };
            if canonical != alone {
                failures.push(format!(
                    "{}: `{check}` is written\n{canonical}",
                    path.display()
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
