use super::*;
use apiforge_script::complete::{Handle, Port};
use apiforge_script::{Diagnostic, Fix};
use eframe::egui::Event;

/// Draws the editor once with the given events and returns what it reported.
struct Harness {
    context: egui::Context,
    editor: CodeEditor,
    text: String,
    top_space: f32,
    request_focus: bool,
    diagnostics: Vec<Diagnostic>,
    shapes: Vec<egui::epaint::ClippedShape>,
}

impl Harness {
    fn new(text: &str) -> Self {
        let context = egui::Context::default();
        crate::theme::apply(&context);
        Self {
            context,
            editor: CodeEditor::default(),
            text: text.to_owned(),
            top_space: 0.0,
            request_focus: true,
            diagnostics: Vec::new(),
            shapes: Vec::new(),
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
        let top_space = self.top_space;
        let request_focus = self.request_focus;
        let diagnostics = self.diagnostics.clone();
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
                ui.add_space(top_space);
                let outcome = editor.show(
                    ui,
                    egui::Id::new("test-editor"),
                    text,
                    &Context {
                        scope: Scope {
                            handles: &handles,
                            ..Scope::default()
                        },
                        diagnostics: &diagnostics,
                        ..Context::default()
                    },
                );
                if request_focus && !outcome.response.has_focus() {
                    outcome.response.request_focus();
                }
                changed = outcome.changed;
                cursor = outcome.response.has_focus().then_some(0);
            },
        );
        output.textures_delta.clear();
        self.shapes = output.shapes;
        (changed, cursor)
    }

    /// The line numbers the gutter painted this frame, in the order drawn.
    fn gutter_numbers(&self) -> Vec<usize> {
        self.shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) => text.galley.text().parse::<usize>().ok(),
                _ => None,
            })
            .collect()
    }

    /// How many diagnostic markers the gutter painted this frame.
    fn gutter_markers(&self) -> usize {
        self.shapes
            .iter()
            .filter(|clipped| matches!(&clipped.shape, egui::Shape::Circle(_)))
            .count()
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

    fn shift(&mut self, key: Key) {
        self.frame(vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::SHIFT,
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

    // A request body opens its blocks with brackets, and is printed with the
    // same two spaces, so they earn a level too.
    let mut text = "{\n  \"items\": [\n".to_owned();
    let at = text.len();
    auto_indent(&mut text, at);
    assert_eq!(text, "{\n  \"items\": [\n    ");

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
    // is worth; the text is still shown, in one piece. A megabyte body is the
    // size the plan asks the editor to survive.
    let big = "var a = 1\n".repeat(110_000);
    assert!(big.len() >= 1024 * 1024);
    assert!(big.len() > MAX_COLOURED_BYTES);
    let job = layout_job(&big, &[], FontId::monospace(13.0));
    assert_eq!(job.sections.len(), 1);
    assert_eq!(job.text.len(), big.len());
    // Below the limit it is coloured as usual.
    let small = layout_job("var a = 1", &[], FontId::monospace(13.0));
    assert!(small.sections.len() > 1);
}

#[test]
fn blur_dismisses_completion_and_enter_in_another_field_keeps_code_unchanged() {
    let mut harness = Harness::new("scenario \"S\" { total = pro");
    harness.settle();
    harness.type_text("d");
    assert!(harness.editor.completions.popup.is_some());
    harness.request_focus = false;
    harness
        .context
        .memory_mut(|m| m.request_focus(egui::Id::new("inspector-field")));
    let before = harness.text.clone();
    harness.press(Key::Enter);
    assert_eq!(harness.text, before);
    assert!(harness.editor.completions.popup.is_none());
}

#[test]
fn popup_follows_the_caret_after_diagnostics_move_the_editor() {
    let mut harness = Harness::new("scenario \"S\" { total = pro");
    harness.settle();
    harness.type_text("d");
    let before = harness.editor.completions.popup.as_ref().unwrap().at;
    harness.top_space = 100.0;
    harness.frame(vec![]);
    let after = harness.editor.completions.popup.as_ref().unwrap().at;
    assert!(
        (after.y - before.y - 100.0).abs() < 1.0,
        "{before:?} -> {after:?}"
    );
}

#[test]
fn manual_completion_is_consumed_only_by_the_focused_editor() {
    let ctx = egui::Context::default();
    let mut first = CodeEditor::default();
    let mut second = CodeEditor::default();
    let mut a = String::new();
    let mut b = "scenario \"S\" { total = prod".to_owned();
    let handles = [Handle::new("products", "request")];
    ctx.memory_mut(|m| m.request_focus(egui::Id::new("second")));
    let mut output = ctx.run_ui(
        egui::RawInput {
            events: vec![Event::Key {
                key: Key::Space,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::CTRL,
            }],
            ..Default::default()
        },
        |ui| {
            let context = Context {
                scope: Scope {
                    handles: &handles,
                    ..Scope::default()
                },
                ..Context::default()
            };
            first.show(ui, egui::Id::new("first"), &mut a, &context);
            second.show(ui, egui::Id::new("second"), &mut b, &context);
        },
    );
    output.textures_delta.clear();
    assert!(first.completions.popup.is_none());
    assert!(second.completions.popup.is_some());
}

#[test]
fn reveal_scrolls_an_offscreen_statement_into_view() {
    let ctx = egui::Context::default();
    let mut editor = CodeEditor::default();
    let mut text = (0..100)
        .map(|i| format!("value_{i} = {i}\n"))
        .collect::<String>();
    let mut offset = 0.0;
    for index in 0..4 {
        if index == 1 {
            editor.reveal(text.find("value_90").unwrap());
        }
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(500.0, 300.0))),
                ..Default::default()
            },
            |ui| {
                let scroll = egui::ScrollArea::both()
                    .id_salt("reveal-scroll")
                    .animated(false)
                    .show(ui, |ui| {
                        editor.show(
                            ui,
                            egui::Id::new("long-editor"),
                            &mut text,
                            &Context::default(),
                        );
                    });
                offset = scroll.state.offset.y;
            },
        );
        output.textures_delta.clear();
    }
    assert!(offset > 1000.0, "offset: {offset}");
}

#[test]
fn moving_the_caret_forward_dismisses_a_completion_for_the_old_location() {
    let mut harness = Harness::new("scenario \"S\" { total = pro\n}");
    harness.settle();
    harness.editor.reveal(harness.text.find("pro").unwrap() + 3);
    harness.frame(vec![]);
    harness.frame(vec![]);
    harness.type_text("d");
    assert!(harness.editor.completions.popup.is_some());
    harness.press(Key::ArrowRight);
    assert!(
        harness.editor.completions.popup.is_none(),
        "the caret moved outside the completion prefix"
    );
    let before = harness.text.clone();
    harness.press(Key::Enter);
    assert!(
        !harness.text.contains("products"),
        "{before:?} -> {:?}",
        harness.text
    );
}

#[test]
fn typing_immediately_after_a_completion_uses_its_new_caret() {
    let mut harness = Harness::new("scenario \"S\" { total = pro");
    harness.settle();
    harness.type_text("d");
    harness.press(Key::Enter);
    harness.type_text(".");
    assert!(harness.text.ends_with("products."), "{}", harness.text);
}

#[test]
fn backspace_removes_the_indentation_instead_of_writing_it_back() {
    let mut harness = Harness::new("scenario \"S\" {");
    harness.settle();
    harness.press(Key::Enter);
    assert_eq!(
        harness.text, "scenario \"S\" {\n  ",
        "a broken line keeps one level of indentation"
    );
    harness.press(Key::Backspace);
    assert_eq!(harness.text, "scenario \"S\" {\n ");
    harness.press(Key::Backspace);
    assert_eq!(
        harness.text, "scenario \"S\" {\n",
        "the last space of the indentation goes too"
    );
    harness.press(Key::Backspace);
    assert_eq!(
        harness.text, "scenario \"S\" {",
        "and then the line break itself"
    );
}

#[test]
fn backspace_at_a_line_start_joins_the_lines() {
    let mut harness = Harness::new("scenario \"S\" {\n  a = 1\n  b");
    harness.settle();
    // Back over `b` and the indentation of its line.
    for _ in 0..3 {
        harness.press(Key::Backspace);
    }
    assert_eq!(
        harness.text, "scenario \"S\" {\n  a = 1\n",
        "the indentation of the previous line is not written back"
    );
    harness.press(Key::Backspace);
    assert_eq!(harness.text, "scenario \"S\" {\n  a = 1");
}

#[test]
fn pasted_lines_keep_their_own_indentation() {
    let mut harness = Harness::new("scenario \"S\" {");
    harness.settle();
    harness.frame(vec![Event::Paste("\n    a = 1\n".to_owned())]);
    assert_eq!(
        harness.text, "scenario \"S\" {\n    a = 1\n",
        "a paste ending in a line break gains no indentation of its own"
    );
}

#[test]
fn tab_indents_by_the_width_the_language_is_printed_at() {
    let mut harness = Harness::new("scenario \"S\" {\na = 1");
    harness.settle();
    harness.press(Key::Tab);
    assert_eq!(
        harness.text, "scenario \"S\" {\na = 1 ",
        "a caret on its own moves to the next tab stop, never inserting a tab"
    );
    assert!(!harness.text.contains('\t'));
    harness.press(Key::Tab);
    assert_eq!(
        harness.text, "scenario \"S\" {\na = 1   ",
        "and on to the next"
    );
}

#[test]
fn shift_tab_takes_off_exactly_one_level() {
    let mut harness = Harness::new("scenario \"S\" {\n  a = 1");
    harness.settle();
    harness.shift(Key::Tab);
    assert_eq!(
        harness.text, "scenario \"S\" {\na = 1",
        "the printer's own two-space indentation comes off"
    );
    // Nothing left to take away.
    harness.shift(Key::Tab);
    assert_eq!(harness.text, "scenario \"S\" {\na = 1");

    let mut harness = Harness::new("scenario \"S\" {\n    a = 1");
    harness.settle();
    harness.shift(Key::Tab);
    assert_eq!(
        harness.text, "scenario \"S\" {\n  a = 1",
        "four spaces lose one level, not two"
    );
}

#[test]
fn tab_shifts_a_selected_block_instead_of_replacing_it() {
    let mut harness = Harness::new("scenario \"S\" {\n  a = 1\n  b = 2");
    harness.settle();
    // Select the two assignments, from the end of the text backwards.
    for _ in 0.."  a = 1\n  b = 2".len() {
        harness.shift(Key::ArrowLeft);
    }
    harness.press(Key::Tab);
    assert_eq!(
        harness.text, "scenario \"S\" {\n    a = 1\n    b = 2",
        "every selected line gains a level and none of the text is lost"
    );
    harness.shift(Key::Tab);
    assert_eq!(
        harness.text, "scenario \"S\" {\n  a = 1\n  b = 2",
        "the selection survives the first Tab, so Shift+Tab puts it back"
    );
}

#[test]
fn indenting_a_block_leaves_blank_lines_alone() {
    let mut harness = Harness::new("a = 1\n\nb = 2");
    harness.settle();
    for _ in 0.."a = 1\n\nb = 2".len() {
        harness.shift(Key::ArrowLeft);
    }
    harness.press(Key::Tab);
    assert_eq!(
        harness.text, "  a = 1\n\n  b = 2",
        "a blank line gains no trailing spaces"
    );
}

#[test]
fn reindenting_moves_the_lines_a_selection_touches_and_no_others() {
    // A selection ending exactly at a line start does not reach into that line.
    let mut text = "a\nb\nc".to_owned();
    assert_eq!(reindent(&mut text, (0, 2), true), Some((0, 4)));
    assert_eq!(text, "  a\nb\nc");

    // A caret part-way along a line goes to the next tab stop.
    let mut text = "abc".to_owned();
    assert_eq!(reindent(&mut text, (1, 1), true), Some((2, 2)));
    assert_eq!(text, "a bc");

    // Nothing to take away leaves the text untouched.
    let mut text = "a\nb".to_owned();
    assert_eq!(reindent(&mut text, (0, 3), false), None);
    assert_eq!(text, "a\nb");

    // A reversed selection keeps its direction.
    let mut text = "  a".to_owned();
    assert_eq!(reindent(&mut text, (3, 0), false), Some((1, 0)));
    assert_eq!(text, "a");
}

#[test]
fn the_gutter_numbers_every_line_once_and_marks_the_ones_with_problems() {
    let mut harness = Harness::new("scenario \"S\" {\n  a = oops\n\n  b = two\n}");
    harness.settle();
    assert_eq!(
        harness.gutter_numbers(),
        [1, 2, 3, 4, 5],
        "every line is numbered once, blank lines included"
    );
    assert_eq!(harness.gutter_markers(), 0);

    // A problem on the second line marks that line, and only that line.
    harness.diagnostics = vec![Diagnostic {
        range: 21..25,
        severity: Severity::Error,
        message: "unknown name".into(),
        fix: None,
    }];
    harness.settle();
    assert_eq!(harness.gutter_numbers(), [1, 2, 3, 4, 5]);
    assert_eq!(harness.gutter_markers(), 1);
}
