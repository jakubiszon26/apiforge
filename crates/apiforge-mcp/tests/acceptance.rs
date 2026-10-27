//! MCP-01 (#60): the read tools `project_overview`, `read_source`, `check` and
//! `print` on the demo shop, criteria 3 to 9. The stdio transport (criteria 1
//! and 2) is tested in `crates/apiforge-cli/tests/mcp.rs`, where Cargo builds
//! the `apiforge` binary.
use apiforge_mcp::{
    ErrorCode, positions,
    tools::{self, Holder, Kind, Severity},
};
use apiforge_project::{ProjectSession, prepare_project};
use apiforge_storage::project::FileState;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

const PRODUCT: &str = "components/product.afs";
const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const MISMATCH: &str = "The value's type does not match this input";

/// The demo shop as a fresh Git checkout: `.apiforge/local` is the
/// machine-local part a clone does not have.
fn checkout() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &apiforge_core::demos::shop_project()).unwrap();
    fs::remove_dir_all(directory.path().join(".apiforge/local")).unwrap();
    directory
}

fn edit(root: &Path, path: &str, change: impl FnOnce(&str) -> String) {
    let file = root.join(path);
    let text = fs::read_to_string(&file).unwrap();
    let changed = change(&text);
    assert_ne!(changed, text, "the fixture must change {path}");
    fs::write(file, changed).unwrap();
}

/// KTLO-25's `75c55f5` shape: `fetch_product` names no contract, so the
/// export that promises a `Product` takes a value of another type.
fn inject_mismatch(text: &str) -> String {
    text.replace("    contract \"Product\"\n", "").replace(
        ", contractId: \"Product\" })\n  fetch_product",
        " })\n  fetch_product",
    )
}

fn sha256(path: &Path) -> String {
    match FileState::of(&fs::read(path).unwrap()) {
        FileState::File { sha256, .. } => sha256,
        _ => unreachable!(),
    }
}

fn tree(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn walk(root: &Path, at: &Path, files: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(root).unwrap().to_owned();
            if path.is_dir() {
                files.insert(relative, None);
                walk(root, &path, files);
            } else {
                files.insert(relative, Some(fs::read(&path).unwrap()));
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(root, root, &mut files);
    files
}

#[test]
fn criterion_3_the_overview_describes_the_demo_shop() {
    let directory = checkout();
    let root = directory.path();
    let overview = tools::project_overview(root).unwrap();

    assert_eq!(overview.name, "Demo shop");
    assert_eq!(overview.base_url, "http://127.0.0.1:4318/demo/shop");
    assert!(overview.variables.is_empty());
    assert!(overview.secret_names.is_empty());
    let contracts: Vec<_> = overview.contracts.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(contracts, ["Product", "Cart"]);

    let documents: Vec<_> = overview
        .documents
        .iter()
        .map(|d| {
            (
                d.kind,
                d.path.as_str(),
                d.handle.as_deref(),
                d.base_revision,
            )
        })
        .collect();
    assert_eq!(
        documents,
        [
            (Kind::Scenario, SCENARIO, None, None),
            (
                Kind::Component,
                "components/add-to-cart.afs",
                Some("add_to_cart"),
                Some(1)
            ),
            (Kind::Component, PRODUCT, Some("product"), Some(1)),
            (
                Kind::Component,
                "components/active-promotion.afs",
                Some("active_promotion"),
                Some(1)
            ),
        ]
    );
    for document in &overview.documents {
        assert_eq!(
            document.source_sha256.as_deref(),
            Some(sha256(&root.join(&document.path)).as_str()),
            "{}",
            document.path
        );
    }

    assert_eq!(overview.revisions.len(), 3);
    for revision in &overview.revisions {
        assert_eq!(revision.revision, 1);
        assert!(revision.path.starts_with(".apiforge/revisions/"));
        assert_eq!(revision.source_sha256, sha256(&root.join(&revision.path)));
    }
    let mut handles: Vec<_> = overview
        .revisions
        .iter()
        .map(|r| r.handle.as_str())
        .collect();
    handles.sort();
    assert_eq!(handles, ["active_promotion", "add_to_cart", "product"]);

    // A clone has no head file: generation 0 until the first commit.
    assert_eq!(overview.generation, 0);
    assert_eq!(overview.holder, Holder::None);
    assert!(!overview.live_requests.allowed);
    assert_eq!(overview.live_requests.source, "no --allow-live");
    assert_eq!(overview.server_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(overview.reference_version, "");
}

#[test]
fn criterion_3_the_generation_is_the_head_generation_after_a_commit() {
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &apiforge_core::demos::shop_project()).unwrap();
    let overview = tools::project_overview(directory.path()).unwrap();
    let head: serde_json::Value = serde_json::from_slice(
        &fs::read(directory.path().join(".apiforge/local/head.json")).unwrap(),
    )
    .unwrap();
    assert!(overview.generation > 0);
    assert_eq!(serde_json::json!(overview.generation), head["generation"]);
}

#[test]
fn criterion_4_read_source_returns_the_saved_text_and_its_versions() {
    let directory = checkout();
    let root = directory.path();
    let source = tools::read_source(root, PRODUCT).unwrap();
    assert_eq!(source.path, PRODUCT);
    assert_eq!(source.text, fs::read_to_string(root.join(PRODUCT)).unwrap());
    assert_eq!(source.source_sha256, Some(sha256(&root.join(PRODUCT))));
    assert_eq!(source.generation, 0);
    assert!(source.valid);
    assert_eq!(source.unavailable, None);
    assert!(source.diagnostics.is_empty());
    assert_eq!(source.omitted, 0);
}

#[test]
fn criterion_4_positions_count_scalar_values_after_multi_byte_characters() {
    let directory = checkout();
    let root = directory.path();
    edit(root, PRODUCT, |text| {
        text.replace(
            "description \"Fetch a product by ID together with its promotion.\"",
            "description \"Zażółć gęślą 🦀 jaźń\" ?? \"ł\"",
        )
    });
    let source = tools::read_source(root, PRODUCT).unwrap();
    assert!(!source.valid);
    let line = source.text.lines().nth(1).unwrap();
    let column = line[..line.find("??").unwrap()].chars().count() + 1;
    assert_ne!(
        column,
        line.find("??").unwrap() + 1,
        "the line has multi-byte text"
    );
    let diagnostic = &source.diagnostics[0];
    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(
        (
            diagnostic.line,
            diagnostic.column,
            diagnostic.end_line,
            diagnostic.end_column
        ),
        (2, column, 2, column + 1)
    );
}

#[test]
fn criterion_4_a_crlf_source_counts_the_carriage_return_as_part_of_the_break() {
    let directory = checkout();
    let root = directory.path();
    edit(root, PRODUCT, |text| {
        inject_mismatch(text).replace('\n', "\r\n")
    });
    let source = tools::read_source(root, PRODUCT).unwrap();
    assert!(source.text.contains("\r\n"));
    assert!(source.valid, "a warning leaves the definition valid");
    let diagnostic = &source.diagnostics[0];
    assert_eq!(diagnostic.severity, Severity::Warning);
    assert_eq!(diagnostic.message, MISMATCH);
    let width = "fetch_product.body".chars().count();
    assert_eq!(
        (
            diagnostic.line,
            diagnostic.column,
            diagnostic.end_line,
            diagnostic.end_column
        ),
        (15, 28, 15, 28 + width)
    );
}

#[test]
fn criterion_4_the_position_rule() {
    // Scalar values, not bytes or UTF-16 units.
    assert_eq!(positions::range("ż🦀x", 6..7), (1, 3, 1, 4));
    // The `\r` of a CRLF belongs to the break: an offset on it or between it
    // and the `\n` is the end of the line; after the `\n` is the next line.
    assert_eq!(positions::range("ab\r\ncd", 2..4), (1, 3, 2, 1));
    assert_eq!(positions::range("ab\r\ncd", 3..5), (1, 3, 2, 2));
    // A lone `\r` is an ordinary character.
    assert_eq!(positions::range("a\rb\nc", 2..5), (1, 3, 2, 2));
    // The end of the text, and an empty range.
    assert_eq!(positions::range("ab\n", 3..3), (2, 1, 2, 1));
}

#[test]
fn criterion_4_a_fix_comes_with_its_replacement() {
    let directory = checkout();
    let root = directory.path();
    edit(root, SCENARIO, |text| {
        text.replacen(
            "product = product@1(id: 123)",
            "product = product(id: 123)",
            1,
        )
    });
    let source = tools::read_source(root, SCENARIO).unwrap();
    let fixed: Vec<_> = source
        .diagnostics
        .iter()
        .filter_map(|d| d.fix.as_ref().map(|fix| (d.line, d.column, fix)))
        .collect();
    assert_eq!(fixed.len(), 1, "{:#?}", source.diagnostics);
    let (line, column, fix) = fixed[0];
    assert_eq!(fix.replacement, "product@1");
    assert_eq!((fix.line, fix.column), (line, column));
    assert_eq!(
        (fix.end_line, fix.end_column),
        (line, column + "product".len())
    );
}

#[test]
fn criterion_4_an_unknown_path_names_the_registered_paths() {
    let directory = checkout();
    let error = tools::read_source(directory.path(), "components/missing.afs").unwrap_err();
    assert_eq!(error.code, ErrorCode::UnknownDocument);
    assert!(!error.wrote);
    let mut paths: Vec<_> = error.details["registered_paths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap().to_owned())
        .collect();
    paths.sort();
    assert_eq!(
        paths,
        [
            "components/active-promotion.afs",
            "components/add-to-cart.afs",
            PRODUCT,
            SCENARIO
        ]
    );
}

#[test]
fn criterion_5_check_reports_the_injected_warning_with_file_line_and_column() {
    let directory = checkout();
    let root = directory.path();
    edit(root, PRODUCT, inject_mismatch);

    let all = tools::check(root, None).unwrap();
    assert_eq!(all.documents.len(), 4);
    assert!(all.valid);
    let problems: Vec<_> = all
        .documents
        .iter()
        .flat_map(|document| {
            document.diagnostics.iter().map(move |d| {
                (
                    document.path.as_str(),
                    d.line,
                    d.column,
                    d.severity,
                    d.message.as_str(),
                )
            })
        })
        .collect();
    assert_eq!(problems, [(PRODUCT, 15, 28, Severity::Warning, MISMATCH)]);

    let one = tools::check(root, Some(PRODUCT)).unwrap();
    assert_eq!(one.documents.len(), 1);
    assert_eq!(one.documents[0].path, PRODUCT);
    assert_eq!(
        one.documents[0].source_sha256,
        Some(sha256(&root.join(PRODUCT)))
    );
    assert_eq!(one.documents[0].diagnostics.len(), 1);
}

#[test]
fn criterion_5_a_project_with_errors_is_reported_not_refused() {
    let directory = checkout();
    let root = directory.path();
    edit(root, PRODUCT, |text| {
        text.replacen("export product", "export ???", 1)
    });
    let result = tools::check(root, None).unwrap();
    assert!(!result.valid);
    let product = result.documents.iter().find(|d| d.path == PRODUCT).unwrap();
    assert!(!product.valid);
    assert!(
        product
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    );
    assert!(
        result
            .documents
            .iter()
            .filter(|d| d.path != PRODUCT)
            .all(|d| d.valid)
    );
}

#[test]
fn criterion_5_beyond_a_thousand_diagnostics_the_rest_are_counted() {
    let directory = checkout();
    let root = directory.path();
    // 1 101 comments that belong to no statement: 1 101 warnings.
    edit(root, PRODUCT, |text| {
        let mut text = text.to_owned();
        for index in 0..1101 {
            text.push_str(&format!("// note {index}\n"));
        }
        text
    });
    let session = ProjectSession::open_saved_read_only(root).unwrap();
    let document = session
        .documents()
        .iter()
        .find(|d| d.entry().path.as_str() == PRODUCT)
        .unwrap();
    assert_eq!(
        document.diagnostics().len(),
        1000,
        "the analysis keeps 1 000"
    );

    let result = tools::check(root, Some(PRODUCT)).unwrap();
    let product = &result.documents[0];
    assert_eq!(product.diagnostics.len(), 1000);
    assert_eq!(product.omitted, 101);
    assert!(
        product
            .diagnostics
            .iter()
            .all(|d| d.severity == Severity::Warning)
    );
    let source = tools::read_source(root, PRODUCT).unwrap();
    assert_eq!((source.diagnostics.len(), source.omitted), (1000, 101));
}

#[test]
fn criterion_6_print_returns_the_canonical_text() {
    let directory = checkout();
    let root = directory.path();
    let session = ProjectSession::open_saved_read_only(root).unwrap();
    for document in session.documents() {
        let path = document.entry().path.as_str();
        let expected =
            apiforge_script::print_folder_document(session.context(), document.model().unwrap())
                .unwrap();
        let printed = tools::print(root, path).unwrap();
        assert_eq!(printed.text, expected, "{path}");
        assert_eq!(printed.path, path);
        assert_eq!(printed.source_sha256, Some(sha256(&root.join(path))));
    }
}

#[test]
fn criterion_6_print_of_an_invalid_source_is_invalid_text_with_the_diagnostics() {
    let directory = checkout();
    let root = directory.path();
    edit(root, PRODUCT, |text| {
        text.replacen("export product", "export ???", 1)
    });
    let error = tools::print(root, PRODUCT).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidText);
    assert!(!error.wrote);
    let diagnostics = error.details["diagnostics"].as_array().unwrap();
    assert!(!diagnostics.is_empty());
    assert_eq!(diagnostics[0]["severity"], "error");
    assert!(diagnostics[0]["line"].as_u64().unwrap() >= 1);
}

#[test]
fn criterion_7_a_session_of_all_four_tools_writes_nothing() {
    let directory = checkout();
    let root = directory.path();
    let before = tree(root);
    tools::project_overview(root).unwrap();
    for path in [SCENARIO, PRODUCT] {
        tools::read_source(root, path).unwrap();
        tools::check(root, Some(path)).unwrap();
        tools::print(root, path).unwrap();
    }
    tools::check(root, None).unwrap();
    let _ = tools::read_source(root, "components/missing.afs");
    assert_eq!(tree(root), before);
    assert!(!root.join(".apiforge/local").exists());
}

#[test]
fn criterion_8_outside_a_project_every_tool_answers_not_a_project() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let codes = [
        tools::project_overview(root).unwrap_err().code,
        tools::read_source(root, PRODUCT).unwrap_err().code,
        tools::check(root, None).unwrap_err().code,
        tools::check(root, Some(PRODUCT)).unwrap_err().code,
        tools::print(root, PRODUCT).unwrap_err().code,
    ];
    assert_eq!(codes, [ErrorCode::NotAProject; 5]);
    assert_eq!(fs::read_dir(root).unwrap().count(), 0, "nothing is created");
}

#[test]
fn criterion_9_errors_have_the_contract_shape() {
    let directory = checkout();
    let error = tools::read_source(directory.path(), "components/missing.afs").unwrap_err();
    let value = serde_json::to_value(&error).unwrap();
    let mut keys: Vec<_> = value.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["code", "details", "message", "wrote"]);
    assert_eq!(value["code"], "unknown_document");
    assert_eq!(value["wrote"], false);
}

#[test]
fn criterion_9_paths_that_fail_working_source_are_rejected() {
    let directory = checkout();
    let root = directory.path();
    for path in [
        "../x.afs",
        "/etc/x.afs",
        ".apiforge/diagram.json",
        ".apiforge/revisions/get-product/1.afs",
        "components/CON.afs",
        "components/com1.afs",
    ] {
        for code in [
            tools::read_source(root, path).unwrap_err(),
            tools::check(root, Some(path)).unwrap_err(),
            tools::print(root, path).unwrap_err(),
        ]
        .map(|error| {
            assert!(!error.message.contains(path), "the argument is not quoted");
            error.code
        }) {
            assert_eq!(code, ErrorCode::PathRejected, "{path}");
        }
    }
}

#[test]
fn criterion_9_an_unreadable_project_is_project_unreadable() {
    let directory = checkout();
    let root = directory.path();
    fs::write(root.join("apiforge.project.json"), "{").unwrap();
    let error = tools::project_overview(root).unwrap_err();
    assert_eq!(error.code, ErrorCode::ProjectUnreadable);
    assert!(!error.wrote);
    assert!(!error.message.is_empty());
}
