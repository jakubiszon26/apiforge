//! LAY-40 (#436): a scenario in a project never opened on this computer
//! opens in Diagram; a view the project was left in still wins (owner, #296
//! Q8 = A). A project with no scenario, and a new one, open in Code; a mode
//! asked for at startup is shown and not remembered. Each test opens a
//! temporary copy of the demo shop with no stored view.
use super::*;
use crate::testing::{first_open_harness, quiesce, shop_project};
use egui_kittest::kittest::NodeT;
use std::collections::BTreeMap;
use std::path::Path;

/// Where the window keeps the view a project was left in, on this computer.
const VIEW_STATE: &str = ".apiforge/local/workspace.json";

fn selected(harness: &FolderHarness, label: &str) -> bool {
    harness.get_by_label(label).accesskit_node().toggled() == Some(egui::accesskit::Toggled::True)
}

fn active(harness: &FolderHarness) -> (String, DocumentKind) {
    let id = folder(harness)
        .workbench
        .active()
        .expect("a first open shows a document")
        .to_owned();
    let kind = folder(harness)
        .session
        .as_ref()
        .unwrap()
        .document(&id)
        .unwrap()
        .entry()
        .kind;
    (id, kind)
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

/// Every file under `root`, by path relative to it, with its bytes.
fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, at: &Path, into: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, into);
            } else {
                let relative = path.strip_prefix(root).unwrap();
                let name = relative.to_string_lossy().replace('\\', "/");
                into.insert(name, fs::read(&path).unwrap());
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(root, root, &mut files);
    files
}

/// Criterion 1: the first open of a project whose first tab is a scenario
/// shows Diagram, the Diagram switch is the selected one, and the board of
/// that scenario is drawn with its cards.
#[test]
fn lay40_criterion_1_a_first_open_shows_the_scenario_in_diagram() {
    let project = shop_project();
    let harness = first_open_harness(project.path(), WIDE, None);
    let (id, kind) = active(&harness);
    assert_eq!(kind, DocumentKind::Scenario, "the first tab is the scenario");
    assert_eq!(folder(&harness).workbench.mode, Mode::Diagram);
    assert!(selected(&harness, "Diagram"), "the Diagram switch is selected");
    assert!(!selected(&harness, "Code") && !selected(&harness, "Split"));
    let board = folder(&harness)
        .canvases
        .get(&id)
        .expect("the scenario's board is drawn");
    assert!(!board.debug_cards().is_empty(), "the board has cards");
}

/// Criterion 2: the view a project is left in comes back on the next open,
/// whichever it is.
#[test]
fn lay40_criterion_2_a_stored_view_wins() {
    for (label, mode) in [
        ("Code", Mode::Code),
        ("Split", Mode::Split),
        ("Diagram", Mode::Diagram),
    ] {
        let project = shop_project();
        let mut harness = first_open_harness(project.path(), WIDE, None);
        harness.get_by_label(label).click();
        settle(&mut harness);
        assert_eq!(folder(&harness).workbench.mode, mode, "after {label}");
        close(harness);
        assert!(project.path().join(VIEW_STATE).exists(), "the view is kept");

        let harness = first_open_harness(project.path(), WIDE, None);
        assert_eq!(
            folder(&harness).workbench.mode,
            mode,
            "a project left in {label} opens in {label}"
        );
        assert!(selected(&harness, label));
    }
}

/// Criterion 2: the Diagram a first open shows is the view the project is
/// left in when nobody changes it.
#[test]
fn lay40_criterion_2_the_first_diagram_is_remembered() {
    let project = shop_project();
    let harness = first_open_harness(project.path(), WIDE, None);
    close(harness);
    let harness = first_open_harness(project.path(), WIDE, None);
    assert_eq!(folder(&harness).workbench.mode, Mode::Diagram);
}

/// Criterion 3: a project with no scenario, its scenarios removed from the
/// manifest and the folder, opens in Code on its component.
#[test]
fn lay40_criterion_3_a_project_without_a_scenario_opens_in_code() {
    let project = shop_project();
    {
        let mut session = apiforge_project::ProjectSession::open(project.path()).unwrap();
        let scenarios: Vec<String> = session
            .documents()
            .iter()
            .filter(|document| document.entry().kind == DocumentKind::Scenario)
            .map(|document| document.entry().path.as_str().to_owned())
            .collect();
        assert!(!scenarios.is_empty(), "the demo has a scenario");
        let paths: Vec<&str> = scenarios.iter().map(String::as_str).collect();
        let preview = session.preview_delete(&paths).unwrap();
        session
            .delete(&preview, apiforge_project::Origin::SESSION)
            .unwrap();
        for path in &scenarios {
            assert!(!project.path().join(path).exists(), "{path} is gone");
        }
    }
    let harness = first_open_harness(project.path(), WIDE, None);
    let (_, kind) = active(&harness);
    assert_eq!(kind, DocumentKind::Component, "the first tab");
    assert_eq!(folder(&harness).workbench.mode, Mode::Code);
    assert!(selected(&harness, "Code"));
}

/// Criterion 4: a mode asked for at startup is the one a first open shows,
/// and the project does not remember it: the next open without a request
/// shows the project's own first view.
#[test]
fn lay40_criterion_4_a_requested_mode_is_shown_and_not_remembered() {
    for (request, label) in [(Mode::Code, "Code"), (Mode::Split, "Split")] {
        let project = shop_project();
        let harness = first_open_harness(project.path(), WIDE, Some(request));
        assert_eq!(folder(&harness).workbench.mode, request, "--mode {label}");
        assert!(selected(&harness, label));
        close(harness);

        let harness = first_open_harness(project.path(), WIDE, None);
        assert_eq!(
            folder(&harness).workbench.mode,
            Mode::Diagram,
            "--mode {label} is not what the project remembers"
        );
    }
}

/// What opening did to a folder: each file it added, changed or removed,
/// by path, with what happened to it.
fn written(before: &BTreeMap<String, Vec<u8>>, after: &BTreeMap<String, Vec<u8>>) -> Vec<String> {
    let mut written: Vec<String> = after
        .iter()
        .filter_map(|(path, bytes)| match before.get(path) {
            None => Some(format!("added {path}")),
            Some(old) if old != bytes => Some(format!("changed {path}")),
            Some(_) => None,
        })
        .collect();
    written.extend(
        before
            .keys()
            .filter(|path| !after.contains_key(*path))
            .map(|path| format!("removed {path}")),
    );
    written.sort();
    written
}

/// Criterion 5: a first open in Diagram writes nothing under the project
/// folder that a first open in Code does not. Two copies of the demo shop
/// are opened for the first time, one shown in Diagram by the rule and one
/// asked for Code, as `main` shows it; each folder is listed before and
/// after, and the files each open wrote are the same. Neither changes a
/// file the project shares; only the view kept on this computer
/// (`.apiforge/local/`) may be written.
#[test]
fn lay40_criterion_5_a_first_open_writes_nothing_more() {
    let diagram = shop_project();
    let code = shop_project();
    let (before_diagram, before_code) = (files(diagram.path()), files(code.path()));

    let mut shown = first_open_harness(diagram.path(), WIDE, None);
    assert_eq!(folder(&shown).workbench.mode, Mode::Diagram);
    quiesce(&mut shown);
    let mut asked = first_open_harness(code.path(), WIDE, Some(Mode::Code));
    assert_eq!(folder(&asked).workbench.mode, Mode::Code);
    quiesce(&mut asked);

    let in_diagram = written(&before_diagram, &files(diagram.path()));
    let in_code = written(&before_code, &files(code.path()));
    errln!("LAY-40 a first open in Diagram wrote {in_diagram:?}, in Code {in_code:?}");
    assert_eq!(in_diagram, in_code, "the same files written");
    for change in &in_diagram {
        assert!(
            change.contains(" .apiforge/local/"),
            "{change}: a first open changes no file the project shares"
        );
    }
}
