//! LANG-06 (#143) with KTLO-37 (#211): *Format* keeps every comment, and
//! `fmt` writes a statement's attributes in the one order `print` uses.
//!
//! A note — the comments directly above a statement and the one beside it —
//! stays that statement's note, written above it. Any other comment is free:
//! Format keeps it at the place it stands, and one inside something Format
//! writes on one line goes on its own line directly above that line.
use apiforge_core::format::Project;
use apiforge_script::{
    Document, LowerContext, Severity,
    ast::{Form, Statement},
    format, free_comments,
    lexer::{TokenKind, lex},
    lower, parse, print_folder_document, print_stored,
};
use std::path::{Path, PathBuf};

const EVERY_PLACE: &str = include_str!("fixtures/lang06/every-place.afs");
const EVERY_PLACE_FORMATTED: &str = include_str!("fixtures/lang06/every-place.formatted.afs");

/// `source` formatted is `expected`, and `expected` is already formatted.
fn formats(source: &str, expected: &str) {
    let formatted = format(source).unwrap_or_else(|errors| panic!("{source}\n{errors:#?}"));
    assert_eq!(formatted, expected, "source:\n{source}");
    assert_eq!(format(expected).unwrap(), expected, "not idempotent");
}

/// The text of every comment of `source`, as written after `//` without the
/// one space a note drops and without blanks at its end, in source order.
fn comments(source: &str) -> Vec<String> {
    let (tokens, _) = lex(source);
    tokens
        .iter()
        .filter(|token| token.kind == TokenKind::Comment)
        .map(|token| normalized(&source[token.range.clone()]))
        .collect()
}

fn normalized(comment: &str) -> String {
    let text = comment.strip_prefix("//").unwrap_or(comment).trim_end();
    text.strip_prefix(' ').unwrap_or(text).to_owned()
}

/// The free comments of `source`, in order, as written.
fn free(source: &str) -> Vec<String> {
    free_comments(source)
        .into_iter()
        .map(|range| source[range].trim_end().to_owned())
        .collect()
}

/// The note of every statement, by handle, in the order written.
fn notes(source: &str) -> Vec<(String, Option<String>)> {
    fn walk(statements: &[Statement], out: &mut Vec<(String, Option<String>)>) {
        for statement in statements {
            out.push((statement.handle.clone(), statement.note.clone()));
            if let Form::If {
                then, otherwise, ..
            } = &statement.form
            {
                walk(then, out);
                walk(otherwise.as_deref().unwrap_or_default(), out);
            }
        }
    }
    let file = parse(source).file.expect("a file");
    let mut out = vec![];
    walk(&file.statements, &mut out);
    walk(file.cleanup.as_deref().unwrap_or_default(), &mut out);
    out
}

/// Every `.afs` under the repository's `fixtures/`, with its path.
fn fixtures() -> Vec<(PathBuf, String)> {
    fn walk(directory: &Path, out: &mut Vec<(PathBuf, String)>) {
        let mut entries: Vec<_> = std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|extension| extension == "afs") {
                let text = std::fs::read_to_string(&path).unwrap();
                out.push((path, text));
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut out = vec![];
    walk(&root, &mut out);
    assert!(out.len() >= 10, "the fixtures are found: {}", out.len());
    out
}

/// The fixtures and the file of criterion 1.
fn every_file() -> Vec<(PathBuf, String)> {
    let mut files = fixtures();
    files.push((
        PathBuf::from("fixtures/lang06/every-place.afs"),
        EVERY_PLACE.to_owned(),
    ));
    files
}

// Criterion 1: no comment is lost, wherever it stands.

#[test]
fn lang06_criterion_1_a_note_stays_above_its_statement() {
    formats(
        "scenario \"Notes\" {\n  begin = start\n  // Above.\n  // Two lines.\n  list = GET \"/items\" // Beside.\n}\n",
        "scenario \"Notes\" {\n  begin = start\n  // Above.\n  // Two lines.\n  // Beside.\n  list = GET \"/items\"\n}\n",
    );
}

#[test]
fn lang06_criterion_1_between_statements() {
    formats(
        "scenario \"Between\" {\n  begin = start\n\n  // Between two statements.\n\n  list = GET \"/items\"\n}\n",
        "scenario \"Between\" {\n  begin = start\n  // Between two statements.\n\n  list = GET \"/items\"\n}\n",
    );
}

#[test]
fn lang06_criterion_1_at_the_top_of_a_block() {
    formats(
        "scenario \"Top\" {\n  // At the top.\n\n  begin = start\n}\n",
        "scenario \"Top\" {\n  // At the top.\n\n  begin = start\n}\n",
    );
}

#[test]
fn lang06_criterion_1_at_the_end_of_a_block() {
    formats(
        "scenario \"End\" {\n  begin = start\n  // At the end.\n}\n",
        "scenario \"End\" {\n  begin = start\n  // At the end.\n}\n",
    );
}

#[test]
fn lang06_criterion_1_in_a_branch() {
    formats(
        "scenario \"Branch\" {\n  begin = start\n  list = GET \"/items\"\n  found = if list.status == 200 {\n    // Top of then.\n\n    item = GET \"/items/1\"\n    // End of then.\n  }\n  // Between the braces.\n  else {\n    // Top of else.\n\n    gone = assert list.status == 404\n    // End of else.\n  }\n}\n",
        "scenario \"Branch\" {\n  begin = start\n  list = GET \"/items\"\n  found = if list.status == 200 {\n    // Top of then.\n\n    item = GET \"/items/1\"\n    // End of then.\n    // Between the braces.\n  } else {\n    // Top of else.\n\n    gone = assert list.status == 404\n    // End of else.\n  }\n}\n",
    );
}

#[test]
fn lang06_criterion_1_in_the_cleanup() {
    formats(
        "scenario \"Cleanup\" {\n  begin = start\n  // Above the cleanup.\n  cleanup {\n    // Top of the cleanup.\n\n    done = POST \"/done\"\n    // End of the cleanup.\n  }\n}\n",
        "scenario \"Cleanup\" {\n  begin = start\n  // Above the cleanup.\n  cleanup {\n    // Top of the cleanup.\n\n    done = POST \"/done\"\n    // End of the cleanup.\n  }\n}\n",
    );
}

#[test]
fn lang06_criterion_1_between_the_lines_of_the_head() {
    formats(
        "component lookup \"Lookup\" {\n  // Above the description.\n  description \"Finds one.\"\n  @label(\"Id\") // Beside an attribute.\n  // Between the attributes.\n  @required(true)\n  input id: string\n  // Above the action.\n  action result\n  result = GET \"/items/{{input.id}}\"\n}\n",
        "component lookup \"Lookup\" {\n  // Above the description.\n  description \"Finds one.\"\n  // Beside an attribute.\n  // Between the attributes.\n  @label(\"Id\")\n  @required(true)\n  input id: string\n  // Above the action.\n  action result\n  result = GET \"/items/{{input.id}}\"\n}\n",
    );
}

#[test]
fn lang06_criterion_1_between_the_lines_of_a_request_block() {
    formats(
        "scenario \"Request\" {\n  begin = start\n  list = GET \"/items\" { // After the brace.\n    query { page: 1 }\n    // Between two lines.\n    timeout 5s\n    // At the end.\n  }\n}\n",
        "scenario \"Request\" {\n  begin = start\n  list = GET \"/items\" {\n    // After the brace.\n    query { page: 1 }\n    // Between two lines.\n    timeout 5s\n    // At the end.\n  }\n}\n",
    );
}

#[test]
fn lang06_criterion_1_before_the_first_line() {
    formats(
        "// Before the first line.\n\nscenario // Inside the first line.\n\"First\" {\n  begin = start\n}\n",
        "// Before the first line.\n// Inside the first line.\nscenario \"First\" {\n  begin = start\n}\n",
    );
}

#[test]
fn lang06_criterion_1_after_the_last_brace() {
    formats(
        "scenario \"Last\" {\n  begin = start\n} // Beside the brace.\n\n// After it.\n",
        "scenario \"Last\" {\n  begin = start\n}\n// Beside the brace.\n// After it.\n",
    );
}

/// A comment inside a line of a request's block goes directly above that
/// line, where no statement begins.
#[test]
fn lang06_criterion_1_inside_a_line_of_a_request_block() {
    formats(
        "scenario \"Inside\" {\n  begin = start\n  list = POST \"/items\" {\n    body {\n      name: \"a\", // Beside a field.\n      // Above a field.\n      size: 2\n    }\n  }\n}\n",
        "scenario \"Inside\" {\n  begin = start\n  list = POST \"/items\" {\n    // Beside a field.\n    // Above a field.\n    body { name: \"a\", size: 2 }\n  }\n}\n",
    );
}

/// A comment inside a statement Format writes on one line goes above the
/// statement with a blank line between, so that it stays free.
#[test]
fn lang06_criterion_1_inside_a_statement_on_one_line() {
    formats(
        "scenario \"One line\" {\n  begin = start\n  items = [\n    1, // In a list.\n    2\n  ]\n  seen = assert items == [ // In a check.\n    1, 2]\n  empty = GET \"/items\" {\n    // In braces Format leaves out.\n  }\n  @label(\"Last\")\n  // Between the attribute and the statement.\n  last = 1\n}\n",
        "scenario \"One line\" {\n  begin = start\n  // In a list.\n\n  items = [1, 2]\n  // In a check.\n\n  seen = assert items == [1, 2]\n  // In braces Format leaves out.\n\n  empty = GET \"/items\"\n  // Between the attribute and the statement.\n\n  @label(\"Last\")\n  last = 1\n}\n",
    );
}

/// A comment among a condition's checks goes above the check that holds it
/// or that it stands before.
#[test]
fn lang06_criterion_1_inside_a_condition() {
    formats(
        "scenario \"Condition\" {\n  begin = start\n  list = GET \"/items\"\n  ready = condition any {\n    list.status == 200, // Beside a check.\n    // Above a check.\n    list.status == 204,\n    // At the end.\n  } with list.body\n}\n",
        "scenario \"Condition\" {\n  begin = start\n  list = GET \"/items\"\n  ready = condition any {\n    // Beside a check.\n    list.status == 200,\n    // Above a check.\n    list.status == 204,\n    // At the end.\n  } with list.body\n}\n",
    );
}

/// Mixed: a note beside a request that holds comments inside.
#[test]
fn lang06_criterion_1_a_note_beside_a_request_with_comments_inside() {
    formats(
        "scenario \"Mixed\" {\n  begin = start\n  list = GET \"/items\" { // Opens the block.\n    // Inside.\n    body { size: 2 }\n  } // Beside the request: its note.\n}\n",
        "scenario \"Mixed\" {\n  begin = start\n  // Beside the request: its note.\n  list = GET \"/items\" {\n    // Opens the block.\n    // Inside.\n    body { size: 2 }\n  }\n}\n",
    );
    assert_eq!(
        notes(EVERY_PLACE_FORMATTED)
            .into_iter()
            .find(|(handle, _)| handle == "begin")
            .unwrap()
            .1
            .as_deref(),
        Some("Directly above start: its note.\nBeside start: joins its note.")
    );
}

/// Mixed: a comment inside a value's object, on a statement with a note.
#[test]
fn lang06_criterion_1_a_comment_inside_a_value_object() {
    formats(
        "scenario \"Mixed\" {\n  begin = start\n  // The total's note.\n  total = { count: 1, // Inside the object.\n    items: [] }\n}\n",
        "scenario \"Mixed\" {\n  begin = start\n  // Inside the object.\n\n  // The total's note.\n  total = { count: 1, items: [] }\n}\n",
    );
}

/// Mixed: a free comment directly above a note, a blank line between.
#[test]
fn lang06_criterion_1_a_free_comment_directly_above_a_note() {
    let source =
        "scenario \"Mixed\" {\n  begin = start\n  // Free.\n\n  // The note.\n  next = 1\n}\n";
    formats(source, source);
    assert_eq!(free(source), ["// Free."]);
    assert_eq!(
        notes(source)[1],
        ("next".to_owned(), Some("The note.".to_owned()))
    );
}

/// Every place of the criterion in one file.
#[test]
fn lang06_criterion_1_every_place_in_one_file() {
    formats(EVERY_PLACE, EVERY_PLACE_FORMATTED);
    let mut before = comments(EVERY_PLACE);
    let mut after = comments(EVERY_PLACE_FORMATTED);
    assert_eq!(before.len(), 21);
    before.sort();
    after.sort();
    assert_eq!(before, after);
}

/// A deterministic generator, as in `properties.rs`.
fn random(state: &mut u64) -> usize {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    (*state >> 32) as usize
}

/// `source` with comments inserted after random tokens: beside the token,
/// on a line of their own, or between blank lines.
fn sprinkled(source: &str, state: &mut u64, count: usize) -> String {
    let (tokens, _) = lex(source);
    let mut at: Vec<usize> = (0..count)
        .map(|_| tokens[random(state) % tokens.len()].range.end)
        .collect();
    at.sort_unstable();
    let mut out = String::new();
    let mut from = 0;
    for (index, at) in at.into_iter().enumerate() {
        out.push_str(&source[from..at]);
        out.push_str(&match random(state) % 3 {
            0 => format!(" // g{index}\n"),
            1 => format!("\n// g{index}\n"),
            _ => format!("\n\n// g{index}\n\n"),
        });
        from = at;
    }
    out.push_str(&source[from..]);
    out
}

/// Criterion 1 over generated sources: the comments before and after
/// formatting are the same multiset; criteria 2 and 3 on the same sources.
#[test]
fn lang06_criterion_1_every_comment_survives() {
    let mut state = 0x1a06_c0de_u64;
    let mut checked = 0;
    for (path, base) in every_file() {
        for _ in 0..40 {
            let count = 1 + random(&mut state) % 12;
            let source = sprinkled(&base, &mut state, count);
            if parse(&source).has_errors() {
                continue;
            }
            let formatted = format(&source).unwrap();
            let mut before = comments(&source);
            let mut after = comments(&formatted);
            before.sort();
            after.sort();
            assert_eq!(
                before,
                after,
                "{}:\n{source}\nformatted:\n{formatted}",
                path.display()
            );
            assert_eq!(notes(&formatted), notes(&source), "{source}\n{formatted}");
            assert_eq!(free(&formatted), free(&source), "{source}\n{formatted}");
            assert_eq!(format(&formatted).unwrap(), formatted, "{source}");
            checked += 1;
        }
    }
    assert!(checked > 300, "only {checked} generated sources parse");
}

// Criterion 2: formatting changes no note.

#[test]
fn lang06_criterion_2_notes_stay_notes() {
    for (path, source) in every_file() {
        let formatted =
            format(&source).unwrap_or_else(|errors| panic!("{}: {errors:#?}", path.display()));
        assert_eq!(notes(&formatted), notes(&source), "{}", path.display());
        assert_eq!(free(&formatted), free(&source), "{}", path.display());
    }
    assert_eq!(free(EVERY_PLACE).len(), 19);
}

// Criterion 3: formatting twice gives the text of formatting once.

#[test]
fn lang06_criterion_3_idempotent() {
    for (path, source) in every_file() {
        let once = format(&source).unwrap();
        assert_eq!(format(&once).unwrap(), once, "{}", path.display());
    }
}

// Criterion 4: no warning; the list of free comments stays.

#[test]
fn lang06_criterion_4_no_warning() {
    let project = apiforge_core::demos::shop_project();
    let source = "// Above the file.\nscenario \"Free\" {\n  begin = start\n\n  // Between two statements.\n\n  // The note.\n  list = GET \"/products\" {\n    // In the block.\n    timeout 5s\n  }\n  // At the end.\n}\n";
    let tree = parse(source);
    assert!(
        tree.diagnostics.is_empty(),
        "parsing warns: {:#?}",
        tree.diagnostics
    );
    let lowered = lower(&tree, &LowerContext::folder(&project));
    assert!(
        !lowered
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Warning),
        "check warns: {:#?}",
        lowered.diagnostics
    );
    assert_eq!(
        free(source),
        [
            "// Above the file.",
            "// Between two statements.",
            "// In the block.",
            "// At the end."
        ]
    );
    assert!(free("scenario \"Noted\" {\n  // A note.\n  begin = start\n}\n").is_empty());
}

// Criterion 5 (KTLO-37): one attribute order.

/// Every ordering of `items`.
fn permutations(items: &[&'static str]) -> Vec<Vec<&'static str>> {
    if items.len() <= 1 {
        return vec![items.to_vec()];
    }
    let mut out = vec![];
    for index in 0..items.len() {
        let mut rest = items.to_vec();
        let first = rest.remove(index);
        for mut tail in permutations(&rest) {
            tail.insert(0, first);
            out.push(tail);
        }
    }
    out
}

#[test]
fn ktlo37_one_attribute_order() {
    let statement = [
        "@label(\"Load\")",
        "@note(\"Loads the list.\")",
        "@after(begin)",
        "@config({ method: \"GET\" })",
        "@inputs({ extra: 1 })",
    ];
    for order in permutations(&statement) {
        let source = format!(
            "scenario \"Order\" {{\n  begin = start\n  {}\n  list = GET \"/items\"\n}}\n",
            order.join("\n  ")
        );
        let expected = format!(
            "scenario \"Order\" {{\n  begin = start\n  {}\n  list = GET \"/items\"\n}}\n",
            statement.join("\n  ")
        );
        assert_eq!(format(&source).unwrap(), expected, "{source}");
    }
    for input in [
        [
            "@label(\"Id\")",
            "@contract(\"Item\")",
            "@required(true)",
            "@exposed(true)",
        ],
        [
            "@label(\"Id\")",
            "@contract(\"Item\")",
            "@optional",
            "@hidden",
        ],
    ] {
        for order in permutations(&input) {
            let source = format!(
                "component lookup \"Lookup\" {{\n  {} input id: string\n  result = GET \"/items\"\n}}\n",
                order.join(" ")
            );
            let expected = format!(
                "component lookup \"Lookup\" {{\n  {}\n  input id: string\n  result = GET \"/items\"\n}}\n",
                input.join("\n  ")
            );
            assert_eq!(format(&source).unwrap(), expected, "{source}");
        }
    }
}

/// The demo shop with the room booking's published `sign_in`: every
/// component a fixture calls.
fn project() -> Project {
    let mut project = apiforge_core::demos::shop_project();
    let revision =
        include_str!("../../../fixtures/scenarios/lay/rooms/.apiforge/revisions/sign-in/1.afs");
    let lowered = lower(&parse(revision), &LowerContext::folder(&project).stored());
    let Some(Document::Component(sign_in)) = lowered.document else {
        panic!("a component");
    };
    project.components.push(sign_in);
    project
}

#[test]
fn ktlo37_print_is_canonical() {
    let project = project();
    let mut printed = 0;
    for (path, source) in fixtures() {
        let stored = path
            .components()
            .any(|part| part.as_os_str() == "revisions");
        let context = if stored {
            LowerContext::folder(&project).stored()
        } else {
            LowerContext::folder(&project)
        };
        let lowered = lower(&parse(&source), &context);
        if lowered.has_errors() {
            continue;
        }
        let document = lowered.document.unwrap();
        let text = if stored {
            print_stored(&project, &document)
        } else {
            print_folder_document(&project, &document)
        }
        .unwrap_or_else(|errors| panic!("{}: {errors:#?}", path.display()));
        assert_eq!(format(&text).unwrap(), text, "{}", path.display());
        printed += 1;
    }
    assert!(printed >= 8, "only {printed} fixtures print");
}
