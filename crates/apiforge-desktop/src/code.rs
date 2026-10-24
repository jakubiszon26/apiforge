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

/// Text above this size is coloured as plain text: colouring every token of a
/// very large buffer on every frame costs more than it is worth, and the editor
/// has to stay responsive at 1 MB.
const MAX_COLOURED_BYTES: usize = 256 * 1024;

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
    /// Set after an insertion, so the caret can be placed once the text edit has
    /// been laid out again.
    pending_caret: Option<usize>,
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
        if let Some(caret) = self.pending_caret.take() {
            let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::one(
                    egui::text::CCursor::new(epaint_index(char_index(text, caret))),
                )));
            state.store(ui.ctx(), id);
            self.prepared_scroll = Some(caret);
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

        let wants_popup = keys.open
            || (typed
                && cursor
                    .is_some_and(|cursor| opens_completion(text, cursor) || self.popup.is_some()));
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
                self.pending_caret = Some(chosen.replace.start + chosen.caret);
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
        self.completions.pending_caret = Some(at);
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
        let keys = self.completions.take_keys(ui, id);
        self.completions.prepare(ui, id, text);
        let diagnostics: Vec<Diagnostic> = context.diagnostics.to_vec();
        let font_for_layout = font.clone();
        let wrap = context.wrap;
        let mut layouter = move |ui: &Ui, buffer: &dyn egui::TextBuffer, wrap_width: f32| {
            let mut job = layout_job(buffer.as_str(), &diagnostics, font_for_layout.clone());
            // Code is read by its indentation, so a long line scrolls rather than
            // folding back under itself. A one-line field wraps as a field does.
            job.wrap.max_width = if wrap { wrap_width } else { f32::INFINITY };
            ui.ctx().fonts_mut(|fonts| fonts.layout_job(job))
        };

        let gutter_width = if context.line_numbers {
            let digits = text.lines().count().max(1).to_string().len().max(2);
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
                .lock_focus(true)
                .layouter(&mut layouter)
                .show(ui);
            if let Some(gutter) = gutter {
                self.paint_gutter(ui, gutter, text, &output, context);
            }
            outcome = Some(self.after_edit(ui, text, context, &output, keys, id));
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
        let font = FontId::monospace(context.font_size);
        let row_height = ui.ctx().fonts_mut(|fonts| fonts.row_height(&font));
        let marked = marked_lines(text, context.diagnostics);
        for (number, start) in line_starts(text) {
            let top = output.galley_pos.y
                + output
                    .galley
                    .pos_from_cursor(egui::text::CCursor::new(epaint_index(char_index(
                        text, start,
                    ))))
                    .top();
            if top > visible.bottom() {
                break;
            }
            if top + row_height < visible.top() {
                continue;
            }
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
    }

    /// Everything that happens once the text edit has been laid out: the caret,
    /// the popup, hover and the shortcuts that need a name under the cursor.
    fn after_edit(
        &mut self,
        ui: &mut Ui,
        text: &mut String,
        context: &Context<'_>,
        output: &egui::text_edit::TextEditOutput,
        keys: Keys,
        id: egui::Id,
    ) -> Outcome {
        let response = output.response.response.clone();
        let cursor = output
            .cursor_range
            .map(|range| byte_offset(text, range.primary.index.0));
        let mut changed = response.changed();
        changed |= self.completions.update(ui, id, text, context, output, keys);

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
        if changed && self.completions.popup.is_none() {
            // Auto-indent happens on the text, not on the layout, so the buffer
            // stays the single source of truth.
            if let Some(cursor) = cursor
                && let Some(caret) = auto_indent(text, cursor)
            {
                self.completions.pending_caret = Some(caret);
            }
        }

        Outcome {
            cursor: response.has_focus().then_some(cursor).flatten(),
            response,
            changed,
        }
    }
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

/// Keeps the indentation of the previous line after a newline, one level deeper
/// after an opening brace. Returns the caret that follows the inserted spaces.
fn auto_indent(text: &mut String, cursor: usize) -> Option<usize> {
    if cursor == 0 || cursor > text.len() || !text.is_char_boundary(cursor) {
        return None;
    }
    if !text[..cursor].ends_with('\n') {
        return None;
    }
    let previous = text[..cursor - 1].rsplit('\n').next().unwrap_or_default();
    let indent = previous.len() - previous.trim_start_matches(' ').len();
    let deeper = previous.trim_end().ends_with('{');
    let spaces = indent + if deeper { 2 } else { 0 };
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

/// Every line's number and the byte offset it starts at.
fn line_starts(text: &str) -> impl Iterator<Item = (usize, usize)> + '_ {
    std::iter::once((1, 0)).chain(
        text.match_indices('\n')
            .enumerate()
            .map(|(index, (at, _))| (index + 2, at + 1)),
    )
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
