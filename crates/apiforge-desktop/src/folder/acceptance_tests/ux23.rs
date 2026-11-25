//! UX-23 (#231, from #177; owner decisions 14 = A and 15 = A of
//! 2026-11-02): `apiforge.project.json` is shown in the Explorer but
//! protected. Its menu offers no *Open as text*, *Rename…*, *Move to…* or
//! *Delete…*, F2 and Delete do nothing on it, and a click explains what the
//! file is in a muted notice instead of an error banner, leaving the open tab
//! as it was. A text file such as `.gitignore` opened in Diagram says it has
//! no diagram and offers *Show code*, which shows it in Code.
use super::ux31::window;
use super::*;
use crate::testing::{MINIMUM, capture, files_row};

const MANIFEST: &str = "apiforge.project.json";
const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
/// A text file of the demo shop that is not protected: the control.
const ATTRIBUTES: &str = ".gitattributes";
const IGNORE: &str = ".gitignore";
const NO_DIAGRAM: &str =
    ".gitignore is a text file, not a scenario or component, so it has no diagram.";
const FILE_ACTIONS: [&str; 4] = ["Open as text", "Rename…", "Move to…", "Delete…"];

/// The Explorer row named `name`, clicked with `button`.
fn click_row(harness: &mut FolderHarness, name: &str, button: egui::PointerButton) {
    files_row(harness, name).click_button(button);
    settle(harness);
}

/// The file actions `name`'s context menu offers, after checking that it
/// opened.
fn menu_actions(harness: &mut FolderHarness, name: &str) -> Vec<&'static str> {
    click_row(harness, name, egui::PointerButton::Secondary);
    assert!(
        has_label(harness, &format!("{name} menu")),
        "right-clicking {name} opens its menu"
    );
    assert!(
        has_label(harness, "Reveal in file manager"),
        "{name}'s menu still reveals it"
    );
    let offered = FILE_ACTIONS
        .into_iter()
        .filter(|action| has_label(harness, action))
        .collect();
    harness.key_press(egui::Key::Escape);
    settle(harness);
    offered
}

/// The demo shop with a `.gitignore`, so the scan lists it.
fn shop_with_ignore() -> tempfile::TempDir {
    let project = crate::testing::shop_project();
    std::fs::write(project.path().join(IGNORE), "/target\n").unwrap();
    project
}

/// Criterion 1: the manifest's menu has no file actions, which another text
/// file's menu has; F2 and Delete on its selected row open no prompt and
/// leave the file in place.
#[test]
fn ux23_criterion_1_the_manifest_has_no_file_actions_by_menu_or_key() {
    let project = crate::testing::shop_project();
    let (mut harness, _) = window(project.path(), WIDE, SCENARIO, "Code");
    assert_eq!(
        menu_actions(&mut harness, ATTRIBUTES),
        FILE_ACTIONS,
        "another text file keeps its file actions"
    );
    assert_eq!(
        menu_actions(&mut harness, MANIFEST),
        Vec::<&str>::new(),
        "the project file offers none"
    );

    click_row(&mut harness, MANIFEST, egui::PointerButton::Primary);
    assert_eq!(
        folder(&harness).explorer.selected.as_deref(),
        Some(MANIFEST)
    );
    for key in [egui::Key::F2, egui::Key::Delete] {
        harness.key_press(key);
        settle(&mut harness);
        assert!(
            folder(&harness).prompt.is_none(),
            "{key:?} on the project file opens no prompt"
        );
    }
    assert!(project.path().join(MANIFEST).is_file(), "it stays in place");
}

/// Criterion 2: a click on the manifest raises a muted notice, not an error,
/// that says what the file is; the scenario's tab stays the open one and no
/// tab opens for the manifest. Enter on its row does the same.
#[test]
fn ux23_criterion_2_a_click_on_the_manifest_explains_it_and_keeps_the_tab() {
    for size in [MINIMUM, WIDE] {
        let project = crate::testing::shop_project();
        let (mut harness, scenario) = window(project.path(), size, SCENARIO, "Code");
        let tabs = folder(&harness).workbench.tabs().len();
        click_row(&mut harness, MANIFEST, egui::PointerButton::Primary);
        let context = format!("at {size:?}");
        let notice = folder(&harness)
            .notice()
            .map(|notice| (notice.text.clone(), notice.error));
        let Some((text, error)) = notice else {
            panic!("{context}: the click explains the project file");
        };
        assert!(!error, "{context}: {text:?} is not an error");
        assert!(
            text.contains("project file"),
            "{context}: {text:?} says what the file is"
        );
        assert!(has_label(&harness, &text), "{context}: it is shown");
        assert_eq!(
            folder(&harness).workbench.active(),
            Some(scenario.as_str()),
            "{context}: the scenario's tab stays the open one"
        );
        assert_eq!(
            folder(&harness).workbench.tabs().len(),
            tabs,
            "{context}: no tab opens for the project file"
        );
        assert_eq!(folder(&harness).workbench.mode, Mode::Code, "{context}");

        // The same from the keyboard, once the notice has gone.
        app(&mut harness).notices.clear();
        harness.key_press(egui::Key::Enter);
        settle(&mut harness);
        assert!(
            folder(&harness)
                .notice()
                .is_some_and(|notice| !notice.error && notice.text == text),
            "{context}: Enter explains it too"
        );
        assert_eq!(
            folder(&harness).workbench.active(),
            Some(scenario.as_str()),
            "{context}: Enter keeps the tab too"
        );
    }
}

/// Criterion 3: `.gitignore` opened from *Files* in Diagram says it has no
/// diagram and offers *Show code*, which switches to Code with the file open
/// in its editor.
#[test]
fn ux23_criterion_3_a_text_file_in_diagram_offers_show_code() {
    for size in [MINIMUM, WIDE] {
        let project = shop_with_ignore();
        let (mut harness, _) = window(project.path(), size, SCENARIO, "Diagram");
        let context = format!("at {size:?}");
        choose(&mut harness, "Files");
        click_row(&mut harness, IGNORE, egui::PointerButton::Primary);
        assert_eq!(folder(&harness).workbench.mode, Mode::Diagram, "{context}");
        assert!(has_label(&harness, NO_DIAGRAM), "{context}: the message");
        let message = harness.get_by_label(NO_DIAGRAM).rect();
        let button = harness.get_by_label("Show code").rect();
        assert!(
            button.top() >= message.bottom(),
            "{context}: *Show code* is under the message"
        );

        choose(&mut harness, "Show code");
        assert_eq!(folder(&harness).workbench.mode, Mode::Code, "{context}");
        let active = folder(&harness).workbench.active().unwrap().to_owned();
        assert_eq!(
            folder(&harness).text_of(&active),
            Some(IGNORE),
            "{context}: the file stays open"
        );
        assert!(
            has_label(&harness, &format!("Contents of {IGNORE}")),
            "{context}: its text is in the editor"
        );
    }
}

/// The two captures of the QA report, at 960 × 540: the explanation after a
/// click on the manifest in Code, and `.gitignore` in Diagram with *Show
/// code*.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_ux23() {
    let project = crate::testing::shop_project();
    let (mut harness, _) = window(project.path(), MINIMUM, SCENARIO, "Code");
    click_row(&mut harness, MANIFEST, egui::PointerButton::Primary);
    harness.step();
    capture(&mut harness, "ux23-manifest-click-960x540", None);

    let project = shop_with_ignore();
    let (mut harness, _) = window(project.path(), MINIMUM, SCENARIO, "Diagram");
    choose(&mut harness, "Files");
    click_row(&mut harness, IGNORE, egui::PointerButton::Primary);
    harness.step();
    capture(&mut harness, "ux23-gitignore-show-code-960x540", None);
}
