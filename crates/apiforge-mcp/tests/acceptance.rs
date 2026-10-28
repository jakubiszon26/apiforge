//! MCP-01 (#60): the read tools `project_overview`, `read_source`, `check` and
//! `print` on the demo shop, criteria 3 to 9. The stdio transport (criteria 1
//! and 2) is tested in `crates/apiforge-cli/tests/mcp.rs`, where Cargo builds
//! the `apiforge` binary.
//!
//! MCP-03 (#62): `check_text`, `run`, `language_reference` and the resources,
//! criteria 1 to 12, and the five carry-overs from the review of MCP-01 (the
//! `mcp03_` tests). Their stdio side is in `crates/apiforge-cli/tests/mcp.rs`.
//!
//! MCP-04 (#63): `write_source`, `create_document`, `finish_editing` and the
//! agent session that holds the writer lock, criteria 1 to 10 (the `mcp04_`
//! tests). EOF on stdin and the client's name are tested over stdio in
//! `crates/apiforge-cli/tests/mcp.rs`; the window's side (criteria 6 and 11)
//! in `crates/apiforge-desktop/src/folder/agent_session_tests.rs`.
//!
//! MCP-06 (#65): live runs through `run(path, live: true)`, the permission
//! slot, the run journal and the ends of a run, secrets (the `mcp06_`
//! tests), against the demo API in this process or a loopback listener that
//! never answers. EOF, a killed server and the whole session's secret grep
//! are over stdio in `crates/apiforge-cli/tests/mcp.rs`.
use apiforge_mcp::{
    ErrorCode, ToolError, Wrote,
    executor::{self, CancellationToken, DEADLINE, EndedBy},
    positions, reference, resources,
    tools::{
        self, Holder, Kind, Phase, RunArguments, RunSettings, Runs, Severity, Step, StepStatus,
        Verdict,
    },
};
use apiforge_project::{ProjectSession, prepare_project};
use apiforge_storage::project::FileState;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
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
    // MCP-03 criterion 12: the version of the reference `language_reference`
    // serves (empty until MCP-03).
    assert_eq!(overview.reference_version, "1");
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
    assert_eq!(error.wrote, Wrote::No);
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
    assert_eq!(error.wrote, Wrote::No);
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
    assert_eq!(error.wrote, Wrote::No);
    assert!(!error.message.is_empty());
}

// MCP-03 (#62) ---------------------------------------------------------------

const PROMOTION: &str = "components/active-promotion.afs";
const MANIFEST: &str = "apiforge.project.json";
const DEMO_BASE_URL: &str = "http://127.0.0.1:4318/demo/shop";

/// Every top-level step of the demo scenario, the cleanup's last.
const DEMO_STEPS: [&str; 8] = [
    "product",
    "active_promotion",
    "expected_price",
    "add_to_cart",
    "promotional_price",
    "field_inna_nazwa",
    "delete_cart",
    "cart_was_deleted",
];

/// The sample product of `components/product.afs`, a valid `Product`.
fn sample_product() -> Value {
    json!({
        "id": 123,
        "name": "Whole bean coffee",
        "price": 100,
        "promotion": {
            "type": "fixed_price",
            "active": true,
            "price": 80,
            "inna_nazwa": "Autumn offer"
        }
    })
}

fn arguments(path: &str) -> RunArguments {
    RunArguments {
        path: path.into(),
        ..RunArguments::default()
    }
}

fn with_inputs(path: &str, inputs: Value) -> RunArguments {
    RunArguments {
        inputs: Some(inputs.as_object().unwrap().clone()),
        ..arguments(path)
    }
}

fn simulate(root: &Path, arguments: &RunArguments) -> Result<tools::RunReport, ToolError> {
    tools::run(root, arguments, &Runs::default(), CancellationToken::new())
}

fn count(steps: &[Step]) -> usize {
    steps
        .iter()
        .map(|step| 1 + step.steps.as_deref().map_or(0, count))
        .sum()
}

fn flatten(steps: &[Step]) -> Vec<&Step> {
    let mut all = Vec::new();
    for step in steps {
        all.push(step);
        if let Some(children) = &step.steps {
            all.extend(flatten(children));
        }
    }
    all
}

fn handles(steps: &[Step]) -> Vec<&str> {
    steps.iter().map(|step| step.handle.as_str()).collect()
}

/// The text a diagnostic's range covers, from its 1-based line and column in
/// Unicode scalar values.
fn covered<'a>(text: &'a str, diagnostic: &tools::Diagnostic) -> &'a str {
    let offset = |line: usize, column: usize| {
        let start: usize = text
            .split_inclusive('\n')
            .take(line - 1)
            .map(str::len)
            .sum();
        start
            + text[start..]
                .char_indices()
                .nth(column - 1)
                .map_or(text.len() - start, |(at, _)| at)
    };
    &text[offset(diagnostic.line, diagnostic.column)
        ..offset(diagnostic.end_line, diagnostic.end_column)]
}

/// A listener that counts the connections it accepts and answers none.
struct Listener {
    address: std::net::SocketAddr,
    accepted: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
}

impl Listener {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let accepted = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (counter, stopped) = (accepted.clone(), stop.clone());
        thread::spawn(move || {
            // Accepted sockets stay open and unanswered until the test ends.
            let mut open = Vec::new();
            while !stopped.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((socket, _)) => {
                        counter.fetch_add(1, Ordering::AcqRel);
                        open.push(socket);
                    }
                    Err(_) => thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        Self {
            address,
            accepted,
            stop,
        }
    }

    fn accepted(&self) -> usize {
        self.accepted.load(Ordering::Acquire)
    }

    fn base_url(&self) -> String {
        format!("http://{}/demo/shop", self.address)
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

/// The demo scenario as `apiforge run --folder` prepares it.
fn demo_request(root: &Path, mode: apiforge_core::run::RunMode) -> apiforge_runner::RunRequest {
    let mut session = ProjectSession::open_saved_read_only(root).unwrap();
    let id = session
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == SCENARIO)
        .unwrap()
        .id()
        .to_owned();
    let snapshot = session.current_snapshot(&id).unwrap();
    let apiforge_storage::project::IdentityBase::Scenario(scenario) = snapshot.document() else {
        unreachable!()
    };
    let mut project = snapshot.context().clone();
    project.scenarios.push(scenario.clone());
    let mut request = apiforge_runner::RunRequest::new(project, scenario.id.clone(), mode);
    request.target = apiforge_runner::RunTarget::Scenario;
    request
}

#[test]
fn mcp03_criterion_1_check_text_reports_the_proposal_for_a_registered_path() {
    let directory = checkout();
    let root = directory.path();
    let before = tree(root);
    let saved = fs::read_to_string(root.join(PRODUCT)).unwrap();
    let overview = tools::project_overview(root).unwrap();

    let proposal = inject_mismatch(&saved);
    let result = tools::check_text(root, PRODUCT, &proposal).unwrap();
    assert!(result.registered);
    assert_eq!(result.path, PRODUCT);
    assert_eq!(result.kind, Some(Kind::Component));
    assert_eq!(result.handle.as_deref(), Some("product"));
    assert!(result.valid, "a warning leaves the text valid");
    assert!(result.problems.is_empty());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].severity, Severity::Warning);
    assert_eq!(result.diagnostics[0].message, MISMATCH);
    assert_eq!(result.omitted, 0);
    assert_eq!(result.generation, overview.generation);
    assert_eq!(
        result.source_sha256,
        Some(sha256(&root.join(PRODUCT))),
        "the saved document the text would replace"
    );
    let mut used: Vec<_> = result
        .used
        .iter()
        .map(|used| {
            (
                used.handle.as_str(),
                used.revision,
                used.source_sha256.as_str(),
            )
        })
        .collect();
    used.sort();
    let mut revisions: Vec<_> = overview
        .revisions
        .iter()
        .map(|r| (r.handle.as_str(), r.revision, r.source_sha256.as_str()))
        .collect();
    revisions.sort();
    assert_eq!(used, revisions);

    // Positions as `read_source` gives them for the same text saved.
    let saved_copy = checkout();
    fs::write(saved_copy.path().join(PRODUCT), &proposal).unwrap();
    let read = tools::read_source(saved_copy.path(), PRODUCT).unwrap();
    assert_eq!(result.diagnostics, read.diagnostics);

    let broken = saved.replacen("export product", "export ???", 1);
    let result = tools::check_text(root, PRODUCT, &broken).unwrap();
    assert!(!result.valid);
    assert_eq!(result.diagnostics[0].severity, Severity::Error);
    fs::write(saved_copy.path().join(PRODUCT), &broken).unwrap();
    let read = tools::read_source(saved_copy.path(), PRODUCT).unwrap();
    assert_eq!(result.diagnostics, read.diagnostics);

    assert_eq!(tree(root), before, "the file on disk is unchanged");
    assert!(!root.join(".apiforge/local").exists());
}

#[test]
fn mcp03_criterion_1_a_text_over_one_mebibyte_is_too_large_before_anything_else() {
    let directory = checkout();
    let text = "a".repeat(tools::MAX_TEXT_BYTES + 1);
    for path in [PRODUCT, "../x.afs", "components/new.afs"] {
        let error = tools::check_text(directory.path(), path, &text).unwrap_err();
        assert_eq!(error.code, ErrorCode::TooLarge, "{path}");
        assert_eq!(error.wrote, Wrote::No);
    }
    let error = tools::check_text(tempfile::tempdir().unwrap().path(), PRODUCT, &text).unwrap_err();
    assert_eq!(error.code, ErrorCode::TooLarge, "outside a project too");
}

#[test]
fn mcp03_criterion_2_an_unregistered_path_is_checked_as_a_new_document() {
    let directory = checkout();
    let root = directory.path();
    let before = tree(root);
    let component = fs::read_to_string(root.join(PRODUCT)).unwrap().replacen(
        "component product",
        "component new_product",
        1,
    );

    let result = tools::check_text(root, "components/new-product.afs", &component).unwrap();
    assert!(!result.registered);
    assert_eq!(result.kind, Some(Kind::Component));
    assert_eq!(result.handle.as_deref(), Some("new_product"));
    assert!(result.problems.is_empty(), "{:?}", result.problems);
    assert!(result.valid, "{:?}", result.diagnostics);
    assert_eq!(result.source_sha256, None);
    assert_eq!(result.used.len(), 3);
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity == Severity::Warning)
    );

    let scenario = fs::read_to_string(root.join(SCENARIO)).unwrap();
    let result = tools::check_text(root, "scenarios/second.afs", &scenario).unwrap();
    assert!(!result.registered);
    assert_eq!(result.kind, Some(Kind::Scenario));
    assert_eq!(result.handle, None);
    assert!(result.valid, "{:?}", result.diagnostics);

    assert_eq!(tree(root), before, "nothing is created or registered");
    assert!(!root.join(".apiforge/local").exists());
}

#[test]
fn mcp03_criterion_2_the_errors_a_registration_would_meet() {
    let directory = checkout();
    let root = directory.path();
    fs::write(
        root.join("scenarios/draft.afs"),
        "scenario \"Draft\" {\n}\n",
    )
    .unwrap();
    let before = tree(root);
    let product = fs::read_to_string(root.join(PRODUCT)).unwrap();
    let renamed = product.replacen("component product", "component other_product", 1);
    let codes = |path: &str, text: &str| {
        let result = tools::check_text(root, path, text).unwrap();
        assert!(!result.registered, "{path}");
        assert!(!result.valid, "{path}");
        for problem in &result.problems {
            assert!(!problem.message.contains(path), "the path is not quoted");
            assert!(!problem.message.is_empty());
        }
        result
            .problems
            .iter()
            .map(|problem| problem.code)
            .collect::<Vec<_>>()
    };

    assert_eq!(
        codes("components/copy.afs", &product),
        [ErrorCode::HandleExists],
        "the handle is taken"
    );
    assert_eq!(
        codes("components/Product.afs", &renamed),
        [ErrorCode::PathExists],
        "a registered path differing in case"
    );
    assert_eq!(
        codes("scenarios/draft.afs", "scenario \"Draft\" {\n}\n"),
        [ErrorCode::PathExists],
        "an unregistered file on disk"
    );
    assert_eq!(
        codes("scenarios/DRAFT.afs", "scenario \"Draft\" {\n}\n"),
        [ErrorCode::PathExists],
        "a file on disk differing in case"
    );

    let result = tools::check_text(root, "components/headless.afs", "hello = 1\n").unwrap();
    assert_eq!(result.kind, None);
    assert_eq!(result.handle, None);
    assert!(!result.valid);
    assert_eq!(
        result.problems.iter().map(|p| p.code).collect::<Vec<_>>(),
        [ErrorCode::InvalidText],
        "the header has no kind"
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error),
        "with the ordinary diagnostics"
    );

    assert_eq!(tree(root), before);
    assert!(!root.join(".apiforge/local").exists());
}

#[test]
fn mcp03_criterion_3_unpublished_revisions_unknown_handles_and_missing_variables_are_named() {
    let directory = checkout();
    let root = directory.path();
    let text = "scenario \"Calls\" {\n  next = product@2(id: 123)\n  other = nosuch@1(id: 1)\n  missing = GET \"/x/{{env.MISSING}}\" {\n    sample 200 {}\n  }\n}\n";
    for path in [SCENARIO, "scenarios/calls.afs"] {
        let result = tools::check_text(root, path, text).unwrap();
        assert!(!result.valid, "{path}");
        let named = |name: &str, message: &str| {
            assert!(
                result
                    .diagnostics
                    .iter()
                    .any(|d| d.severity == Severity::Error
                        && d.message == message
                        && covered(text, d).contains(name)),
                "{path}: no error {message:?} on {name:?}: {:#?}",
                result.diagnostics
            );
        };
        named("product@2", "Component revision does not exist");
        named("nosuch@1", "Unknown operator or nested component call");
        named("env.MISSING", "Unknown environment name");
    }
}

#[test]
fn mcp03_criterion_4_run_simulates_the_saved_scenario() {
    let directory = checkout();
    let root = directory.path();
    let before = tree(root);
    let overview = tools::project_overview(root).unwrap();
    let report = simulate(root, &arguments(SCENARIO)).unwrap();

    assert_eq!(report.status, Verdict::Passed);
    assert_eq!(report.ended_by, EndedBy::Completed);
    assert_eq!(report.path, SCENARIO);
    assert!(report.redacted);
    assert_eq!(report.generation, overview.generation);
    assert_eq!(report.source_sha256, Some(sha256(&root.join(SCENARIO))));
    assert_eq!(report.used.len(), 3);
    assert!(report.report.is_none(), "compact by default");
    let steps = report.steps.as_deref().unwrap();
    assert_eq!(report.step_count, 18);
    assert_eq!(count(steps), 18, "as `apiforge run` prints 18 lines");
    assert_eq!(handles(steps), DEMO_STEPS);
    assert_eq!(steps[0].label.as_deref(), Some("Product"));
    assert_eq!(
        handles(steps[0].steps.as_deref().unwrap()),
        ["fetch_product", "product"],
        "a component's steps nested beneath it"
    );
    assert!(
        flatten(steps)
            .iter()
            .all(|s| s.status == StepStatus::Passed)
    );
    assert_eq!(steps[6].phase, Phase::Cleanup);
    assert_eq!(steps[0].phase, Phase::Main);
    assert_eq!(tree(root), before, "a simulation writes nothing");
    assert!(!root.join(".apiforge/local").exists());
}

#[test]
fn mcp03_criterion_4_a_failed_verdict_is_a_result_not_an_error() {
    let directory = checkout();
    let root = directory.path();
    edit(root, SCENARIO, |text| {
        text.replace("expected_price = 80 * 1", "expected_price = 81 * 1")
    });
    let report = simulate(root, &arguments(SCENARIO)).unwrap();
    assert_eq!(report.status, Verdict::Failed);
    assert_eq!(report.ended_by, EndedBy::Completed);
    let steps = report.steps.unwrap();
    let price = steps
        .iter()
        .find(|step| step.handle == "promotional_price")
        .unwrap();
    assert_eq!(price.status, StepStatus::Failed);
    assert!(price.message.is_some());
    assert_eq!(price.expected, Some(json!(81)));
    assert_eq!(price.actual, Some(json!(80)));
    assert!(
        steps
            .iter()
            .filter(|step| step.status == StepStatus::Passed)
            .all(|step| step.expected.is_none() && step.actual.is_none()),
        "expected and actual only on a failed comparison"
    );
}

#[test]
fn mcp03_criterion_5_a_component_runs_alone_with_its_inputs() {
    let directory = checkout();
    let root = directory.path();

    // `id` has the default 123.
    let report = simulate(root, &arguments(PRODUCT)).unwrap();
    assert_eq!(report.status, Verdict::Passed);
    let steps = report.steps.unwrap();
    assert_eq!(handles(&steps), ["fetch_product", "product"]);
    assert!(steps.iter().all(|step| step.phase == Phase::Main));

    for inputs in [json!({ "id": 7 }), json!({ "id": "7" })] {
        let report = simulate(root, &with_inputs(PRODUCT, inputs.clone())).unwrap();
        assert_eq!(report.status, Verdict::Passed, "{inputs}");
    }
    let report = simulate(
        root,
        &with_inputs(PROMOTION, json!({ "product": sample_product() })),
    )
    .unwrap();
    assert_eq!(report.status, Verdict::Passed);

    let error = simulate(root, &arguments(PROMOTION)).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidInputs);
    assert_eq!(error.details["missing"], json!(["product"]));
    assert_eq!(error.wrote, Wrote::No);

    let error = simulate(root, &with_inputs(PRODUCT, json!({ "nope": 1 }))).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidInputs);
    assert_eq!(error.details["unknown"], json!(["nope"]));

    let error = simulate(root, &with_inputs(PRODUCT, json!({ "id": "seven" }))).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidInputs);
    assert_eq!(error.details["invalid"], json!(["id"]));

    let error = simulate(root, &with_inputs(SCENARIO, json!({ "id": 1 }))).unwrap_err();
    assert_eq!(
        error.code,
        ErrorCode::InvalidInputs,
        "a scenario takes none"
    );
}

#[test]
fn mcp03_criterion_6_compact_by_default_and_the_full_report_on_request() {
    let directory = checkout();
    let root = directory.path();
    let compact = simulate(root, &arguments(SCENARIO)).unwrap();
    let text = serde_json::to_string_pretty(&compact).unwrap();
    assert!(
        text.chars().count() < 10_000,
        "{} characters",
        text.chars().count()
    );

    let full = simulate(
        root,
        &RunArguments {
            full: true,
            ..arguments(SCENARIO)
        },
    )
    .unwrap();
    assert!(full.steps.is_none());
    let report = full.report.unwrap();
    assert_eq!(report["status"], "passed");
    assert_eq!(report["mode"], "simulation");
    assert_eq!(report["steps"].as_array().unwrap().len(), 8);
    assert!(report["steps"][0]["outputs"].is_object());
    assert_eq!(full.step_count, 18);
}

#[test]
fn mcp03_criterion_6_a_result_over_two_mebibytes_is_refused_never_cut() {
    let directory = checkout();
    let root = directory.path();
    let strings = vec![format!("\"{}\"", "x".repeat(90_000)); 8].join(", ");
    fs::write(
        root.join(SCENARIO),
        format!(
            "scenario \"Big\" {{\n  big = GET \"/big\" {{\n    sample 200 [{strings}]\n  }}\n  a = big.body\n  b = big.body\n  c = big.body\n}}\n"
        ),
    )
    .unwrap();
    let compact = simulate(root, &arguments(SCENARIO)).unwrap();
    assert_eq!(compact.status, Verdict::Passed);

    let error = simulate(
        root,
        &RunArguments {
            full: true,
            ..arguments(SCENARIO)
        },
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResultTooLarge);
    assert!(error.message.contains("full: false"), "{}", error.message);
    assert_eq!(error.details["limit"], json!(tools::MAX_RESULT_BYTES));
    assert!(error.details["bytes"].as_u64().unwrap() > tools::MAX_RESULT_BYTES as u64);
}

#[test]
fn mcp03_criterion_7_run_refuses_an_invalid_or_unregistered_document() {
    let directory = checkout();
    let root = directory.path();
    edit(root, PRODUCT, |text| {
        text.replacen("export product", "export ???", 1)
    });
    let error = simulate(root, &arguments(PRODUCT)).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidText);
    let diagnostics = error.details["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics[0]["severity"], "error");

    let error = simulate(root, &arguments("components/missing.afs")).unwrap_err();
    assert_eq!(error.code, ErrorCode::UnknownDocument);
    assert!(error.details["registered_paths"].is_array());
    let error = simulate(root, &arguments("../x.afs")).unwrap_err();
    assert_eq!(error.code, ErrorCode::PathRejected);
}

#[test]
fn mcp03_criterion_8_live_is_refused_and_sends_nothing() {
    let directory = checkout();
    let root = directory.path();
    let api = Listener::start();
    edit(root, MANIFEST, |text| {
        text.replace(DEMO_BASE_URL, &api.base_url())
    });
    // The listener is reachable: a live run of the CLI's executor reaches it.
    let error = simulate(
        root,
        &RunArguments {
            live: true,
            request_id: Some("live-1".into()),
            ..arguments(SCENARIO)
        },
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::LiveDisabled);
    assert_eq!(
        error.message,
        "live requests are not enabled on this server"
    );
    assert_eq!(error.wrote, Wrote::No);
    let report = simulate(root, &arguments(SCENARIO)).unwrap();
    assert_eq!(report.status, Verdict::Passed, "a simulation sends nothing");
    thread::sleep(Duration::from_millis(200));
    assert_eq!(api.accepted(), 0, "the API saw no request");
}

#[test]
fn mcp03_criterion_9_the_shared_executor_returns_data_and_prints_nothing() {
    let source = include_str!("../src/executor.rs");
    for printing in [
        "println!",
        "print!(",
        "eprintln!",
        "eprint!(",
        "stdout",
        "stderr",
    ] {
        assert!(!source.contains(printing), "{printing} in the executor");
    }
    let directory = checkout();
    let request = demo_request(directory.path(), apiforge_core::run::RunMode::Simulation);
    let outcome = executor::execute(request, None, CancellationToken::new(), DEADLINE).unwrap();
    assert_eq!(outcome.ended_by, EndedBy::Completed);
    assert_eq!(outcome.report.status, apiforge_core::run::RunStatus::Passed);
    assert_eq!(outcome.report.steps.len(), 8);
    assert!(outcome.interrupted_before.is_empty());
    assert!(!outcome.journal_unreadable);
}

#[test]
fn mcp03_criterion_10_cancelling_a_run_stops_it_and_runs_the_cleanup() {
    let directory = checkout();
    let root = directory.path().to_owned();
    let runs = Runs::new(RunSettings {
        deadline: DEADLINE,
        step_delay: Some(Duration::from_millis(200)),
    });
    let cancel = CancellationToken::new();
    let running = {
        let (root, runs, cancel) = (root.clone(), runs.clone(), cancel.clone());
        thread::spawn(move || tools::run(&root, &arguments(SCENARIO), &runs, cancel))
    };
    let started = Instant::now();
    while !runs.is_busy() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the run did not start"
        );
        thread::sleep(Duration::from_millis(5));
    }
    let second = tools::run(&root, &arguments(PRODUCT), &runs, CancellationToken::new());
    assert_eq!(second.unwrap_err().code, ErrorCode::RunInProgress);
    assert_eq!(
        tools::project_overview(&root).unwrap().name,
        "Demo shop",
        "reads are answered meanwhile"
    );
    thread::sleep(Duration::from_millis(500));
    cancel.cancel();
    let report = running.join().unwrap().unwrap();

    assert_eq!(report.status, Verdict::Cancelled);
    assert_eq!(report.ended_by, EndedBy::Cancelled);
    let steps = report.steps.unwrap();
    let main: Vec<_> = steps.iter().filter(|s| s.phase == Phase::Main).collect();
    assert_eq!(main[0].status, StepStatus::Passed, "the steps done so far");
    assert!(
        main.iter().any(|s| s.status == StepStatus::Cancelled),
        "{main:#?}"
    );
    let cleanup: Vec<_> = steps.iter().filter(|s| s.phase == Phase::Cleanup).collect();
    assert_eq!(handles(&steps[steps.len() - 2..]), DEMO_STEPS[6..]);
    assert!(
        cleanup.iter().all(|s| s.status != StepStatus::Cancelled),
        "the cleanup ran: {cleanup:#?}"
    );
    assert!(!runs.is_busy(), "the slot is free again");
}

#[test]
fn mcp03_criterion_11_the_deadline_ends_a_run_as_timed_out() {
    let directory = checkout();
    let runs = Runs::new(RunSettings {
        deadline: Duration::from_millis(300),
        step_delay: Some(Duration::from_millis(200)),
    });
    let report = tools::run(
        directory.path(),
        &arguments(SCENARIO),
        &runs,
        CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(report.status, Verdict::Cancelled);
    assert_eq!(report.ended_by, EndedBy::TimedOut);
    let steps = report.steps.unwrap();
    assert!(steps.iter().any(|s| s.status == StepStatus::Cancelled));
    assert!(steps.iter().any(|s| s.phase == Phase::Cleanup));
    assert!(!runs.is_busy());
    // The server's tools still answer.
    assert_eq!(
        simulate(directory.path(), &arguments(PRODUCT))
            .unwrap()
            .status,
        Verdict::Passed
    );
}

#[test]
fn mcp03_criterion_11_the_in_flight_request_is_marked_as_possibly_delivered() {
    for cancelled_by_caller in [false, true] {
        let directory = checkout();
        let root = directory.path();
        // A slow step: a listener that accepts and never answers.
        let api = Listener::start();
        edit(root, MANIFEST, |text| {
            text.replace(DEMO_BASE_URL, &api.base_url())
        });
        let request = demo_request(root, apiforge_core::run::RunMode::Live);
        let cancel = CancellationToken::new();
        if cancelled_by_caller {
            let cancel = cancel.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(400));
                cancel.cancel();
            });
        }
        let deadline = if cancelled_by_caller {
            DEADLINE
        } else {
            Duration::from_millis(400)
        };
        let outcome = executor::execute(request, None, cancel, deadline).unwrap();
        assert_eq!(
            outcome.ended_by,
            if cancelled_by_caller {
                EndedBy::Cancelled
            } else {
                EndedBy::TimedOut
            }
        );
        assert_eq!(
            outcome.report.status,
            apiforge_core::run::RunStatus::Cancelled
        );
        assert!(api.accepted() >= 1, "the request was on its way");
        let steps = tools::steps(&outcome.report);
        let marked: Vec<_> = flatten(&steps)
            .into_iter()
            .filter(|step| step.may_have_reached_api)
            .collect();
        assert_eq!(marked.len(), 1, "{steps:#?}");
        assert_eq!(marked[0].handle, "fetch_product");
        assert_eq!(marked[0].status, StepStatus::Cancelled);
    }
}

#[test]
fn mcp03_criterion_12_the_language_reference_and_its_version() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/language/reference.md");
    let text = fs::read_to_string(file).unwrap();
    let served = tools::language_reference().unwrap();
    assert_eq!(served.text, text);
    assert_eq!(served.version, "1");
    assert!(text.starts_with("Reference version: 1 "));
    assert_eq!(reference::version(), "1");
    let directory = checkout();
    assert_eq!(
        tools::project_overview(directory.path())
            .unwrap()
            .reference_version,
        served.version
    );
}

#[test]
fn mcp03_criterion_12_resources_list_and_read() {
    let directory = checkout();
    let root = directory.path();
    let before = tree(root);
    let listed = resources::list(root).unwrap();
    let uris: Vec<_> = listed.iter().map(|r| r.uri.as_str()).collect();
    assert_eq!(
        uris,
        [
            "apiforge://reference",
            "apiforge://source/scenarios/promotion-and-cart-price.afs",
            "apiforge://source/components/add-to-cart.afs",
            "apiforge://source/components/product.afs",
            "apiforge://source/components/active-promotion.afs",
        ]
    );
    assert!(listed.iter().all(|r| !r.name.is_empty()));
    assert_eq!(resources::MIME_TYPE, "text/plain");

    assert_eq!(
        resources::read(root, "apiforge://reference").unwrap(),
        reference::TEXT
    );
    for uri in [
        "apiforge://source/components/product.afs",
        "apiforge://source/components%2Fproduct.afs",
    ] {
        assert_eq!(
            resources::read(root, uri).unwrap(),
            fs::read_to_string(root.join(PRODUCT)).unwrap(),
            "{uri}"
        );
    }
    for uri in [
        "apiforge://source/%2e%2e/x.afs",
        "apiforge://source/..%2Fx.afs",
        "apiforge://source/.apiforge/diagram.json",
        "apiforge://source/%2Eapiforge%2Frevisions%2Fget-product%2F1.afs",
        "apiforge://source//etc/x.afs",
        "apiforge://source/components/CON.afs",
        "apiforge://source/%ff.afs",
    ] {
        let error = resources::read(root, uri).unwrap_err();
        assert_eq!(error.code, ErrorCode::PathRejected, "{uri}");
        assert!(!error.message.contains(uri));
    }
    let error = resources::read(root, "apiforge://source/components/missing.afs").unwrap_err();
    assert_eq!(error.code, ErrorCode::UnknownDocument);
    let error = resources::read(root, "apiforge://elsewhere").unwrap_err();
    assert_eq!(error.code, ErrorCode::UnknownDocument);
    assert_eq!(tree(root), before);
}

#[test]
fn mcp03_carry_over_1_project_level_errors_are_kept_within_the_limit() {
    let notes = (0..1101)
        .map(|index| format!("// note {index}\n"))
        .collect::<String>();

    // The folder-pin error is found while lowering, after 1 000 warnings.
    let directory = checkout();
    let root = directory.path();
    let scenario = fs::read_to_string(root.join(SCENARIO)).unwrap();
    edit(root, SCENARIO, |text| {
        format!(
            "{}{notes}",
            text.replace("product = product@1(id: 123)", "product = product(id: 123)")
        )
    });
    // A component file that holds a scenario: the kind check comes last.
    fs::write(root.join(PRODUCT), format!("{scenario}{notes}")).unwrap();

    let session = ProjectSession::open_saved_read_only(root).unwrap();
    for (path, message) in [
        (
            SCENARIO,
            "Folder component calls require an explicit revision",
        ),
        (PRODUCT, "This code describes a different kind of document"),
    ] {
        let document = session
            .documents()
            .iter()
            .find(|d| d.entry().path.as_str() == path)
            .unwrap();
        assert_eq!(document.diagnostics().len(), 1000, "{path}");
        assert_eq!(document.omitted_diagnostics(), 102, "{path}: 1 102 found");
        assert!(
            document.diagnostics().iter().any(|d| d.message == message),
            "{path}: the project-level error is kept"
        );
        let result = tools::check(root, Some(path)).unwrap();
        let checked = &result.documents[0];
        assert!(!checked.valid);
        assert_eq!((checked.diagnostics.len(), checked.omitted), (1000, 102));
        assert!(checked.diagnostics.iter().any(|d| d.message == message));
    }
}

#[test]
fn mcp03_carry_over_3_a_source_over_one_mebibyte_is_unavailable_and_the_rest_served() {
    let directory = checkout();
    let root = directory.path();
    let large = "// padding\n".repeat(tools::MAX_TEXT_BYTES / 11 + 1);
    assert!(large.len() > tools::MAX_TEXT_BYTES);
    fs::write(root.join(PRODUCT), &large).unwrap();

    let overview = tools::project_overview(root).unwrap();
    let product = overview
        .documents
        .iter()
        .find(|d| d.path == PRODUCT)
        .unwrap();
    assert_eq!(product.unavailable.as_deref(), Some("too_large"));
    assert_eq!(product.source_sha256, None);

    let source = tools::read_source(root, PRODUCT).unwrap();
    assert_eq!(source.unavailable.as_deref(), Some("too_large"));
    assert!(source.text.is_empty());
    assert!(!source.valid);
    assert!(tools::read_source(root, SCENARIO).unwrap().valid);

    let checked = tools::check(root, None).unwrap();
    assert!(!checked.valid);
    for document in &checked.documents {
        if document.path == PRODUCT {
            assert_eq!(document.unavailable.as_deref(), Some("too_large"));
            assert!(!document.valid);
        } else {
            assert!(document.valid, "{}", document.path);
            assert_eq!(document.unavailable, None);
        }
    }
    let report = simulate(root, &arguments(SCENARIO)).unwrap();
    assert_eq!(
        report.status,
        Verdict::Passed,
        "it runs the published revision"
    );
    let listed = resources::list(root).unwrap();
    assert_eq!(listed.len(), 5);
}

#[test]
fn mcp03_carry_over_4_positions_do_not_rescan_a_long_line() {
    // One line of 500 000 two-byte characters, looked up 200 000 times near
    // its end, as 200 diagnostics per character of the last thousand: a
    // lookup that rescans the line prefix counts 10^11 characters.
    let text = "é".repeat(500_000);
    let lines = positions::Lines::new(&text);
    let started = Instant::now();
    for index in 0..200_000 {
        let back = index % 1_000;
        let offset = text.len() - 2 * back;
        assert_eq!(lines.position(offset), (1, 500_001 - back));
    }
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(lines.position(1), (1, 1), "inside a character: its start");
}

#[test]
fn mcp03_carry_over_5_errors_are_bounded_as_results_are() {
    let large = ToolError::new(ErrorCode::InvalidText, "The text has errors")
        .with_details(json!({ "diagnostics": "d".repeat(3 * 1024 * 1024) }));
    let error = tools::bound::<()>(Err(large)).unwrap_err();
    assert_eq!(error.code, ErrorCode::ResultTooLarge);
    assert_eq!(error.details["limit"], json!(tools::MAX_RESULT_BYTES));
    assert_eq!(error.wrote, Wrote::No);

    let small = ToolError::new(
        ErrorCode::UnknownDocument,
        "No registered document has this path",
    );
    assert_eq!(tools::bound::<()>(Err(small.clone())).unwrap_err(), small);
    assert_eq!(tools::bound(Ok(7)).unwrap(), 7);
    let large = "r".repeat(3 * 1024 * 1024);
    assert_eq!(
        tools::bound(Ok(large)).unwrap_err().code,
        ErrorCode::ResultTooLarge
    );
}

// Review round 1 of PR #109 ---------------------------------------------------

/// A name or value that is not Unicode on this platform.
fn not_unicode() -> std::ffi::OsString {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        std::ffi::OsString::from_vec(vec![b'a', 0xff])
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        std::ffi::OsString::from_wide(&[0x61, 0xD800])
    }
}

#[test]
fn mcp03_review_a_character_across_a_checkpoint_counts_once() {
    // The crab takes bytes 255 to 258, across the checkpoint at byte 256.
    let text = format!("{}🦀b", "a".repeat(255));
    let lines = positions::Lines::new(&text);
    assert_eq!(lines.position(255), (1, 256), "the crab");
    for inside in 256..259 {
        assert_eq!(
            lines.position(inside),
            (1, 256),
            "inside the crab: {inside}"
        );
    }
    assert_eq!(lines.position(259), (1, 257), "the b after it");
    assert_eq!(lines.position(260), (1, 258), "the end");
}

#[test]
fn mcp03_review_a_variable_that_is_not_unicode_is_skipped_not_a_panic() {
    let environment = vec![
        ("APIFORGE_SECRET_TOKEN".into(), "t".into()),
        (not_unicode(), "x".into()),
        ("APIFORGE_SECRET_BROKEN".into(), not_unicode()),
        ("PATH".into(), not_unicode()),
    ];
    let found = executor::secrets_from(None, environment).unwrap();
    // Names only: a failing assertion must not print a secret's value.
    assert_eq!(found.keys().collect::<Vec<_>>(), ["TOKEN"]);
    assert!(found["TOKEN"] == "t");
}

#[test]
fn mcp03_review_a_secrets_file_over_the_limit_is_refused_by_its_size() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("secrets.json");
    fs::write(&file, r#"{"TOKEN": "t"}"#).unwrap();
    let found = executor::secrets_from(Some(&file), Vec::new()).unwrap();
    assert!(found["TOKEN"] == "t");

    fs::write(&file, vec![b' '; executor::MAX_SECRETS_BYTES as usize + 1]).unwrap();
    assert_eq!(
        executor::secrets_from(Some(&file), Vec::new()).unwrap_err(),
        "The secrets file is too large"
    );
    // Sixteen gibibytes, sparse: reading it in full before the check would
    // not finish in time, or at all. (NTFS would allocate it, so Unix only.)
    #[cfg(unix)]
    {
        fs::File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_len(16 << 30)
            .unwrap();
        let started = Instant::now();
        assert_eq!(
            executor::secrets_from(Some(&file), Vec::new()).unwrap_err(),
            "The secrets file is too large"
        );
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}

#[test]
fn mcp03_review_a_file_in_place_of_a_folder_is_a_registration_problem() {
    let directory = checkout();
    let root = directory.path();
    let text = "scenario \"Nested\" {\n}\n";
    for path in [
        "components/product.afs/nested.afs",
        "components/PRODUCT.afs/deeper/nested.afs",
    ] {
        let result = tools::check_text(root, path, text).unwrap();
        assert!(!result.registered, "{path}");
        assert!(!result.valid, "{path}");
        assert_eq!(
            result.problems.iter().map(|p| p.code).collect::<Vec<_>>(),
            [ErrorCode::PathExists],
            "{path}"
        );
    }
    let result = tools::check_text(root, "scenarios/new/nested.afs", text).unwrap();
    assert!(result.problems.is_empty(), "{:?}", result.problems);
}

#[test]
fn mcp03_review_a_case_collision_beyond_the_tree_scan_limit_is_found() {
    let directory = checkout();
    let root = directory.path();
    // More entries than a tree scan returns, before the collision.
    let bulk = root.join("aaa-bulk");
    fs::create_dir(&bulk).unwrap();
    for index in 0..10_001 {
        fs::write(bulk.join(format!("{index}.txt")), "").unwrap();
    }
    let text = "scenario \"Draft\" {\n}\n";
    fs::create_dir_all(root.join("zzz/Deep")).unwrap();
    fs::write(root.join("zzz/Deep/Draft.afs"), text).unwrap();
    let result = tools::check_text(root, "zzz/deep/draft.afs", text).unwrap();
    assert_eq!(
        result.problems.iter().map(|p| p.code).collect::<Vec<_>>(),
        [ErrorCode::PathExists]
    );
}

// ----- MCP-04 (#63): the agent session, write_source and create_document -----

use apiforge_mcp::agent::{AgentSession, CreateDocumentArguments, WriteSourceArguments};
use apiforge_storage::project::{
    DocumentKind, Holder as LockHolder, HolderRole, ProjectError, ProjectStore,
    faults::{self, Boundary, Phase as IoPhase},
};

const DIAGRAM: &str = ".apiforge/diagram.json";
const DISCOUNT: &str = "components/discount.afs";
const DISCOUNT_TEXT: &str = "component discount \"Discount\" {\n  description \"A tenth off a price.\"\n  @label(\"Price\")\n  input price: number = 100\n  @label(\"Discounted\")\n  export discounted: number = param.price * 0.9\n}\n";
const DRAFT: &str = "scenarios/draft.afs";
const DRAFT_TEXT: &str = "scenario \"Draft\" {\n}\n";
const SECRET: &str = "s3cr3t-token-for-mcp04";

/// An agent session that reads no secret from the process environment.
fn agent(root: &Path) -> AgentSession {
    AgentSession::with_environment(root.to_owned(), Vec::new())
}

fn write_args(path: &str, text: &str, expected: &str) -> WriteSourceArguments {
    WriteSourceArguments {
        path: path.into(),
        text: text.into(),
        expected_source_sha256: expected.into(),
        expected_generation: None,
    }
}

fn create_args(kind: Kind, path: &str, text: &str) -> CreateDocumentArguments {
    CreateDocumentArguments {
        kind,
        path: path.into(),
        text: text.into(),
        expected_generation: None,
    }
}

/// The saved hash `read_source` returns: the version a model writes from.
fn read(root: &Path, path: &str) -> (String, String, u64) {
    let read = tools::read_source(root, path).unwrap();
    (read.text, read.source_sha256.unwrap(), read.generation)
}

/// The project as Git sees it: every file outside the machine-local
/// `.apiforge/local`, which is not even read: on Windows the writer's lock
/// file there cannot be read while the lock is held.
fn shared(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn walk(root: &Path, at: &Path, files: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(root).unwrap().to_owned();
            if relative == Path::new(".apiforge").join("local") {
                continue;
            }
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

/// The head generation, 0 before the first commit on this machine.
fn head_generation(root: &Path) -> u64 {
    match fs::read(root.join(".apiforge/local/head.json")) {
        Ok(bytes) => serde_json::from_slice::<Value>(&bytes).unwrap()["generation"]
            .as_u64()
            .unwrap(),
        Err(_) => 0,
    }
}

fn head_active(root: &Path) -> bool {
    match fs::read(root.join(".apiforge/local/head.json")) {
        Ok(bytes) => {
            !serde_json::from_slice::<Value>(&bytes).unwrap()["activeTransactionId"].is_null()
        }
        Err(_) => false,
    }
}

/// Another writer's attempt to take the project: `true` while the lock is
/// held. A successful attempt releases the lock at once.
fn locked(root: &Path) -> bool {
    match ProjectStore::open_exclusive(root) {
        Err(ProjectError::Locked) => true,
        Ok(_) => false,
        Err(error) => panic!("{error}"),
    }
}

/// The checkpoint records by the document id of a working document, or by
/// their whole key for a published revision.
fn records(root: &Path) -> BTreeMap<String, Value> {
    let bytes = fs::read(root.join(DIAGRAM)).unwrap();
    let file: Value = serde_json::from_slice(&bytes).unwrap();
    file["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| {
            let key = &record["key"];
            let key = key["documentId"]
                .as_str()
                .map_or_else(|| key.to_string(), str::to_owned);
            (key, record.clone())
        })
        .collect()
}

/// The registry entry of `path`, if one is registered.
fn entry(root: &Path, path: &str) -> Option<Value> {
    let registry: Value = serde_json::from_slice(&fs::read(root.join(MANIFEST)).unwrap()).unwrap();
    registry["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["path"] == path)
        .cloned()
}

fn document_id(root: &Path, path: &str) -> String {
    entry(root, path).expect("registered")["documentId"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn copy_tree(from: &Path, to: &Path) {
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            fs::create_dir(&target).unwrap();
            copy_tree(&path, &target);
        } else {
            fs::copy(&path, &target).unwrap();
        }
    }
}

/// A change a model might make that keeps the document valid.
fn commented(text: &str) -> String {
    let changed = text.replacen(" {\n", " {\n  // Checked by the model.\n", 1);
    assert_ne!(changed, text);
    changed
}

/// The demo scenario with one more request block before its cleanup.
fn with_ping(text: &str) -> String {
    let changed = text.replacen(
        "  cleanup {\n",
        "  @label(\"Ping\")\n  ping = GET \"/ping\" {\n    sample 200 {}\n  }\n  cleanup {\n",
        1,
    );
    assert_ne!(changed, text);
    changed
}

fn error_text(error: &ToolError) -> String {
    serde_json::to_string(error).unwrap()
}

fn injected(path: &Path, what: &str) -> ProjectError {
    ProjectError::Io {
        path: path.to_owned(),
        source: std::io::Error::other(what.to_owned()),
    }
}

/// `apiforge apply`, as the command does it: the saved text replaced under
/// the writer lock by a command-line holder.
fn apply(root: &Path, path: &str, text: &str) {
    let mut session =
        ProjectSession::open_saved_exclusive_as(root, &LockHolder::command("apply")).unwrap();
    let id = session
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == path)
        .unwrap()
        .id()
        .to_owned();
    session
        .edit_source(&id, text, apiforge_project::Origin::SESSION)
        .unwrap();
    session.settle(&id).unwrap();
    session.save(&id).unwrap();
}

/// Criterion 1: the text replaces the document's, exactly; the checkpoint is
/// written; the result names what the commit wrote; every other file is
/// untouched. The same edit saved by the window in a copy of the project
/// leaves the same files with the same bytes.
#[test]
fn mcp04_criterion_1_write_source_saves_exactly_with_the_checkpoint() {
    let directory = checkout();
    let root = directory.path();
    let window = tempfile::tempdir().unwrap();
    copy_tree(root, window.path());
    let (text, hash, generation) = read(root, SCENARIO);
    let text = commented(&text);
    let before = shared(root);
    let records_before = records(root);

    let agent = agent(root);
    let result = agent
        .write_source(&write_args(SCENARIO, &text, &hash))
        .unwrap();
    assert!(result.wrote);
    assert_eq!(result.path, SCENARIO);
    let mut changed = result.changed.clone();
    changed.sort();
    assert_eq!(changed, [DIAGRAM, SCENARIO]);
    assert_eq!(fs::read_to_string(root.join(SCENARIO)).unwrap(), text);
    assert_eq!(result.source_sha256, sha256(&root.join(SCENARIO)));
    assert_eq!(result.generation, head_generation(root));
    assert!(result.generation > generation);
    assert!(result.valid);
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity == Severity::Warning),
        "only warnings"
    );

    // The checkpoint describes the saved text; every other record is kept.
    let records_after = records(root);
    let id = document_id(root, SCENARIO);
    assert_eq!(
        records_after[&id]["sourceHash"].as_str(),
        Some(result.source_sha256.as_str())
    );
    for (other, record) in &records_before {
        if *other != id {
            assert_eq!(&records_after[other], record, "{other}");
        }
    }

    // Every other file is untouched.
    let after = shared(root);
    assert_eq!(after.len(), before.len());
    for (path, bytes) in &before {
        if path != Path::new(SCENARIO) && path != Path::new(DIAGRAM) {
            assert_eq!(after.get(path), Some(bytes), "{}", path.display());
        }
    }

    // The window's save of the same text: the same files, the same bytes.
    let mut session = ProjectSession::open(window.path()).unwrap();
    let id = session
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == SCENARIO)
        .unwrap()
        .id()
        .to_owned();
    session
        .edit_source(&id, &text, apiforge_project::Origin::SESSION)
        .unwrap();
    session.save(&id).unwrap();
    drop(session);
    assert_eq!(shared(window.path()), shared(root));
}

/// Criterion 1: the same text and hash twice is a no-op.
#[test]
fn mcp04_criterion_1_the_same_write_twice_changes_nothing_more() {
    let directory = checkout();
    let root = directory.path();
    let (text, hash, _) = read(root, SCENARIO);
    let arguments = write_args(SCENARIO, &commented(&text), &hash);
    let agent = agent(root);
    let first = agent.write_source(&arguments).unwrap();
    let after_first = shared(root);
    let again = agent.write_source(&arguments).unwrap();
    assert!(!again.wrote);
    assert!(again.changed.is_empty());
    assert_eq!(again.source_sha256, first.source_sha256);
    assert_eq!(again.generation, first.generation);
    assert_eq!(shared(root), after_first);
}

/// Criterion 2: a text with errors is not saved; the diagnostics come with
/// their positions.
#[test]
fn mcp04_criterion_2_a_text_with_errors_is_not_saved() {
    let directory = checkout();
    let root = directory.path();
    let (text, hash, _) = read(root, PRODUCT);
    let before = shared(root);
    // A call of a component that does not exist, on the second line.
    let broken = text.replacen(" {\n", " {\n  broken = nosuch@1(id: 1)\n", 1);
    assert_ne!(broken, text);
    let error = agent(root)
        .write_source(&write_args(PRODUCT, &broken, &hash))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidText);
    assert_eq!(error.wrote, Wrote::No);
    let diagnostics = error.details["diagnostics"].as_array().unwrap();
    let found = diagnostics
        .iter()
        .find(|d| d["severity"] == "error" && d["line"] == 2)
        .unwrap_or_else(|| panic!("{diagnostics:?}"));
    assert!(found["column"].as_u64().unwrap() > 1);
    assert_eq!(found["end_line"], 2);
    assert!(found["end_column"].as_u64().unwrap() > found["column"].as_u64().unwrap());
    assert_eq!(shared(root), before, "the file is unchanged");

    // A warning alone does not stop the save (KTLO-25's type mismatch).
    let saved = agent(root)
        .write_source(&write_args(PRODUCT, &inject_mismatch(&text), &hash))
        .unwrap();
    assert!(saved.wrote);
    assert!(saved.diagnostics.iter().any(|d| d.message == MISMATCH));
}

/// Criterion 3: the disk changed with `apply` between the model's read and
/// its write: `stale_source` with the current hash; a different generation
/// is `stale_project`. Nothing is written either way.
#[test]
fn mcp04_criterion_3_a_stale_hash_or_generation_writes_nothing() {
    let directory = checkout();
    let root = directory.path();
    let (text, hash, generation) = read(root, SCENARIO);
    let applied = text.replace("on_error continue", "on_error stop");
    assert_ne!(applied, text);
    apply(root, SCENARIO, &applied);
    let current = sha256(&root.join(SCENARIO));
    let current_generation = head_generation(root);
    assert_ne!(current_generation, generation);
    let before = shared(root);

    let agent = agent(root);
    let error = agent
        .write_source(&write_args(SCENARIO, &commented(&text), &hash))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleSource);
    assert_eq!(error.wrote, Wrote::No);
    assert_eq!(error.details["source_sha256"], current.as_str());
    assert_eq!(shared(root), before);

    let mut stale = write_args(SCENARIO, &commented(&applied), &current);
    stale.expected_generation = Some(generation);
    let error = agent.write_source(&stale).unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleProject);
    assert_eq!(error.wrote, Wrote::No);
    assert_eq!(error.details["generation"], current_generation);
    assert_eq!(shared(root), before);

    let mut stale = create_args(Kind::Scenario, DRAFT, DRAFT_TEXT);
    stale.expected_generation = Some(generation);
    let error = agent.create_document(&stale).unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleProject);
    assert_eq!(shared(root), before);

    // Based on the current version, the write is saved.
    let mut fresh = write_args(SCENARIO, &commented(&applied), &current);
    fresh.expected_generation = Some(current_generation);
    assert!(agent.write_source(&fresh).unwrap().wrote);
}

/// Criterion 4: `check_new_document` validates without writing and compares
/// the header's kind with the kind asked for.
#[test]
fn mcp04_criterion_4_check_new_document_compares_the_kind_and_writes_nothing() {
    let directory = checkout();
    let root = directory.path();
    let before = tree(root);
    let session = ProjectSession::open_saved_read_only(root).unwrap();
    let check = session
        .check_new_document(Some(DocumentKind::Component), DRAFT, DRAFT_TEXT)
        .unwrap();
    assert_eq!(
        check.problems,
        [apiforge_project::RegistrationProblem::KindMismatch]
    );
    assert!(!check.is_valid());
    let check = session
        .check_new_document(Some(DocumentKind::Component), DISCOUNT, DISCOUNT_TEXT)
        .unwrap();
    assert!(check.is_valid(), "{:?}", check.diagnostics);
    assert_eq!(check.handle.as_deref(), Some("discount"));
    drop(session);
    assert_eq!(tree(root), before);
}

/// Criterion 4: every refusal of `create_document` leaves no source, no
/// registry entry and no checkpoint.
#[test]
fn mcp04_criterion_4_create_document_refuses_on_any_error() {
    let directory = checkout();
    let root = directory.path();
    fs::write(root.join(DRAFT), DRAFT_TEXT).unwrap();
    let product = fs::read_to_string(root.join(PRODUCT)).unwrap();
    let renamed = product.replacen("component product", "component other_product", 1);
    let broken = "scenario \"Broken\" {\n  x = nosuch@1(id: 1)\n}\n";
    let before = shared(root);
    let agent = agent(root);
    let cases = [
        (
            Kind::Component,
            PRODUCT,
            renamed.as_str(),
            ErrorCode::PathExists,
        ),
        (
            Kind::Component,
            "components/Product.afs",
            renamed.as_str(),
            ErrorCode::PathExists,
        ),
        (Kind::Scenario, DRAFT, DRAFT_TEXT, ErrorCode::PathExists),
        (
            Kind::Scenario,
            "scenarios/DRAFT.afs",
            DRAFT_TEXT,
            ErrorCode::PathExists,
        ),
        (
            Kind::Component,
            "components/copy.afs",
            product.as_str(),
            ErrorCode::HandleExists,
        ),
        (
            Kind::Scenario,
            "../outside.afs",
            DRAFT_TEXT,
            ErrorCode::PathRejected,
        ),
        (
            Kind::Scenario,
            ".apiforge/inside.afs",
            DRAFT_TEXT,
            ErrorCode::PathRejected,
        ),
        (
            Kind::Scenario,
            "scenarios/notes.txt",
            DRAFT_TEXT,
            ErrorCode::PathRejected,
        ),
        (
            Kind::Component,
            "components/draft.afs",
            DRAFT_TEXT,
            ErrorCode::InvalidText,
        ),
        (
            Kind::Scenario,
            "scenarios/broken.afs",
            broken,
            ErrorCode::InvalidText,
        ),
    ];
    for (kind, path, text, code) in cases {
        let error = agent
            .create_document(&create_args(kind, path, text))
            .unwrap_err();
        assert_eq!(error.code, code, "{path}");
        assert_eq!(error.wrote, Wrote::No, "{path}");
        assert!(!error.message.contains(path), "the path is not quoted");
        if code == ErrorCode::InvalidText {
            assert!(error.details["diagnostics"].is_array(), "{path}");
        }
        assert_eq!(shared(root), before, "{path}: nothing is written");
    }
    let error = agent
        .create_document(&create_args(Kind::Scenario, "scenarios/broken.afs", broken))
        .unwrap_err();
    assert!(
        error.details["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["severity"] == "error" && d["line"] == 2),
        "an error on line 2"
    );
}

/// Criterion 5: source, registry entry and checkpoint in one transaction,
/// one head generation advance.
#[test]
fn mcp04_criterion_5_create_document_commits_everything_in_one_transaction() {
    let directory = checkout();
    let root = directory.path();
    let agent = agent(root);
    // The session starts with a write that changes nothing, so the head
    // generation below counts only the creation.
    let (text, hash, _) = read(root, SCENARIO);
    assert!(
        !agent
            .write_source(&write_args(SCENARIO, &text, &hash))
            .unwrap()
            .wrote
    );
    let before = head_generation(root);

    let created = agent
        .create_document(&create_args(Kind::Component, DISCOUNT, DISCOUNT_TEXT))
        .unwrap();
    assert!(created.wrote);
    assert_eq!(created.path, DISCOUNT);
    assert_eq!(created.kind, Kind::Component);
    assert_eq!(created.handle.as_deref(), Some("discount"));
    assert_eq!(
        created.generation,
        before + 1,
        "one head generation advance"
    );
    assert_eq!(created.generation, head_generation(root));
    let mut changed = created.changed.clone();
    changed.sort();
    assert_eq!(changed, [DIAGRAM, MANIFEST, DISCOUNT]);
    assert_eq!(
        fs::read_to_string(root.join(DISCOUNT)).unwrap(),
        DISCOUNT_TEXT
    );
    assert_eq!(created.source_sha256, sha256(&root.join(DISCOUNT)));
    let entry = entry(root, DISCOUNT).expect("registered");
    assert_eq!(entry["kind"], "component");
    let record = &records(root)[entry["documentId"].as_str().unwrap()];
    assert_eq!(
        record["sourceHash"].as_str(),
        Some(created.source_sha256.as_str())
    );

    // The window opening the project finds the identities sealed: nothing
    // is left to write.
    assert!(agent.finish_editing().finished);
    let sealed = fs::read(root.join(DIAGRAM)).unwrap();
    let session = ProjectSession::open(root).unwrap();
    assert!(
        session
            .documents()
            .iter()
            .any(|document| document.entry().path.as_str() == DISCOUNT)
    );
    drop(session);
    assert_eq!(fs::read(root.join(DIAGRAM)).unwrap(), sealed);
    assert_eq!(head_generation(root), before + 1);
}

/// Criterion 5: a fault between the staged files, before activation, leaves
/// nothing installed.
#[test]
fn mcp04_criterion_5_a_fault_before_activation_installs_nothing() {
    let directory = checkout();
    let root = directory.path();
    let agent = agent(root);
    let (text, hash, _) = read(root, SCENARIO);
    agent
        .write_source(&write_args(SCENARIO, &text, &hash))
        .unwrap();
    let before = shared(root);
    let generation = head_generation(root);
    let staged = std::rc::Rc::new(std::cell::Cell::new(0));
    let seen = staged.clone();
    let error = faults::with_hook(
        move |event| {
            let in_staging = event
                .path
                .components()
                .any(|part| part.as_os_str() == "transactions");
            if in_staging
                && event.boundary == Boundary::TemporaryCreate
                && event.phase == IoPhase::Before
            {
                seen.set(seen.get() + 1);
                if seen.get() == 2 {
                    return Err(injected(&event.path, "between the staged files"));
                }
            }
            Ok(())
        },
        || agent.create_document(&create_args(Kind::Component, DISCOUNT, DISCOUNT_TEXT)),
    )
    .unwrap_err();
    assert_eq!(staged.get(), 2, "the fault was reached");
    assert_eq!(error.wrote, Wrote::No);
    assert_ne!(error.code, ErrorCode::RecoveryRequired);
    assert!(!head_active(root));
    assert_eq!(head_generation(root), generation);
    assert_eq!(shared(root), before, "nothing installed");
    // The session goes on.
    assert!(
        agent
            .create_document(&create_args(Kind::Component, DISCOUNT, DISCOUNT_TEXT))
            .unwrap()
            .wrote
    );
}

/// Criterion 6: a fault after activation, on the checkpoint's install, is
/// `recovery_required` with `wrote: "unknown"`, and every later write of the
/// session is refused with the same code. The server does not repair; the
/// next writer finishes the operation (the window's recovery view in the
/// desktop's `agent_session_tests`).
#[test]
fn mcp04_criterion_6_a_failure_after_activation_is_recovery_required() {
    let directory = checkout();
    let root = directory.path();
    let agent = agent(root);
    let error = faults::with_hook(
        |event| {
            if event.boundary == Boundary::Replace
                && event.phase == IoPhase::Before
                && event.path.ends_with(DIAGRAM)
            {
                return Err(injected(&event.path, "on the checkpoint's install"));
            }
            Ok(())
        },
        || agent.create_document(&create_args(Kind::Component, DISCOUNT, DISCOUNT_TEXT)),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::RecoveryRequired);
    assert_eq!(error.wrote, Wrote::Unknown);
    assert_eq!(serde_json::to_value(&error).unwrap()["wrote"], "unknown");
    assert!(head_active(root));

    let (text, hash) = (
        fs::read_to_string(root.join(PRODUCT)).unwrap(),
        sha256(&root.join(PRODUCT)),
    );
    let error = agent
        .write_source(&write_args(PRODUCT, &commented(&text), &hash))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::RecoveryRequired);
    let error = agent
        .create_document(&create_args(Kind::Scenario, DRAFT, DRAFT_TEXT))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::RecoveryRequired);
    assert!(!root.join(DRAFT).exists());
    assert_eq!(fs::read_to_string(root.join(PRODUCT)).unwrap(), text);
    assert!(head_active(root), "the server does not repair on its own");

    // The next writer finishes the operation.
    assert!(agent.finish_editing().finished);
    let store = ProjectStore::open_exclusive(root).unwrap();
    store.recover().unwrap();
    drop(store);
    assert!(!head_active(root));
    assert_eq!(
        fs::read_to_string(root.join(DISCOUNT)).unwrap(),
        DISCOUNT_TEXT
    );
    assert!(records(root).contains_key(&document_id(root, DISCOUNT)));
}

/// Criterion 7: the first write takes the writer lock and the session keeps
/// it across calls until `finish_editing`; the holder file describes the
/// agent.
#[test]
fn mcp04_criterion_7_the_session_holds_the_lock_until_finish_editing() {
    let directory = checkout();
    let root = directory.path();
    let agent = agent(root);
    agent.set_client(Some("Claude Code".into()));
    assert!(!agent.is_active());
    assert!(!locked(root), "reading takes no lock");

    let (text, hash, _) = read(root, SCENARIO);
    let written = agent
        .write_source(&write_args(SCENARIO, &commented(&text), &hash))
        .unwrap();
    assert!(agent.is_active());
    assert!(locked(root), "the lock is kept after the write returns");

    let holder: Value =
        serde_json::from_slice(&fs::read(root.join(".apiforge/local/holder.json")).unwrap())
            .unwrap();
    assert_eq!(holder["program"], "apiforge");
    assert_eq!(holder["role"], "agent");
    assert_eq!(holder["client"], "Claude Code");
    assert!(holder["since"].as_str().unwrap().ends_with('Z'));
    let described = ProjectStore::open_read_only(root)
        .unwrap()
        .holder()
        .unwrap()
        .expect("a live writer");
    assert_eq!(described.role(), Some(HolderRole::Agent));
    assert_eq!(described.client(), Some("Claude Code"));

    // The read tools answer meanwhile, and a pause does not end the session.
    thread::sleep(Duration::from_millis(300));
    assert_eq!(
        tools::read_source(root, SCENARIO).unwrap().source_sha256,
        Some(written.source_sha256.clone())
    );
    assert!(locked(root));
    let second = agent
        .write_source(&write_args(
            SCENARIO,
            &commented(&commented(&text)),
            &written.source_sha256,
        ))
        .unwrap();
    assert!(second.wrote);
    assert!(locked(root));

    let finished = Instant::now();
    assert!(agent.finish_editing().finished);
    assert!(!agent.is_active());
    while locked(root) {
        assert!(
            finished.elapsed() < Duration::from_secs(1),
            "the lock is released within a second of finish_editing"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!agent.finish_editing().finished, "nothing left to finish");

    // A later write starts a new session.
    agent
        .create_document(&create_args(Kind::Scenario, DRAFT, DRAFT_TEXT))
        .unwrap();
    assert!(agent.is_active());
    assert!(locked(root));
}

/// Criterion 8: a write while another process holds the lock quotes the
/// holder, writes nothing and leaves the read tools working.
#[test]
fn mcp04_criterion_8_a_write_while_another_process_holds_the_project_is_refused() {
    let cases = [
        (
            LockHolder::window(),
            ErrorCode::HeldByWindow,
            "API Forge window",
        ),
        (
            LockHolder::command("apply"),
            ErrorCode::HeldByOther,
            "apiforge apply",
        ),
        (
            LockHolder::new(HolderRole::Agent, Some("Other client".into())),
            ErrorCode::HeldByOther,
            "Other client via apiforge mcp",
        ),
    ];
    for (holder, code, named) in cases {
        let directory = checkout();
        let root = directory.path();
        let (text, hash, _) = read(root, SCENARIO);
        let owner = ProjectStore::open_exclusive_as(root, &holder).unwrap();
        let since = holder.since().unwrap().to_owned();
        let before = shared(root);
        let agent = agent(root);
        for error in [
            agent
                .write_source(&write_args(SCENARIO, &commented(&text), &hash))
                .unwrap_err(),
            agent
                .create_document(&create_args(Kind::Scenario, DRAFT, DRAFT_TEXT))
                .unwrap_err(),
        ] {
            assert_eq!(error.code, code, "{named}");
            assert_eq!(error.wrote, Wrote::No);
            assert!(error.message.contains(named), "names {named}");
            assert!(error.message.contains(&since), "says since when");
            let lower = error.message.to_lowercase();
            assert!(
                !lower.contains("directly") && !lower.contains("edit the file"),
                "no advice to edit the file directly"
            );
            assert_eq!(error.details["holder"]["since"], since.as_str());
        }
        assert!(!agent.is_active());
        assert_eq!(shared(root), before, "nothing written");
        assert!(tools::read_source(root, SCENARIO).is_ok());
        assert!(tools::check(root, None).is_ok());
        assert!(tools::check_text(root, SCENARIO, &text).is_ok());

        // Once the holder lets go, the write goes through.
        drop(owner);
        assert!(
            agent
                .write_source(&write_args(SCENARIO, &commented(&text), &hash))
                .unwrap()
                .wrote
        );
    }
}

/// Criterion 8: a holder that left no readable description is another
/// process.
#[test]
fn mcp04_criterion_8_a_holder_without_a_description_is_held_by_other() {
    let directory = checkout();
    let root = directory.path();
    let _owner = ProjectStore::open_exclusive(root).unwrap();
    fs::remove_file(root.join(".apiforge/local/holder.json")).unwrap();
    let error = agent(root)
        .create_document(&create_args(Kind::Scenario, DRAFT, DRAFT_TEXT))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::HeldByOther);
    assert!(
        error.message.contains("Another process"),
        "the generic wording"
    );
    assert!(!root.join(DRAFT).exists());
}

/// Criterion 9: a known secret value in the text is refused and never echoed.
#[test]
fn mcp04_criterion_9_a_known_secret_in_the_text_is_refused() {
    let directory = checkout();
    let root = directory.path();
    let agent = AgentSession::with_environment(
        root.to_owned(),
        vec![("APIFORGE_SECRET_TOKEN".into(), SECRET.into())],
    );
    let (text, hash, _) = read(root, SCENARIO);
    let leaked = text.replacen(" {\n", &format!(" {{\n  // token {SECRET}\n"), 1);
    let component = DISCOUNT_TEXT.replace("A tenth off a price.", SECRET);
    let before = shared(root);
    for error in [
        agent
            .write_source(&write_args(SCENARIO, &leaked, &hash))
            .unwrap_err(),
        agent
            .create_document(&create_args(Kind::Component, DISCOUNT, &component))
            .unwrap_err(),
    ] {
        assert_eq!(error.code, ErrorCode::SecretInText);
        assert_eq!(error.wrote, Wrote::No);
        assert!(
            !error_text(&error).contains(SECRET),
            "the value is not echoed"
        );
    }
    assert_eq!(shared(root), before, "nothing written");
}

/// Criterion 10: blocks whose handle and kind did not change keep their
/// identities and positions; a new block has the identity the server sealed.
#[test]
fn mcp04_criterion_10_identities_survive_a_server_save() {
    type Identity = (String, Value, Value);
    let directory = checkout();
    let root = directory.path();
    let nodes = |session: &ProjectSession| -> BTreeMap<String, Identity> {
        let document = session
            .documents()
            .iter()
            .find(|document| document.entry().path.as_str() == SCENARIO)
            .unwrap();
        let Some(apiforge_project::Document::Scenario(scenario)) = document.model() else {
            panic!("a valid scenario")
        };
        scenario
            .graph
            .nodes
            .iter()
            .chain(&scenario.cleanup.nodes)
            .map(|node| {
                (
                    node.handle.clone(),
                    (
                        node.id.clone(),
                        serde_json::to_value(node.kind).unwrap(),
                        serde_json::to_value(&node.position).unwrap(),
                    ),
                )
            })
            .collect()
    };
    let before = nodes(&ProjectSession::open_saved_read_only(root).unwrap());
    let (text, hash, _) = read(root, SCENARIO);
    let agent = agent(root);
    agent
        .write_source(&write_args(SCENARIO, &with_ping(&text), &hash))
        .unwrap();
    assert!(agent.finish_editing().finished);

    let sealed = fs::read(root.join(DIAGRAM)).unwrap();
    let session = ProjectSession::open(root).unwrap();
    let after = nodes(&session);
    assert!(!before.is_empty());
    for (handle, identity) in &before {
        assert_eq!(after.get(handle), Some(identity), "{handle}");
    }
    let ping = &after["ping"];
    let record = &records(root)[&document_id(root, SCENARIO)];
    let checkpointed: Vec<&Value> = record["identityBase"]["document"]["graph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|node| node["handle"] == "ping")
        .collect();
    assert_eq!(checkpointed.len(), 1, "{record}");
    assert_eq!(checkpointed[0]["id"].as_str(), Some(ping.0.as_str()));
    assert_eq!(checkpointed[0]["position"], ping.2);
    drop(session);
    assert_eq!(
        fs::read(root.join(DIAGRAM)).unwrap(),
        sealed,
        "opening the project in the window changes no identity"
    );
}

// ----- MCP-04 review round 1 of #111 -----

/// A new document under a folder that exists only in another letter case
/// would create a second spelling of that folder: `path_exists`, checked
/// and created alike, and nothing is written.
#[test]
fn mcp04_review_a_folder_in_another_letter_case_is_path_exists() {
    let directory = checkout();
    let root = directory.path();
    let before = shared(root);
    let check = tools::check_text(root, "Components/discount.afs", DISCOUNT_TEXT).unwrap();
    assert_eq!(
        check.problems.iter().map(|p| p.code).collect::<Vec<_>>(),
        [ErrorCode::PathExists]
    );
    let error = agent(root)
        .create_document(&create_args(
            Kind::Component,
            "Components/discount.afs",
            DISCOUNT_TEXT,
        ))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::PathExists);
    assert_eq!(error.wrote, Wrote::No);
    assert_eq!(shared(root), before);
}

/// The storage's own check finds a file changed on disk after the session
/// read it (another program writing without the lock): the document itself
/// is `stale_source`, a new document's path `path_exists`, the checkpoint
/// `stale_project`. Nothing is written. The overview's question whether the
/// session is active is answered during the write.
#[test]
fn mcp04_review_a_file_changed_during_the_commit_names_what_changed() {
    let directory = checkout();
    let root = directory.path().to_owned();
    let agent = Arc::new(agent(&root));
    let (text, hash, _) = read(&root, SCENARIO);
    // The session starts with a write that changes nothing.
    agent
        .write_source(&write_args(SCENARIO, &text, &hash))
        .unwrap();
    let cases = [
        (SCENARIO, ErrorCode::StaleSource, false),
        (DISCOUNT, ErrorCode::PathExists, true),
        (DIAGRAM, ErrorCode::StaleProject, false),
    ];
    for (changed, code, creating) in cases {
        let target = root.join(changed);
        let original = fs::read(&target).ok();
        let during = agent.clone();
        let error = faults::with_hook(
            {
                let target = target.clone();
                let mut done = false;
                move |event| {
                    let in_staging = event
                        .path
                        .components()
                        .any(|part| part.as_os_str() == "transactions");
                    if in_staging && !done {
                        done = true;
                        assert!(
                            during.is_active(),
                            "answered while the write holds the session"
                        );
                        let mut bytes = fs::read(&target).unwrap_or_default();
                        bytes.extend_from_slice(b"\n// written by another program\n");
                        fs::write(&target, bytes).unwrap();
                    }
                    Ok(())
                }
            },
            || {
                if creating {
                    agent.create_document(&create_args(Kind::Component, DISCOUNT, DISCOUNT_TEXT))
                } else {
                    agent
                        .write_source(&write_args(SCENARIO, &commented(&text), &hash))
                        .map(|_| unreachable!("the write must be refused"))
                }
            },
        )
        .unwrap_err();
        assert_eq!(error.code, code, "{changed}");
        assert_eq!(error.wrote, Wrote::No, "{changed}");
        assert!(!head_active(&root), "{changed}: nothing activated");
        match original {
            Some(bytes) => fs::write(&target, bytes).unwrap(),
            None => fs::remove_file(&target).unwrap(),
        }
    }
    assert_eq!(fs::read_to_string(root.join(SCENARIO)).unwrap(), text);
    assert!(entry(&root, DISCOUNT).is_none());
}

// ----- MCP-06 (#65): live requests, the four ends of a run, secrets -----

use apiforge_core::journal::{EffectState, JournalEntry, JournalRun, JournalSink};
use apiforge_mcp::{ApiforgeServer, ServerSettings, live::Permission, secrets::Secrets};

/// The value of `APIFORGE_SECRET_TOKEN` in these tests.
const TOKEN: &str = "tok-mcp06-9f3c1e-live";
const JOURNAL: &str = ".apiforge/local/journal.json";
const NOT_ENABLED: &str = "live requests are not enabled on this server";
const OFF_IN_SETTINGS: &str = "live requests are off in this project's settings";

/// A server as `apiforge mcp [--allow-live]` starts it, reading
/// `APIFORGE_SECRET_<NAME>` from `environment` instead of the process's.
fn live_server(root: &Path, allow_live: bool, environment: &[(&str, &str)]) -> ApiforgeServer {
    live_server_with(root, allow_live, environment, RunSettings::default())
}

fn live_server_with(
    root: &Path,
    allow_live: bool,
    environment: &[(&str, &str)],
    run: RunSettings,
) -> ApiforgeServer {
    ApiforgeServer::with_settings(
        root.to_owned(),
        ServerSettings {
            run,
            allow_live,
            secrets: Secrets::with_environment(
                None,
                environment
                    .iter()
                    .map(|(name, value)| ((*name).into(), (*value).into()))
                    .collect(),
            ),
            cleanup_delay: None,
        },
    )
}

fn live(path: &str) -> RunArguments {
    RunArguments {
        live: true,
        ..arguments(path)
    }
}

/// Points the project's base URL at `base_url`, the shop's root.
fn point_at(root: &Path, base_url: &str) {
    let manifest = fs::read_to_string(root.join(MANIFEST)).unwrap();
    let current: Value = serde_json::from_str(&manifest).unwrap();
    let current = current["environment"]["baseUrl"]
        .as_str()
        .unwrap()
        .to_owned();
    edit(root, MANIFEST, |text| text.replace(&current, base_url));
}

/// The demo API in this process on a free loopback port, with the project
/// pointed at it.
fn demo_api(root: &Path) -> apiforge_demo_api::DemoServerHandle {
    let api = apiforge_demo_api::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    point_at(root, &format!("{}/shop", api.base_url()));
    api
}

/// The demo scenario with a declared secret `TOKEN` that one more request
/// sends in its query and a header.
fn with_secret(root: &Path) {
    edit(root, MANIFEST, |text| {
        text.replace("\"secretNames\": []", "\"secretNames\": [\"TOKEN\"]")
    });
    edit(root, SCENARIO, |text| {
        text.replacen(
            "  cleanup {\n",
            "  @label(\"Echo\")\n  echo = GET \"/products/124?q={{env.TOKEN}}\" {\n    headers { \"X-Echo\": \"{{env.TOKEN}}\" }\n    sample 200 { id: 124 }\n  }\n  cleanup {\n",
            1,
        )
    });
}

fn journal(root: &Path) -> Option<Value> {
    fs::read(root.join(JOURNAL))
        .ok()
        .map(|bytes| serde_json::from_slice(&bytes).unwrap())
}

/// Waits up to ten seconds for a request to reach `api`, then for the run
/// journal to show it. A request is journalled before it is sent, so the
/// file is read only once the request has arrived: polling it meanwhile
/// would race the runner's atomic replace, which Windows refuses while the
/// file is open.
fn wait_for_journal(root: &Path, api: &Listener) -> Value {
    let started = Instant::now();
    while api.accepted() == 0 {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "no request was sent"
        );
        thread::sleep(Duration::from_millis(5));
    }
    loop {
        if let Some(journal) = journal(root)
            && journal["runs"][0]["entries"]
                .as_array()
                .is_some_and(|entries| !entries.is_empty())
        {
            return journal;
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "no request was journalled"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

fn wait_until_busy(server: &ApiforgeServer) {
    let started = Instant::now();
    while !server.runs().is_busy() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the run did not start"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

/// Criterion 1: without `--allow-live` a live run is `live_disabled`, sends
/// nothing and takes nothing; the overview says why.
#[test]
fn mcp06_criterion_1_without_allow_live_nothing_is_sent() {
    let directory = checkout();
    let root = directory.path();
    let api = Listener::start();
    point_at(root, &api.base_url());
    let server = live_server(root, false, &[]);
    let error = server
        .run_tool(live(SCENARIO), CancellationToken::new())
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::LiveDisabled);
    assert_eq!(error.message, NOT_ENABLED);
    assert_eq!(error.wrote, Wrote::No);
    let overview = server.overview_tool().unwrap();
    assert!(!overview.live_requests.allowed);
    assert_eq!(overview.live_requests.source, "no --allow-live");
    thread::sleep(Duration::from_millis(200));
    assert_eq!(api.accepted(), 0, "the API saw no request");
    assert!(!locked(root), "the refusal took no lock");
    assert!(journal(root).is_none());

    let allowed = live_server(root, true, &[]).overview_tool().unwrap();
    assert!(allowed.live_requests.allowed);
    assert_eq!(allowed.live_requests.source, "--allow-live");
}

/// Criterion 2: with `--allow-live` the demo shop runs against the demo API
/// and passes; the declared secret is `[REDACTED]` everywhere in the result.
#[test]
fn mcp06_criterion_2_a_live_run_against_the_demo_api_passes_redacted() {
    let directory = checkout();
    let root = directory.path();
    let _api = demo_api(root);
    with_secret(root);
    let server = live_server(root, true, &[("APIFORGE_SECRET_TOKEN", TOKEN)]);
    let report = server
        .run_tool(live(SCENARIO), CancellationToken::new())
        .unwrap();
    assert_eq!(report.status, Verdict::Passed);
    assert!(report.live);
    assert_eq!(report.ended_by, EndedBy::Completed);
    assert!(report.redacted);
    assert_eq!(report.generation, head_generation(root));
    assert_eq!(
        report.source_sha256.as_deref(),
        Some(&*sha256(&root.join(SCENARIO)))
    );
    assert!(report.interrupted.is_empty());
    assert!(
        handles(report.steps.as_deref().unwrap()).contains(&"echo"),
        "the secret's request ran"
    );
    assert!(!serde_json::to_string(&report).unwrap().contains(TOKEN));

    let full = server
        .run_tool(
            RunArguments {
                full: true,
                ..live(SCENARIO)
            },
            CancellationToken::new(),
        )
        .unwrap();
    assert_eq!(full.status, Verdict::Passed);
    let text = serde_json::to_string(&full).unwrap();
    assert!(text.contains("[REDACTED]"), "the value is masked");
    assert!(!text.contains(TOKEN), "the value never leaves the server");
    assert!(journal(root).is_none(), "a finished run leaves no journal");
}

/// Criterion 3: a live run needs the agent session: refused with the holder
/// while a window or another process holds the project, a simulation still
/// works; a free project is taken for the session, which keeps it.
#[test]
fn mcp06_criterion_3_a_live_run_needs_the_agent_session() {
    let directory = checkout();
    let root = directory.path();
    let _api = demo_api(root);
    let server = live_server(root, true, &[]);
    for (holder, code) in [
        (LockHolder::window(), ErrorCode::HeldByWindow),
        (LockHolder::command("apply"), ErrorCode::HeldByOther),
    ] {
        let owner = ProjectStore::open_exclusive_as(root, &holder).unwrap();
        let error = server
            .run_tool(live(SCENARIO), CancellationToken::new())
            .unwrap_err();
        assert_eq!(error.code, code);
        assert_eq!(error.wrote, Wrote::No);
        assert!(error.details["holder"]["since"].is_string(), "{error:?}");
        let simulation = server
            .run_tool(arguments(SCENARIO), CancellationToken::new())
            .unwrap();
        assert_eq!(simulation.status, Verdict::Passed);
        assert!(!simulation.live);
        assert!(journal(root).is_none(), "nothing was sent or journalled");
        drop(owner);
    }
    let report = server
        .run_tool(live(SCENARIO), CancellationToken::new())
        .unwrap();
    assert_eq!(report.status, Verdict::Passed);
    assert!(server.agent().is_active(), "the run took the agent session");
    assert!(locked(root), "and the session keeps the lock");
    assert_eq!(server.overview_tool().unwrap().holder, Holder::AgentSession);
    let again = server
        .run_tool(live(SCENARIO), CancellationToken::new())
        .unwrap();
    assert_eq!(again.status, Verdict::Passed, "the session's lock is used");
    assert!(server.agent().finish_editing().finished);
    assert!(!locked(root));
}

/// Criterion 4: the permission slot. A window decides while it is
/// connected, the flag otherwise; the test sets the slot as MCP-09's channel
/// will.
#[test]
fn mcp06_criterion_4_the_window_decides_while_connected_the_flag_otherwise() {
    let directory = checkout();
    let root = directory.path();
    let _api = demo_api(root);

    let server = live_server(root, false, &[]);
    assert_eq!(server.permission().current(), Permission::Flag(false));
    server.permission().set_window(true);
    assert_eq!(
        server.permission().current(),
        Permission::Window { live: true }
    );
    let overview = server.overview_tool().unwrap();
    assert!(overview.live_requests.allowed);
    assert_eq!(overview.live_requests.source, "window setting");
    let report = server
        .run_tool(live(SCENARIO), CancellationToken::new())
        .unwrap();
    assert_eq!(report.status, Verdict::Passed, "allowed without the flag");
    assert!(report.live);
    server.permission().window_disconnected();
    assert_eq!(server.permission().current(), Permission::Flag(false));
    let error = server
        .run_tool(live(SCENARIO), CancellationToken::new())
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::LiveDisabled);
    assert_eq!(error.message, NOT_ENABLED);
    assert!(server.agent().finish_editing().finished);

    let flagged = live_server(root, true, &[]);
    flagged.permission().set_window(false);
    let error = flagged
        .run_tool(live(SCENARIO), CancellationToken::new())
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::LiveDisabled);
    assert_eq!(
        error.message, OFF_IN_SETTINGS,
        "the window overrides the flag"
    );
    let overview = flagged.overview_tool().unwrap();
    assert!(!overview.live_requests.allowed);
    assert_eq!(
        overview.live_requests.source,
        "off in the project's settings"
    );
    flagged.permission().window_disconnected();
    assert_eq!(flagged.permission().current(), Permission::Flag(true));
    let report = flagged
        .run_tool(live(SCENARIO), CancellationToken::new())
        .unwrap();
    assert_eq!(report.status, Verdict::Passed, "the flag applies again");
}

/// Criterion 5: the permission is checked when the run starts; a change
/// after the start does not stop it (decision 16 A); the next run is
/// refused.
#[test]
fn mcp06_criterion_5_a_change_after_the_start_does_not_stop_the_run() {
    let directory = checkout();
    let root = directory.path().to_owned();
    let _api = demo_api(&root);
    let server = live_server_with(
        &root,
        true,
        &[],
        RunSettings {
            deadline: DEADLINE,
            step_delay: Some(Duration::from_millis(100)),
        },
    );
    let running = {
        let server = server.clone();
        thread::spawn(move || server.run_tool(live(SCENARIO), CancellationToken::new()))
    };
    wait_until_busy(&server);
    thread::sleep(Duration::from_millis(250));
    server.permission().set_window(false);
    let report = running.join().unwrap().unwrap();
    assert_eq!(report.status, Verdict::Passed);
    assert_eq!(report.ended_by, EndedBy::Completed);
    let error = server
        .run_tool(live(SCENARIO), CancellationToken::new())
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::LiveDisabled);
    assert_eq!(error.message, OFF_IN_SETTINGS);
}

/// Criterion 6: a live run cancelled, or past its deadline, against an
/// endpoint that never answers: the main phase stops, the cleanup runs and
/// its steps are in the report, the in-flight request may have reached the
/// API, and the journal's entry is gone at the end.
#[test]
fn mcp06_criterion_6_a_cancelled_or_timed_out_live_run() {
    for timed_out in [false, true] {
        let directory = checkout();
        let root = directory.path().to_owned();
        let api = Listener::start();
        point_at(&root, &api.base_url());
        let deadline = if timed_out {
            Duration::from_millis(600)
        } else {
            DEADLINE
        };
        let server = live_server_with(
            &root,
            true,
            &[],
            RunSettings {
                deadline,
                step_delay: None,
            },
        );
        let cancel = CancellationToken::new();
        let running = {
            let (server, cancel) = (server.clone(), cancel.clone());
            thread::spawn(move || server.run_tool(live(SCENARIO), cancel))
        };
        let during = wait_for_journal(&root, &api);
        let entry = &during["runs"][0]["entries"][0];
        assert_eq!(entry["method"], "GET");
        assert_eq!(entry["state"], "inFlight", "journalled before it was sent");
        if !timed_out {
            thread::sleep(Duration::from_millis(200));
            cancel.cancel();
        }
        let report = running.join().unwrap().unwrap();
        assert_eq!(report.status, Verdict::Cancelled);
        assert_eq!(
            report.ended_by,
            if timed_out {
                EndedBy::TimedOut
            } else {
                EndedBy::Cancelled
            }
        );
        assert!(api.accepted() >= 1, "the request was on its way");
        let steps = report.steps.unwrap();
        let marked: Vec<_> = flatten(&steps)
            .into_iter()
            .filter(|step| step.may_have_reached_api)
            .collect();
        assert_eq!(marked.len(), 1, "{} steps", steps.len());
        assert_eq!(marked[0].handle, "fetch_product");
        assert_eq!(handles(&steps[steps.len() - 2..]), DEMO_STEPS[6..]);
        assert!(
            steps[steps.len() - 2..]
                .iter()
                .all(|step| step.phase == Phase::Cleanup),
            "the cleanup's steps are in the report"
        );
        assert!(journal(&root).is_none(), "the entry is removed at the end");
        assert!(!server.runs().is_busy());
    }
}

/// Criterion 8, in process: a run an earlier process left in the journal is
/// returned under `interrupted` by the next run, live or not, with method,
/// URL and state as `apiforge run` prints them; the overview counts it. The
/// real kill is in `crates/apiforge-cli/tests/mcp.rs`.
#[test]
fn mcp06_criterion_8_an_interrupted_run_is_reported_by_the_next_run() {
    let directory = checkout();
    let root = directory.path();
    let _api = demo_api(root);
    {
        // A server that died with a request on its way.
        let store = ProjectStore::open_exclusive(root).unwrap();
        let loaded = apiforge_storage::JournalStore::open_project(store).unwrap();
        let run = JournalRun::new(
            "run-1",
            "promotion",
            "Promotion and cart price",
            "2026-10-28T10:00:00.000Z",
        );
        loaded.store.begin_run(&run).unwrap();
        loaded
            .store
            .record(
                "run-1",
                &JournalEntry {
                    sequence: 1,
                    node_id: "node-1".into(),
                    handle: "add_to_cart".into(),
                    label: Some("Add to cart".into()),
                    phase: apiforge_core::run::Phase::Main,
                    method: "POST".into(),
                    url: "http://127.0.0.1:9/demo/shop/carts".into(),
                    started_at: "2026-10-28T10:00:01.000Z".into(),
                    state: EffectState::InFlight,
                    status: None,
                    detail: None,
                },
            )
            .unwrap();
    }
    assert!(journal(root).is_some());
    let server = live_server(root, true, &[]);
    assert_eq!(server.overview_tool().unwrap().interrupted_runs, 1);
    for arguments in [arguments(SCENARIO), live(SCENARIO)] {
        let report = server
            .run_tool(arguments.clone(), CancellationToken::new())
            .unwrap();
        assert_eq!(report.status, Verdict::Passed);
        assert_eq!(report.interrupted.len(), 1, "live: {}", arguments.live);
        let run = &report.interrupted[0];
        assert_eq!(run.scenario, "Promotion and cart price");
        assert_eq!(run.started_at, "2026-10-28T10:00:00.000Z");
        assert_eq!(run.requests.len(), 1);
        let request = &run.requests[0];
        assert_eq!(request.method, "POST");
        assert_eq!(request.url, "http://127.0.0.1:9/demo/shop/carts");
        assert_eq!(request.state, EffectState::InFlight.label());
        assert_eq!(request.step, "Add to cart");
    }
    // The interrupted run stays until the window marks it handled.
    assert_eq!(server.overview_tool().unwrap().interrupted_runs, 1);
}

/// Criterion 9: a live run repeated with a `request_id` the server answered
/// returns the recorded result and sends nothing.
#[test]
fn mcp06_criterion_9_an_answered_request_id_returns_the_recorded_result() {
    let directory = checkout();
    let root = directory.path();
    let api = demo_api(root);
    let server = live_server(root, true, &[]);
    let with_id = RunArguments {
        request_id: Some("live-run-1".into()),
        ..live(SCENARIO)
    };
    let first = server
        .run_tool(with_id.clone(), CancellationToken::new())
        .unwrap();
    assert_eq!(first.status, Verdict::Passed);
    assert!(!first.replayed);
    assert_eq!(first.request_id.as_deref(), Some("live-run-1"));
    // Anything sent now would reach a listener that counts it.
    drop(api);
    let listener = Listener::start();
    point_at(root, &listener.base_url());
    let second = server
        .run_tool(with_id.clone(), CancellationToken::new())
        .unwrap();
    assert!(second.replayed, "the recorded result");
    assert_eq!(second.status, first.status);
    assert_eq!(second.steps, first.steps);
    assert_eq!(second.generation, first.generation);
    thread::sleep(Duration::from_millis(200));
    assert_eq!(listener.accepted(), 0, "nothing was sent");
    // Another id is a new run: it reaches the listener and is cancelled.
    let cancel = CancellationToken::new();
    let other = {
        let (server, cancel) = (server.clone(), cancel.clone());
        thread::spawn(move || {
            server.run_tool(
                RunArguments {
                    request_id: Some("live-run-2".into()),
                    ..live(SCENARIO)
                },
                cancel,
            )
        })
    };
    wait_for_journal(root, &listener);
    cancel.cancel();
    let other = other.join().unwrap().unwrap();
    assert!(!other.replayed);
    assert!(listener.accepted() >= 1, "a new id sends");
}

/// Criterion 9's second half as written: a new server answers
/// `unknown_outcome` for an id it never saw. A server cannot tell an id it
/// never saw from a new one, so this waits for the owner's decision.
#[test]
#[ignore = "MCP-06: criterion 9's unknown_outcome for an unseen request_id awaits the owner's decision on #65"]
fn mcp06_criterion_9_a_new_server_answers_unknown_outcome_for_an_unseen_id() {
    let directory = checkout();
    let root = directory.path();
    let listener = Listener::start();
    point_at(root, &listener.base_url());
    let server = live_server(root, true, &[]);
    let error = server
        .run_tool(
            RunArguments {
                request_id: Some("lost-before-a-restart".into()),
                ..live(SCENARIO)
            },
            CancellationToken::new(),
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::UnknownOutcome);
    assert!(error.details["interrupted"].is_array());
    thread::sleep(Duration::from_millis(200));
    assert_eq!(listener.accepted(), 0);
}

/// Criterion 11: a saved source that contains a known secret value is
/// withheld by `read_source`, `print` and the resource with
/// `secret_in_source` and the line, never a modified text.
#[test]
fn mcp06_criterion_11_a_saved_source_with_a_secret_is_withheld() {
    let directory = checkout();
    let root = directory.path();
    edit(root, PRODUCT, |text| {
        text.replacen(" {\n", &format!(" {{\n  // {TOKEN}\n"), 1)
    });
    let server = live_server(root, false, &[("APIFORGE_SECRET_TOKEN", TOKEN)]);
    let uri = format!("{}{PRODUCT}", resources::SOURCE_PREFIX);
    for error in [
        server.read_source_tool(PRODUCT).unwrap_err(),
        server.print_tool(PRODUCT).unwrap_err(),
        server.resource_text(&uri).unwrap_err(),
    ] {
        assert_eq!(error.code, ErrorCode::SecretInSource, "{error:?}");
        assert_eq!(error.details["line"], 2);
        assert_eq!(error.wrote, Wrote::No);
        assert!(!error_text(&error).contains(TOKEN));
    }
    // The other documents are served; without the secret the file is read.
    assert!(server.read_source_tool(SCENARIO).is_ok());
    let plain = live_server(root, false, &[]);
    assert!(
        plain
            .read_source_tool(PRODUCT)
            .unwrap()
            .text
            .contains(TOKEN)
    );
}

/// Criterion 10, in process: a text argument or a request id that carries a
/// known value is refused, never echoed. The whole session over stdio, with
/// stderr and the journal, is in `crates/apiforge-cli/tests/mcp.rs`.
#[test]
fn mcp06_criterion_10_no_tool_accepts_a_secret_value() {
    let directory = checkout();
    let root = directory.path();
    let server = live_server(root, false, &[("APIFORGE_SECRET_TOKEN", TOKEN)]);
    let text = fs::read_to_string(root.join(SCENARIO)).unwrap();
    let leaked = text.replacen(" {\n", &format!(" {{\n  // {TOKEN}\n"), 1);
    let error = server.check_text_tool(SCENARIO, &leaked).unwrap_err();
    assert_eq!(error.code, ErrorCode::SecretInText);
    assert!(!error_text(&error).contains(TOKEN));
    let error = server
        .run_tool(
            RunArguments {
                request_id: Some(TOKEN.into()),
                ..arguments(SCENARIO)
            },
            CancellationToken::new(),
        )
        .unwrap_err();
    assert_eq!(
        error.code,
        ErrorCode::SecretInText,
        "a request id is echoed"
    );
    assert!(!error_text(&error).contains(TOKEN));
}

/// Criterion 12: the development guide's MCP section explains the flag, the
/// environment, the journal and the four ends.
#[test]
fn mcp06_criterion_12_the_development_guide_explains_live_runs() {
    let guide =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/development.md"))
            .unwrap();
    let start = guide.find("## MCP server").unwrap();
    let section = &guide[start..];
    let end = section[3..]
        .find("\n## ")
        .map_or(section.len(), |end| end + 3);
    let section = &section[..end];
    for needle in [
        "### Live requests",
        "--allow-live",
        "--secrets",
        "APIFORGE_SECRET_<NAME>",
        ".apiforge/local/journal.json",
        "interrupted",
        "notifications/cancelled",
        "timed_out",
        "end of stdin",
        "killed",
        "APIFORGE_MCP_TEST_CLEANUP_DELAY_MS",
    ] {
        assert!(section.contains(needle), "the MCP section names {needle}");
    }
}

// ----- MCP-06 review round 1 -----

/// A `request_id` names one run: the same id for another document is
/// refused, and an id whose result the server dropped to keep its list short
/// is `unknown_outcome`, never run again.
#[test]
fn mcp06_review_a_request_id_names_one_run_and_is_never_run_twice() {
    let directory = checkout();
    let root = directory.path();
    let api = demo_api(root);
    let server = live_server(root, true, &[]);
    let with_id = |id: &str, path: &str| RunArguments {
        request_id: Some(id.into()),
        ..live(path)
    };
    for number in 0..=64 {
        let report = server
            .run_tool(
                with_id(&number.to_string(), SCENARIO),
                CancellationToken::new(),
            )
            .unwrap();
        assert_eq!(report.status, Verdict::Passed);
    }
    let error = server
        .run_tool(with_id("64", PRODUCT), CancellationToken::new())
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidInputs);
    assert_eq!(error.details["path"], SCENARIO);
    drop(api);
    let listener = Listener::start();
    point_at(root, &listener.base_url());
    let replayed = server
        .run_tool(with_id("64", SCENARIO), CancellationToken::new())
        .unwrap();
    assert!(replayed.replayed);
    let error = server
        .run_tool(with_id("0", SCENARIO), CancellationToken::new())
        .unwrap_err();
    assert_eq!(
        error.code,
        ErrorCode::UnknownOutcome,
        "the oldest was dropped"
    );
    thread::sleep(Duration::from_millis(200));
    assert_eq!(listener.accepted(), 0, "nothing was sent");
}

/// While this server's own live run is in progress its journal entry is not
/// an interrupted run; a journal that cannot be read does not fail a
/// simulation.
#[test]
fn mcp06_review_the_server_own_run_is_not_interrupted() {
    let directory = checkout();
    let root = directory.path().to_owned();
    let api = Listener::start();
    point_at(&root, &api.base_url());
    let server = live_server(&root, true, &[]);
    let cancel = CancellationToken::new();
    let running = {
        let (server, cancel) = (server.clone(), cancel.clone());
        thread::spawn(move || server.run_tool(live(SCENARIO), cancel))
    };
    wait_for_journal(&root, &api);
    assert_eq!(server.overview_tool().unwrap().interrupted_runs, 0);
    cancel.cancel();
    assert_eq!(running.join().unwrap().unwrap().status, Verdict::Cancelled);
    assert!(server.agent().finish_editing().finished);

    fs::write(root.join(JOURNAL), b"not a journal").unwrap();
    let report = server
        .run_tool(arguments(SCENARIO), CancellationToken::new())
        .unwrap();
    assert_eq!(report.status, Verdict::Passed);
    assert!(report.journal_unreadable);
    assert!(report.interrupted.is_empty());
}
