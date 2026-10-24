//! Code buffers: the text of a scenario or component while it is being edited.
//!
//! The document is the truth and code is printed from it, but text under a cursor
//! is often not yet a document. A buffer is what holds that in-between state, with
//! the rules of [ADR 0007](../../../docs/decisions/0007-code-editing-state.md):
//!
//! - **Clean** — the text is what the document prints. Nothing is stored.
//! - **Applied** — the text lowers to the document that is already saved, but is
//!   spelled differently (spacing, line order). It is kept as the user wrote it.
//! - **Invalid** — the text has errors. The document holds the last valid version
//!   and the board is read-only until the text is fixed or discarded.
//!
//! Text becomes a document after a pause in typing, never on every keystroke: a
//! half-typed name is not a rename. Nothing here touches storage or egui; the
//! panel drives it and the editor applies what it returns.

use apiforge_core::format::{Component, Project, Scenario};
use apiforge_script::{
    Diagnostic, Document, LowerContext, Severity, lower, parse, print_component, print_scenario,
    reconcile,
};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

/// How long typing must pause before the text is turned into a document.
pub const APPLY_DELAY: Duration = Duration::from_millis(250);

/// Which document a buffer belongs to. A request body opened in the panel is a
/// view of its scenario, not a context of its own.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Context {
    Scenario {
        project: String,
        id: String,
    },
    Component {
        project: String,
        id: String,
        revision: u32,
    },
}

/// What the text currently is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    /// The text is exactly what the document prints.
    #[default]
    Clean,
    /// The text is a valid spelling of the document that is already applied.
    Applied,
    /// The text has errors; the document is the last valid version.
    Invalid,
}

/// One context's text, with what is known about it.
#[derive(Clone, Debug, Default)]
pub struct Buffer {
    pub text: String,
    pub state: State,
    pub diagnostics: Vec<Diagnostic>,
    /// Where the caret was, so a draft comes back where it was left.
    pub cursor: usize,
    /// When the text last changed, for the pause that precedes applying it.
    pub typed_at: Option<Instant>,
    /// The document revision this text was applied at. A document that has moved
    /// on since is reprinted over the text, as ADR 0007 asks.
    pub applied_revision: Option<u64>,
    /// Restored from a draft written against a document that has moved on since.
    /// The user decides whether to keep it; the hash it was compared against
    /// belongs to the draft, not to the buffer, and is taken again at each write.
    pub stale: bool,
    /// Whether the text is waiting to be applied.
    pending: bool,
}

impl Buffer {
    fn clean(text: String) -> Self {
        Self {
            text,
            ..Self::default()
        }
    }

    /// Whether the board must stay read-only: a graph edit cannot be merged into
    /// text that does not parse.
    pub fn blocks_the_board(&self) -> bool {
        self.state == State::Invalid
    }

    /// Errors only; warnings do not stop anything.
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
    }
}

/// A draft as it was stored, on its way back into a buffer.
pub struct Draft {
    pub text: String,
    pub cursor: usize,
    /// The document moved on while the draft was away.
    pub stale: bool,
}

/// What a buffer wants done to the document.
#[derive(Debug)]
pub enum Apply {
    /// Nothing to do: either nothing changed or the pause is not over.
    Nothing,
    /// The text lowers to this scenario; apply it as one edit.
    Scenario(Box<Scenario>),
    /// The text lowers to this component; apply it as one edit.
    Component(Box<Component>),
    /// The text does not lower. The document keeps its last valid version.
    Failed,
}

/// The buffers of every context the user has touched.
#[derive(Default)]
pub struct Buffers {
    open: HashMap<Context, Buffer>,
}

impl Buffers {
    /// The text to show for a context, printed from the document when the buffer
    /// has nothing of its own to say.
    ///
    /// Three rules, from ADR 0007. Text with errors is never overwritten: it is
    /// the only copy of what someone typed. Text that was applied keeps the
    /// spelling its author chose until the document moves on without it, and even
    /// then not while the caret is in it — reformatting under a cursor is its own
    /// kind of loss. Everything else is what the document prints.
    ///
    /// A document that cannot be printed — it says something the language cannot
    /// write — produces `None`, and the panel says so rather than showing text
    /// that would destroy the document if it were applied.
    pub fn text(
        &mut self,
        context: &Context,
        printed: Option<String>,
        revision: u64,
        focused: bool,
    ) -> Option<&mut Buffer> {
        let keep = match self.open.get(context) {
            Some(buffer) => match buffer.state {
                State::Invalid => true,
                State::Applied => focused || buffer.applied_revision == Some(revision),
                State::Clean => false,
            },
            None => false,
        };
        if keep {
            return self.open.get_mut(context);
        }
        let printed = printed?;
        let buffer = self
            .open
            .entry(context.clone())
            .or_insert_with(|| Buffer::clean(printed.clone()));
        buffer.text = printed;
        buffer.state = State::Clean;
        buffer.applied_revision = None;
        buffer.diagnostics.clear();
        Some(buffer)
    }

    /// Records the document revision a buffer's text was applied at.
    pub fn applied(&mut self, context: &Context, revision: u64) {
        if let Some(buffer) = self.open.get_mut(context) {
            buffer.applied_revision = Some(revision);
        }
    }

    pub fn get(&self, context: &Context) -> Option<&Buffer> {
        self.open.get(context)
    }

    pub fn get_mut(&mut self, context: &Context) -> Option<&mut Buffer> {
        self.open.get_mut(context)
    }

    /// Whether a context's text is holding the board.
    pub fn blocks_the_board(&self, context: &Context) -> bool {
        self.open.get(context).is_some_and(Buffer::blocks_the_board)
    }

    /// Records that the user typed, which starts the pause before applying.
    pub fn typed(&mut self, context: &Context, text: String, now: Instant) {
        let buffer = self.open.entry(context.clone()).or_default();
        if buffer.text == text {
            return;
        }
        buffer.text = text;
        buffer.typed_at = Some(now);
        buffer.pending = true;
        // Typing is the answer to "is this draft still what you meant?".
        buffer.stale = false;
    }

    /// Throws the text away and goes back to what the document prints.
    pub fn discard(&mut self, context: &Context) {
        self.open.remove(context);
    }

    /// Puts a draft back as a buffer. It arrives as unapplied text, so the next
    /// pause parses it and decides whether it is valid.
    pub fn restore(&mut self, context: Context, draft: Draft) {
        self.open.insert(
            context,
            Buffer {
                text: draft.text,
                state: State::Invalid,
                cursor: draft.cursor,
                applied_revision: None,
                stale: draft.stale,
                typed_at: None,
                diagnostics: Vec::new(),
                pending: true,
            },
        );
    }

    /// Buffers worth keeping between sessions: everything the document does not
    /// already say.
    pub fn drafts(&self) -> impl Iterator<Item = (&Context, &Buffer)> {
        self.open
            .iter()
            .filter(|(_, buffer)| buffer.state != State::Clean)
    }

    /// Turns a context's text into a document once typing has paused.
    ///
    /// The diagnostics are refreshed whether or not the text applies, so the
    /// editor underlines the problem while it is being typed.
    pub fn apply(
        &mut self,
        context: &Context,
        project: &Project,
        current: &Document,
        now: Instant,
    ) -> Apply {
        let Some(buffer) = self.open.get_mut(context) else {
            return Apply::Nothing;
        };
        if !buffer.pending {
            return Apply::Nothing;
        }
        if buffer
            .typed_at
            .is_some_and(|typed| now.saturating_duration_since(typed) < APPLY_DELAY)
        {
            return Apply::Nothing;
        }
        buffer.pending = false;
        let tree = parse(&buffer.text);
        let lowered = lower(&tree, &LowerContext::document(project));
        buffer.diagnostics.clone_from(&lowered.diagnostics);
        if lowered.has_errors() {
            buffer.state = State::Invalid;
            return Apply::Failed;
        }
        let Ok(reconciled) = reconcile(current, &lowered) else {
            buffer.diagnostics.push(Diagnostic::error(
                0..buffer.text.len(),
                "This code describes a different document",
            ));
            buffer.state = State::Invalid;
            return Apply::Failed;
        };
        buffer.state = State::Applied;
        match reconciled.document {
            Document::Scenario(scenario) => Apply::Scenario(Box::new(scenario)),
            Document::Component(component) => Apply::Component(Box::new(component)),
        }
    }
}

/// The text a document prints, or `None` when it cannot be written as code.
pub fn print(project: &Project, document: &Document) -> Option<String> {
    match document {
        Document::Scenario(scenario) => print_scenario(project, scenario).ok(),
        Document::Component(component) => print_component(project, component).ok(),
    }
}

#[cfg(test)]
mod tests;
