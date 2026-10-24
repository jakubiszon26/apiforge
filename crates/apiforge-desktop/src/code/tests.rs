use super::*;
use apiforge_script::complete::{Handle, Port};
use apiforge_script::{Diagnostic, Fix};
use eframe::egui::Event;

/// Draws the editor once with the given events and returns what it reported.
struct Harness {
    context: egui::Context,
    editor: CodeEditor,
    text: String,
}

impl Harness {
    fn new(text: &str) -> Self {
        let context = egui::Context::default();
        crate::theme::apply(&context);
        Self {
            context,
            editor: CodeEditor::default(),
            text: text.to_owned(),
        }
    }

    /// One frame. `events` are delivered to the editor, which keeps focus.
    fn frame(&mut self, events: Vec<Event>) -> (bool, Option<usize>) {
        let mut changed = false;
        let mut cursor = None;
        let handles = vec![
            Handle::new("products", "request")
                .with_ports(vec![Port::new("status", ValueType::Number)]),
            Handle::new("promotion", "value").with_ports(vec![Port::new("value", ValueType::Any)]),
        ];
        let text = &mut self.text;
        let editor = &mut self.editor;
        let mut output = self.context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 600.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                let outcome = editor.show(
                    ui,
                    egui::Id::new("test-editor"),
                    text,
                    &Context {
                        scope: Scope {
                            handles: &handles,
                            ..Scope::default()
                        },
                        ..Context::default()
                    },
                );
                outcome.response.request_focus();
                changed = outcome.changed;
                cursor = outcome.response.has_focus().then_some(0);
            },
        );
        output.textures_delta.clear();
        (changed, cursor)
    }

    /// Several frames with no events, to let focus and layout settle.
    fn settle(&mut self) {
        for _ in 0..3 {
            self.frame(Vec::new());
        }
    }

    fn type_text(&mut self, text: &str) {
        self.frame(vec![Event::Text(text.to_owned())]);
    }

    fn press(&mut self, key: Key) {
        self.frame(vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
    }
}

use apiforge_core::model::ValueType;

#[test]
fn tokens_are_coloured_by_what_they_are() {
    let source = "// note\nvar total = GET \"/x\" 12";
    let job = highlight(source, &[], FontId::monospace(13.0));
    // Neighbouring runs with the same format are laid out as one section, so a
    // section is compared by what it holds rather than by its exact bounds.
    let coloured: Vec<(String, Color32)> = job
        .sections
        .iter()
        .map(|section| {
            (
                job.text[section.byte_range.start.0..section.byte_range.end.0]
                    .trim()
                    .to_owned(),
                section.format.color,
            )
        })
        .filter(|(text, _)| !text.is_empty())
        .collect();
    let colour_of = |text: &str| {
        coloured
            .iter()
            .find(|(found, _)| found == text)
            .map(|(_, colour)| *colour)
    };
    assert_eq!(colour_of("// note"), Some(theme::MUTED));
    assert_eq!(colour_of("var"), Some(theme::LINK));
    assert_eq!(colour_of("GET"), Some(theme::ACCENT));
    assert_eq!(colour_of("\"/x\""), Some(theme::SUCCESS));
    assert_eq!(colour_of("12"), Some(theme::VARIABLE));
    assert_eq!(colour_of("total"), Some(theme::TEXT));
    // Every byte of the source is laid out, including the whitespace.
    assert_eq!(job.text, source);
}

#[test]
fn a_diagnostic_underlines_exactly_what_it_covers() {
    let source = "var total = oops";
    let diagnostics = [Diagnostic {
        range: 12..16,
        severity: Severity::Error,
        message: "unknown name".into(),
        fix: None::<Fix>,
    }];
    let job = highlight(source, &diagnostics, FontId::monospace(13.0));
    for section in &job.sections {
        let text = &job.text[section.byte_range.start.0..section.byte_range.end.0];
        let underlined = section.format.underline != Stroke::NONE;
        assert_eq!(
            underlined,
            text == "oops",
            "{text:?} underline {underlined}"
        );
        if underlined {
            assert_eq!(section.format.underline.color, theme::DANGER);
        }
    }
}

#[test]
fn a_warning_underlines_in_its_own_colour() {
    let diagnostics = [Diagnostic {
        range: 0..3,
        severity: Severity::Warning,
        message: "a warning".into(),
        fix: None,
    }];
    let job = highlight("abc", &diagnostics, FontId::monospace(13.0));
    assert_eq!(job.sections[0].format.underline.color, theme::WARNING);
}

#[test]
fn lines_are_numbered_from_one_and_marked_where_a_problem_is() {
    let text = "a\nbb\n\nccc";
    assert_eq!(
        line_starts(text).collect::<Vec<_>>(),
        [(1, 0), (2, 2), (3, 5), (4, 6)]
    );
    assert_eq!(line_of(text, 0), 1);
    assert_eq!(line_of(text, 3), 2);
    assert_eq!(line_of(text, 7), 4);
    assert_eq!(line_of(text, 9999), 4);
    let diagnostics = [Diagnostic {
        range: 6..9,
        severity: Severity::Error,
        message: "x".into(),
        fix: None,
    }];
    let marked = marked_lines(text, &diagnostics);
    assert!(marked.contains(&4) && marked.len() == 1);
}

#[test]
fn a_new_line_keeps_the_indentation_and_a_brace_adds_one_level() {
    let mut text = "  var a = 1\n".to_owned();
    let at = text.len();
    let caret = auto_indent(&mut text, at);
    assert_eq!(text, "  var a = 1\n  ");
    assert_eq!(caret, Some(text.len()));

    let mut text = "scenario \"S\" {\n".to_owned();
    let at = text.len();
    auto_indent(&mut text, at);
    assert_eq!(text, "scenario \"S\" {\n  ");

    // No indentation to keep, nothing inserted.
    let mut text = "a = 1\n".to_owned();
    let at = text.len();
    assert_eq!(auto_indent(&mut text, at), None);
    assert_eq!(text, "a = 1\n");

    // Not at a line start: nothing happens.
    let mut text = "  a = 1".to_owned();
    assert_eq!(auto_indent(&mut text, 3), None);
}

#[test]
fn accepting_a_completion_replaces_the_word_being_typed() {
    let mut text = "total = prod".to_owned();
    let completion = Completion {
        label: "products".into(),
        kind: Kind::Handle,
        detail: None,
        insert: "products".into(),
        caret: 8,
        replace: 8..12,
        score: 0,
    };
    apply(&mut text, &completion);
    assert_eq!(text, "total = products");
}

#[test]
fn a_completion_outside_the_text_is_refused_rather_than_panicking() {
    let mut text = "ą".to_owned();
    for replace in [0..99, 1..2, 0..1] {
        let before = text.clone();
        apply(
            &mut text,
            &Completion {
                label: "x".into(),
                kind: Kind::Handle,
                detail: None,
                insert: "x".into(),
                caret: 1,
                replace: replace.clone(),
                score: 0,
            },
        );
        if replace == (0..1) {
            // A boundary that is in the middle of a character is refused too.
            assert_eq!(text, before);
        } else {
            assert_eq!(text, before, "{replace:?} must change nothing");
        }
    }
}

#[test]
fn offsets_convert_both_ways_through_multi_byte_text() {
    let text = "żółw = 1";
    for chars in 0..=text.chars().count() {
        let byte = byte_offset(text, chars);
        assert!(text.is_char_boundary(byte));
        assert_eq!(char_index(text, byte), chars);
    }
    assert_eq!(byte_offset(text, 9999), text.len());
    assert_eq!(char_index(text, 9999), text.chars().count());
}

#[test]
fn what_was_typed_decides_whether_the_popup_opens_by_itself() {
    assert!(opens_completion("products.", 9));
    assert!(opens_completion("x = {{", 6));
    assert!(opens_completion("prod", 4));
    assert!(!opens_completion("x = ", 4));
    assert!(!opens_completion("x = (", 5));
    assert!(!opens_completion("", 0));
}

#[test]
fn typing_a_name_offers_it_and_enter_chooses_it() {
    // The caret sits at the end of the text, where a name is being typed.
    let mut harness = Harness::new("scenario \"S\" {\n  total = pro");
    harness.settle();
    harness.type_text("d");
    assert_eq!(harness.text, "scenario \"S\" {\n  total = prod");
    // While the popup is open, Enter chooses rather than breaking the line.
    harness.press(Key::Enter);
    assert_eq!(
        harness.text, "scenario \"S\" {\n  total = products",
        "the typed word was replaced by the chosen name"
    );
}

#[test]
fn escape_dismisses_the_offer_and_gives_enter_back() {
    let mut harness = Harness::new("scenario \"S\" {\n  total = pro");
    harness.settle();
    harness.type_text("d");
    harness.press(Key::Escape);
    harness.press(Key::Enter);
    assert_eq!(
        harness.text, "scenario \"S\" {\n  total = prod\n  ",
        "after escape, Enter breaks the line and keeps the indentation"
    );
}

#[test]
fn moving_through_the_offers_chooses_another_name() {
    let mut harness = Harness::new("scenario \"S\" {\n  x = p");
    harness.settle();
    harness.type_text("r");
    harness.press(Key::ArrowDown);
    harness.press(Key::Enter);
    // Two names start with `pr`; the second one is the one taken.
    assert_eq!(harness.text, "scenario \"S\" {\n  x = promotion");
}

#[test]
fn the_editor_draws_and_takes_focus_without_a_gpu() {
    let mut harness = Harness::new("scenario \"S\" {\n  begin = start\n}");
    harness.settle();
    let (_, focused) = harness.frame(Vec::new());
    assert!(focused.is_some(), "the editor can be typed into");
}

#[test]
fn a_buffer_too_large_to_colour_is_laid_out_plainly() {
    // Colouring every token of a very large buffer costs more per frame than it
    // is worth; the text is still shown, in one piece.
    let big = "var a = 1\n".repeat(30_000);
    assert!(big.len() > MAX_COLOURED_BYTES);
    let job = layout_job(&big, &[], FontId::monospace(13.0));
    assert_eq!(job.sections.len(), 1);
    assert_eq!(job.text.len(), big.len());
    // Below the limit it is coloured as usual.
    let small = layout_job("var a = 1", &[], FontId::monospace(13.0));
    assert!(small.sections.len() > 1);
}
