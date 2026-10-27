//! MCP-01 (#60), criterion 5: the analysis keeps at most `MAX_DIAGNOSTICS`
//! per document and counts what it drops, so nothing is cut silently.
use apiforge_script::{LowerContext, MAX_DIAGNOSTICS, Severity, lower, parse};

fn with_notes(body: &str, notes: usize) -> String {
    let mut source = format!("scenario \"Notes\" {{\n{body}}}\n");
    for index in 0..notes {
        source.push_str(&format!("// note {index}\n"));
    }
    source
}

#[test]
fn diagnostics_beyond_the_limit_are_counted_by_the_parser_and_kept_by_lowering() {
    let source = with_notes("", 1101);
    let tree = parse(&source);
    assert_eq!(MAX_DIAGNOSTICS, 1000);
    assert_eq!(tree.diagnostics.len(), 1000);
    assert!(
        tree.diagnostics
            .iter()
            .all(|d| d.severity == Severity::Warning)
    );
    assert_eq!(tree.omitted_diagnostics, 101);

    let project = apiforge_core::demos::shop_project();
    let lowered = lower(&tree, &LowerContext::folder(&project));
    assert_eq!(lowered.diagnostics.len(), 1000);
    assert_eq!(lowered.omitted_diagnostics, 101);
}

#[test]
fn an_error_that_replaces_a_warning_at_the_limit_counts_the_warning() {
    // A full list of warnings, then an error found by lowering: the error is
    // kept in place of the last warning, which is counted as omitted.
    let source = with_notes("  export x: number = 1\n", 1000);
    let tree = parse(&source);
    assert_eq!(tree.diagnostics.len(), 1000);
    assert_eq!(tree.omitted_diagnostics, 0);

    let project = apiforge_core::demos::shop_project();
    let lowered = lower(&tree, &LowerContext::folder(&project));
    assert!(lowered.has_errors(), "{:#?}", &lowered.diagnostics[..1]);
    assert_eq!(lowered.diagnostics.len(), 1000);
    assert_eq!(lowered.omitted_diagnostics, 1);
}
