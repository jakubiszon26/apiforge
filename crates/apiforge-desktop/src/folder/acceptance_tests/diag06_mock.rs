//! MOCK (DIAG-06, #391): design-review views of the board that keeps the
//! chain. Synthetic scenarios only: a plain chain in the shape of the
//! reference scenario `labels` (requests to one endpoint, none reading
//! another) and a room booking with an `if`. The steps are written as
//! language 2 will lower them: each follows the step before it.
use super::lay23::{board, id_of, select, show};
use super::*;
use crate::canvas::diag06 as mock;
use crate::testing::{MINIMUM, folder_harness_in};
use egui::{Rect, Vec2, vec2};

const TIDY: &str = "Tidy up";
const FIT: &str = "Fit the view";

/// One request of the plain chain.
fn label_step(handle: &str, label: &str, body: &str, status: u16, error: &str) -> String {
    format!(
        "  @label(\"{label}\")\n  {handle} = POST \"/parcels/labels\" {{\n    body {{ {body} }}\n    sample {status} {{ error: \"{error}\" }}\n  }}\n  {handle}_refused = assert {handle}.status == {status}\n"
    )
}

/// The plain chain: `steps` as (handle, label, body, status, error).
fn labels(steps: &[(&str, &str, &str, u16, &str)]) -> String {
    let mut text = String::from(
        "scenario \"Label validation\" {\n  description \"Synthetic: one endpoint, no request reads another.\"\n  begin = start\n",
    );
    let mut previous = "begin";
    for (handle, label, body, status, error) in steps {
        text.push_str(&format!("  @after({previous})\n"));
        text.push_str(&label_step(handle, label, body, *status, error));
        previous = handle;
    }
    text.push_str("}\n");
    text
}

const NO_TOKEN: (&str, &str, &str, u16, &str) = (
    "no_token",
    "No token",
    "weightKg: 2, postcode: \"00-950\", service: \"standard\"",
    401,
    "unauthorized",
);
const HEAVY: (&str, &str, &str, u16, &str) = (
    "heavy",
    "Weight 31 kg",
    "weightKg: 31, postcode: \"00-950\", service: \"standard\"",
    422,
    "weight_too_high",
);
const WEIGHTLESS: (&str, &str, &str, u16, &str) = (
    "weightless",
    "Weight 0 kg",
    "weightKg: 0, postcode: \"00-950\", service: \"standard\"",
    422,
    "weight_too_low",
);
const UNKNOWN: (&str, &str, &str, u16, &str) = (
    "unknown_parcel",
    "Unknown parcel",
    "parcel: \"p-0\", postcode: \"00-950\", service: \"standard\"",
    404,
    "parcel_not_found",
);
const SYMBOLS: (&str, &str, &str, u16, &str) = (
    "symbols",
    "Postcode of symbols",
    "weightKg: 2, postcode: \"**-***\", service: \"standard\"",
    422,
    "postcode_invalid",
);
const ONE_DIGIT: (&str, &str, &str, u16, &str) = (
    "one_digit",
    "Postcode: one digit",
    "weightKg: 2, postcode: \"0\", service: \"standard\"",
    422,
    "postcode_invalid",
);
const EMPTY_POSTCODE: (&str, &str, &str, u16, &str) = (
    "empty_postcode",
    "Empty postcode",
    "weightKg: 2, postcode: \"\", service: \"standard\"",
    422,
    "postcode_missing",
);
const NO_SERVICE: (&str, &str, &str, u16, &str) = (
    "no_service",
    "Unknown service",
    "weightKg: 2, postcode: \"00-950\", service: \"teleport\"",
    422,
    "service_unknown",
);
const EMPTY_SERVICE: (&str, &str, &str, u16, &str) = (
    "empty_service",
    "Empty service",
    "weightKg: 2, postcode: \"00-950\", service: \"\"",
    422,
    "service_missing",
);

fn plain_chain() -> String {
    labels(&[
        NO_TOKEN,
        HEAVY,
        WEIGHTLESS,
        UNKNOWN,
        SYMBOLS,
        ONE_DIGIT,
        EMPTY_POSTCODE,
        NO_SERVICE,
        EMPTY_SERVICE,
    ])
}

/// A room booking with an `if`: two steps in *Yes*, one in *No*, one after
/// the whole `if`.
const ROOMS: &str = r#"scenario "Book a room" {
  description "Synthetic: a booking, then a branch on its answer."
  begin = start
  @after(begin)
  rooms = GET "/rooms" {
    query { floor: 2 }
    sample 200 { items: [{ id: "room-a", seats: 8 }] }
  }
  has_rooms = assert nonempty rooms.body.items
  @after(rooms)
  first = POST "/bookings" {
    body { roomId: rooms.body.items[0].id, start: "2026-12-01T10:00:00Z" }
    sample 201 { id: "bk-1", roomId: "room-a" }
  }
  @after(first)
  booked = if first.status == 201 {
    invite = POST "/invitations" {
      body { booking: "bk-1", team: "qa" }
      sample 201 { id: "i-1", invited: 5 }
    }
    @after(invite)
    notice = POST "/notifications" {
      body { channel: "email", text: "Your booking is confirmed." }
      sample 202 { id: "n-1" }
    }
  } else {
    incident = POST "/incidents" {
      body { text: "The booking was refused." }
      sample 201 { id: "inc-1" }
    }
  }
  @after(booked)
  calendar = GET "/rooms/{{rooms.body.items[0].id}}/bookings" {
    sample 200 { items: [{ id: "bk-1" }] }
  }
  one_booking = assert calendar.status == 200
}
"#;

fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
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

/// A copy of the room project with `text` as the scenario `name`, open on
/// its board at `size`, tidied with nothing selected.
fn open(name: &str, text: &str, size: egui::Vec2) -> (tempfile::TempDir, FolderHarness, String) {
    let project = tempfile::tempdir().unwrap();
    copy_tree(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scenarios/lay/rooms"),
        project.path(),
    );
    // The scenario takes the place of `double-booking` in the manifest.
    fs::remove_file(project.path().join("scenarios/double-booking.afs")).unwrap();
    let manifest = project.path().join("apiforge.project.json");
    let registered = fs::read_to_string(&manifest)
        .unwrap()
        .replace(
            "scenarios/double-booking.afs",
            &format!("scenarios/{name}.afs"),
        )
        .replace(
            "\"entityId\": \"double-booking\"",
            &format!("\"entityId\": \"{name}\""),
        );
    fs::write(&manifest, registered).unwrap();
    let path = format!("scenarios/{name}.afs");
    fs::write(project.path().join(&path), text).unwrap();
    let mut harness = folder_harness_in(project.path(), size, Mode::Diagram);
    app(&mut harness).review_open(&path, None);
    settle(&mut harness);
    let id = folder(&harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == path)
        .unwrap_or_else(|| panic!("the project has {path}"))
        .id()
        .to_owned();
    app(&mut harness).selections.insert(id.clone(), Vec::new());
    settle(&mut harness);
    choose(&mut harness, TIDY);
    harness.step();
    choose(&mut harness, FIT);
    harness.step();
    settle(&mut harness);
    (project, harness, id)
}

fn sizes() -> [(egui::Vec2, &'static str); 2] {
    [(MINIMUM, "960"), (WIDE, "1600")]
}

fn canvas_mut<'a>(harness: &'a mut FolderHarness, id: &str) -> &'a mut crate::canvas::Canvas {
    app(harness)
        .canvases
        .get_mut(id)
        .expect("the board is drawn")
}

fn world(harness: &FolderHarness, id: &str, handle: &str) -> Rect {
    board(harness, id)
        .debug_world(&id_of(harness, id, handle))
        .unwrap_or_else(|| panic!("{handle} is drawn"))
}

fn fit(harness: &mut FolderHarness) {
    choose(harness, FIT);
    harness.step();
    settle(harness);
    harness.step();
}

fn snap(harness: &mut FolderHarness, name: &str) {
    harness.step();
    harness.step();
    crate::testing::capture(harness, name, None);
}

fn link(from: &str, to: &str, look: mock::Look) -> mock::Link {
    mock::Link {
        from: from.to_owned(),
        to: to.to_owned(),
        look,
    }
}

/// The lines of a chain through `names`, at rest.
fn chain(names: &[&str]) -> Vec<mock::Link> {
    names
        .windows(2)
        .map(|pair| link(pair[0], pair[1], mock::Look::Plain))
        .collect()
}

fn lines(text: &[&str]) -> Vec<String> {
    text.iter().map(|line| (*line).to_owned()).collect()
}

const CHAIN: [&str; 10] = [
    "begin",
    "No token",
    "Weight 31 kg",
    "Weight 0 kg",
    "Unknown parcel",
    "Postcode of symbols",
    "Postcode: one digit",
    "Empty postcode",
    "Unknown service",
    "Empty service",
];

/// The chain of `names` with the lines `fading` about to go.
fn chain_fading(names: &[&str], fading: &[(&str, &str)]) -> Vec<mock::Link> {
    names
        .windows(2)
        .map(|pair| {
            let look = if fading.contains(&(pair[0], pair[1])) {
                mock::Look::Fading
            } else {
                mock::Look::Plain
            };
            link(pair[0], pair[1], look)
        })
        .collect()
}

/// The views, one after another: they share the scene.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_diag06() {
    chain_views();
    branch_views();
}

/// The plain chain and the gestures on it.
fn chain_views() {
    for (size, suffix) in sizes() {
        let close = if suffix == "960" { 0.6 } else { 0.85 };
        // 1. The chain at rest.
        mock::set(mock::Scene {
            on: true,
            links: chain(&CHAIN),
            ..Default::default()
        });
        let (_project, mut harness, id) = open("label-validation", &plain_chain(), size);
        fit(&mut harness);
        snap(&mut harness, &format!("diag06-1-chain-fit-{suffix}"));
        show(&mut harness, &id, "weightless", close);
        snap(&mut harness, &format!("diag06-1-chain-close-{suffix}"));

        // 2. A second wire from a Next socket that has one: a new step.
        mock::set(mock::Scene {
            on: true,
            links: chain_fading(&CHAIN, &[("Weight 31 kg", "Weight 0 kg")]),
            carry: Some(mock::Carry {
                from: "Weight 31 kg".into(),
                to: mock::Target::Board("Weight 31 kg".into(), vec2(60.0, 230.0)),
                caption: lines(&[
                    "New step after Weight 31 kg",
                    "Weight 0 kg will follow it. Release to choose the step.",
                ]),
                caption_at: vec2(18.0, 24.0),
                refused: false,
            }),
            ..Default::default()
        });
        show(&mut harness, &id, "weightless", close);
        snap(&mut harness, &format!("diag06-2-insert-new-{suffix}"));

        // 3. The same wire dropped on a step of the chain: the step moves.
        mock::set(mock::Scene {
            on: true,
            links: chain_fading(
                &CHAIN,
                &[
                    ("No token", "Weight 31 kg"),
                    ("Postcode of symbols", "Postcode: one digit"),
                    ("Postcode: one digit", "Empty postcode"),
                ],
            ),
            carry: Some(mock::Carry {
                from: "No token".into(),
                to: mock::Target::Card("Postcode: one digit".into()),
                caption: lines(&[
                    "Move Postcode: one digit here",
                    "After No token, before Weight 31 kg. Empty postcode will follow Postcode of symbols.",
                ]),
                caption_at: vec2(24.0, -64.0),
                refused: false,
            }),
            ..Default::default()
        });
        fit(&mut harness);
        if suffix == "960" {
            show(&mut harness, &id, "symbols", 0.42);
            canvas_mut(&mut harness, &id).debug_pan(vec2(60.0, 60.0));
            settle(&mut harness);
        }
        snap(&mut harness, &format!("diag06-3-move-{suffix}"));

        // 5. A block from the Library over a line of the chain.
        mock::set(mock::Scene {
            on: true,
            links: chain(&CHAIN),
            drop: Some(mock::Drop {
                block: "HTTP request".into(),
                after: "Weight 31 kg".into(),
                before: "Weight 0 kg".into(),
                caption: lines(&["New step after Weight 31 kg", "Weight 0 kg will follow it."]),
            }),
            ..Default::default()
        });
        show(&mut harness, &id, "weightless", close);
        snap(&mut harness, &format!("diag06-5-drop-{suffix}"));

        // 5b. The pointer on a line: the place a step can be added.
        mock::set(mock::Scene {
            on: true,
            links: chain(&CHAIN),
            plus: Some(("Weight 31 kg".into(), "Weight 0 kg".into())),
            ..Default::default()
        });
        show(&mut harness, &id, "weightless", close);
        snap(&mut harness, &format!("diag06-5b-plus-{suffix}"));

        // 2b. After the insert: the row made room.
        let longer = [
            "begin",
            "No token",
            "Weight 31 kg",
            "Weight 30 kg",
            "Weight 0 kg",
            "Unknown parcel",
            "Postcode of symbols",
            "Postcode: one digit",
            "Empty postcode",
            "Unknown service",
            "Empty service",
        ];
        mock::set(mock::Scene {
            on: true,
            links: chain(&longer),
            ..Default::default()
        });
        let text = labels(&[
            NO_TOKEN,
            HEAVY,
            (
                "limit",
                "Weight 30 kg",
                "weightKg: 30, postcode: \"00-950\", service: \"standard\"",
                201,
                "",
            ),
            WEIGHTLESS,
            UNKNOWN,
            SYMBOLS,
            ONE_DIGIT,
            EMPTY_POSTCODE,
            NO_SERVICE,
            EMPTY_SERVICE,
        ]);
        let (_project, mut harness, id) = open("label-validation", &text, size);
        if suffix == "960" {
            show(&mut harness, &id, "limit", close);
        } else {
            select(&mut harness, &id, &["limit"]);
            fit(&mut harness);
        }
        snap(&mut harness, &format!("diag06-2b-insert-after-{suffix}"));

        // 4b. After the delete: the neighbours are joined.
        let shorter: Vec<&str> = CHAIN
            .iter()
            .copied()
            .filter(|name| *name != "Weight 0 kg")
            .collect();
        mock::set(mock::Scene {
            on: true,
            links: chain(&shorter),
            notice: Some("Weight 0 kg deleted. Unknown parcel now follows Weight 31 kg.".into()),
            ..Default::default()
        });
        let text = labels(&[
            NO_TOKEN,
            HEAVY,
            UNKNOWN,
            SYMBOLS,
            ONE_DIGIT,
            EMPTY_POSTCODE,
            NO_SERVICE,
            EMPTY_SERVICE,
        ]);
        let (_project, mut harness, id) = open("label-validation", &text, size);
        show(&mut harness, &id, "heavy", close);
        canvas_mut(&mut harness, &id).debug_pan(vec2(-160.0 * close, 0.0));
        settle(&mut harness);
        snap(&mut harness, &format!("diag06-4b-remove-after-{suffix}"));
    }
    mock::clear();
}

/// Where the scene puts the cards of the room booking: the chain on one
/// row, the branches of `booked` under it.
fn branch_layout(harness: &FolderHarness, id: &str) -> Vec<(String, Vec2)> {
    let begin = world(harness, id, "begin").min;
    let booked = world(harness, id, "booked");
    let invite = world(harness, id, "first").min.x;
    let notice = invite + 380.0;
    let lower = begin.y + 300.0;
    [
        ("calendar", booked.right() + 170.0, begin.y),
        ("invite", invite, lower),
        ("notice", notice, lower),
        ("incident", notice + 522.0, lower),
    ]
    .into_iter()
    .map(|(handle, x, y)| {
        (
            handle.to_owned(),
            egui::pos2(x, y) - world(harness, id, handle).min,
        )
    })
    .collect()
}

fn branches() -> Vec<mock::Band> {
    vec![
        mock::Band {
            condition: "booked".into(),
            yes: true,
            steps: vec!["invite".into(), "notice".into()],
            then: Some("calendar".into()),
            room: 0.3,
        },
        mock::Band {
            condition: "booked".into(),
            yes: false,
            steps: vec!["incident".into()],
            then: Some("calendar".into()),
            room: 0.3,
        },
    ]
}

fn room_links() -> Vec<mock::Link> {
    let mut links = chain(&["begin", "rooms", "first", "booked", "calendar"]);
    links.push(link("invite", "notice", mock::Look::Plain));
    links
}

/// A step inside a branch, and a gesture the board refuses.
fn branch_views() {
    for (size, suffix) in sizes() {
        let close = if suffix == "960" { 0.55 } else { 0.85 };
        mock::clear();
        let (_project, mut harness, id) = open("book-a-room", ROOMS, size);
        let moved = branch_layout(&harness, &id);
        let scene = mock::Scene {
            on: true,
            links: room_links(),
            bands: branches(),
            moved,
            ..Default::default()
        };
        // 6. The `if` at rest.
        mock::set(scene.clone());
        harness.step();
        fit(&mut harness);
        snap(&mut harness, &format!("diag06-6-branch-fit-{suffix}"));
        show(&mut harness, &id, "booked", close);
        let up = if suffix == "960" { -130.0 } else { -94.0 };
        canvas_mut(&mut harness, &id).debug_pan(vec2(0.0, up));
        settle(&mut harness);
        snap(&mut harness, &format!("diag06-6-branch-close-{suffix}"));

        // 7. A second wire from a Next socket inside the braces.
        let mut inside = scene.clone();
        for line in &mut inside.links {
            if line.from == "invite" {
                line.look = mock::Look::Fading;
            }
        }
        inside.carry = Some(mock::Carry {
            from: "invite".into(),
            to: mock::Target::Board("invite".into(), vec2(52.0, 96.0)),
            caption: lines(&[
                "New step after invite, in booked → Yes",
                "notice will follow it. Release to choose the step.",
            ]),
            caption_at: vec2(18.0, 24.0),
            refused: false,
        });
        mock::set(inside);
        snap(&mut harness, &format!("diag06-7-branch-insert-{suffix}"));

        // 8. A gesture the board refuses.
        if suffix == "960" {
            canvas_mut(&mut harness, &id).debug_pan(vec2(0.0, 76.0));
            settle(&mut harness);
        }
        let mut refused = scene.clone();
        refused.carry = Some(mock::Carry {
            from: "notice".into(),
            to: mock::Target::Card("booked".into()),
            caption: lines(&[
                "booked cannot follow notice",
                "notice is inside booked → Yes. Nothing changes.",
            ]),
            caption_at: vec2(-30.0, -78.0),
            refused: true,
        });
        mock::set(refused);
        snap(&mut harness, &format!("diag06-8-refused-{suffix}"));

        // 8b. The same, released: what stays on the board.
        mock::set(scene.clone());
        canvas_mut(&mut harness, &id)
            .set_error("booked cannot follow notice: notice is inside booked → Yes.");
        snap(&mut harness, &format!("diag06-8b-refused-after-{suffix}"));
    }
    mock::clear();
}
