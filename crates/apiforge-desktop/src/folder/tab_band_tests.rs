//! KTLO-24: the band with the open files' tabs hides itself when no file is
//! open, driven through named widgets (ADR 0013); see docs/tickets/KTLO-24.md.
//! Criterion 5 is the tab-strip tests in `tests.rs` and the KTLO-12 tests in
//! `header_tests.rs`, unchanged; the audit of criterion 6 is
//! `the_window_with_no_file_open_names_every_control` in
//! `accessibility_tests.rs`.
use super::*;
use crate::testing::{FolderHarness, MINIMUM, WIDE, folder, folder_harness, settle};
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use std::fs;

/// The demo scenario's file name, which its tab and its project files row show.
const NAME: &str = "promotion-and-cart-price.afs";
const MODES: [&str; 3] = ["Code", "Diagram", "Split"];
/// The ticket's tolerance on an edge, in points.
const EDGE: f32 = 1.0;
/// The band's height with a tab open, as it is before this ticket.
const BAND: f32 = 40.0;
/// The line under a top panel, inside its outer rect.
const SEPARATOR: f32 = 1.0;
/// The inner margin of the code area's hint with no file open.
const CODE_HINT_MARGIN: f32 = 24.0;
const CODE_HINT: &str = "Open a scenario or component from the document list.";
const BOARD_HINT: &str = "Open a scenario or component to see its diagram.";
const NOTICE: &str = "Moved to the trash.";
const BACKUP: &str = "Unsaved work from an earlier session is in the recovery backup.";

fn open(project: &tempfile::TempDir, size: egui::Vec2, mode: &str) -> FolderHarness {
    let mut harness = folder_harness(project.path(), size);
    click(&mut harness, mode);
    harness
}

fn click(harness: &mut FolderHarness, label: &str) {
    harness.get_by_label(label).click();
    settle(harness);
}

/// The outer rect of the panel `id` in the last frame that showed it.
fn panel(harness: &FolderHarness, id: &str) -> egui::Rect {
    egui::containers::panel::PanelState::load(&harness.ctx, egui::Id::new(id))
        .unwrap_or_else(|| panic!("the panel {id:?} was never shown"))
        .outer_rect
}

/// The bottom edge of the app header, the bar with Project, the modes and Run.
fn header_bottom(harness: &FolderHarness) -> f32 {
    let header = panel(harness, "folder-header");
    let project = harness.get_by_label("Project").rect();
    assert!(
        header.contains_rect(project),
        "the app header {header:?} holds Project at {project:?}"
    );
    header.bottom()
}

/// The top edge of each part of the central area in `mode`: the project files
/// panel and the code area in Code, the board in Diagram, the code panel and
/// the board in Split.
fn central_tops(harness: &FolderHarness, mode: &str) -> Vec<(&'static str, f32)> {
    let empty = folder(harness).workbench.active().is_none();
    let code_area =
        |harness: &FolderHarness| harness.get_by_label(CODE_HINT).rect().top() - CODE_HINT_MARGIN;
    let board = |harness: &FolderHarness| {
        if empty {
            harness.get_by_label(BOARD_HINT).rect().top()
        } else {
            harness.get_by_label("Diagram board").rect().top()
        }
    };
    let mut tops = Vec::new();
    match mode {
        "Code" => {
            tops.push((
                "project files panel",
                panel(harness, "folder-documents").top(),
            ));
            if empty {
                tops.push(("code area", code_area(harness)));
            }
        }
        "Diagram" => tops.push(("board", board(harness))),
        _ => {
            tops.push(("code panel", panel(harness, "folder-split-code").top()));
            if empty {
                tops.push(("code area", code_area(harness)));
            }
            tops.push(("board", board(harness)));
        }
    }
    tops
}

/// The tab strip's nodes: a tab is a button named after a project file, its
/// cross is "Close <file>". In Code the project files panel lists the same
/// names, so its rows are not counted.
fn strip_nodes(harness: &FolderHarness, mode: &str) -> Vec<String> {
    let files: Vec<String> = folder(harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .map(|document| file_name(document.entry().path.as_str()).to_owned())
        .collect();
    let explorer = (mode == "Code").then(|| panel(harness, "folder-documents"));
    harness
        .query_all_by(|node| node.role() == Role::Button)
        .filter_map(|node| {
            let label = node.accesskit_node().label()?;
            let tab = files.contains(&label);
            let cross = label
                .strip_prefix("Close ")
                .is_some_and(|file| files.iter().any(|known| known == file));
            let in_explorer = explorer.is_some_and(|explorer| explorer.contains_rect(node.rect()));
            ((tab && !in_explorer) || cross).then(|| format!("{label} at {:?}", node.rect()))
        })
        .collect()
}

fn assert_edge(found: f32, expected: f32, what: &str, context: &str) {
    assert!(
        (found - expected).abs() <= EDGE,
        "{context}: {what} at y = {found}, expected {expected} (±{EDGE})"
    );
}

/// Criterion 1: no tab, no cross and no other node of the strip, and the
/// central area meets the app header.
fn assert_no_band(harness: &FolderHarness, mode: &str, context: &str) {
    assert!(
        folder(harness).workbench.tabs().is_empty(),
        "{context}: no tab is open"
    );
    let strip = strip_nodes(harness, mode);
    assert!(strip.is_empty(), "{context}: the strip's nodes {strip:#?}");
    let header = header_bottom(harness);
    for (part, top) in central_tops(harness, mode) {
        assert_edge(top, header, &format!("the top of the {part}"), context);
    }
}

/// The band is shown directly under the app header and the central area
/// starts under it.
fn assert_band(harness: &FolderHarness, mode: &str, context: &str) -> egui::Rect {
    let band = panel(harness, "folder-tabs");
    assert_edge(
        band.top(),
        header_bottom(harness),
        "the top of the band",
        context,
    );
    for (part, top) in central_tops(harness, mode) {
        assert_edge(
            top,
            band.bottom(),
            &format!("the top of the {part}"),
            context,
        );
    }
    band
}

/// Criterion 2: the file's tab and its cross are in the tree, and the band
/// is as high as it is today.
fn assert_band_with_tab(harness: &FolderHarness, mode: &str, context: &str) {
    let band = assert_band(harness, mode, context);
    assert_edge(
        band.height(),
        BAND + SEPARATOR,
        "the band's height with its separator",
        context,
    );
    let strip = strip_nodes(harness, mode);
    for wanted in [NAME.to_owned(), format!("Close {NAME}")] {
        assert!(
            strip
                .iter()
                .any(|node| node.starts_with(&format!("{wanted} at"))),
            "{context}: {wanted:?} is in the strip {strip:#?}"
        );
    }
    let close = harness.get_by_label(&format!("Close {NAME}")).rect();
    assert!(
        band.contains_rect(close),
        "{context}: the cross {close:?} is on the band {band:?}"
    );
}

/// Criterion 4: the band carries `notice` and no tab.
fn assert_band_with_notice(harness: &FolderHarness, mode: &str, notice: &str, context: &str) {
    let band = assert_band(harness, mode, context);
    let strip = strip_nodes(harness, mode);
    assert!(strip.is_empty(), "{context}: the strip's nodes {strip:#?}");
    let shown = harness.get_by_label(notice).rect();
    assert!(
        band.contains_rect(shown),
        "{context}: the notice {shown:?} is on the band {band:?}"
    );
    // Before this ticket an empty strip, 28 points high, sat above the notice.
    let empty_strip = theme::CONTROL_HEIGHT - 4.0;
    assert!(
        shown.top() - band.top() < empty_strip,
        "{context}: the notice at {shown:?} is the band's first row, not under an \
         empty strip, on the band {band:?}"
    );
}

/// Closes every tab with its cross.
fn close_every_tab(harness: &mut FolderHarness) {
    while let Some(tab) = folder(harness).workbench.tabs().first() {
        let path = folder(harness)
            .session
            .as_ref()
            .unwrap()
            .document(&tab.document_id)
            .unwrap()
            .entry()
            .path
            .as_str()
            .to_owned();
        click(harness, &format!("Close {}", file_name(&path)));
    }
}

/// Criterion 1: with no tab open, in every mode at both sizes, the strip has
/// no node and the central area starts at the app header's bottom edge.
#[test]
fn with_no_tab_open_the_central_area_meets_the_app_header() {
    for size in [MINIMUM, WIDE] {
        let project = shop();
        let mut harness = open(&project, size, "Code");
        close_every_tab(&mut harness);
        for mode in MODES {
            click(&mut harness, mode);
            assert_no_band(&harness, mode, &format!("{mode} at {size:?}"));
        }
    }
}

/// Criterion 2: opening a file from the project files panel with no tab open
/// brings the band back with the file's tab and cross, 40 points high, and
/// the central area starts under it, in every mode.
#[test]
fn opening_a_file_from_the_project_files_brings_the_band_back() {
    for size in [MINIMUM, WIDE] {
        let project = shop();
        let mut harness = open(&project, size, "Code");
        close_every_tab(&mut harness);
        harness.get_by_role_and_label(Role::Button, NAME).click();
        settle(&mut harness);
        assert_eq!(folder(&harness).workbench.tabs().len(), 1, "one tab opens");
        for mode in MODES {
            click(&mut harness, mode);
            assert_band_with_tab(&harness, mode, &format!("{mode} at {size:?}"));
        }
    }
}

/// Criterion 3: closing the last tab with its cross, and with Ctrl+W, takes
/// the band away again.
#[test]
fn closing_the_last_tab_by_its_cross_or_ctrl_w_takes_the_band_away() {
    for size in [MINIMUM, WIDE] {
        for mode in MODES {
            let context = format!("{mode} at {size:?}");
            let project = shop();
            let mut harness = open(&project, size, mode);
            assert_band_with_tab(&harness, mode, &format!("{context}, before"));
            click(&mut harness, &format!("Close {NAME}"));
            assert_no_band(&harness, mode, &format!("{context}, closed by its cross"));

            let project = shop();
            let mut harness = open(&project, size, mode);
            harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::W);
            settle(&mut harness);
            assert_no_band(&harness, mode, &format!("{context}, closed by Ctrl+W"));
        }
    }
}

/// Criterion 3: a project last closed with no file open opens without the
/// band.
#[test]
fn a_project_opened_with_no_file_open_starts_without_the_band() {
    for size in [MINIMUM, WIDE] {
        for mode in MODES {
            let context = format!("{mode} at {size:?}, reopened");
            let project = shop();
            let mut harness = open(&project, size, mode);
            close_every_tab(&mut harness);
            let deadline = Instant::now() + Duration::from_secs(15);
            while !harness.state_mut().as_mut().unwrap().prepare_close() {
                assert!(Instant::now() < deadline, "close did not finish");
                harness.step();
            }
            drop(harness);

            let reopened = folder_harness(project.path(), size);
            assert_eq!(
                format!("{:?}", folder(&reopened).workbench.mode),
                mode,
                "{context}: the mode comes back"
            );
            assert_no_band(&reopened, mode, &context);
        }
    }
}

/// Criterion 4: with no tab open, the dismissable notice brings the band back
/// carrying only the notice; dismissing it takes the band away again.
#[test]
fn a_dismissable_notice_brings_the_band_back_until_it_is_dismissed() {
    for size in [MINIMUM, WIDE] {
        let project = shop();
        let mut harness = open(&project, size, "Code");
        close_every_tab(&mut harness);
        harness.state_mut().as_mut().unwrap().notify(NOTICE, false);
        settle(&mut harness);
        for mode in MODES {
            click(&mut harness, mode);
            let context = format!("{mode} at {size:?}");
            assert_band_with_notice(&harness, mode, NOTICE, &context);
        }
        click(&mut harness, "Dismiss");
        for mode in MODES {
            click(&mut harness, mode);
            let context = format!("{mode} at {size:?}, dismissed");
            assert_no_band(&harness, mode, &context);
        }
    }
}

/// The demo shop whose recovery file is unreadable while its backup holds a
/// draft of the scenario, as a crash during a recovery write leaves it.
fn shop_with_a_recovery_backup() -> tempfile::TempDir {
    let project = shop();
    let mut session = ProjectSession::open(project.path()).unwrap();
    let id = session
        .documents()
        .iter()
        .find(|document| document.entry().kind == DocumentKind::Scenario)
        .unwrap()
        .id()
        .to_owned();
    let text = session.document(&id).unwrap().text().to_owned();
    for edit in ["// first draft\n", "// second draft\n"] {
        session
            .edit_source(&id, &format!("{text}{edit}"), CODE)
            .unwrap();
        session.save_recovery().unwrap();
    }
    drop(session);
    let local = project.path().join(".apiforge/local");
    assert!(local.join("recovery.json.bak").exists(), "a backup exists");
    fs::write(local.join("recovery.json"), b"{truncated").unwrap();
    project
}

/// Criterion 4: with no tab open, the recovery backup's notice brings the
/// band back carrying only the notice; restoring the backup takes it away.
#[test]
fn the_recovery_backup_notice_brings_the_band_back_until_it_is_resolved() {
    for size in [MINIMUM, WIDE] {
        let project = shop_with_a_recovery_backup();
        let mut harness = open(&project, size, "Code");
        assert!(
            folder(&harness)
                .session
                .as_ref()
                .unwrap()
                .recovery_decision_required(),
            "the fixture asks for a recovery decision"
        );
        close_every_tab(&mut harness);
        for mode in MODES {
            click(&mut harness, mode);
            let context = format!("{mode} at {size:?}");
            assert_band_with_notice(&harness, mode, BACKUP, &context);
        }
        click(&mut harness, "Restore backup");
        assert!(
            !folder(&harness)
                .session
                .as_ref()
                .unwrap()
                .recovery_decision_required(),
            "the backup is restored"
        );
        for mode in MODES {
            click(&mut harness, mode);
            let context = format!("{mode} at {size:?}, restored");
            assert_no_band(&harness, mode, &context);
        }
    }
}

/// Waits until the view change of the last click is written to the
/// project's view state, which starts a second after the change. A walk of
/// the keyboard focus that overlapped that write skipped the project files
/// panel.
fn view_written(harness: &mut FolderHarness) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while folder(harness).changed_at.is_some() {
        assert!(Instant::now() < deadline, "the view state was not written");
        harness.step();
        std::thread::sleep(Duration::from_millis(10));
    }
    settle(harness);
}

/// The controls Tab visits from the app header's first one, Project, until it
/// comes back to Project: each one's name and rect.
fn tab_cycle(harness: &mut FolderHarness) -> Vec<(String, egui::Rect)> {
    view_written(harness);
    harness.get_by_label("Project").focus();
    settle(harness);
    let mut visited = Vec::new();
    for _ in 0..60 {
        harness.key_press(egui::Key::Tab);
        settle(harness);
        let Some((name, at)) = harness
            .query_by(|node| node.is_focused() && node.role() != Role::Window)
            .map(|node| {
                let name = node.accesskit_node().label();
                (name.unwrap_or_else(|| "<unnamed>".to_owned()), node.rect())
            })
        else {
            continue;
        };
        if name == "Project" {
            return visited;
        }
        visited.push((name, at));
    }
    panic!("Tab never came back to Project: {visited:#?}");
}

/// Criterion 6: with no tab open, Tab from the app header goes on to the
/// central area and never to the strip. In Code the first stop after the
/// header is a control of the project files panel; in Diagram and Split the
/// central area holds only hints with no file open, so Tab cycles through the
/// header and the code panel's resize handle.
#[test]
fn tab_from_the_app_header_lands_on_the_central_area() {
    for size in [MINIMUM, WIDE] {
        let project = shop();
        let mut harness = open(&project, size, "Code");
        close_every_tab(&mut harness);
        for mode in MODES {
            click(&mut harness, mode);
            let context = format!("{mode} at {size:?}");
            let header = panel(&harness, "folder-header");
            let top = central_tops(&harness, mode)
                .into_iter()
                .map(|(_, top)| top)
                .fold(f32::MAX, f32::min);
            let visited = tab_cycle(&mut harness);
            for (name, at) in &visited {
                assert!(
                    header.contains_rect(*at) || at.top() >= top - EDGE,
                    "{context}: Tab visits {name:?} at {at:?}, between the app header \
                     {header:?} and the central area at y = {top}: {visited:#?}"
                );
            }
            let first = visited.iter().find(|(_, at)| !header.contains_rect(*at));
            if mode == "Code" {
                let explorer = panel(&harness, "folder-documents");
                let (name, at) = first.unwrap_or_else(|| {
                    panic!("{context}: Tab never leaves the app header: {visited:#?}")
                });
                assert!(
                    explorer.contains_rect(*at),
                    "{context}: the first stop after the header, {name:?} at {at:?}, is in \
                     the project files panel {explorer:?}"
                );
            }
        }
    }
}

/// Criterion 5: with one tab open, and with more tabs than fit the width, the
/// strip is as before this ticket: 40 points high, every tab with its cross,
/// the active tab revealed, and a notice drawn under the tabs. The existing
/// tab-strip tests (`a_tabs_close_control_closes_only_its_own_file_and_keeps_asking`,
/// `equal_file_names_are_told_apart_in_the_tab_strip_and_long_ones_are_bounded`,
/// `ctrl_tab_brings_the_active_tab_back_into_view`) and the KTLO-12 header
/// tests run unchanged beside this one.
#[test]
fn with_tabs_open_the_strip_and_its_notices_are_as_before() {
    for size in [MINIMUM, WIDE] {
        let project = shop();
        let mut harness = open(&project, size, "Code");
        for mode in MODES {
            click(&mut harness, mode);
            let context = format!("{mode} at {size:?}, one tab");
            assert_band_with_tab(&harness, mode, &context);
        }

        let app = harness.state_mut().as_mut().unwrap();
        let first = app.workbench.tabs()[0].document_id.clone();
        for index in 0..16 {
            let id = app
                .session
                .as_mut()
                .unwrap()
                .create_document(
                    DocumentKind::Scenario,
                    &format!("task-{index:02}.afs"),
                    &format!("scenario \"Task {index}\" {{}}\n"),
                )
                .unwrap();
            app.workbench.open(&id);
        }
        // The first tab is active again, far to the left of the strip.
        app.workbench.open(&first);
        settle(&mut harness);
        for notice in [false, true] {
            if notice {
                harness.state_mut().as_mut().unwrap().notify(NOTICE, false);
            }
            for mode in MODES {
                click(&mut harness, mode);
                let context = format!("{mode} at {size:?}, seventeen tabs, notice {notice}");
                let band = assert_band(&harness, mode, &context);
                let crosses = harness
                    .query_all_by(|node| {
                        node.role() == Role::Button
                            && node
                                .label()
                                .is_some_and(|label| label.starts_with("Close task-"))
                    })
                    .count();
                assert_eq!(crosses, 16, "{context}: every tab has its cross");
                let active = harness.get_by_label(&format!("Close {NAME}")).rect();
                assert!(
                    band.contains_rect(active),
                    "{context}: the active tab's cross {active:?} is revealed on the band {band:?}"
                );
                if !notice {
                    assert_edge(
                        band.height(),
                        BAND + SEPARATOR,
                        "the band's height with its separator",
                        &context,
                    );
                    continue;
                }
                let shown = harness.get_by_label(NOTICE).rect();
                assert!(
                    band.contains_rect(shown) && shown.top() >= active.bottom(),
                    "{context}: the notice {shown:?} is under the tabs {active:?} on the band \
                 {band:?}"
                );
            }
        }
    }
}
