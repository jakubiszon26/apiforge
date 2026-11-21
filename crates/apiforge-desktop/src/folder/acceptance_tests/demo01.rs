//! DEMO-01 (#523): a first start in an empty data directory writes the demo
//! of `fixtures/demo/` — *Demo shop* with the scenarios *Buy coffee on
//! promotion* and *The cart refuses bad orders* and the component `product`
//! at revision 1 — and opens the first scenario on a board where no two
//! cards overlap. A data directory that already holds a project opens that
//! project and leaves it as it is (owner, 2026-11-21). The command line's
//! side (format, check, both runs) is `crates/apiforge-cli/tests/demo01.rs`.
use super::*;
use crate::canvas::Command;
use crate::testing::MINIMUM;
use apiforge_core::format::Position;
use egui::Rect;
use egui_kittest::Harness;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const BUY_COFFEE: &str = "scenarios/buy-coffee-on-promotion.afs";
const BAD_ORDERS: &str = "scenarios/the-cart-refuses-bad-orders.afs";
const PRODUCT: &str = "components/product.afs";
const REVISION: &str = ".apiforge/revisions/product/1.afs";

/// What a person does not write and the board must not add (criterion 4).
const PRINTED_ONLY: [&str; 5] = ["@config", "@inputs", "@required", "@exposed", "@after"];

fn demo_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/demo")
}

/// Every file under `root` by its path from it, with its bytes, leaving out
/// what this computer keeps for itself (`.apiforge/local/`).
fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, directory: &Path, found: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if relative == ".apiforge/local" {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, found);
            } else {
                found.insert(relative, fs::read(&path).unwrap());
            }
        }
    }
    let mut found = BTreeMap::new();
    walk(root, root, &mut found);
    found
}

/// The window a person's first start opens, at `size`: managed startup on
/// `data`, with the view the application chooses for a project never opened
/// on this computer (LAY-40), not the Code view the other managed tests pin.
fn first_start(data: &Path, size: egui::Vec2) -> FolderHarness {
    let mut harness = Harness::builder().with_size(size).build_ui_state(
        |ui, app: &mut Option<FolderApp>| {
            if let Some(app) = app {
                app.ui(ui);
            }
        },
        None,
    );
    let context = harness.ctx.clone();
    *harness.state_mut() = Some(FolderApp::start_managed(&context, data.to_owned()));
    settle(&mut harness);
    harness
}

fn session(harness: &FolderHarness) -> &apiforge_project::ProjectSession {
    folder(harness).session.as_ref().expect("a project is open")
}

fn document_at(harness: &FolderHarness, path: &str) -> String {
    session(harness)
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == path)
        .unwrap_or_else(|| panic!("{path} is a document of the project"))
        .id()
        .to_owned()
}

/// Closes the window the way the application closes it.
fn close(mut harness: FolderHarness) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !app(&mut harness).prepare_close() {
        assert!(
            folder(&harness).close_error.is_none(),
            "{:?}",
            folder(&harness).close_error
        );
        assert!(Instant::now() < deadline, "close did not finish");
        harness.step();
    }
}

/// Criterion 1: an empty data directory opens *Demo shop* with the two
/// approved scenarios and `product` at revision 1, nothing else, written as
/// `fixtures/demo/` holds them; the first scenario's tab is open.
#[test]
fn demo01_criterion_1_first_start() {
    let data = tempfile::tempdir().unwrap();
    let harness = first_start(data.path(), WIDE);
    let window = folder(&harness);
    assert!(window.open_error.is_none(), "{:?}", window.open_error);
    let session = session(&harness);
    assert_eq!(session.manifest().name, "Demo shop");

    let mut documents: Vec<(String, DocumentKind)> = session
        .documents()
        .iter()
        .map(|document| {
            (
                document.entry().path.as_str().to_owned(),
                document.entry().kind,
            )
        })
        .collect();
    documents.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(
        documents,
        [
            (PRODUCT.to_owned(), DocumentKind::Component),
            (BUY_COFFEE.to_owned(), DocumentKind::Scenario),
            (BAD_ORDERS.to_owned(), DocumentKind::Scenario),
        ],
        "two scenarios and one component, no other"
    );
    let revisions: Vec<(String, String, u32)> = session
        .manifest()
        .revisions
        .iter()
        .map(|revision| {
            (
                revision.handle.clone(),
                revision.path.as_str().to_owned(),
                revision.revision,
            )
        })
        .collect();
    assert_eq!(
        revisions,
        [("product".to_owned(), REVISION.to_owned(), 1)],
        "product is published at revision 1, nothing else is"
    );

    // Every file of the demo is on disk as the fixture holds it: the texts
    // as approved, the manifest, the published revision and the layout.
    let root = window.root.clone();
    let written = files(&root);
    let fixture = files(&demo_fixture());
    for path in [BUY_COFFEE, BAD_ORDERS, PRODUCT, REVISION] {
        assert!(fixture.contains_key(path), "fixtures/demo has {path}");
    }
    for (path, bytes) in &fixture {
        assert_eq!(
            written
                .get(path)
                .map(|written| String::from_utf8_lossy(written)),
            Some(String::from_utf8_lossy(bytes)),
            "{path} is written as fixtures/demo holds it"
        );
    }
    for path in [BUY_COFFEE, BAD_ORDERS, PRODUCT] {
        let id = document_at(&harness, path);
        assert_eq!(
            session.document(&id).unwrap().text().as_bytes(),
            &fixture[path][..],
            "{path} opens as written"
        );
        assert_eq!(
            session.document(&id).unwrap().analysis(),
            AnalysisState::Valid,
            "{path}"
        );
    }

    assert_eq!(
        window.workbench.active(),
        Some(document_at(&harness, BUY_COFFEE).as_str()),
        "Buy coffee on promotion is the open tab"
    );
}

/// Criterion 4: no file of the demo holds what only the window prints, and
/// moving a card on the first board and saving adds none.
#[test]
fn demo01_criterion_4_written_as_a_person_writes() {
    let data = tempfile::tempdir().unwrap();
    let mut harness = first_start(data.path(), WIDE);
    let root = folder(&harness).root.clone();
    let assert_none = |when: &str| {
        for (path, bytes) in files(&root) {
            let text = String::from_utf8_lossy(&bytes);
            for attribute in PRINTED_ONLY {
                assert!(
                    !text.contains(attribute),
                    "{when}: {path} holds {attribute}:\n{text}"
                );
            }
        }
    };
    assert_none("a first start");

    let id = document_at(&harness, BUY_COFFEE);
    let before = fs::read_to_string(root.join(BUY_COFFEE)).unwrap();
    let node = graph_of(
        session(&harness)
            .document(&id)
            .unwrap()
            .model()
            .expect("a valid model"),
        false,
    )
    .nodes
    .iter()
    .find(|node| node.handle == "cart")
    .expect("a step cart")
    .clone();
    app(&mut harness).active_board_command(Command::Move(vec![(
        node.id.clone(),
        Position {
            x: node.position.x + 240.0,
            y: node.position.y + 160.0,
        },
    )]));
    settle(&mut harness);
    app(&mut harness).save_all();
    settle(&mut harness);
    let document = session(&harness).document(&id).unwrap();
    assert!(
        !document.is_dirty() && !document.is_layout_dirty(),
        "the move is saved"
    );
    assert_eq!(
        fs::read_to_string(root.join(BUY_COFFEE)).unwrap(),
        before,
        "a move changes no text"
    );
    assert_none("after moving a card and saving");
}

/// Criterion 5: on the first board, at the minimum and the default window,
/// no two cards overlap and every card of the chain is in view at Fit.
#[test]
fn demo01_criterion_5_first_board() {
    for size in [MINIMUM, WIDE] {
        let data = tempfile::tempdir().unwrap();
        let mut harness = first_start(data.path(), size);
        let id = document_at(&harness, BUY_COFFEE);
        assert_eq!(folder(&harness).workbench.active(), Some(id.as_str()));
        assert_eq!(
            folder(&harness).workbench.mode,
            Mode::Diagram,
            "{size:?}: a first start shows the board"
        );
        choose(&mut harness, "Fit the view");
        harness.step();
        settle(&mut harness);
        let board = folder(&harness)
            .canvases
            .get(&id)
            .expect("the board is drawn");
        let steps = graph_of(
            session(&harness)
                .document(&id)
                .unwrap()
                .model()
                .expect("a valid model"),
            false,
        )
        .nodes
        .len();
        let cards: Vec<(String, Rect)> = board.debug_cards().to_vec();
        assert!(
            cards.len() >= 10,
            "{size:?}: the chain is drawn: {} cards of {steps} steps",
            cards.len()
        );
        let focus = board.debug_focus();
        for (card, rect) in &cards {
            assert!(
                focus.contains_rect(*rect),
                "{size:?}: {card} {rect:?} is out of view {focus:?}"
            );
        }
        for (index, (first, a)) in cards.iter().enumerate() {
            for (second, b) in &cards[index + 1..] {
                assert!(
                    !a.shrink(0.5).intersects(b.shrink(0.5)),
                    "{size:?}: {first} {a:?} overlaps {second} {b:?}"
                );
            }
        }
    }
}

/// Criterion 6: a data directory holding a project, as v0.2.0 left one
/// after its own first start, opens that project, unchanged byte for byte.
#[test]
fn demo01_criterion_6_existing_project_untouched() {
    let data = tempfile::tempdir().unwrap();
    let root = data
        .path()
        .join(apiforge_project::PROJECTS_DIRECTORY)
        .join("shop");
    fs::create_dir_all(&root).unwrap();
    apiforge_project::prepare_project(&root, &apiforge_core::demos::shop_project()).unwrap();
    let root = std::path::absolute(&root).unwrap();
    let recent = serde_json::json!({
        "formatVersion": 1,
        "active": root,
        "projects": [{ "id": "shop", "name": "Demo shop", "path": root }],
    });
    fs::write(
        data.path().join("recent-projects.json"),
        serde_json::to_vec_pretty(&recent).unwrap(),
    )
    .unwrap();
    // v0.2.0 opened it once in the window, which laid its board out and
    // saved that layout: that is the install a new version finds.
    close(first_start(data.path(), WIDE));
    let before = files(&root);
    assert!(before.contains_key("scenarios/promotion-and-cart-price.afs"));

    let harness = first_start(data.path(), WIDE);
    let window = folder(&harness);
    assert!(window.open_error.is_none(), "{:?}", window.open_error);
    assert_eq!(window.root, root, "the project of the data directory opens");
    assert!(
        session(&harness)
            .documents()
            .iter()
            .any(|document| document.entry().path.as_str()
                == "scenarios/promotion-and-cart-price.afs"),
        "its own scenario, not the new demo"
    );
    close(harness);
    let projects: Vec<_> = fs::read_dir(data.path().join(apiforge_project::PROJECTS_DIRECTORY))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(projects.len(), 1, "no second project: {projects:?}");
    let after = files(&root);
    assert_eq!(
        after.keys().collect::<Vec<_>>(),
        before.keys().collect::<Vec<_>>(),
        "the same files"
    );
    for (path, bytes) in &before {
        assert!(after[path] == *bytes, "{path} changed");
    }
}

/// Every block of a scenario, main flow and cleanup, at its rounded position.
fn positions(model: &Document) -> BTreeMap<String, (i64, i64)> {
    [false, true]
        .into_iter()
        .flat_map(|cleanup| graph_of(model, cleanup).nodes.iter())
        .map(|node| {
            (
                node.handle.clone(),
                (
                    node.position.x.round() as i64,
                    node.position.y.round() as i64,
                ),
            )
        })
        .collect()
}

/// The layout of the first board is *Tidy up*'s, as in the approved
/// mock-up, and it is on disk once the window has opened: the first start
/// writes the demo's texts and the window lays each board out as its first
/// open does (LAY-33), at the density this computer draws, and saves that.
#[test]
fn demo01_the_first_board_is_tidy_ups_and_saved() {
    for size in [MINIMUM, WIDE] {
        let data = tempfile::tempdir().unwrap();
        let mut harness = first_start(data.path(), size);
        let root = folder(&harness).root.clone();
        let id = document_at(&harness, BUY_COFFEE);
        let positions = |harness: &FolderHarness| {
            positions(session(harness).document(&id).unwrap().model().unwrap())
        };
        let opened = positions(&harness);
        let document = session(&harness).document(&id).unwrap();
        assert!(
            !document.is_dirty() && !document.is_layout_dirty(),
            "{size:?}: a first start leaves nothing unsaved"
        );
        // Read back by a session without a window, as MCP or the command
        // line would read it.
        let saved = {
            let session = apiforge_project::ProjectSession::open(&root).unwrap();
            let document = session
                .documents()
                .iter()
                .find(|document| document.entry().path.as_str() == BUY_COFFEE)
                .unwrap();
            self::positions(document.model().unwrap())
        };
        assert_eq!(saved, opened, "{size:?}: the first board's layout is saved");

        app(&mut harness).active_board_command(Command::TidyUp(Vec::new()));
        settle(&mut harness);
        let tidied = positions(&harness);
        let moved: Vec<String> = opened
            .iter()
            .filter(|(handle, at)| {
                let to = tidied[*handle];
                (to.0 - at.0).abs() > 1 || (to.1 - at.1).abs() > 1
            })
            .map(|(handle, at)| format!("{handle} {at:?} -> {:?}", tidied[handle]))
            .collect();
        assert!(
            moved.is_empty(),
            "{size:?}: Tidy up moves {} cards of the first board: {moved:#?}",
            moved.len()
        );
    }
}
