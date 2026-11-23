//! #433: a check added from Run results on a step that stands inside an `if`
//! branch is written in that step's braces, right after the step's existing
//! checks, and nothing else in the file changes. Found in the LAY acceptance
//! trial on the room-booking scenario, criterion 5.
use super::*;
use egui_kittest::kittest::NodeT;

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";

/// The room-booking scenario of the trial, without its sign-in component:
/// `second` stands in `booked`'s `then`, checked by `refused`.
const SOURCE: &str = r#"scenario "Double booking is refused" {
  begin = start
  first = POST "/bookings" {
    sample 201 { id: "bk-1" }
  }
  booked = if first.status == 201 {
    second = POST "/bookings" {
      sample 409 { error: { code: "room_unavailable" } }
    }
    refused = assert second.status == 409
    calendar = GET "/bookings" {
      sample 200 { items: [{ id: "bk-1" }] }
    }
    one_booking = assert calendar.body.items[0].id == first.body.id
  } else {
    not_booked = assert first.status == 201
  }
  cleanup {
    cancelled = DELETE "/bookings/{{first.body.id}}" {
      sample 204 {}
    }
    gone = assert cancelled.status == 204
  }
}
"#;

/// The scenario above as the demo shop's, open in Split at the wide window.
fn open() -> (tempfile::TempDir, FolderHarness, String) {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), SOURCE).unwrap();
    let mut harness = folder_harness(project.path(), WIDE);
    let id = only(&harness, DocumentKind::Scenario);
    app(&mut harness).activate_tab(&id);
    settle(&mut harness);
    choose(&mut harness, "Split");
    (project, harness, id)
}

/// Chooses `handle`'s row in the step list of Run results.
fn select(harness: &mut FolderHarness, handle: &str) {
    let row = format!("{handle} · ");
    let label = harness
        .query_all_by_label_contains(&row)
        .filter_map(|found| found.accesskit_node().label())
        .find(|label| label.starts_with(&row))
        .unwrap_or_else(|| panic!("{handle} has a row in the step list"));
    harness.get_by_label(&label).scroll_to_me();
    settle(harness);
    harness.get_by_label(&label).click();
    settle(harness);
}

/// Opens the nested value of `field` in the shown value tree: the toggle on
/// its row, unless the row is open already, as Run results open a value to
/// its leaves since REP-08 (#455).
fn expand(harness: &mut FolderHarness, field: &str) {
    let y = harness.get_by_label(field).rect().center().y;
    let on_row = |label: &str| {
        harness
            .query_all_by_label(label)
            .any(|toggle| (toggle.rect().center().y - y).abs() < 13.0)
    };
    if on_row("Collapse field") {
        return;
    }
    harness
        .query_all_by_label("Expand field")
        .find(|toggle| (toggle.rect().center().y - y).abs() < 13.0)
        .unwrap_or_else(|| panic!("the {field} row opens"))
        .click();
    settle(harness);
}

/// Run results → `second` → Outputs → `error` → *Check* on `code`, then
/// *Save*: the saved file has the check in `booked`'s `then`, right after
/// `refused` and before `@after(refused) calendar`, and nothing else changed.
#[test]
fn issue433_a_check_from_run_results_is_written_in_its_steps_braces() {
    let (project, mut harness, id) = open();
    choose(&mut harness, "Run simulation");
    let run = finish_run(&mut harness);
    assert_eq!(run.status, RunStatus::Passed, "{:?}", run.error);
    choose(&mut harness, "Maximise run results");
    select(&mut harness, "second");
    expand(&mut harness, "error");
    crate::testing::check_field(&mut harness, "code");
    choose(&mut harness, "Save");
    settle(&mut harness);

    let expected = SOURCE.replace(
        "    refused = assert second.status == 409\n",
        "    refused = assert second.status == 409\n    \
         @label(\"Check error.code\")\n    \
         check_error_code = assert second.body.error.code == \"room_unavailable\"\n",
    );
    assert_eq!(source_of(&harness, &id), expected);
    assert_eq!(
        fs::read_to_string(project.path().join(SCENARIO)).unwrap(),
        expected,
        "the saved file"
    );
}
