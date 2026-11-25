//! WS-05b (#596): *Files* lists a registered scenario or component by its
//! title, then by its file name in grey where the row has room. On WS-05a's
//! project with a component, an unregistered source and a second scenario
//! of one title beside it.
use super::lay16::click_at;
use super::ws05a::{
    DAY, DAY_TITLE, LONG, LONG_TITLE, REFUSED, REFUSED_TITLE, broken, enter, id_at, open_at, rooms,
    scenario, type_title, write_code,
};
use super::*;
use crate::testing::{MINIMUM, capture, files_row, hover_until};
use egui::{Color32, Pos2, Rect, Shape, Vec2, accesskit::Role, vec2};
use egui_kittest::kittest::NodeT;

const SIGN_IN: &str = "components/sign-in.afs";
const SIGN_IN_TITLE: &str = "Sign in to the booking desk";
/// A second scenario of [`REFUSED_TITLE`].
const AGAIN: &str = "scenarios/refused-again.afs";
/// On disk and not registered.
const LOOSE: &str = "scenarios/loose.afs";
/// The file names of the titled rows.
const FILES: [(&str, &str); 5] = [
    ("sign-in.afs", SIGN_IN_TITLE),
    ("booking-day.afs", DAY_TITLE),
    ("double-booking-notice-to-the-organiser.afs", LONG_TITLE),
    ("double-booking.afs", REFUSED_TITLE),
    ("refused-again.afs", REFUSED_TITLE),
];
/// The rows in the order they have had since P6: folders, then file names.
const ORDER: [&str; 13] = [
    "",
    "components",
    SIGN_IN,
    "scenarios",
    DAY,
    LONG,
    REFUSED,
    LOOSE,
    AGAIN,
    ".gitattributes",
    ".gitignore",
    "apiforge.project.json",
    "notes.md",
];
/// The least width a file name is cut to.
const LEAST: f32 = 40.0;

fn component(name: &str) -> String {
    format!(
        r#"component sign_in "{name}" {{
  @label("User")
  input user: string = "tester"
  session = POST "/sessions" {{
    body {{ user: param.user }}
    sample 201 {{ token: "tok-1" }}
  }}
  export token: string = session.body.token
}}
"#
    )
}

fn desk() -> tempfile::TempDir {
    let directory = rooms();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    session
        .create_document(DocumentKind::Component, SIGN_IN, &component(SIGN_IN_TITLE))
        .unwrap();
    session
        .create_document(DocumentKind::Scenario, AGAIN, &scenario(REFUSED_TITLE))
        .unwrap();
    drop(session);
    fs::write(directory.path().join(LOOSE), scenario("Loose")).unwrap();
    fs::write(directory.path().join("notes.md"), "Notes.\n").unwrap();
    directory
}

/// `root` open on `path` in Diagram with every folder of *Files* unfolded.
fn files_at(root: &std::path::Path, path: &str, size: Vec2) -> (FolderHarness, String) {
    let (mut harness, id) = open_at(root, path, size);
    for folder in ["components", "scenarios"] {
        app(&mut harness)
            .workbench
            .expanded
            .insert(folder.to_owned());
    }
    choose(&mut harness, "Files");
    follow_disk(&mut harness);
    harness.step();
    (harness, id)
}

fn files(path: &str, size: Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    let project = desk();
    let (harness, id) = files_at(project.path(), path, size);
    (project, harness, id)
}

/// A text painted on a row.
#[derive(Debug)]
struct Text {
    /// Without what was cut.
    shown: String,
    whole: bool,
    rect: Rect,
    colour: Color32,
    size: f32,
}

fn each_shape(shape: &Shape, visit: &mut impl FnMut(&Shape)) {
    match shape {
        Shape::Vec(shapes) => shapes.iter().for_each(|shape| each_shape(shape, visit)),
        shape => visit(shape),
    }
}

/// The shapes painted in `row` and nowhere else, from the last frame.
fn shapes_in(harness: &FolderHarness, row: Rect, mut visit: impl FnMut(&Shape)) {
    for clipped in &harness.output().shapes {
        if row.expand(1.0).contains_rect(clipped.clip_rect) {
            each_shape(&clipped.shape, &mut visit);
        }
    }
}

/// The texts of the row of `file`, from left to right.
fn texts(harness: &FolderHarness, file: &str) -> Vec<Text> {
    let row = files_row(harness, file).rect();
    let mut found = Vec::new();
    shapes_in(harness, row, |shape| {
        let Shape::Text(text) = shape else { return };
        let format = &text.galley.job.sections[0].format;
        found.push(Text {
            shown: text
                .galley
                .rows
                .iter()
                .flat_map(|row| row.glyphs.iter().map(|glyph| glyph.chr))
                .collect(),
            whole: !text.galley.elided,
            rect: text.galley.rect.translate(text.pos.to_vec2()),
            colour: format.color,
            size: format.font_id.size,
        });
    });
    found.sort_by(|left, right| left.rect.left().total_cmp(&right.rect.left()));
    found
}

fn shown(harness: &FolderHarness, file: &str) -> Vec<String> {
    texts(harness, file)
        .into_iter()
        .map(|text| text.shown)
        .collect()
}

/// Whether the title the row of `file` reads is `title`, cut or whole.
fn reads(harness: &FolderHarness, file: &str, title: &str) -> bool {
    let texts = texts(harness, file);
    let cut = texts[0].shown.trim_end_matches('…');
    title.starts_with(cut) && (texts[0].whole == (cut == title)) && !cut.is_empty()
}

fn name_of(harness: &FolderHarness, file: &str) -> String {
    files_row(harness, file)
        .accesskit_node()
        .label()
        .unwrap_or_default()
}

/// The circle at the row's right edge: its centre, fill and outline.
fn mark(harness: &FolderHarness, file: &str) -> Option<(Pos2, Color32, Color32)> {
    let row = files_row(harness, file).rect();
    let mut found = None;
    shapes_in(harness, row, |shape| {
        if let Shape::Circle(circle) = shape {
            found = Some((circle.center, circle.fill, circle.stroke.color));
        }
    });
    found
}

/// The list alone, `width` wide, as wide as no panel of the window is yet.
fn list(root: &std::path::Path, width: f32) -> FolderHarness {
    let (mut harness, _) = files_at(root, REFUSED, WIDE);
    let application = harness.state_mut().take().unwrap();
    let mut alone = egui_kittest::Harness::builder()
        .with_size(vec2(width, 600.0))
        .build_ui_state(
            |ui, app: &mut Option<FolderApp>| {
                if let Some(app) = app {
                    app.explorer_panel(ui);
                }
            },
            None,
        );
    theme::apply(&alone.ctx);
    *alone.state_mut() = Some(application);
    alone.run_steps(4);
    alone
}

fn drag(harness: &mut FolderHarness, from: Pos2, to: Pos2) {
    harness.hover_at(from);
    harness.step();
    harness.drag_at(from);
    harness.step();
    for step in 1..=8 {
        harness.hover_at(from + (to - from) * (step as f32 / 8.0));
        harness.step();
    }
    harness.drop_at(to);
}

fn f2(harness: &mut FolderHarness, file: &str) {
    files_row(harness, file).click();
    settle(harness);
    harness.key_press(egui::Key::F2);
    settle(harness);
    harness.step();
}

/// `item` chosen in the menu of the row of `file`.
fn in_menu(harness: &mut FolderHarness, file: &str, item: &str) {
    files_row(harness, file).click_secondary();
    settle(harness);
    harness.step();
    harness.get_by_label(item).click();
    settle(harness);
    harness.step();
}

/// The grey line under the heading of an open dialog that says `title`.
fn title_line(harness: &FolderHarness, heading: &str, title: &str) -> Option<Rect> {
    let heading = harness.get_by_label(heading).rect();
    harness
        .query_all_by_role_and_label(Role::Label, title)
        .map(|node| node.rect())
        .find(|rect| {
            rect.top() >= heading.bottom() - 1.0
                && rect.top() - heading.bottom() < 16.0
                && (rect.left() - heading.left()).abs() < 1.0
        })
}

fn cancel(harness: &mut FolderHarness) {
    harness.key_press(egui::Key::Escape);
    settle(harness);
    harness.step();
    assert!(folder(harness).prompt.is_none());
}

fn active_path(harness: &FolderHarness) -> String {
    let app = folder(harness);
    path_of(harness, app.workbench.active().unwrap())
}

/// Criterion 1: what each kind of row reads, and how it is named.
#[test]
fn rows_read_titles() {
    let (_project, harness, _) = files(REFUSED, WIDE);
    for (file, title) in FILES {
        assert!(
            reads(&harness, file, title),
            "{file}: {:?}",
            texts(&harness, file)
        );
        assert_eq!(name_of(&harness, file), format!("{title}, {file}"));
        let texts = texts(&harness, file);
        assert!(texts.len() <= 2, "{file}: {texts:?}");
        assert_eq!(texts[0].colour, theme::TEXT, "{file}");
        assert_eq!(texts[0].size, theme::FONT_BODY, "{file}");
        if let Some(name) = texts.get(1) {
            assert!(file.starts_with(name.shown.trim_end_matches('…')), "{file}");
            assert_eq!(name.colour, theme::MUTED, "{file}");
            assert!(name.size < theme::FONT_BODY, "{file}");
        }
    }
    let as_file = [
        ("loose.afs", theme::MUTED),
        ("notes.md", theme::MUTED),
        ("apiforge.project.json", theme::MUTED),
        ("scenarios", theme::TEXT),
        ("Room booking", theme::TEXT),
    ];
    for (file, colour) in as_file {
        assert_eq!(shown(&harness, file), [file]);
        assert_eq!(texts(&harness, file)[0].colour, colour, "{file}");
        assert_eq!(name_of(&harness, file), file);
    }

    // No model yet, an empty title, and a file that is gone.
    let project = desk();
    for (path, text) in [
        (REFUSED, "scenario \"Broken\" {\n  x =\n".to_owned()),
        (DAY, scenario("")),
        (AGAIN, scenario("   ")),
    ] {
        fs::write(project.path().join(path), text).unwrap();
    }
    fs::remove_file(project.path().join(LONG)).unwrap();
    let (harness, _) = files_at(project.path(), SIGN_IN, WIDE);
    for file in ["double-booking.afs", "booking-day.afs", "refused-again.afs"] {
        assert_eq!(shown(&harness, file), [file]);
        assert_eq!(texts(&harness, file)[0].colour, theme::TEXT, "{file}");
        assert_eq!(name_of(&harness, file), file);
    }
    let gone = "double-booking-notice-to-the-organiser.afs";
    let gone_texts = texts(&harness, gone);
    assert_eq!(gone_texts.len(), 1, "{gone_texts:?}");
    assert!(gone.starts_with(gone_texts[0].shown.trim_end_matches('…')));
    assert_eq!(gone_texts[0].colour, theme::MUTED);
    assert_eq!(name_of(&harness, gone), gone);
    assert!(reads(&harness, "sign-in.afs", SIGN_IN_TITLE));
}

/// The two lines shown where `at` is hovered, or the one when there is no
/// `title`.
fn assert_hover(
    project: &std::path::Path,
    at: impl Fn(&FolderHarness) -> Rect,
    title: Option<&str>,
    line: &str,
) {
    // Another document's, so that the header says neither.
    let (mut harness, _) = files_at(project, DAY, WIDE);
    for path in [REFUSED, SIGN_IN] {
        app(&mut harness).review_open(path, None);
        settle(&mut harness);
    }
    app(&mut harness).review_open(DAY, None);
    settle(&mut harness);
    let labels = |harness: &FolderHarness, text: &str| {
        harness
            .query_all_by_role_and_label(Role::Label, text)
            .map(|node| node.rect())
            .collect::<Vec<_>>()
    };
    let before = labels(&harness, line).len();
    assert!(
        hover_until(
            &mut harness,
            |harness| Some(at(harness)),
            |harness| labels(harness, line).len() == before + 1
        ),
        "the hover reads {line:?}"
    );
    if let Some(title) = title {
        let last = |text: &str| {
            labels(&harness, text)
                .iter()
                .map(|rect| rect.top())
                .fold(f32::MIN, f32::max)
        };
        assert!(
            last(title) < last(line) && last(line) - last(title) < 30.0,
            "{title:?} is above {line:?}"
        );
    }
}

fn tab(harness: &FolderHarness, file: &str) -> Rect {
    let close = harness.get_by_label(&format!("Close {file}")).rect();
    harness
        .query_all_by_role_and_label(Role::Button, file)
        .map(|node| node.rect())
        .find(|tab| tab.contains(close.center()))
        .unwrap_or_else(|| panic!("the tab of {file}"))
}

/// Criterion 2: the hover of a titled row and of its tab is the title, then
/// the path; the other rows' as before.
#[test]
fn hover_names_title_and_path() {
    let project = desk();
    for (path, title) in [(REFUSED, REFUSED_TITLE), (SIGN_IN, SIGN_IN_TITLE)] {
        let file = file_name(path);
        assert_hover(
            project.path(),
            |harness| files_row(harness, file).rect(),
            Some(title),
            path,
        );
        assert_hover(
            project.path(),
            |harness| tab(harness, file),
            Some(title),
            path,
        );
    }
    for (file, line) in [
        (
            "loose.afs",
            "scenarios/loose.afs — not registered in this project",
        ),
        ("notes.md", "notes.md"),
        ("scenarios", "scenarios"),
        ("Room booking", "The project folder"),
    ] {
        assert_hover(
            project.path(),
            |harness| files_row(harness, file).rect(),
            None,
            line,
        );
    }
    fs::remove_file(project.path().join(AGAIN)).unwrap();
    assert_hover(
        project.path(),
        |harness| files_row(harness, "refused-again.afs").rect(),
        None,
        "scenarios/refused-again.afs — missing from the folder",
    );
}

/// What every titled row keeps at any width: the title first and cut only at
/// its end, the file name whole, or cut to at least [`LEAST`], or absent,
/// and all of it inside the row and clear of its mark.
fn assert_fits(harness: &FolderHarness, context: &str) {
    for (file, title) in FILES {
        let row = files_row(harness, file).rect();
        let context = format!("{context}: {file}");
        assert_eq!(row.height(), 24.0, "{context}");
        assert!(reads(harness, file, title), "{context}");
        let texts = texts(harness, file);
        let limit = mark(harness, file).map_or(row.right(), |(centre, ..)| centre.x - 4.0);
        for text in &texts {
            assert!(
                text.rect.right() <= limit && row.y_range().contains(text.rect.center().y),
                "{context}: {text:?} in {row:?}"
            );
        }
        assert!(texts.len() <= 2, "{context}: {texts:?}");
        if let Some(name) = texts.get(1) {
            assert!(texts[0].whole, "{context}: a cut title has the row");
            assert!(texts[0].rect.right() + 4.0 <= name.rect.left(), "{context}");
            assert!(
                name.whole || name.rect.width() >= LEAST,
                "{context}: {name:?}"
            );
        }
    }
}

/// Criterion 3: at the default width a long title ends in an ellipsis, with
/// a mark and without one.
#[test]
fn long_title_at_the_default_width() {
    let long = "double-booking-notice-to-the-organiser.afs";
    for size in [MINIMUM, WIDE] {
        let (_project, mut harness, id) = files(LONG, size);
        let width = files_row(&harness, long).rect().width();
        assert!((width - 216.0).abs() < 2.0, "the list at 240 px: {width}");
        assert_fits(&harness, "240 px");
        assert!(shown(&harness, long)[0].ends_with('…'));
        let text = source_of(&harness, &id);
        write_code(&mut harness, &id, &format!("{text}// Unsaved.\n"));
        assert!(mark(&harness, long).is_some(), "the unsaved mark");
        assert_fits(&harness, "240 px, unsaved");
        assert!(shown(&harness, long)[0].ends_with('…'));
    }
    let project = desk();
    let mut harness = list(project.path(), 176.0);
    for width in (176..=620).step_by(3) {
        harness.set_size(vec2(width as f32, 600.0));
        harness.run_steps(3);
        assert_fits(&harness, &format!("{width} px"));
    }
}

/// Criterion 4: a list of 480 px shows the title and the file name of every
/// row whole that a list of 240 px cuts; of the longest, longer together than
/// any list, it shows the title whole and the file name cut.
#[test]
fn wide_list_shows_both() {
    let project = desk();
    let long = file_name(LONG);
    let mut harness = list(project.path(), 480.0);
    for (file, title) in FILES {
        let shown = shown(&harness, file);
        assert_eq!(shown[0], title);
        if file != long {
            assert_eq!(shown[1], file);
        }
    }
    assert!(shown(&harness, long)[1].ends_with('…'));
    assert_fits(&harness, "480 px");
    harness.set_size(vec2(720.0, 600.0));
    harness.run_steps(3);
    assert_eq!(shown(&harness, long), [LONG_TITLE, long]);

    // In the window, the panel as wide as its edge goes.
    let (_project, mut harness, _) = files(LONG, WIDE);
    let file = file_name(REFUSED);
    assert!(!texts(&harness, file)[0].whole, "cut at 240 px");
    // On the panel's side of the edge: the board is drawn over the other.
    let edge = harness.get_by_label("Resize project files").rect().center() - vec2(2.0, 0.0);
    drag(&mut harness, edge, edge + vec2(400.0, 0.0));
    settle(&mut harness);
    assert_eq!(shown(&harness, file), [REFUSED_TITLE, file]);
    assert_eq!(shown(&harness, long)[0].chars().last(), Some('…'));
    assert_fits(&harness, "the widest panel");
}

/// Criterion 5: two scenarios of one title are two rows, each of its own
/// file.
#[test]
fn one_title_twice() {
    let (_project, mut harness, _) = files(DAY, WIDE);
    let names = ["double-booking.afs", "refused-again.afs"].map(|file| name_of(&harness, file));
    assert_ne!(names[0], names[1]);
    for path in [AGAIN, REFUSED, AGAIN] {
        let file = file_name(path);
        assert!(reads(&harness, file, REFUSED_TITLE), "{file}");
        files_row(&harness, file).click();
        settle(&mut harness);
        assert_eq!(active_path(&harness), path);
    }
    assert!(
        folder(&harness)
            .session
            .as_ref()
            .unwrap()
            .documents()
            .iter()
            .all(|document| document.diagnostics().is_empty()),
        "no problem is reported"
    );
}

/// Criterion 6: the rows stand in the order of their files, whatever their
/// titles.
#[test]
fn order_is_by_file() {
    let (_project, mut harness, id) = files(REFUSED, WIDE);
    let order = |harness: &FolderHarness| {
        let rows: Vec<_> = folder(harness)
            .explorer_rows()
            .into_iter()
            .map(|row| row.path)
            .collect();
        let tops: Vec<_> = rows[1..]
            .iter()
            .map(|path| files_row(harness, file_name(path)).rect().top())
            .collect();
        assert!(tops.is_sorted(), "{tops:?}");
        rows
    };
    assert_eq!(order(&harness), ORDER);
    for title in ["AAA", "zzz", "0"] {
        write_code(&mut harness, &id, &scenario(title));
        assert!(reads(&harness, "double-booking.afs", title));
        assert_eq!(order(&harness), ORDER, "{title}");
    }
}

/// Criterion 7: a title edited in Code or in the header shows in the row,
/// saved or not; code with errors keeps the last valid one and the mark.
#[test]
fn row_follows_the_title() {
    let file = "double-booking.afs";
    let (_project, mut harness, id) = files(REFUSED, WIDE);
    write_code(&mut harness, &id, &scenario("From the code"));
    assert!(folder(&harness).snapshot(&id).unwrap().dirty);
    assert_eq!(shown(&harness, file)[0], "From the code");
    assert_eq!(name_of(&harness, file), format!("From the code, {file}"));
    type_title(&mut harness, "From the code", "From the header");
    enter(&mut harness);
    assert_eq!(shown(&harness, file)[0], "From the header");
    choose(&mut harness, "Save");
    assert!(!folder(&harness).snapshot(&id).unwrap().dirty);
    assert_eq!(shown(&harness, file)[0], "From the header");

    let text = source_of(&harness, &id);
    let changed = text.replace("From the header", "Not analysed");
    write_code(&mut harness, &id, &broken(&changed));
    assert_eq!(shown(&harness, file)[0], "From the header");
    assert_eq!(
        mark(&harness, file).map(|(_, fill, _)| fill),
        Some(theme::DANGER)
    );
    write_code(&mut harness, &id, &changed);
    assert_eq!(shown(&harness, file)[0], "Not analysed");
}

/// The widest the menu of a row gets.
const MENU: f32 = 320.0;

/// The file name and the title above the items of the open menu of `file`,
/// in the order they stand in.
fn menu_head(harness: &FolderHarness, file: &str) -> Vec<(String, Rect)> {
    let menu = harness.get_by_label(&format!("{file} menu")).rect();
    let first = harness
        .query_by_label("Open")
        .unwrap_or_else(|| harness.get_by_label("Open as text"))
        .rect()
        .top();
    let mut head: Vec<_> = harness
        .query_all_by_role(Role::Label)
        .filter(|node| menu.contains_rect(node.rect()) && node.rect().bottom() <= first)
        .map(|node| (node.value().unwrap_or_default(), node.rect()))
        .collect();
    head.sort_by(|left, right| left.1.top().total_cmp(&right.1.top()));
    head
}

fn open_menu(harness: &mut FolderHarness, file: &str) -> Vec<(String, Rect)> {
    files_row(harness, file).click_secondary();
    settle(harness);
    harness.step();
    menu_head(harness, file)
}

/// Criterion 8, the row's menu: the file name is its visible heading and the
/// title the grey line under it, so two scenarios of one title have
/// different menus; a title of any length keeps the menu's width.
#[test]
fn menu_names_the_file() {
    let said = |head: &[(String, Rect)]| -> Vec<String> {
        head.iter().map(|(text, _)| text.clone()).collect()
    };
    for (file, title) in FILES {
        let (_project, mut harness, _) = files(DAY, MINIMUM);
        let head = open_menu(&mut harness, file);
        assert_eq!(said(&head), [file, title], "the file name, then the title");
        assert!(head[0].1.bottom() <= head[1].1.top() + 1.0, "{head:?}");
        let menu = harness.get_by_label(&format!("{file} menu")).rect();
        assert!(menu.width() <= MENU, "{file}: {menu:?}");
    }
    // A row that reads its file name already says it once.
    let (_project, mut harness, _) = files(DAY, MINIMUM);
    assert!(open_menu(&mut harness, "loose.afs").is_empty());

    let (_project, mut harness, id) = files(DAY, MINIMUM);
    let title = "A title of many words ".repeat(6);
    let title = &title[..125];
    let name = format!("{}.afs", "a-file-name-of-many-words-".repeat(3));
    write_code(&mut harness, &id, &scenario(title));
    app(&mut harness)
        .relocate(DAY, &format!("scenarios/{name}"))
        .unwrap();
    settle(&mut harness);
    follow_disk(&mut harness);
    let head = open_menu(&mut harness, &name);
    let menu = harness.get_by_label(&format!("{name} menu")).rect();
    assert!(menu.width() <= MENU, "{menu:?}");
    assert_eq!(said(&head), [name.as_str(), title], "named whole");
    let mut cut = Vec::new();
    for clipped in &harness.output().shapes {
        each_shape(&clipped.shape, &mut |shape| {
            let Shape::Text(text) = shape else { return };
            let rect = text.galley.rect.translate(text.pos.to_vec2());
            if menu.contains_rect(rect) && text.galley.elided {
                assert_eq!(text.galley.rows.len(), 1, "on one line");
                cut.push(text.galley.job.text.clone());
            }
        });
    }
    assert_eq!(cut, [name.as_str(), title], "both end in an ellipsis");
}

/// Criterion 8: the dialogs keep the file name in their headings and say the
/// title under it; renaming a file leaves its title.
#[test]
fn dialogs_name_the_file() {
    let file = "double-booking.afs";
    let (project, mut harness, id) = files(REFUSED, WIDE);
    let text = source_of(&harness, &id);

    for (item, heading) in [
        ("Rename…", format!("Rename {file}")),
        ("Move to…", format!("Move {file}")),
        ("Delete…", format!("Delete {file}?")),
    ] {
        in_menu(&mut harness, file, item);
        assert!(
            title_line(&harness, &heading, REFUSED_TITLE).is_some(),
            "{heading}"
        );
        cancel(&mut harness);
    }
    // Rows without a title: no line.
    for (file, item, heading) in [
        ("loose.afs", "Rename…", "Rename loose.afs"),
        ("notes.md", "Delete…", "Delete notes.md?"),
        ("scenarios", "Delete…", "Delete scenarios?"),
    ] {
        in_menu(&mut harness, file, item);
        assert!(has_label(&harness, heading));
        for title in [REFUSED_TITLE, "Loose"] {
            assert!(title_line(&harness, heading, title).is_none(), "{heading}");
        }
        cancel(&mut harness);
    }

    f2(&mut harness, file);
    let field = harness.get_by_role_and_label(Role::TextInput, "Name");
    assert_eq!(field.accesskit_node().value().as_deref(), Some(file));
    field.focus();
    harness.step();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, "Name")
        .type_text("overlap.afs");
    settle(&mut harness);
    harness.key_press(egui::Key::Enter);
    settle(&mut harness);
    follow_disk(&mut harness);
    harness.step();
    assert_eq!(path_of(&harness, &id), "scenarios/overlap.afs");
    assert!(project.path().join("scenarios/overlap.afs").is_file());
    assert_eq!(source_of(&harness, &id), text, "the text is as it was");
    assert!(reads(&harness, "overlap.afs", REFUSED_TITLE));
}

/// Criterion 9: a dragged row moves its file, and the marks stand where they
/// did.
#[test]
fn drag_and_marks_unchanged() {
    let (project, mut harness, id) = files(REFUSED, WIDE);
    let from = files_row(&harness, "double-booking.afs").rect().center();
    let to = files_row(&harness, "components").rect().center();
    drag(&mut harness, from, to);
    settle(&mut harness);
    follow_disk(&mut harness);
    harness.step();
    assert_eq!(path_of(&harness, &id), "components/double-booking.afs");
    assert!(
        project
            .path()
            .join("components/double-booking.afs")
            .is_file()
    );
    assert!(reads(&harness, "double-booking.afs", REFUSED_TITLE));

    let (project, mut harness, unsaved) = files(REFUSED, WIDE);
    let error = id_at(&harness, AGAIN);
    let conflict = id_at(&harness, LONG);
    for id in [&unsaved, &error, &conflict] {
        let text = source_of(&harness, id);
        let text = if *id == error {
            broken(&text)
        } else {
            format!("{text}// Unsaved.\n")
        };
        write_code(&mut harness, id, &text);
    }
    fs::write(
        project.path().join(LONG),
        scenario(LONG_TITLE) + "// Outside.\n",
    )
    .unwrap();
    follow_disk(&mut harness);
    harness.step();
    let long = file_name(LONG);
    for (file, fill, outline) in [
        ("double-booking.afs", theme::MUTED, None),
        ("refused-again.afs", theme::DANGER, None),
        (long, Color32::TRANSPARENT, Some(theme::WARNING)),
    ] {
        let row = files_row(&harness, file).rect();
        let (centre, filled, stroke) = mark(&harness, file).unwrap_or_else(|| panic!("{file}"));
        assert!(
            (centre.x - (row.right() - 10.0)).abs() < 1.0
                && (centre.y - row.center().y).abs() < 1.0,
            "{file}: {centre:?} in {row:?}"
        );
        assert_eq!(filled, fill, "{file}");
        if let Some(outline) = outline {
            assert_eq!(stroke, outline, "{file}");
        }
    }
    assert_fits(&harness, "marks");
    assert!(mark(&harness, "booking-day.afs").is_none());
}

/// Criterion 11 and the captures the ticket asks for: the list at both
/// sizes, one title twice, the dialogs, the menu and a row with errors.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_ws05_files() {
    for (size, suffix) in [(WIDE, "1600"), (MINIMUM, "960")] {
        let (_project, mut harness, _) = files(REFUSED, size);
        capture(&mut harness, &format!("ws05b-files-{suffix}"), None);
        let (_project, mut harness, _) = files(SIGN_IN, size);
        capture(&mut harness, &format!("ws05b-component-{suffix}"), None);
        let (_project, mut harness, id) = files(LONG, size);
        let text = source_of(&harness, &id);
        write_code(&mut harness, &id, &broken(&text));
        capture(&mut harness, &format!("ws05b-errors-{suffix}"), None);
        let (_project, mut harness, _) = files(REFUSED, size);
        choose(&mut harness, "Code");
        capture(&mut harness, &format!("ws05b-code-{suffix}"), None);

        for path in [REFUSED, LONG] {
            let (_project, mut harness, _) = files(path, size);
            let file = file_name(path);
            for (item, name) in [
                ("Rename…", "rename"),
                ("Move to…", "move"),
                ("Delete…", "delete"),
            ] {
                in_menu(&mut harness, file, item);
                let stem = file.trim_end_matches(".afs");
                capture(&mut harness, &format!("ws05b-{name}-{stem}-{suffix}"), None);
                cancel(&mut harness);
            }
            let at = files_row(&harness, file).rect().center();
            click_at(&mut harness, at, egui::PointerButton::Secondary, 1);
            let stem = file.trim_end_matches(".afs");
            capture(&mut harness, &format!("ws05b-menu-{stem}-{suffix}"), None);
        }
    }
    let project = desk();
    for width in [240.0, 480.0] {
        let mut harness = list(project.path(), width);
        capture(&mut harness, &format!("ws05b-list-{width}"), None);
    }
}
