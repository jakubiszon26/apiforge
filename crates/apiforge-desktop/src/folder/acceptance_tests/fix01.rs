//! FIX-01 (#454): the `labels` reference scenario, sixteen requests to one
//! endpoint that none reads another, opens as a folder project of its own
//! through `crate::testing::labels_board`, for the interaction tests of the
//! tickets that follow. It is also a reference board of LAY-06 (`lay06.rs`),
//! whose board file the layout crate's LAY-28 and LAY-29 tests route.
use super::*;
use crate::testing::{LABELS, labels_board};

/// Criterion 3: the helper opens the fixture, unchanged, as the only
/// document of a folder project, on its board, with no problem.
#[test]
fn fix01_criterion_3_labels_opens_as_a_folder_project() {
    let fixture = fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scenarios/lay/labels.afs"),
    )
    .unwrap();
    let (project, harness, id) = labels_board(WIDE);
    assert_eq!(
        fs::read_to_string(project.path().join(LABELS)).unwrap(),
        fixture
    );
    let app = folder(&harness);
    assert_eq!(
        app.session.as_ref().unwrap().documents().len(),
        1,
        "the scenario and nothing else"
    );
    assert_eq!(app.workbench.active().unwrap().to_string(), id);
    assert!(app.canvases.contains_key(&id), "its board is drawn");
    let snapshot = app.snapshot(&id).unwrap();
    assert_eq!(snapshot.path, LABELS);
    assert_eq!(snapshot.text, fixture);
    assert_eq!(snapshot.diagnostics, [], "no problem");
    let model = snapshot.model.expect("a valid model");
    let apiforge_script::Document::Scenario(scenario) = model.as_ref() else {
        panic!("labels is a scenario");
    };
    assert_eq!(scenario.name, "Label validation");
    assert_eq!(scenario.graph.nodes.len(), 50);
}
