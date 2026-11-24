//! DIAG-17 (#551) reproduction: a step added on the *Cleanup* board takes a
//! name already used by a step of *Main*, and the edit is refused with "The
//! edited model is not a valid definition for this document." Main and its
//! cleanup share one namespace of handles (the cleanup graph is validated
//! with Main's nodes as external), but `Board::add_node` numbers the new
//! handle against the cleanup graph alone.
use super::diag06a::{graph, shop_with};
use super::*;
use crate::canvas::Command;
use apiforge_core::{block::Block, format::Position};

const SCENARIO: &str = r#"scenario "Cleanup names" {
  begin = start
  request = GET "/items" {
    sample 200 { id: "i-1" }
  }
  check = assert request.status == 200
  cleanup {
    removed = DELETE "/items/{{request.body.id}}" {
      sample 204 {}
    }
  }
}
"#;

/// The window on [`SCENARIO`] with the *Cleanup* board in view.
fn on_cleanup() -> (tempfile::TempDir, FolderHarness, String) {
    let (project, mut harness, id) = shop_with(SCENARIO);
    harness.get_by_label("Cleanup").click();
    settle(&mut harness);
    harness.step();
    assert!(
        folder(&harness).cleanup(&id),
        "the Cleanup board is in view"
    );
    (project, harness, id)
}

/// Sends `command` to the board and returns the handle of the step it added,
/// or the board's banner when nothing was added.
fn added(harness: &mut FolderHarness, id: &str, command: Command) -> Result<String, String> {
    let before = graph(harness, id);
    app(harness).active_board_command(command);
    settle(harness);
    harness.step();
    let after = graph(harness, id);
    after
        .nodes
        .iter()
        .find(|node| !before.nodes.iter().any(|old| old.id == node.id))
        .map(|node| node.handle.clone())
        .ok_or_else(|| {
            [
                "This edit was not applied: duplicate handle. Nothing was changed.",
                "Duplicate handle",
            ]
            .into_iter()
            .find(|message| has_label(harness, message))
            .unwrap_or("no step added, no banner")
            .to_owned()
        })
}

const AT: Position = Position { x: 40.0, y: 400.0 };

#[test]
fn diag17_a_request_added_on_cleanup_from_the_library_is_not_refused() {
    let (_project, mut harness, id) = on_cleanup();
    let handle = added(
        &mut harness,
        &id,
        Command::AddNode {
            block: Block::Request,
            component: None,
            position: AT,
            source: None,
        },
    );
    assert_eq!(
        handle.as_deref(),
        Ok("request_2"),
        "{}",
        source_of(&harness, &id)
    );
}

#[test]
fn diag17_a_comparison_added_on_cleanup_from_the_library_is_not_refused() {
    let (_project, mut harness, id) = on_cleanup();
    let handle = added(
        &mut harness,
        &id,
        Command::AddNode {
            block: Block::Comparison,
            component: None,
            position: AT,
            source: None,
        },
    );
    assert_eq!(
        handle.as_deref(),
        Ok("check_2"),
        "{}",
        source_of(&harness, &id)
    );
}

#[test]
fn diag17_a_request_added_on_cleanup_with_add_step_is_not_refused() {
    let (_project, mut harness, id) = on_cleanup();
    let handle = added(
        &mut harness,
        &id,
        Command::AddStep {
            block: Block::Request,
            component: None,
            position: AT,
        },
    );
    assert_eq!(
        handle.as_deref(),
        Ok("request_2"),
        "{}",
        source_of(&harness, &id)
    );
}

#[test]
fn diag17_a_request_added_from_a_cleanup_cards_plus_is_not_refused() {
    let (_project, mut harness, id) = on_cleanup();
    let removed = graph(&harness, &id)
        .nodes
        .iter()
        .find(|node| node.handle == "removed")
        .unwrap()
        .id
        .clone();
    let handle = added(
        &mut harness,
        &id,
        Command::AddNode {
            block: Block::Request,
            component: None,
            position: AT,
            source: Some((removed, "__order".to_owned())),
        },
    );
    assert_eq!(
        handle.as_deref(),
        Ok("request_2"),
        "{}",
        source_of(&harness, &id)
    );
}

/// The real gesture: a click on *HTTP request* in the *Library* panel.
#[test]
fn diag17_a_click_in_the_library_on_cleanup_is_not_refused() {
    let (_project, mut harness, id) = on_cleanup();
    let before = graph(&harness, &id).nodes.len();
    harness.get_by_label("HTTP request").click();
    settle(&mut harness);
    harness.step();
    assert!(
        !has_label(
            &harness,
            "This edit was not applied: duplicate handle. Nothing was changed."
        ),
        "the add is refused"
    );
    assert_eq!(
        graph(&harness, &id).nodes.len(),
        before + 1,
        "one step is added"
    );
}
