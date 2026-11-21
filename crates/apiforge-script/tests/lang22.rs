//! LANG-22 (#522): a type check that cannot pass is reported when it is
//! written.
//!
//! A type check names one of six types in quotes. Another name written as a
//! literal is an error at the literal (the owner's decision on #522), so the
//! file does not pass `check` and does not run; one of the six written
//! without quotes is told to put them on, with the fix that writes them,
//! unless a statement of that name exists. A type read from a value is left
//! to the run.
use apiforge_core::format::Project;
use apiforge_script::{Diagnostic, Fix, LowerContext, Lowered, Severity, lower, parse};

const TYPES: [&str; 6] = ["string", "number", "boolean", "object", "array", "null"];

fn project() -> Project {
    apiforge_core::demos::shop_project()
}

fn lowered(text: &str) -> Lowered {
    let project = project();
    lower(&parse(text), &LowerContext::folder(&project))
}

/// A scenario whose cart total, 160, is checked by each of `checks`.
fn scenario(checks: &str) -> String {
    format!(
        r#"scenario "Types" {{
  param kind = "number"
  begin = start
  cart = GET "/carts/c-1" {{
    sample 200 {{ id: "c-1", total: 160 }}
  }}
{checks}
}}
"#
    )
}

fn errors(lowered: &Lowered) -> Vec<&Diagnostic> {
    lowered
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .collect()
}

fn unknown_type(name: &str) -> String {
    format!(
        "Unknown type \"{name}\". A type is \"string\", \"number\", \"boolean\", \"object\", \"array\" or \"null\"."
    )
}

fn quotes(name: &str) -> String {
    format!("A type is written in quotes: is \"{name}\"")
}

/// The range of the only occurrence of `needle` in `text`.
fn range_of(text: &str, needle: &str) -> std::ops::Range<usize> {
    assert_eq!(text.matches(needle).count(), 1, "{needle} once in\n{text}");
    let start = text.find(needle).unwrap();
    start..start + needle.len()
}

fn apply(text: &str, fix: &Fix) -> String {
    let mut fixed = text.to_owned();
    fixed.replace_range(fix.range.clone(), &fix.replacement);
    fixed
}

#[test]
fn lang22_criterion_1_unknown_type_name() {
    // An assertion, an `if` and a condition's check report it alike.
    for (checks, literal) in [
        (
            r#"  typed = assert cart.body.total is "nubmer""#,
            r#""nubmer""#,
        ),
        (
            r#"  typed = if cart.body.total is "Number" {
  }"#,
            r#""Number""#,
        ),
        (
            r#"  typed = condition all { cart.status == 200, cart.body.total is "integer" }"#,
            r#""integer""#,
        ),
        (
            r#"  typed = assert cart.body.total is ("text")"#,
            r#""text""#,
        ),
    ] {
        let text = scenario(checks);
        let lowered = lowered(&text);
        let name = literal.trim_matches('"');
        assert_eq!(
            errors(&lowered),
            [&Diagnostic {
                range: range_of(&text, literal),
                severity: Severity::Error,
                message: unknown_type(name),
                fix: None,
            }],
            "{text}"
        );
        // An error: the document is not committed, so `check` fails and it
        // does not run.
        assert!(lowered.has_errors(), "{text}");
    }

    // The six names give none, in every form.
    for name in TYPES {
        for checks in [
            format!(r#"  typed = assert cart.body.total is "{name}""#),
            format!("  typed = if cart.body.total is \"{name}\" {{\n  }}"),
            format!(r#"  typed = condition any {{ cart.body.total is "{name}" }}"#),
        ] {
            let text = scenario(&checks);
            let lowered = lowered(&text);
            assert!(
                lowered.diagnostics.is_empty(),
                "{text}\n{:#?}",
                lowered.diagnostics
            );
        }
    }
}

#[test]
fn lang22_criterion_2_bare_type_word() {
    for name in TYPES {
        let text = scenario(&format!("  typed = assert cart.body.total is {name}"));
        let lowered = lowered(&text);
        let word = text.rfind(name).unwrap()..text.rfind(name).unwrap() + name.len();
        assert_eq!(
            errors(&lowered),
            [&Diagnostic {
                range: word.clone(),
                severity: Severity::Error,
                message: quotes(name),
                fix: Some(Fix {
                    range: word,
                    replacement: format!("\"{name}\""),
                }),
            }],
            "{text}"
        );
        // The fix writes the quotes, and the file then passes.
        let fixed = apply(&text, lowered.diagnostics[0].fix.as_ref().unwrap());
        assert_eq!(
            fixed,
            scenario(&format!("  typed = assert cart.body.total is \"{name}\""))
        );
        assert!(lowered_is_clean(&fixed), "{fixed}");
    }

    // A statement that really is called `number` is read as that statement.
    let text = scenario(
        r#"  number = "string"
  typed = assert cart.body.total is number"#,
    );
    let lowered = lowered(&text);
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
}

fn lowered_is_clean(text: &str) -> bool {
    lowered(text).diagnostics.is_empty()
}

#[test]
fn lang22_criterion_4_computed_type() {
    // A parameter, a field and an operator are read when the run comes: the
    // check is left to it, as today.
    for checks in [
        "  typed = assert cart.body.total is param.kind",
        "  typed = assert cart.body.total is cart.body.id",
        r#"  typed = assert cart.body.total is op.concat("num", "ber")"#,
    ] {
        let text = scenario(checks);
        let lowered = lowered(&text);
        assert!(
            lowered.diagnostics.is_empty(),
            "{text}\n{:#?}",
            lowered.diagnostics
        );
    }
}
