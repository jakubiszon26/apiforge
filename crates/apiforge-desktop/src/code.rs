//! The code editor: scenario text with colour, diagnostics and completion.
//!
//! It is one widget over `egui::TextEdit`, kept behind this module boundary so a
//! different text engine can replace it without the panels noticing. Everything it
//! knows about the language comes from `apiforge-script`: the lexer colours the
//! text, the completion engine answers what can be written, and the same engine
//! says what a name means and where it is defined.
//!
//! The widget owns no text. The caller holds the buffer and decides what to do
//! with a change, which is what lets the code panel and the small fields share it.

use crate::theme;
use apiforge_script::{
    Diagnostic, Severity,
    complete::{self, Completion, Kind, Mode, Request, Scope},
    lexer::{Token, TokenKind, lex},
};
use eframe::egui::{
    self, Color32, FontId, Key, Modifiers, Rect, Stroke, TextFormat, Ui, pos2, text::LayoutJob,
    vec2,
};
use std::ops::Range;

/// Rows the completion popup shows at once.
const POPUP_ROWS: usize = 8;

/// A source above this size is large: work that reads the whole text costs
/// more per frame or keystroke than it is worth, and the editor has to stay
/// responsive at 1 MB. Such text is laid out plainly, completion opens only on
/// Ctrl+Space, and views avoid whole-source work while it is being typed.
pub const MAX_COLOURED_BYTES: usize = 256 * 1024;

/// What the editor is editing and what exists around it.
pub struct Context<'a> {
    pub mode: Mode,
    pub scope: Scope<'a>,
    /// Problems to underline, in byte ranges of the same text.
    pub diagnostics: &'a [Diagnostic],
    /// Line numbers in a gutter, which a one-line field does not want.
    pub line_numbers: bool,
    /// Whether a long line folds back under itself. Code scrolls instead.
    pub wrap: bool,
    pub font_size: f32,
    /// Plain text, not the scenario language: no colouring and no completion.
    /// A project text file such as `.gitignore` is not a definition, so the
    /// language's names and shapes say nothing about it.
    pub plain: bool,
}

impl Default for Context<'_> {
    fn default() -> Self {
        Self {
            mode: Mode::Script,
            scope: Scope::default(),
            diagnostics: &[],
            line_numbers: true,
            wrap: false,
            font_size: 13.0,
            plain: false,
        }
    }
}

/// What the editor did this frame.
///
/// Going to a block's definition and renaming it are not here: both change what
/// is selected on the board, and the panel that owns both the editor and the
/// board arrives with the code panel. Hovering a name already explains it, which
/// needs nothing outside the widget.
pub struct Outcome {
    pub response: egui::Response,
    pub changed: bool,
    /// The caret, as a byte offset, while the editor has the keyboard. The panel
    /// uses it to keep the board's selection in step with the text.
    pub cursor: Option<usize>,
}

/// The editor's own state: what the popup is showing and what it is for.
///
/// It is `Clone` so that a panel without a place to keep it can park it in the
/// interface's own temporary storage between frames.
#[derive(Clone, Default)]
pub struct CodeEditor {
    completions: Completions,
}

/// The completion popup of one text edit, whatever widget drew it.
///
/// A small field wants completion without line numbers, colouring or a gutter, so
/// the popup lives on its own and the code editor is one of its callers.
#[derive(Clone, Default)]
pub struct Completions {
    popup: Option<Popup>,
    /// Set after an insertion, so the selection can be placed once the text edit
    /// has been laid out again. Secondary first, then the primary end, which is
    /// the one that moves and the one scrolled to.
    pending_caret: Option<(usize, usize)>,
    prepared_scroll: Option<usize>,
}

#[derive(Clone)]
struct Popup {
    completions: Vec<Completion>,
    selected: usize,
    /// Where the popup is anchored, in screen coordinates.
    at: egui::Pos2,
    /// The caret the popup was opened for; a caret elsewhere closes it.
    cursor: usize,
}

impl Completions {
    /// Apply a programmatic caret before the widget handles new input.
    pub fn prepare(&mut self, ui: &Ui, id: egui::Id, text: &str) {
        if let Some((secondary, primary)) = self.pending_caret.take() {
            let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
            state.cursor.set_char_range(Some(egui::text::CCursorRange {
                primary: egui::text::CCursor::new(epaint_index(char_index(text, primary))),
                secondary: egui::text::CCursor::new(epaint_index(char_index(text, secondary))),
                h_pos: None,
            }));
            state.store(ui.ctx(), id);
            self.prepared_scroll = Some(primary);
        }
    }

    /// Keys the popup claims this frame, taken before the text edit sees them.
    pub fn take_keys(&mut self, ui: &Ui, id: egui::Id) -> Keys {
        if !ui.is_enabled() || !ui.memory(|memory| memory.has_focus(id)) {
            self.popup = None;
            return Keys::default();
        }
        if self.popup.is_none() {
            return Keys {
                open: ui.input_mut(|input| {
                    input.consume_key(Modifiers::CTRL, Key::Space)
                        || input.consume_key(Modifiers::COMMAND, Key::Space)
                }),
                ..Keys::default()
            };
        }
        ui.input_mut(|input| Keys {
            open: false,
            up: input.consume_key(Modifiers::NONE, Key::ArrowUp),
            down: input.consume_key(Modifiers::NONE, Key::ArrowDown),
            accept: input.consume_key(Modifiers::NONE, Key::Enter)
                || input.consume_key(Modifiers::NONE, Key::Tab),
            dismiss: input.consume_key(Modifiers::NONE, Key::Escape),
        })
    }

    /// Opens, moves, draws and applies the popup for a text edit that has just
    /// been laid out. Returns whether the text changed.
    pub fn update(
        &mut self,
        ui: &mut Ui,
        id: egui::Id,
        text: &mut String,
        context: &Context<'_>,
        output: &egui::text_edit::TextEditOutput,
        keys: Keys,
    ) -> bool {
        let cursor = output
            .cursor_range
            .map(|range| byte_offset(text, range.primary.index.0));
        if let Some(caret) = self.prepared_scroll.take() {
            let index = epaint_index(char_index(text, caret));
            let rect = output
                .galley
                .pos_from_cursor(egui::text::CCursor::new(index))
                .translate(output.galley_pos.to_vec2());
            ui.scroll_to_rect(rect.expand(8.0), Some(egui::Align::Center));
            ui.ctx().request_repaint();
        }
        if !output.response.has_focus() || !ui.is_enabled() {
            self.popup = None;
            return false;
        }

        let typed = output.response.response.changed();
        // Navigation and selection invalidate the replacement range just as blur
        // does. Only typing may refresh a popup for a new caret position.
        if let Some(popup) = &self.popup
            && ((!typed && cursor != Some(popup.cursor))
                || output
                    .cursor_range
                    .is_some_and(|range| range.primary != range.secondary))
        {
            self.popup = None;
        }
        if keys.dismiss {
            self.popup = None;
        }

        // Offers come from analysing the whole source, so a large one waits to
        // be asked; a popup already open keeps following the typing.
        let wants_popup = keys.open
            || (typed
                && cursor.is_some_and(|cursor| {
                    self.popup.is_some()
                        || (text.len() <= MAX_COLOURED_BYTES && opens_completion(text, cursor))
                }));
        if wants_popup && let Some(cursor) = cursor {
            let found = complete::complete(&Request {
                source: text,
                cursor,
                mode: context.mode,
                scope: Scope {
                    handles: context.scope.handles,
                    environment: context.scope.environment,
                    parameters: context.scope.parameters,
                    inputs: context.scope.inputs,
                    components: context.scope.components,
                    shapes: context.scope.shapes,
                },
            });
            self.popup = (!found.is_empty()).then(|| {
                let at = output.galley_pos
                    + output
                        .galley
                        .pos_from_cursor(egui::text::CCursor::new(epaint_index(char_index(
                            text,
                            found[0].replace.start,
                        ))))
                        .left_bottom()
                        .to_vec2();
                Popup {
                    selected: 0,
                    at,
                    cursor,
                    completions: found,
                }
            });
        }

        if let Some(popup) = &mut self.popup {
            popup.at = output.galley_pos
                + output
                    .galley
                    .pos_from_cursor(egui::text::CCursor::new(epaint_index(char_index(
                        text,
                        cursor.unwrap_or(popup.cursor),
                    ))))
                    .left_bottom()
                    .to_vec2();
            let count = popup.completions.len();
            if keys.down {
                popup.selected = (popup.selected + 1) % count;
            }
            if keys.up {
                popup.selected = (popup.selected + count - 1) % count;
            }
        }
        let accepted = keys
            .accept
            .then_some(self.popup.as_ref())
            .flatten()
            .map(|popup| popup.completions[popup.selected].clone());
        let chosen = match accepted {
            Some(chosen) => {
                ui.ctx().request_repaint();
                Some(chosen)
            }
            None => self.draw(ui, id.with("completion-popup"), context),
        };
        match chosen {
            Some(chosen) => {
                apply(text, &chosen);
                let caret = chosen.replace.start + chosen.caret;
                self.pending_caret = Some((caret, caret));
                self.popup = None;
                true
            }
            None => false,
        }
    }

    /// Draws the popup and returns a completion the user clicked.
    fn draw(
        &mut self,
        ui: &mut Ui,
        popup_id: egui::Id,
        context: &Context<'_>,
    ) -> Option<Completion> {
        let popup = self.popup.as_ref()?;
        let mut chosen = None;
        let selected = popup.selected;
        let completions = popup.completions.clone();
        let at = popup.at;
        egui::Area::new(popup_id)
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style())
                    .fill(theme::WHITE)
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .show(ui, |ui| {
                        ui.set_max_width(420.0);
                        egui::ScrollArea::vertical()
                            .max_height(
                                ui.text_style_height(&egui::TextStyle::Body) * POPUP_ROWS as f32
                                    + 8.0,
                            )
                            .show(ui, |ui| {
                                for (index, completion) in completions.iter().enumerate() {
                                    let row = row_label(completion, context.font_size);
                                    let response = ui.selectable_label(index == selected, row);
                                    if index == selected {
                                        response.scroll_to_me(None);
                                    }
                                    if response.clicked() {
                                        chosen = Some(completion.clone());
                                    }
                                }
                            });
                    });
            });
        chosen
    }
}

impl CodeEditor {
    /// Puts the caret at a byte offset on the next frame, for a selection made
    /// outside the editor.
    pub fn reveal(&mut self, at: usize) {
        self.completions.pending_caret = Some((at, at));
    }

    /// Selects a byte range on the next frame, keeping its direction: the caret
    /// is the end that moves.
    pub fn select(&mut self, anchor: usize, caret: usize) {
        self.completions.pending_caret = Some((anchor, caret));
    }

    /// Draws the editor and returns what the user did.
    ///
    /// `id` is the text edit's own id, so a panel with several editors keeps each
    /// one's focus and caret apart.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        id: egui::Id,
        text: &mut String,
        context: &Context<'_>,
    ) -> Outcome {
        let font = FontId::monospace(context.font_size);
        let row_height = ui.ctx().fonts_mut(|fonts| fonts.row_height(&font));
        // Keys the popup owns must not reach the text edit, so they are taken
        // before it is drawn.
        let keys = if context.plain {
            Keys::default()
        } else {
            self.completions.take_keys(ui, id)
        };
        // Tab is the language's own indentation. Left to the text edit it would
        // insert a tab character the printer never writes, and replace whatever
        // was selected with it.
        let indent = take_indent(ui, id);
        if !context.plain {
            self.completions.prepare(ui, id, text);
        }
        let diagnostics: Vec<Diagnostic> = context.diagnostics.to_vec();
        let font_for_layout = font.clone();
        let wrap = context.wrap;
        let plain = context.plain;
        let mut layouter = move |ui: &Ui, buffer: &dyn egui::TextBuffer, wrap_width: f32| {
            let mut job = if plain {
                LayoutJob::simple(
                    buffer.as_str().to_owned(),
                    font_for_layout.clone(),
                    theme::TEXT,
                    f32::INFINITY,
                )
            } else {
                layout_job(buffer.as_str(), &diagnostics, font_for_layout.clone())
            };
            // Code is read by its indentation, so a long line scrolls rather than
            // folding back under itself. A one-line field wraps as a field does.
            job.wrap.max_width = if wrap { wrap_width } else { f32::INFINITY };
            ui.ctx().fonts_mut(|fonts| fonts.layout_job(job))
        };

        let gutter_width = if context.line_numbers {
            // The last number is one past the line breaks: text ending in one
            // has an empty last line, which `lines()` does not count but the
            // gutter does number.
            let lines = text.bytes().filter(|byte| *byte == b'\n').count() + 1;
            let digits = lines.to_string().len().max(2);
            ui.ctx().fonts_mut(|fonts| fonts.glyph_width(&font, '0')) * digits as f32 + 12.0
        } else {
            0.0
        };

        let mut outcome = None;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let gutter = context.line_numbers.then(|| {
                let lines = text.lines().count().max(1);
                let (rect, _) = ui.allocate_exact_size(
                    vec2(gutter_width, row_height * lines as f32 + 16.0),
                    egui::Sense::hover(),
                );
                rect
            });
            let output = egui::TextEdit::multiline(text)
                .id(id)
                .font(egui::TextStyle::Monospace)
                .code_editor()
                .desired_width(f32::INFINITY)
                // KTLO-13 mock-up: no field box; the text fills the panel.
                .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(4, 2)))
                .min_size(vec2(
                    0.0,
                    if context.line_numbers {
                        (ui.clip_rect().bottom() - ui.cursor().top()).max(0.0)
                    } else {
                        0.0
                    },
                ))
                .lock_focus(true)
                .layouter(&mut layouter)
                .show(ui);
            if let Some(gutter) = gutter {
                self.paint_gutter(ui, gutter, text, &output, context);
            }
            outcome = Some(self.after_edit(ui, text, context, &output, Taken { keys, indent }, id));
        });
        outcome.expect("the editor is always drawn")
    }

    /// Draws line numbers and a marker beside every line a diagnostic covers.
    ///
    /// The y of each line comes from the galley, so a wrapped line keeps its
    /// number beside its first visual row instead of drifting out of step.
    fn paint_gutter(
        &self,
        ui: &Ui,
        rect: Rect,
        text: &str,
        output: &egui::text_edit::TextEditOutput,
        context: &Context<'_>,
    ) {
        // The gutter is as tall as the text, which a wrapped line makes taller
        // than the rectangle reserved for it; the panel's own clipping decides
        // what is visible.
        let visible = ui.clip_rect();
        let painter = ui.painter().with_clip_rect(visible);
        // KTLO-13 mock-up: the gutter stays at the left edge while a long line
        // scrolls horizontally beneath it.
        let rect = rect.translate(vec2((visible.left() - rect.left()).max(0.0), 0.0));
        painter.rect_filled(
            Rect::from_x_y_ranges(rect.x_range(), visible.y_range()),
            0.0,
            theme::WHITE,
        );
        let font = FontId::monospace(context.font_size);
        let row_height = ui.ctx().fonts_mut(|fonts| fonts.row_height(&font));
        let marked = marked_lines(text, context.diagnostics);
        // The galley's own rows are walked rather than the text's lines. Asking
        // the galley where a line begins scans the text up to it and then every
        // row before it, which a long scenario pays once per line and per frame:
        // thirty thousand lines took a third of a second that way.
        let mut number = 1;
        let mut begins_a_line = true;
        for row in &output.galley.rows {
            let top = output.galley_pos.y + row.min_y();
            if top > visible.bottom() {
                break;
            }
            // A wrapped line keeps its number beside its first visual row
            // instead of repeating it under itself.
            if begins_a_line && top + row_height >= visible.top() {
                let flagged = marked.contains(&number);
                painter.text(
                    pos2(rect.right() - 8.0, top),
                    egui::Align2::RIGHT_TOP,
                    number.to_string(),
                    font.clone(),
                    if flagged { theme::DANGER } else { theme::MUTED },
                );
                if flagged {
                    painter.circle_filled(
                        pos2(rect.left() + 4.0, top + row_height * 0.5),
                        3.0,
                        theme::DANGER,
                    );
                }
            }
            begins_a_line = row.ends_with_newline;
            number += usize::from(row.ends_with_newline);
        }
    }

    /// Everything that happens once the text edit has been laid out: the caret,
    /// the popup, hover and the shortcuts that need a name under the cursor.
    fn after_edit(
        &mut self,
        ui: &mut Ui,
        text: &mut String,
        context: &Context<'_>,
        output: &egui::text_edit::TextEditOutput,
        taken: Taken,
        id: egui::Id,
    ) -> Outcome {
        let response = output.response.response.clone();
        let cursor = output
            .cursor_range
            .map(|range| byte_offset(text, range.primary.index.0));
        let mut changed = response.changed();
        if !context.plain {
            changed |= self
                .completions
                .update(ui, id, text, context, output, taken.keys);
        }
        if let Some(deeper) = taken.indent.wanted()
            && let Some(range) = output.cursor_range
        {
            let selection = (
                byte_offset(text, range.secondary.index.0),
                byte_offset(text, range.primary.index.0),
            );
            if let Some(moved) = reindent(text, selection, deeper) {
                self.completions.pending_caret = Some(moved);
                ui.ctx().request_repaint();
                changed = true;
            }
        }

        // Hovering a name explains it, from the same index the popup uses, so
        // the two can never disagree about what a name is.
        if let Some(pointer) = response.hover_pos() {
            let at = byte_offset(
                text,
                output
                    .galley
                    .cursor_from_pos(pointer - output.galley_pos)
                    .index
                    .0,
            );
            let request = Request {
                source: text,
                cursor: at,
                mode: context.mode,
                scope: Scope {
                    handles: context.scope.handles,
                    environment: context.scope.environment,
                    parameters: context.scope.parameters,
                    inputs: context.scope.inputs,
                    components: context.scope.components,
                    shapes: context.scope.shapes,
                },
            };
            if let Some(described) = complete::describe(&request) {
                response
                    .clone()
                    .on_hover_text(format!("{}  —  {}", described.name, described.detail));
            }
            if let Some(message) = diagnostic_at(context.diagnostics, at) {
                response.clone().on_hover_text(message);
            }
        }
        // Indentation is added for a line the user has just broken, never for
        // one a deletion happened to end at: Backspace over the indentation must
        // remove it rather than see it written back. The text edit reads its
        // events without taking them, so the key is still here to be seen; the
        // popup took it already if it accepted a completion with it.
        if changed && self.completions.popup.is_none() && breaks_the_line(ui) {
            // Auto-indent happens on the text, not on the layout, so the buffer
            // stays the single source of truth.
            if let Some(cursor) = cursor
                && let Some(caret) = auto_indent(text, cursor)
            {
                self.completions.pending_caret = Some((caret, caret));
            }
        }

        Outcome {
            cursor: response.has_focus().then_some(cursor).flatten(),
            response,
            changed,
        }
    }
}

/// Whether this frame's input breaks the line the caret is on, which is the
/// only edit an indentation follows. Enter is matched the way the text edit
/// matches it, so Shift+Enter counts and Ctrl+Enter does not.
/// The directional selection of a text edit as byte offsets `(anchor, caret)`,
/// once the widget has been shown.
pub fn selection(context: &egui::Context, id: egui::Id, text: &str) -> Option<(usize, usize)> {
    let range = egui::text_edit::TextEditState::load(context, id)?
        .cursor
        .char_range()?;
    Some((
        byte_offset(text, range.secondary.index.0),
        byte_offset(text, range.primary.index.0),
    ))
}

/// Forgets a text edit's undo history, for text replaced from outside it.
pub fn clear_history(context: &egui::Context, id: egui::Id) {
    if let Some(mut state) = egui::text_edit::TextEditState::load(context, id) {
        state.clear_undoer();
        state.store(context, id);
    }
}

fn breaks_the_line(ui: &Ui) -> bool {
    ui.input(|input| {
        input.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::Key {
                    key: Key::Enter,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.matches_logically(Modifiers::NONE)
            )
        })
    })
}

/// The keys taken from this frame before the text edit was drawn.
#[derive(Default, Clone, Copy)]
struct Taken {
    keys: Keys,
    indent: Indent,
}

/// Which way Tab was pressed, once the popup has had its chance at it.
#[derive(Default, Clone, Copy)]
struct Indent {
    deeper: bool,
    shallower: bool,
}

impl Indent {
    /// `Some(true)` to indent, `Some(false)` to take a level away.
    fn wanted(self) -> Option<bool> {
        (self.deeper || self.shallower).then_some(self.deeper)
    }
}

/// Takes Tab from the focused editor, so the text edit never sees it.
///
/// Shift is asked about first: `Modifiers::NONE` matches logically, which also
/// accepts a held Shift, and would otherwise swallow Shift+Tab as a plain one.
fn take_indent(ui: &Ui, id: egui::Id) -> Indent {
    if !ui.is_enabled() || !ui.memory(|memory| memory.has_focus(id)) {
        return Indent::default();
    }
    ui.input_mut(|input| {
        let shallower = input.consume_key(Modifiers::SHIFT, Key::Tab);
        let deeper = input.consume_key(Modifiers::NONE, Key::Tab);
        Indent {
            deeper: deeper && !shallower,
            shallower,
        }
    })
}

/// Keys the popup takes before the text edit sees them.
#[derive(Default, Clone, Copy)]
pub struct Keys {
    open: bool,
    up: bool,
    down: bool,
    accept: bool,
    dismiss: bool,
}

fn row_label(completion: &Completion, size: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.append(
        &completion.label,
        0.0,
        TextFormat {
            font_id: FontId::monospace(size),
            color: colour_of(completion.kind),
            ..Default::default()
        },
    );
    if let Some(detail) = &completion.detail {
        job.append(
            detail,
            12.0,
            TextFormat {
                font_id: FontId::proportional(size - 1.0),
                color: theme::MUTED,
                ..Default::default()
            },
        );
    }
    job
}

fn colour_of(kind: Kind) -> Color32 {
    match kind {
        Kind::Variable => theme::VARIABLE,
        Kind::Operator | Kind::Predicate => theme::OPERATOR,
        Kind::Method | Kind::Keyword => theme::LINK,
        Kind::Environment | Kind::Parameter | Kind::Input => theme::ACCENT,
        _ => theme::TEXT,
    }
}

/// Replaces the typed word with the completion's text.
fn apply(text: &mut String, completion: &Completion) {
    let range = completion.replace.clone();
    if range.end > text.len()
        || !text.is_char_boundary(range.start)
        || !text.is_char_boundary(range.end)
    {
        return;
    }
    text.replace_range(range, &completion.insert);
}

/// Whether what was just typed should open the popup by itself.
fn opens_completion(text: &str, cursor: usize) -> bool {
    let before = &text[..cursor.min(text.len())];
    before.ends_with('.')
        || before.ends_with("{{")
        || before
            .chars()
            .next_back()
            .is_some_and(|last| last.is_alphanumeric() || last == '_')
}

/// One level of indentation, the width the language is printed at.
const INDENT: usize = 2;

/// The byte offset the line holding `at` begins at.
fn line_start(text: &str, at: usize) -> usize {
    text[..at.min(text.len())]
        .rfind('\n')
        .map_or(0, |index| index + 1)
}

/// An offset after an edit of `change` bytes made at `at`, never dragged in
/// front of the line the edit belongs to.
fn shifted(offset: usize, at: usize, change: isize) -> usize {
    if offset > at {
        offset.saturating_add_signed(change).max(at)
    } else {
        offset
    }
}

/// Indents or dedents by one level and returns the selection that is left, or
/// `None` when there was nothing to take away.
///
/// A selection covers every line it touches, so Tab shifts a block instead of
/// replacing it. A caret on its own moves to the next tab stop, which is what
/// pressing Tab part-way along a line means; Shift+Tab always takes a level off
/// the line, wherever on it the caret happens to be.
fn reindent(text: &mut String, selection: (usize, usize), deeper: bool) -> Option<(usize, usize)> {
    let reversed = selection.0 > selection.1;
    let (start, end) = (selection.0.min(selection.1), selection.0.max(selection.1));
    if start == end && deeper {
        let column = text[line_start(text, start)..start].chars().count();
        let spaces = INDENT - column % INDENT;
        text.insert_str(start, &" ".repeat(spaces));
        let caret = start + spaces;
        return Some((caret, caret));
    }
    let first = line_start(text, start);
    // A selection stopping exactly at a line start does not reach into it.
    let last = line_start(
        text,
        if start < end && text[..end].ends_with('\n') {
            end - 1
        } else {
            end
        },
    );

    let mut out = String::with_capacity(text.len() + INDENT);
    out.push_str(&text[..first]);
    let (mut moved_start, mut moved_end) = (start, end);
    let mut touched = false;
    let mut at = first;
    loop {
        let line_end = text[at..].find('\n').map_or(text.len(), |index| at + index);
        let line = &text[at..line_end];
        // A blank line gains no indentation: it would only be trailing spaces.
        let dropped = if deeper {
            if !line.is_empty() {
                out.push_str(&" ".repeat(INDENT));
                touched = true;
                moved_start = shifted(moved_start, at, INDENT as isize);
                moved_end = shifted(moved_end, at, INDENT as isize);
            }
            0
        } else {
            let spaces = line
                .bytes()
                .take(INDENT)
                .take_while(|byte| *byte == b' ')
                .count();
            if spaces > 0 {
                touched = true;
                moved_start = shifted(moved_start, at, -(spaces as isize));
                moved_end = shifted(moved_end, at, -(spaces as isize));
            }
            spaces
        };
        out.push_str(&line[dropped..]);
        if at >= last {
            out.push_str(&text[line_end..]);
            break;
        }
        out.push('\n');
        at = line_end + 1;
    }
    if !touched {
        return None;
    }
    *text = out;
    Some(if reversed {
        (moved_end, moved_start)
    } else {
        (moved_start, moved_end)
    })
}

/// Keeps the indentation of the previous line after a newline, one level deeper
/// after something the line left open. Returns the caret that follows the
/// inserted spaces.
///
/// A brace opens a block of the language; a bracket or a parenthesis opens one
/// of a request body, which is printed the same way.
fn auto_indent(text: &mut String, cursor: usize) -> Option<usize> {
    if cursor == 0 || cursor > text.len() || !text.is_char_boundary(cursor) {
        return None;
    }
    if !text[..cursor].ends_with('\n') {
        return None;
    }
    let previous = text[..cursor - 1].rsplit('\n').next().unwrap_or_default();
    let indent = previous.len() - previous.trim_start_matches(' ').len();
    let deeper = previous.trim_end().ends_with(['{', '[', '(']);
    let spaces = indent + if deeper { INDENT } else { 0 };
    if spaces == 0 {
        return None;
    }
    text.insert_str(cursor, &" ".repeat(spaces));
    Some(cursor + spaces)
}

fn diagnostic_at(diagnostics: &[Diagnostic], at: usize) -> Option<String> {
    diagnostics
        .iter()
        .find(|diagnostic| diagnostic.range.contains(&at))
        .map(|diagnostic| diagnostic.message.clone())
}

/// Lines a diagnostic marks, counted from one.
fn marked_lines(text: &str, diagnostics: &[Diagnostic]) -> std::collections::HashSet<usize> {
    diagnostics
        .iter()
        .map(|diagnostic| line_of(text, diagnostic.range.start))
        .collect()
}

/// The line a byte offset is on, counted from one.
fn line_of(text: &str, at: usize) -> usize {
    text[..at.min(text.len())]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

/// The layout of one buffer: coloured by token, or plain when the buffer is too
/// large for that to be worth a frame.
pub fn layout_job(source: &str, diagnostics: &[Diagnostic], font: FontId) -> LayoutJob {
    if source.len() > MAX_COLOURED_BYTES {
        return LayoutJob::simple(source.to_owned(), font, theme::TEXT, f32::INFINITY);
    }
    highlight(source, diagnostics, font)
}

/// Colours the text by token and underlines what a diagnostic covers.
pub fn highlight(source: &str, diagnostics: &[Diagnostic], font: FontId) -> LayoutJob {
    let (tokens, _) = lex(source);
    let mut job = LayoutJob::default();
    for token in &tokens {
        let text = &source[token.range.clone()];
        let colour = token_colour(token, text);
        let underline = diagnostics
            .iter()
            .filter(|diagnostic| overlaps(&diagnostic.range, &token.range))
            .map(|diagnostic| match diagnostic.severity {
                Severity::Error => theme::DANGER,
                Severity::Warning => theme::WARNING,
            })
            .next();
        job.append(
            text,
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: colour,
                underline: underline.map_or(Stroke::NONE, |colour| Stroke::new(1.0, colour)),
                ..Default::default()
            },
        );
    }
    job
}

fn overlaps(left: &Range<usize>, right: &Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}

fn token_colour(token: &Token, text: &str) -> Color32 {
    match token.kind {
        TokenKind::Comment => theme::MUTED,
        TokenKind::String => theme::SUCCESS,
        TokenKind::Number => theme::VARIABLE,
        TokenKind::Error => theme::DANGER,
        TokenKind::Identifier if KEYWORDS.contains(&text) => theme::LINK,
        TokenKind::Identifier if METHODS.contains(&text) => theme::ACCENT,
        TokenKind::Identifier => theme::TEXT,
        TokenKind::Symbol => theme::MUTED,
        TokenKind::Whitespace => theme::TEXT,
    }
}

const KEYWORDS: [&str; 24] = [
    "scenario",
    "component",
    "cleanup",
    "param",
    "input",
    "var",
    "export",
    "start",
    "assert",
    "condition",
    "if",
    "else",
    "all",
    "any",
    "exists",
    "nonempty",
    "contains",
    "is",
    "within",
    "with",
    "true",
    "false",
    "null",
    "env",
];

const METHODS: [&str; 7] = ["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"];

/// A character index as the text layout counts them.
fn epaint_index(chars: usize) -> egui::text::CharIndex {
    egui::text::CharIndex(chars)
}

/// The byte offset of a character index, clamped to the text.
fn byte_offset(text: &str, chars: usize) -> usize {
    text.char_indices()
        .nth(chars)
        .map_or(text.len(), |(offset, _)| offset)
}

/// The character index of a byte offset, clamped to the text.
fn char_index(text: &str, byte: usize) -> usize {
    text[..byte.min(text.len())].chars().count()
}

#[cfg(test)]
mod tests;
