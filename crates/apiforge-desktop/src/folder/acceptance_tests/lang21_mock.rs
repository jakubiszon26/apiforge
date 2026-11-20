//! MOCK (LANG-21, #482): design-review views of the window offering
//! *Update the project* for a project written in language 1. The project is
//! a copy of `fixtures/scenarios/lay/rooms`, which is written in language 1.
//! Nothing is converted by the window here: the "after" views open a copy
//! whose `@after` lines the test removed, and the notices, the problems and
//! the dialogs are a scene of `folder/lang21.rs`.
use super::*;
use crate::folder::lang21 as mock;
use crate::testing::{MINIMUM, folder_harness_in};
use std::path::Path;

const DAY: &str = "scenarios/booking-day.afs";
const NOTICE: &str = "scenarios/double-booking-notice.afs";
const DOUBLE: &str = "scenarios/double-booking.afs";
const SIGN_IN: &str = "components/sign-in.afs";
const SOURCES: [&str; 4] = [DAY, NOTICE, DOUBLE, SIGN_IN];

const UPDATE: &str = "Update the project";
const OLDER: &str = "This project is written in language 1; this build reads language 2. It does not run \
                     until it is updated.";
/// LANG-15's sentence, naming the button where the command line names the
/// command.
const REFUSED_RUN: &str = "Cannot run this scenario: This project is written in language 1; \
                           this build reads language 2. Use Update the project, above.";
/// LANG-13's messages for a statement the person places by hand.
const NOT_IN_PLACE: &str = "This statement is not written where it runs today; move it there, then delete the `@after` lines";
const RUNS_NOWHERE: &str = "This step runs nowhere today: nothing orders it after `start`; write it where it should run or delete it, then delete the `@after` lines";
const HOLD: &str = "hold = POST \"/payments/holds\"";
const AUDIT: &str = "audit = GET \"/audit/bookings\"";

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// A copy of the room project, written in language 1.
fn rooms() -> tempfile::TempDir {
    let project = tempfile::tempdir().unwrap();
    copy_tree(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scenarios/lay/rooms"),
        project.path(),
    );
    project
}

fn read(root: &Path, path: &str) -> String {
    fs::read_to_string(root.join(path)).unwrap()
}

fn afters(root: &Path, path: &str) -> usize {
    read(root, path).matches("@after(").count()
}

/// What the update would leave of the project: every `@after` line gone.
fn without_afters(root: &Path) {
    for path in SOURCES {
        let text: String = read(root, path)
            .split_inclusive('\n')
            .filter(|line| !line.trim_start().starts_with("@after("))
            .collect();
        fs::write(root.join(path), text).unwrap();
    }
}

/// Two statements the update cannot place: `hold` written above the step it
/// follows, and a request nothing orders.
fn out_of_place(root: &Path) {
    let day = read(root, DAY);
    let from = day.find("  @after(first)\n  hold").unwrap();
    let end = "  held = assert hold.body.amount == 40\n";
    let to = day.find(end).unwrap() + end.len();
    let block = day[from..to].to_owned();
    let rest = format!("{}{}", &day[..from], &day[to..]);
    let at = rest.find("  @after(working_day)\n  first").unwrap();
    fs::write(
        root.join(DAY),
        format!("{}{block}{}", &rest[..at], &rest[at..]),
    )
    .unwrap();
    let notice = read(root, NOTICE).replacen(
        "  cleanup {",
        "  audit = GET \"/audit/bookings\" {\n    headers { Authorization: \"Bearer {{token}}\" }\n    sample 200 { items: [] }\n  }\n  cleanup {",
        1,
    );
    fs::write(root.join(NOTICE), notice).unwrap();
}

fn open(root: &Path, path: &str, size: egui::Vec2) -> FolderHarness {
    let mut harness = folder_harness_in(root, size, Mode::Code);
    app(&mut harness).review_open(path, None);
    settle(&mut harness);
    harness
}

fn snap(harness: &mut FolderHarness, name: &str) {
    harness.step();
    harness.step();
    settle(harness);
    crate::testing::capture(harness, name, None);
}

fn texts(text: &[&str]) -> Vec<String> {
    text.iter().map(|line| (*line).to_owned()).collect()
}

fn offer() -> mock::Offer {
    mock::Offer {
        text: OLDER.into(),
        buttons: texts(&[UPDATE]),
    }
}

fn row(text: String) -> mock::Row {
    mock::Row {
        text,
        ..Default::default()
    }
}

/// What the update changes, per file, counted from the project.
fn changes(root: &Path) -> Vec<mock::Group> {
    let changed = SOURCES
        .iter()
        .filter(|path| afters(root, path) > 0)
        .map(|path| {
            row(format!(
                "{path} — {} @after lines removed",
                afters(root, path)
            ))
        })
        .collect();
    let unchanged = SOURCES
        .iter()
        .filter(|path| afters(root, path) == 0)
        .map(|path| row((*path).to_owned()))
        .collect();
    vec![
        mock::Group {
            title: "Changed by the update".into(),
            rows: changed,
        },
        mock::Group {
            title: "Already in order, not changed".into(),
            rows: unchanged,
        },
    ]
}

fn line_of(root: &Path, path: &str, needle: &str) -> usize {
    let text = read(root, path);
    apiforge_script::line_column(&text, text.find(needle).unwrap()).0
}

fn by_hand() -> Vec<mock::ByHand> {
    vec![
        mock::ByHand {
            path: DAY.into(),
            needle: HOLD.into(),
            message: NOT_IN_PLACE.into(),
        },
        mock::ByHand {
            path: NOTICE.into(),
            needle: AUDIT.into(),
            message: RUNS_NOWHERE.into(),
        },
    ]
}

const WHOLE: &str = "The files and the project file are written together, or none is. \
                     Published revisions are not touched. The window cannot undo it: version \
                     control keeps the files as they were.";
const MEANING: &str = "Language 2 has no @after: the steps of a block run in the order they \
                       are written. Every file here is written in the order it runs today, so \
                       what runs does not change.";

#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lang21() {
    for (size, suffix) in [(MINIMUM, "960"), (WIDE, "1600")] {
        // 1. The project just opened.
        let project = rooms();
        let root = project.path();
        let total: usize = SOURCES.iter().map(|path| afters(root, path)).sum();
        let files = SOURCES
            .iter()
            .filter(|path| afters(root, path) > 0)
            .count();
        mock::set(mock::Scene {
            offer: Some(offer()),
            after_errors: true,
            ..Default::default()
        });
        let mut harness = open(root, DAY, size);
        snap(&mut harness, &format!("lang21-1-offer-{suffix}"));

        // 2. Run pressed before the update.
        app(&mut harness).notify(REFUSED_RUN, true);
        snap(&mut harness, &format!("lang21-2-run-refused-{suffix}"));
        drop(harness);

        // 1b. The alternative: a dialog when the project opens.
        mock::set(mock::Scene {
            after_errors: true,
            dialog: Some(mock::Dialog {
                heading: "This project is written in an older language".into(),
                before: texts(&[
                    "Room booking is written in language 1; this build reads language 2. It \
                     opens for reading and editing, and does not run until it is updated.",
                    "The update removes the @after lines of every file that is written in \
                     the order it runs, in all files or in none.",
                ]),
                primary: Some(UPDATE.into()),
                secondary: texts(&["Not now"]),
                width: 460.0,
                ..Default::default()
            }),
            ..Default::default()
        });
        let mut harness = open(root, DAY, size);
        snap(&mut harness, &format!("lang21-1b-dialog-on-open-{suffix}"));
        drop(harness);

        // 3a. The button pressed: what will change, before anything is written.
        mock::set(mock::Scene {
            offer: Some(offer()),
            after_errors: true,
            dialog: Some(mock::Dialog {
                heading: "Update the project to language 2".into(),
                before: texts(&[MEANING]),
                groups: changes(root),
                after: texts(&[WHOLE]),
                primary: Some(format!("Update {files} files")),
                secondary: texts(&["Cancel"]),
                width: 520.0,
            }),
            ..Default::default()
        });
        let mut harness = open(root, DAY, size);
        snap(&mut harness, &format!("lang21-3a-confirm-{suffix}"));
        drop(harness);

        // 5. The same with unsaved work in the window.
        let mut groups = changes(root);
        groups.insert(
            0,
            mock::Group {
                title: "Unsaved here, saved first".into(),
                rows: vec![row(format!("{DOUBLE} — unsaved code"))],
            },
        );
        mock::set(mock::Scene {
            offer: Some(offer()),
            after_errors: true,
            dialog: Some(mock::Dialog {
                heading: "Update the project to language 2".into(),
                before: texts(&[MEANING]),
                groups,
                after: texts(&[WHOLE]),
                primary: Some("Save all and update".into()),
                secondary: texts(&["Cancel"]),
                width: 520.0,
            }),
            ..Default::default()
        });
        let mut harness = folder_harness_in(root, size, Mode::Code);
        app(&mut harness).review_open(DOUBLE, Some(("floor: 2", "floor: 3")));
        settle(&mut harness);
        snap(&mut harness, &format!("lang21-5-unsaved-{suffix}"));
        drop(harness);

        // 3b. After the update: the files without `@after`.
        without_afters(root);
        mock::set(mock::Scene {
            converted: true,
            ..Default::default()
        });
        let mut harness = open(root, DAY, size);
        app(&mut harness).notify(
            format!(
                "Updated the project to language 2: {files} files changed, {total} @after lines \
                 removed."
            ),
            false,
        );
        snap(&mut harness, &format!("lang21-3b-updated-{suffix}"));
        drop(harness);

        // 4a. An update that cannot be done: nothing was written.
        let project = rooms();
        let root = project.path();
        out_of_place(root);
        let hold = line_of(root, DAY, HOLD);
        let audit = line_of(root, NOTICE, AUDIT);
        let refused = mock::Dialog {
            heading: "The project cannot be updated yet".into(),
            before: texts(&[
                "Nothing was written. 2 statements in 2 files are placed by hand first; then \
                 update again.",
            ]),
            groups: vec![
                mock::Group {
                    title: DAY.into(),
                    rows: vec![mock::Row {
                        text: format!("Line {hold} — hold"),
                        detail: Some(NOT_IN_PLACE.replace('`', "")),
                        link: Some("Show".into()),
                    }],
                },
                mock::Group {
                    title: NOTICE.into(),
                    rows: vec![mock::Row {
                        text: format!("Line {audit} — audit"),
                        detail: Some(RUNS_NOWHERE.replace('`', "")),
                        link: Some("Show".into()),
                    }],
                },
            ],
            after: texts(&[
                "Ready: scenarios/double-booking.afs, components/sign-in.afs. The same lines \
                 stay in each file's Problems.",
            ]),
            primary: None,
            secondary: texts(&["Close"]),
            width: 560.0,
        };
        mock::set(mock::Scene {
            offer: Some(offer()),
            after_errors: true,
            by_hand: by_hand(),
            dialog: Some(refused),
            ..Default::default()
        });
        let mut harness = open(root, DAY, size);
        snap(&mut harness, &format!("lang21-4a-by-hand-list-{suffix}"));
        drop(harness);

        // 4b. *Show* on the first row: the file, the statement, its problem.
        mock::set(mock::Scene {
            offer: Some(mock::Offer {
                text: "Not updated, nothing was written: 2 statements in 2 files are placed \
                       by hand first."
                    .into(),
                buttons: texts(&["Show the list", UPDATE]),
            }),
            after_errors: true,
            by_hand: by_hand(),
            ..Default::default()
        });
        let mut harness = open(root, DAY, size);
        let text = read(root, DAY);
        let (line, column) = apiforge_script::line_column(&text, text.find(HOLD).unwrap());
        let label = format!("Line {line}, column {column}   {NOT_IN_PLACE}");
        harness.get_by_label(&label).click();
        settle(&mut harness);
        snap(&mut harness, &format!("lang21-4b-by-hand-shown-{suffix}"));
    }
}
