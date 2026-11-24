//! #578: after an edit on the board, *Problems* holds what `apiforge check`
//! reports for the text the window then holds: the warnings on the lines the
//! edit did not touch stay, with their ranges moved to the new text. The
//! session's side is `session::tests` of `apiforge-project`.
use super::diag06a::{graph, id_of, shop_with};
use super::project_interaction_tests::{replace_text, reveal};
use super::*;
use apiforge_script::{Diagnostic, FAR_READ, LowerContext};

const URL: &str = "URL or path";

fn far_value() -> String {
    fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scenarios/lay/far-value.afs"),
    )
    .unwrap()
}

fn select(harness: &mut FolderHarness, id: &str, handle: &str) {
    let node = id_of(&graph(harness, id), handle);
    app(harness).selections.insert(id.to_owned(), vec![node]);
    settle(harness);
    harness.step();
}

/// What `apiforge check` reports for the text the window holds.
fn checked(harness: &FolderHarness, id: &str) -> Vec<Diagnostic> {
    let text = source_of(harness, id);
    let project = folder(harness).context_project();
    let tree = apiforge_script::parse(&text);
    let lowered = apiforge_script::lower(&tree, &LowerContext::folder(&project));
    assert!(!lowered.has_errors(), "{:?}", lowered.diagnostics);
    let mut diagnostics = lowered.diagnostics.clone();
    let mut omitted = lowered.omitted_diagnostics;
    apiforge_script::push_warnings(
        &mut diagnostics,
        &mut omitted,
        apiforge_script::far_read_warnings(&tree, lowered.document.as_ref().unwrap(), &project),
    );
    assert_eq!(omitted, 0);
    diagnostics
}

fn diagnostics(harness: &FolderHarness, id: &str) -> Vec<Diagnostic> {
    folder(harness).snapshot(id).unwrap().diagnostics
}

/// The far-read warnings as the reference each covers and its offset in the
/// line, with the trimmed line itself.
fn far_reads(harness: &FolderHarness, id: &str) -> Vec<(String, usize, String)> {
    let text = source_of(harness, id);
    diagnostics(harness, id)
        .into_iter()
        .filter(|diagnostic| diagnostic.message == FAR_READ)
        .map(|diagnostic| {
            let start = text[..diagnostic.range.start]
                .rfind('\n')
                .map_or(0, |at| at + 1);
            let line = text[start..].lines().next().unwrap().to_owned();
            (
                text[diagnostic.range.clone()].to_owned(),
                diagnostic.range.start - start,
                line,
            )
        })
        .collect()
}

/// Criteria 1 and 2.
#[test]
fn issue578_far_read_warnings_stay_after_an_edit_in_the_side_panel() {
    let (_project, mut harness, id) = shop_with(&far_value());
    let before = far_reads(&harness, &id);
    assert_eq!(before.len(), 15, "the far reads of the fixture");
    let opened = diagnostics(&harness, &id);
    assert_eq!(opened, checked(&harness, &id));

    select(&mut harness, &id, "catalogue");
    reveal(&mut harness, URL);
    replace_text(&mut harness, URL, "/products/all");
    settle(&mut harness);
    harness.step();
    let text = source_of(&harness, &id);
    assert!(
        text.contains("GET \"/products/all\""),
        "the edit is written"
    );

    // Criterion 1: the same 15, each on its reference in the new text. No
    // far read is on the edited line, so the lines read as they did.
    let after = far_reads(&harness, &id);
    assert_eq!(after.len(), 15, "the far-read warnings stay");
    assert_eq!(after, before, "the same references on the same lines");
    for (reference, ..) in &after {
        assert!(
            ["cart.id", "cart.productId", "session.body.currency"].contains(&reference.as_str()),
            "a range covers its reference: {reference}"
        );
    }
    assert_ne!(
        diagnostics(&harness, &id),
        opened,
        "the ranges moved with the text"
    );

    // Criterion 2: what a fresh derive of the window's text reports.
    assert_eq!(diagnostics(&harness, &id), checked(&harness, &id));
}

/// Criterion 4, after an edit on the board: the fix offered for a warning
/// is the new text's, and *Apply fix* removes every warning of its value
/// and no other.
#[test]
fn issue578_apply_fix_after_an_edit_in_the_side_panel() {
    let (_project, mut harness, id) = shop_with(&far_value());
    select(&mut harness, &id, "catalogue");
    replace_text(&mut harness, URL, "/products/all");
    settle(&mut harness);
    harness.step();
    let text = source_of(&harness, &id);
    let others = far_reads(&harness, &id)
        .iter()
        .filter(|(reference, ..)| reference != "session.body.currency")
        .count();
    let index = diagnostics(&harness, &id)
        .iter()
        .position(|diagnostic| &text[diagnostic.range.clone()] == "session.body.currency")
        .expect("a warning on the currency after the edit");

    app(&mut harness).apply_source_fix(&id, index);
    settle(&mut harness);

    let fixed = source_of(&harness, &id);
    assert!(
        fixed.contains("  var currency = session.body.currency\n"),
        "{fixed}"
    );
    assert!(fixed.contains("GET \"/products/all\""), "the edit stays");
    let left = far_reads(&harness, &id);
    assert_eq!(left.len(), others, "only the value's warnings are gone");
    assert!(
        left.iter()
            .all(|(reference, ..)| reference != "session.body.currency")
    );
    assert_eq!(diagnostics(&harness, &id), checked(&harness, &id));
}
