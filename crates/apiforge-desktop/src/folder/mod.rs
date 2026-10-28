//! The folder project window (ADR 0012): Code, Diagram and Split over one
//! `ProjectSession`.
//!
//! The session owns every document's source, model and model history. This
//! window owns only views: tabs, each document's text-edit history (kept by its
//! stable editor identity), caret, scroll, board selection and diagram camera.
//! Switching a mode or a tab therefore never saves, closes or resets a document.
//! Folder projects are opened explicitly with `--folder`; the normal startup
//! path still opens the workspace.

mod activation;
pub use activation::{Refusal, Start, decide as managed_start};
mod analysis;
mod chrome;
mod code_view;
mod conflict_view;
mod diagram;
mod dog;
mod explorer;
mod file_prompts;
mod holder;
mod navigation;
mod opening;
mod placement;
mod publication;
mod runs;
mod saving;
mod settings;
mod storage;
mod text_view;
mod transaction_view;
mod watcher;
mod workbench;

use crate::icons::Icon;
use crate::{
    board_edit::{Board, Edit},
    canvas::{self, Canvas, Command},
    code, dialogs,
    inspector::{self, Inspector},
    library::{self, Library},
    preferences::Preferences,
    theme, widgets,
};
use analysis::Analyser;
use apiforge_core::{
    edit,
    format::{Graph, Position, Project},
    graph::create_operator_node,
};
use apiforge_project::{
    AnalysisState, CommitChangeKind, DeletePreview, Document, DocumentKind, EventKind,
    ExternalConflict, ExternalOutcome, ModelState, Origin, ProjectSession, Registration,
    RegistryState, Unavailable,
};
use apiforge_script::{Diagnostic, Severity};
use apiforge_storage::{
    CodeDraft, DraftTarget,
    project::{CameraState, FieldRecovery, TextSelection, TrashIndex, WorkspaceState},
};
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, RichText};
use explorer::ExplorerState;
use file_prompts::{ImportRow, Picked};
use placement::CardPlacer;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, mpsc::Receiver},
    time::{Duration, Instant},
};
use watcher::Watcher;
pub use workbench::Mode;
use workbench::Workbench;

/// Edits typed into the code view; the view recognises its own changes.
const CODE: Origin = Origin(1);
/// Graphical edits, undo and redo of the model, and reverts.
const DIAGRAM: Origin = Origin(2);
/// Unsaved work is written to local recovery after this much quiet.
const RECOVERY_DELAY: Duration = Duration::from_secs(1);
/// Split keeps the inspector beside code and diagram only on wide windows.
const SPLIT_INSPECTOR_WIDTH: f32 = 1440.0;
/// The width of the block library folded to its rail (KTLO-17).
const LIBRARY_RAIL_WIDTH: f32 = 36.0;
/// A tab shows a registered document, by its ID, or a project text file, by
/// its path behind this prefix. Documents are resolved first, so a tab key
/// cannot shadow one.
const TEXT_TAB: &str = "text:";

enum Prompt {
    Working {
        ticket: u64,
        label: &'static str,
    },
    Conflict(Box<conflict_view::ConflictView>),
    Transaction(Box<transaction_view::TransactionView>),
    Settings(Box<dialogs::SettingsDialog>),
    Environment(Box<dialogs::EnvironmentDialog>),
    Run {
        prepared: Box<apiforge_project::PreparedRun>,
        live: bool,
    },
    /// A component's input values, asked for before its run (KTLO-23b).
    RunInputs {
        prepared: Box<apiforge_project::PreparedRun>,
        live: bool,
        dialog: Box<dialogs::RunInputsDialog>,
    },
    Journal(Box<runs::JournalReview>),
    AssignResult {
        document_id: String,
        cleanup: bool,
        dialog: Box<dialogs::AssignVariableDialog>,
    },
    CommentLoss {
        ticket: apiforge_project::AnalysisTicket,
        action: Box<publication::Action>,
    },
    /// A tab with unsaved work: save, discard or keep it open.
    CloseTab(String),
    NewDocument {
        kind: DocumentKind,
        /// The explorer folder it goes into; the kind's default folder if none.
        folder: Option<String>,
        name: String,
        error: Option<String>,
    },
    /// Closing the window could not protect unsaved work.
    CloseFailed(String),
    Rename {
        path: String,
        name: String,
        error: Option<String>,
    },
    MoveTo {
        path: String,
        folder: String,
        error: Option<String>,
    },
    NewFolder {
        parent: String,
        name: String,
        error: Option<String>,
    },
    Delete {
        preview: Box<DeletePreview>,
        paths: Vec<String>,
        error: Option<String>,
    },
    Import {
        rows: Vec<ImportRow>,
        error: Option<String>,
    },
    Trash {
        operations: Vec<TrashIndex>,
        error: Option<String>,
    },
    /// A found source whose header does not say what it is.
    Register {
        path: String,
        kind: DocumentKind,
        error: Option<String>,
    },
}

/// An `.afs` file in a folder without a project, offered for registration.
struct SurveyRow {
    path: String,
    include: bool,
    /// `None` until chosen when the header is unrecognisable.
    kind: Option<DocumentKind>,
}

/// Whether the session has `tab`'s document or text file.
fn session_has(session: Option<&ProjectSession>, tab: &str) -> bool {
    session.is_some_and(|session| match tab.strip_prefix(TEXT_TAB) {
        Some(path) => session.text_file(path).is_some(),
        None => session.document(tab).is_some(),
    })
}

/// The bar's words for a followed commit: its first change, and how many
/// more there were.
fn describe_commit(changes: &[apiforge_project::CommitChange]) -> Option<String> {
    let first = changes.first()?;
    let name = file_name(&first.path);
    let text = match first.kind {
        CommitChangeKind::Published => format!("{name} published"),
        CommitChangeKind::Added => format!("{name} added"),
        CommitChangeKind::Deleted => format!("{name} deleted"),
        CommitChangeKind::Saved => format!("{name} saved"),
        CommitChangeKind::Layout => format!("{name} layout saved"),
        CommitChangeKind::Settings => "project settings saved".to_owned(),
    };
    Some(match changes.len() {
        1 => text,
        count => format!("{text} and {} more", count - 1),
    })
}

/// A message under the tabs; errors stay until dismissed.
struct Notice {
    text: String,
    error: bool,
}

pub struct FolderApp {
    ui_context: egui::Context,
    root: PathBuf,
    session: Option<ProjectSession>,
    /// The folder has no project yet. Nothing is written until the user
    /// initialises it explicitly.
    uninitialised: bool,
    survey: Vec<SurveyRow>,
    new_name: String,
    open_error: Option<String>,
    workbench: Workbench,
    /// The view state last written, so an unchanged one is not rewritten.
    saved_view: Option<WorkspaceState>,
    /// A mode to show instead of the folder's own once the project is open.
    mode_request: Option<Mode>,
    /// The requested mode shown, and the folder's own, which is what the view
    /// state keeps while the requested one is still shown.
    shown_instead: Option<(Mode, Mode)>,
    analyser: Analyser,
    watcher: Watcher,
    explorer: ExplorerState,
    /// File dialogs running on their own threads.
    picked: Vec<Receiver<Picked>>,
    /// The most recent candidate export; it never resolves the pending decision.
    export_status: Option<String>,
    /// The last session event this window has followed.
    seen: u64,
    code: code::CodeEditor,
    /// The document the code editor last showed; its caret is restored on entry.
    shown: Option<String>,
    scroll_restore: Option<f32>,
    /// The Problems section is expanded over the code (KTLO-18). Deliberately
    /// not in the view state: a window reopens with the section at the height
    /// it was dragged to, not expanded.
    problems_expanded: bool,
    /// Changes with every expand and restore, so egui forgets a height the
    /// handle dragged under the other state, as the run results panel does.
    problems_epoch: u64,
    /// The edge of the expanded section has been dragged, so it keeps the
    /// height it was dragged to instead of following the window.
    problems_dragged: bool,
    /// The block whose statement the code view last revealed.
    revealed: Option<String>,
    /// The statement under the caret, for a document, source version and caret:
    /// finding it parses the whole source, which a large one cannot afford
    /// every frame.
    caret_statement: Option<CaretStatement>,
    /// What completion offers in the code view, kept while its model and
    /// context are the same: deriving it reads every block and sample.
    completion_names: Option<CompletionNames>,
    canvases: HashMap<String, Canvas>,
    selections: HashMap<String, Vec<String>>,
    /// Each document's field drafts as the inspector and the session last
    /// agreed on them. A session-side change (a kept, discarded or restored
    /// draft, or a revert) shows as a difference and wins over the inspector.
    agreed_fields: HashMap<String, Vec<FieldRecovery>>,
    clipboard: Option<Clipboard>,
    inspector: Inspector,
    /// When unsaved work or the view last changed, for the recovery pause.
    changed_at: Option<Instant>,
    notice: Option<Notice>,
    /// The active tab last revealed in the strip, and the strip's last scroll
    /// offset, so activating a tab can scroll it into view.
    followed_tab: Option<String>,
    tabs_offset: f32,
    tabs_width: f32,
    prompt: Option<Prompt>,
    close_error: Option<String>,
    /// Set by a decision in the close dialog; the window closes next frame.
    close_requested: bool,
    preferences: Preferences,
    runs: runs::Runs,
    navigation: navigation::Navigation,
    storage: storage::Storage,
    open_generation: u64,
    opening: bool,
    persistence: saving::Persistence,
    activation_generation: u64,
    operation_sequence: u64,
    picked_ready: Vec<Picked>,
    /// The block library shown beside the board (KTLO-05b).
    library: Library,
    /// Whether an empty board animates its drawing (KTLO-20). The test
    /// harness holds it at its rest frame: a board that asks to be drawn
    /// again every frame never settles.
    animation: bool,
    /// Who holds the project while this window observes it (MCP-07); `None`
    /// while this window is the writer, or read-only with no live holder.
    holder: Option<holder::Observed>,
    take_back: holder::TakeBack,
    /// What the holder's last followed commit changed, with its time, for
    /// the bar (MCP-08a): *10:43 · promotion.afs saved*.
    last_change: Option<String>,
}

struct Clipboard {
    context: String,
    fragment: Graph,
    pastes: u32,
}

/// The statement found under the caret, for one source version.
struct CaretStatement {
    document_id: String,
    version: u64,
    cursor: usize,
    handle: Option<String>,
}

/// Completion's names for one model, context and graph.
struct CompletionNames {
    document_id: String,
    model: Option<Arc<Document>>,
    project: Arc<Project>,
    cleanup: bool,
    nodes: Vec<apiforge_core::format::Node>,
    names: crate::shapes::Names,
}

impl CompletionNames {
    fn of(
        document_id: &str,
        model: Option<Arc<Document>>,
        project: Arc<Project>,
        cleanup: bool,
    ) -> Self {
        let nodes = model
            .as_deref()
            .map(|model| graph_of(model, cleanup).nodes.clone())
            .unwrap_or_default();
        let parameters = model.as_deref().map(parameters_of).unwrap_or_default();
        let names = crate::shapes::Names::of_board(&project, &nodes, &parameters, None);
        Self {
            document_id: document_id.to_owned(),
            model,
            project,
            cleanup,
            nodes,
            names,
        }
    }

    fn describes(
        &self,
        document_id: &str,
        model: Option<&Arc<Document>>,
        project: &Arc<Project>,
        cleanup: bool,
    ) -> bool {
        self.document_id == document_id
            && self.cleanup == cleanup
            && Arc::ptr_eq(&self.project, project)
            && match (&self.model, model) {
                (Some(cached), Some(model)) => Arc::ptr_eq(cached, model),
                (None, None) => true,
                _ => false,
            }
    }
}

/// A frame's copy of what one document shows, so views can change the window
/// while they read it.
struct Snapshot {
    path: String,
    kind: DocumentKind,
    text: String,
    diagnostics: Vec<Diagnostic>,
    analysis: AnalysisState,
    model: Option<Arc<Document>>,
    model_state: ModelState,
    unavailable: Option<Unavailable>,
    editable: bool,
    /// Editing is paused while this window observes another holder (MCP-08a):
    /// the board stays legible and selectable, without the read-only notice.
    paused: bool,
    held: bool,
    dirty: bool,
    layout_dirty: bool,
    conflict: Option<ExternalConflict>,
}

impl FolderApp {
    fn shell(context: &egui::Context, root: PathBuf) -> Self {
        theme::apply(context);
        Self {
            ui_context: context.clone(),
            root,
            session: None,
            uninitialised: false,
            survey: Vec::new(),
            new_name: String::new(),
            open_error: None,
            workbench: Workbench::restore(None, |_| false),
            saved_view: None,
            mode_request: None,
            shown_instead: None,
            analyser: Analyser::new(Some(context.clone())),
            watcher: Watcher::new(Some(context.clone())),
            explorer: ExplorerState::default(),
            picked: Vec::new(),
            export_status: None,
            seen: 0,
            code: code::CodeEditor::default(),
            shown: None,
            scroll_restore: None,
            problems_expanded: false,
            problems_epoch: 0,
            problems_dragged: false,
            revealed: None,
            caret_statement: None,
            completion_names: None,
            canvases: HashMap::new(),
            selections: HashMap::new(),
            agreed_fields: HashMap::new(),
            clipboard: None,
            inspector: Inspector::default(),
            changed_at: None,
            notice: None,
            followed_tab: None,
            tabs_offset: 0.0,
            tabs_width: 0.0,
            prompt: None,
            close_error: None,
            close_requested: false,
            preferences: Preferences::default(),
            runs: runs::Runs::default(),
            navigation: navigation::Navigation::default(),
            storage: storage::Storage::new(context.clone()),
            open_generation: 0,
            opening: false,
            persistence: saving::Persistence::default(),
            activation_generation: 0,
            operation_sequence: 0,
            picked_ready: Vec::new(),
            library: Library::default(),
            animation: !cfg!(test),
            holder: None,
            take_back: holder::TakeBack::default(),
            last_change: None,
        }
    }

    pub fn open(context: &egui::Context, root: PathBuf) -> Self {
        let mut app = Self::shell(context, root);
        app.begin_open();
        app
    }

    fn install(&mut self, session: ProjectSession, state: Option<WorkspaceState>) {
        let first_open = state.is_none();
        self.workbench = Workbench::restore(state.clone(), |id| {
            session.document(id).is_some() || id.starts_with(TEXT_TAB)
        });
        self.saved_view = state;
        self.shown_instead = None;
        if let Some(mode) = self.mode_request.take() {
            self.shown_instead = Some((mode, self.workbench.mode));
            self.workbench.mode = mode;
        }
        // A project opened for the first time shows its first scenario.
        if first_open {
            let mut documents: Vec<_> = session.documents().iter().collect();
            documents.sort_by_key(|document| {
                (
                    document.entry().kind != DocumentKind::Scenario,
                    document.entry().path.as_str().to_owned(),
                )
            });
            if let Some(document) = documents.first() {
                self.activate_tab(document.id());
            }
        }
        // Unsaved work comes back with its own caret.
        for document in session.documents() {
            if document.needs_recovery()
                && let Some(tab) = self.workbench.tab_mut(document.id())
            {
                tab.selection = document.selection().clone();
            }
        }
        for tab in self.workbench.tabs() {
            if let Some(camera) = tab.camera {
                let mut canvas = self.new_canvas();
                canvas.restore_camera(
                    &context_key(&tab.document_id, tab.cleanup),
                    (camera.x, camera.y, camera.zoom),
                );
                self.canvases.insert(tab.document_id.clone(), canvas);
            }
        }
        self.holder = None;
        self.take_back = holder::TakeBack::default();
        self.last_change = None;
        self.seen = session.last_sequence();
        self.uninitialised = false;
        self.session = Some(session);
        self.open_run_journal();
        self.reload_field_drafts();
        self.restore_text_tabs();
        self.remember_project();
    }

    /// Unfinished inspector text comes from the session, which owns it.
    fn reload_field_drafts(&mut self) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let project_id = &session.manifest().id;
        let fields = session
            .documents()
            .iter()
            .flat_map(|document| {
                document.field_drafts().iter().map(|field| CodeDraft {
                    project_id: project_id.clone(),
                    target: DraftTarget::Field {
                        owner: Box::new(inspector::drafts::document_target(document.id())),
                        widget: field.widget,
                    },
                    base_hash: field.base_hash.clone(),
                    text: field.text.clone(),
                    cursor: field.selection.caret,
                })
            })
            .collect();
        self.agreed_fields = session
            .documents()
            .iter()
            .map(|document| (document.id().to_owned(), document.field_drafts().to_vec()))
            .collect();
        inspector::drafts::restore_entries(&self.ui_context, fields);
    }

    /// Shows `mode` once the project is open instead of the folder's own,
    /// for `--screenshot --mode` (KTLO-08).
    pub fn request_mode(&mut self, mode: Mode) {
        self.mode_request = Some(mode);
    }

    pub fn has_document(&self) -> bool {
        self.session.is_some()
    }

    /// No storage work, opening, departure, close or project switch is in flight.
    #[cfg(test)]
    pub(crate) fn is_settled(&self) -> bool {
        !(self.storage.is_busy()
            || self.opening
            || self.persistence.departure
            || (self.persistence.closing && !self.persistence.close_ready)
            || (self.switching() && !self.navigation.failed && self.prompt.is_none()))
    }

    /// Settled, and no analysis is running on its own thread. An analysis
    /// thread does not request a repaint until it finishes, so an interface
    /// that stops repainting can still have a Pending result on the way.
    #[cfg(test)]
    pub(crate) fn is_analysed(&self) -> bool {
        self.is_settled() && !self.analyser.is_busy()
    }

    fn new_canvas(&self) -> Canvas {
        let mut canvas = Canvas::default();
        canvas.wire_style = self.preferences.wire_style;
        canvas.bridges = self.preferences.bridges;
        canvas
    }

    fn activate_tab(&mut self, id: &str) {
        self.activation_generation += 1;
        self.workbench.open(id);
    }

    fn snapshot(&self, document_id: &str) -> Option<Snapshot> {
        let session = self.session.as_ref()?;
        let document = session.document(document_id)?;
        let held = session
            .recovered_drafts()
            .iter()
            .any(|(_, draft)| draft.document_id == document_id);
        let paused = self.observing();
        Some(Snapshot {
            path: document.entry().path.as_str().to_owned(),
            kind: document.entry().kind,
            text: document.text().to_owned(),
            diagnostics: document.diagnostics().to_vec(),
            analysis: document.analysis(),
            model: document.shared_model().cloned(),
            model_state: document.model_state(),
            unavailable: document.unavailable(),
            paused,
            editable: document.accepts_model_edits()
                && !paused
                && !held
                && !self.persistence.closing
                && !self.storage.blocks_edits(),
            held,
            dirty: document.is_dirty(),
            layout_dirty: document.is_layout_dirty(),
            conflict: document.conflict(),
        })
    }

    /// Unsaved work or the view changed: protect it after a pause.
    fn touched(&mut self) {
        self.changed_at = Some(Instant::now());
    }

    fn notify(&mut self, text: impl Into<String>, error: bool) {
        self.notice = Some(Notice {
            text: text.into(),
            error,
        });
    }

    /// Shows a dismissable notice, as an action that reports its outcome
    /// does, for design review views outside this module.
    #[cfg(test)]
    pub(crate) fn debug_notify(&mut self, text: &str) {
        self.notify(text, false);
    }

    // ----- Frame -----------------------------------------------------------------

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.poll_storage();
        self.poll_run();
        self.follow_dialogs();
        self.advance_departure();
        self.advance_switch();
        if self.close_requested {
            self.close_requested = false;
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let Some(session) = self.session.as_mut() else {
            self.start_screen(ui);
            self.prompts(ui.ctx());
            return;
        };
        let now = Instant::now();
        if !self.persistence.closing
            && !self.storage.owns_session()
            && let Some(wait) = self.analyser.poll(session, now)
        {
            ui.ctx().request_repaint_after(wait);
        }
        let (outcome, wait) = if self.persistence.closing {
            self.watcher.drain_stopped();
            (None, Duration::from_millis(16))
        } else {
            self.watcher
                .focus(ui.input(|input| input.viewport().focused.unwrap_or(true)));
            self.watcher.poll(session, now)
        };
        ui.ctx().request_repaint_after(wait);
        if let Some(probe) = self.watcher.take_probe() {
            self.follow_holder(probe);
        }
        self.follow_events();
        if let Some(changes) = outcome {
            self.storage_project(
                "Checking external changes…",
                apiforge_project::StorageMode::Exclusive,
                false,
                move |session| session.apply_external(changes),
                |app, result| match result {
                    Ok(outcome) => app.follow_external(outcome),
                    Err(apiforge_project::Error::StaleStorage) => app.watcher.rescan(),
                    Err(error) => app.watcher.problem = Some(error.to_string()),
                },
            );
        }
        if self.watcher.recovery_required {
            self.transaction_screen(ui);
            self.prompts(ui.ctx());
            return;
        }
        if self.prompt.is_none() {
            self.shortcuts(ui.ctx());
        }
        egui::Panel::top("folder-header")
            .frame(
                egui::Frame::new()
                    .fill(theme::WHITE)
                    .inner_margin(egui::Margin::symmetric(16, 8)),
            )
            .show(ui, |ui| {
                ui.add_enabled_ui(
                    !self.persistence.closing && !self.storage.blocks_edits(),
                    |ui| self.header(ui),
                );
            });
        // With no file open and no notice due the band takes no height at all,
        // so the central area meets the app header (KTLO-24).
        if !self.workbench.tabs().is_empty() || self.has_notices() {
            egui::Panel::top("folder-tabs")
                .frame(
                    egui::Frame::new()
                        .fill(theme::PANEL)
                        .inner_margin(egui::Margin::symmetric(8, 4)),
                )
                .show(ui, |ui| {
                    ui.add_enabled_ui(
                        !self.persistence.closing && !self.storage.blocks_edits(),
                        |ui| self.tab_strip(ui),
                    );
                    self.notices(ui);
                });
        }
        // Across the whole window, under every panel and section (MCP-07).
        self.holder_bar(ui);
        let active = self.workbench.active().map(str::to_owned);
        self.run_report(ui, active.as_deref());
        // Maximised run results leave no room for anything else (KTLO-19).
        if self.report_maximised(active.as_deref()) {
            self.prompts(ui.ctx());
            self.protect(now);
            return;
        }
        match self.workbench.mode {
            Mode::Code => {
                egui::Panel::left("folder-documents")
                    .default_size(self.workbench.panels.documents)
                    .size_range(180.0..=420.0)
                    .frame(
                        egui::Frame::new()
                            .fill(theme::PANEL)
                            .inner_margin(egui::Margin::symmetric(12, 10)),
                    )
                    .show(ui, |ui| {
                        self.workbench.panels.documents = ui.available_width() + 24.0;
                        ui.add_enabled_ui(
                            !self.persistence.closing && !self.storage.blocks_edits(),
                            |ui| self.explorer_panel(ui),
                        );
                    });
                widgets::name_resize_handle(
                    ui.ctx(),
                    egui::Id::new("folder-documents"),
                    "Resize project files",
                );
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(theme::WHITE))
                    .show(ui, |ui| self.main_code_area(ui, active.as_deref()));
            }
            Mode::Diagram => {
                self.library_panel(ui, active.as_deref());
                self.inspector_panel(ui, active.as_deref());
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(theme::BG))
                    .show(ui, |ui| self.diagram_area(ui, active.as_deref()));
            }
            Mode::Split => {
                let width = ui.ctx().content_rect().width();
                egui::Panel::left("folder-split-code")
                    .default_size(self.workbench.panels.split_code)
                    .size_range(300.0..=(width - 360.0).max(300.0))
                    .frame(egui::Frame::new().fill(theme::WHITE))
                    .show(ui, |ui| {
                        self.workbench.panels.split_code = ui.available_width();
                        self.main_code_area(ui, active.as_deref());
                    });
                widgets::name_resize_handle(
                    ui.ctx(),
                    egui::Id::new("folder-split-code"),
                    "Resize code",
                );
                if width >= SPLIT_INSPECTOR_WIDTH {
                    self.library_panel(ui, active.as_deref());
                    self.inspector_panel(ui, active.as_deref());
                }
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(theme::BG))
                    .show(ui, |ui| self.diagram_area(ui, active.as_deref()));
            }
        }
        self.prompts(ui.ctx());
        self.protect(now);
    }

    /// Code for the active tab: a definition's source, or a text file.
    fn main_code_area(&mut self, ui: &mut egui::Ui, active: Option<&str>) {
        match active.and_then(|tab| self.text_snapshot(tab)) {
            Some(snapshot) => self.text_area(ui, &snapshot),
            None => self.code_area(ui, active),
        }
    }

    /// The folder has no project, or could not be opened.
    fn start_screen(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::BG).inner_margin(48))
            .show(ui, |ui| {
                ui.set_max_width(560.0);
                self.navigation_menu(ui);
                ui.label(
                    RichText::new("Open a folder project")
                        .size(theme::FONT_TITLE)
                        .family(theme::bold_family()),
                );
                ui.add_space(theme::SPACE_SM);
                widgets::hint(ui, self.root.display().to_string());
                ui.add_space(theme::SPACE_LG);
                if self.opening || self.persistence.closing {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        widgets::hint(
                            ui,
                            if self.persistence.closing {
                                "Closing project…"
                            } else {
                                "Opening project…"
                            },
                        );
                    });
                    return;
                }
                if let Some(error) = &self.open_error {
                    widgets::error_box(ui, std::slice::from_ref(error));
                    if ui.button("Review transaction recovery…").clicked() {
                        self.review_transaction(self.root.clone());
                    }
                    if ui.button("Try opening again").clicked() {
                        *self = Self::open(&self.ui_context.clone(), self.root.clone());
                    }
                    return;
                }
                if !self.uninitialised {
                    return;
                }
                ui.label(
                    "This folder is not an API Forge project yet. Initialising it adds a \
                     project file and keeps existing files as they are.",
                );
                ui.add_space(theme::SPACE_MD);
                let label = widgets::field_name(ui, "Project name");
                ui.add(widgets::text_edit(&mut self.new_name).desired_width(320.0))
                    .labelled_by(label.id);
                if !self.survey.is_empty() {
                    ui.add_space(theme::SPACE_MD);
                    widgets::field_label(ui, "Register existing sources");
                    egui::ScrollArea::vertical()
                        .max_height(260.0)
                        .show(ui, |ui| {
                            for row in &mut self.survey {
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut row.include, &row.path);
                                    if row.include {
                                        let name = format!("Kind of {}", row.path);
                                        let kind = egui::ComboBox::from_id_salt((
                                            "survey-kind",
                                            &row.path,
                                        ))
                                        .selected_text(match row.kind {
                                            Some(kind) => kind_label(kind),
                                            None => "Choose a kind",
                                        })
                                        .show_ui(ui, |ui| {
                                            for kind in
                                                [DocumentKind::Scenario, DocumentKind::Component]
                                            {
                                                ui.selectable_value(
                                                    &mut row.kind,
                                                    Some(kind),
                                                    kind_label(kind),
                                                );
                                            }
                                        });
                                        widgets::named(kind.response, name);
                                    }
                                });
                            }
                        });
                    widgets::hint(
                        ui,
                        "Registered files are not changed. A file whose first declaration is \
                         not recognised needs its kind chosen.",
                    );
                }
                ui.add_space(theme::SPACE_MD);
                let ready = !self.new_name.trim().is_empty()
                    && self
                        .survey
                        .iter()
                        .all(|row| !row.include || row.kind.is_some());
                if ui
                    .add_enabled(ready, widgets::primary_button("Initialise project"))
                    .clicked()
                {
                    self.initialise();
                }
            });
    }

    /// Writes a new project into the folder: only on this explicit request.
    fn initialise(&mut self) {
        let registrations: Vec<Registration> = self
            .survey
            .iter()
            .filter(|row| row.include)
            .filter_map(|row| {
                Some(Registration {
                    path: row.path.clone(),
                    kind: row.kind?,
                })
            })
            .collect();
        self.open_generation += 1;
        self.opening = true;
        let generation = self.open_generation;
        let root = self.root.clone();
        let name = self.new_name.trim().to_owned();
        self.storage_io(
            "Creating project…",
            move || {
                ProjectSession::initialise_with(&root, &name, &registrations, Arc::new(CardPlacer))
            },
            move |app, result| {
                if app.open_generation != generation {
                    return;
                }
                if app.persistence.closing {
                    app.opening = false;
                    return;
                }
                app.opening = false;
                match result {
                    Ok(Ok(session)) => app.install(session, None),
                    Ok(Err(error)) => app.open_error = Some(error.to_string()),
                    Err(error) => app.open_error = Some(error),
                }
            },
        );
    }

    /// Source replaced from outside the code view makes its text history
    /// obsolete: undoing it would type back text that no longer exists.
    fn follow_events(&mut self) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let replaced: Vec<String> = session
            .events_since(self.seen)
            .iter()
            .filter(|event| event.origin != CODE)
            .filter_map(|event| match &event.kind {
                EventKind::SourceChanged { .. } => event.document_id.clone(),
                EventKind::TextFileChanged { path, .. } => Some(text_tab(path)),
                _ => None,
            })
            .collect();
        self.seen = session.last_sequence();
        for document_id in replaced {
            code::clear_history(&self.ui_context, editor_id(&document_id));
            if self.shown.as_deref() == Some(document_id.as_str()) {
                self.code = code::CodeEditor::default();
            }
        }
    }

    /// Forgets selected blocks a reload took away; the others stay selected,
    /// by their identity.
    fn keep_existing_selections(&mut self) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        self.selections.retain(|document_id, selection| {
            let Some(model) = session
                .document(document_id)
                .and_then(|document| document.model())
            else {
                return false;
            };
            selection.retain(|id| {
                [false, true].into_iter().any(|cleanup| {
                    let graph = graph_of(model, cleanup);
                    graph.nodes.iter().any(|node| &node.id == id)
                        || graph.proxies.iter().any(|proxy| &proxy.id == id)
                })
            });
            true
        });
    }

    /// Reports what an external change did; a reloaded project may have lost
    /// documents, so their tabs close.
    fn follow_external(&mut self, outcome: ExternalOutcome) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let followed = !outcome.followed.is_empty();
        if outcome.project_reloaded {
            let gone: Vec<String> = self
                .workbench
                .tabs()
                .iter()
                .filter(|tab| {
                    session.document(&tab.document_id).is_none()
                        && tab.document_id.strip_prefix(TEXT_TAB).is_none_or(|path| {
                            session.text_file(path).is_none()
                                && !session.recovered_text_drafts().contains(&path)
                        })
                })
                .map(|tab| tab.document_id.clone())
                .collect();
            // A followed commit names what it deleted, so the person learns
            // why a tab closed (MCP-08a criterion 7).
            let deleted: Vec<String> = outcome
                .followed
                .iter()
                .filter(|change| change.kind == CommitChangeKind::Deleted)
                .filter(|change| {
                    change
                        .document_id
                        .as_ref()
                        .is_some_and(|id| gone.contains(id))
                })
                .map(|change| file_name(&change.path).to_owned())
                .collect();
            for document_id in gone {
                self.close_tab(&document_id);
            }
            // The person keeps their place: the document shown, its camera
            // and the blocks still there stay selected (MCP-08a).
            if self
                .shown
                .as_deref()
                .is_some_and(|shown| session_has(self.session.as_ref(), shown))
            {
                // Its text history is of a text that is no longer there.
                if let Some(shown) = &self.shown {
                    code::clear_history(&self.ui_context, editor_id(shown));
                }
            } else {
                self.shown = None;
            }
            self.keep_existing_selections();
            self.reload_field_drafts();
            self.restore_text_tabs();
            if !deleted.is_empty() {
                self.notify(
                    format!(
                        "Closed {}: deleted while this window was observing.",
                        deleted.join(", ")
                    ),
                    false,
                );
            }
        }
        if followed {
            self.last_change = describe_commit(&outcome.followed)
                .map(|text| format!("{} · {text}", chrono::Local::now().format("%H:%M")));
        }
        let name = |id: &String| {
            self.session
                .as_ref()
                .and_then(|session| session.document(id))
                .map(|document| file_name(document.entry().path.as_str()).to_owned())
                .unwrap_or_default()
        };
        if !outcome.conflicts.is_empty() {
            let names: Vec<String> = outcome.conflicts.iter().map(name).collect();
            self.notify(
                format!(
                    "Changed outside API Forge while unsaved here: {}. Saving them is paused.",
                    names.join(", ")
                ),
                true,
            );
        } else if !outcome.renamed.is_empty() {
            let names: Vec<String> = outcome.renamed.iter().map(name).collect();
            self.notify(
                format!("Followed a rename outside API Forge: {}.", names.join(", ")),
                false,
            );
        }
        if outcome.registry == Some(RegistryState::Current) && outcome.project_reloaded {
            self.notify(
                "The project file changed outside API Forge and was reloaded.",
                false,
            );
        }
    }

    // ----- Shortcuts -------------------------------------------------------------

    fn shortcuts(&mut self, context: &egui::Context) {
        if self.persistence.closing || self.storage.blocks_edits() {
            return;
        }
        let shortcut = |modifiers, key| {
            context
                .input_mut(|input| input.consume_shortcut(&KeyboardShortcut::new(modifiers, key)))
        };
        if shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::S) {
            self.save_all();
        } else if shortcut(Modifiers::COMMAND, Key::S) {
            self.save_active();
        }
        self.restore_report_on_escape(context);
        if shortcut(Modifiers::COMMAND, Key::E) {
            self.workbench.toggle();
            self.touched();
        }
        // Ctrl+B folds the block library wherever it has room (KTLO-17).
        if self.library_room(context) && shortcut(Modifiers::COMMAND, Key::B) {
            self.toggle_library();
        }
        if shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::Tab) {
            self.workbench.cycle(false);
            self.activation_generation += 1;
            self.touched();
        } else if shortcut(Modifiers::COMMAND, Key::Tab) {
            self.workbench.cycle(true);
            self.activation_generation += 1;
            self.touched();
        }
        if shortcut(Modifiers::COMMAND, Key::W)
            && let Some(active) = self.workbench.active().map(str::to_owned)
        {
            self.request_close_tab(&active);
        }
        // A text field with the keyboard (the code or an inspector field) walks
        // its own history; otherwise Ctrl+Z and Ctrl+Y walk the model's.
        if context.egui_wants_keyboard_input() {
            return;
        }
        let Some(active) = self.workbench.active().map(str::to_owned) else {
            return;
        };
        let redo = shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)
            || shortcut(Modifiers::COMMAND, Key::Y);
        // Check the longer shortcut first: Ctrl+Shift+Z also matches Ctrl+Z.
        let undo = !redo && shortcut(Modifiers::COMMAND, Key::Z);
        // Nothing is undone while observing: editing is paused (MCP-08a).
        if (undo || redo) && !self.observing() {
            self.step_model_history(&active, undo);
        }
        if self.workbench.mode == Mode::Code {
            return;
        }
        if shortcut(Modifiers::COMMAND, Key::D) {
            self.active_board_command(Command::Duplicate);
        }
        let (copy, paste) = context.input(|input| {
            (
                input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Copy)),
                input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Paste(_))),
            )
        });
        if copy {
            self.active_board_command(Command::Copy);
        }
        if paste {
            self.active_board_command(Command::Paste);
        }
    }

    /// A board command from the keyboard, applied to the active diagram.
    fn active_board_command(&mut self, command: Command) {
        let Some(document_id) = self.workbench.active().map(str::to_owned) else {
            return;
        };
        let Some(snapshot) = self.snapshot(&document_id) else {
            return;
        };
        let Some(model) = snapshot.model else {
            return;
        };
        let cleanup = self.cleanup(&document_id);
        self.board_command(&document_id, &model, cleanup, snapshot.editable, command);
    }

    /// The panel left of the board. In Split it is the block library (KTLO-05b),
    /// beside a document's board when the window is wide enough. In Diagram it
    /// has two tabs, *Library* and *Files* (KTLO-11), and is drawn whatever the
    /// active tab, so the project's files stay at hand beside a text file or an
    /// empty board. The blocks are disabled when the board is read-only.
    /// Collapsed, it is a rail with a button per tab that brings it back
    /// (KTLO-17).
    fn library_panel(&mut self, ui: &mut egui::Ui, active: Option<&str>) {
        let tabs = self.workbench.mode == Mode::Diagram;
        let board = active.and_then(|document_id| {
            let snapshot = self.snapshot(document_id)?;
            let model = snapshot.model.clone()?;
            Some((document_id, snapshot, model))
        });
        if board.is_none() && !tabs {
            return;
        }
        if self.workbench.panels.library_collapsed {
            // Folded to a rail (KTLO-17); the board takes the panel's width, and
            // the panel's own width is kept for when it comes back.
            egui::Panel::left("folder-library-rail")
                .resizable(false)
                .exact_size(LIBRARY_RAIL_WIDTH)
                .frame(
                    egui::Frame::new()
                        .fill(theme::PANEL)
                        .inner_margin(egui::Margin::symmetric(5, 10)),
                )
                .show(ui, |ui| {
                    let library = widgets::icon_button(Icon::Panel, "Show block library").compact();
                    if ui.add(library).clicked() {
                        self.expand_library(tabs.then_some(false));
                    }
                    let files = widgets::icon_button(Icon::Folder, "Show project files").compact();
                    if tabs && ui.add(files).clicked() {
                        self.expand_library(Some(true));
                    }
                });
            return;
        }
        let mut files = tabs && self.workbench.panels.library_files;
        // Diagram's panel is wider, for the explorer's names (KTLO-11). Split
        // keeps the library's range under its own name, so that showing it never
        // narrows the width Diagram comes back to.
        let (id, range) = if tabs {
            ("folder-library", 200.0..=420.0)
        } else {
            ("folder-split-library", 200.0..=320.0)
        };
        let project = self.context_project();
        let library = board.as_ref().map(|(document_id, snapshot, model)| {
            let cleanup = self.cleanup(document_id);
            let context = library_context(&project, document_id, model, cleanup, snapshot.editable);
            (context, cleanup)
        });
        let editable = board
            .as_ref()
            .is_some_and(|(_, snapshot, _)| snapshot.editable);
        let enabled = !self.persistence.closing && !self.storage.blocks_edits();
        let mut collapse = false;
        let mut chosen = files;
        let panel = egui::Panel::left(id)
            .default_size(self.workbench.panels.library)
            .size_range(range.clone())
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(12, 10)),
            )
            .show(ui, |ui| {
                // The chevron stays usable on a read-only board; the blocks do not.
                let chevron = |ui: &mut egui::Ui| {
                    let name = if files {
                        "Collapse project files"
                    } else {
                        "Collapse block library"
                    };
                    collapse = ui
                        .add(widgets::icon_button(Icon::ChevronLeft, name).compact())
                        .clicked();
                };
                if !tabs {
                    let (context, _) = library.as_ref().expect("Split draws a board's library");
                    return self
                        .library
                        .show_blocks_with(ui, context, chevron, editable);
                }
                // The caption row: the tabs, drawn as *Main | Cleanup* above the
                // board are, and the chevron.
                ui.horizontal(|ui| {
                    widgets::tab_value(ui, &mut chosen, false, "Library");
                    widgets::tab_value(ui, &mut chosen, true, "Files");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), chevron);
                });
                ui.add_space(theme::SPACE_SM);
                if files {
                    ui.add_enabled_ui(enabled, |ui| self.explorer_panel(ui));
                    return Vec::new();
                }
                match &library {
                    Some((context, _)) => self.library.show_sections(ui, context, editable),
                    None => {
                        widgets::hint(ui, "Open a scenario or component to add its blocks.");
                        Vec::new()
                    }
                }
            });
        // The panel's outer width, kept in the project's view state (KTLO-05c),
        // once it differs from the stored width this panel can show: a resize,
        // not Split's narrower range.
        let width = panel.response.rect.width();
        if (width
            - self
                .workbench
                .panels
                .library
                .clamp(*range.start(), *range.end()))
        .abs()
            > 0.5
        {
            self.workbench.panels.library = width;
        }
        widgets::name_resize_handle(
            ui.ctx(),
            egui::Id::new(id),
            if files {
                "Resize project files"
            } else {
                "Resize block library"
            },
        );
        if chosen != files {
            files = chosen;
            self.workbench.panels.library_files = files;
            self.touched();
        }
        if collapse {
            self.toggle_library();
        }
        let actions = panel.inner;
        if let Some(((document_id, snapshot, model), (_, cleanup))) = board.zip(library) {
            for action in actions {
                self.library_action(document_id, &model, cleanup, snapshot.editable, action);
            }
        }
    }

    /// Brings the panel back from its rail, on the tab its button names when
    /// `files` says one (Diagram); Split has no tabs (KTLO-11).
    fn expand_library(&mut self, files: Option<bool>) {
        if let Some(files) = files {
            self.workbench.panels.library_files = files;
        }
        self.toggle_library();
    }

    /// Folds the block library to its rail, or brings it back (KTLO-17).
    fn toggle_library(&mut self) {
        self.workbench.panels.library_collapsed = !self.workbench.panels.library_collapsed;
        self.touched();
    }

    /// Whether the window has room for the block library, as a panel or a rail,
    /// beside the board: always in Diagram, in Split only when wide enough.
    fn library_room(&self, context: &egui::Context) -> bool {
        match self.workbench.mode {
            Mode::Diagram => true,
            Mode::Split => context.content_rect().width() >= SPLIT_INSPECTOR_WIDTH,
            Mode::Code => false,
        }
    }

    /// Whether the block library panel is in view. When it is not, the board's
    /// *Add step* popup carries its sections (KTLO-05d) and the empty board's
    /// hint does not mention it (KTLO-05a criterion 9, KTLO-17).
    /// In Diagram the panel may show the project's files instead (KTLO-11).
    fn library_in_view(&self, context: &egui::Context) -> bool {
        self.library_room(context)
            && !self.workbench.panels.library_collapsed
            && !(self.workbench.mode == Mode::Diagram && self.workbench.panels.library_files)
    }

    /// Applies a block library action to the board: a clicked block or operator
    /// appears at the board's visible centre, selected (KTLO-05b).
    fn library_action(
        &mut self,
        document_id: &str,
        model: &Document,
        cleanup: bool,
        editable: bool,
        action: library::Action,
    ) {
        if !editable {
            return;
        }
        let centre = self
            .canvases
            .get(document_id)
            .map(Canvas::visible_center)
            .unwrap_or(Position { x: 100.0, y: 100.0 });
        // The folder panel shows only block sections, never workspace
        // navigation, so every action it sends has a board command.
        if let Some(command) = canvas::library_command(action, centre) {
            self.board_command(document_id, model, cleanup, editable, command);
        }
    }

    fn step_model_history(&mut self, document_id: &str, undo: bool) {
        self.component_action(document_id, publication::Action::History { undo });
    }

    fn request_close_tab(&mut self, document_id: &str) {
        let unsaved = match self.text_of(document_id) {
            Some(path) => self
                .session
                .as_ref()
                .and_then(|session| session.text_file(path))
                .is_some_and(apiforge_project::TextFileSession::needs_recovery),
            None => self
                .session
                .as_ref()
                .and_then(|session| session.document(document_id))
                .is_some_and(|document| document.needs_recovery()),
        };
        if unsaved {
            self.prompt = Some(Prompt::CloseTab(document_id.to_owned()));
        } else {
            self.close_tab(document_id);
        }
    }

    fn close_tab(&mut self, document_id: &str) {
        // Closing never drops unsaved text: the session refuses it until the
        // work is saved or reverted.
        if let Some(path) = self.text_of(document_id).map(str::to_owned)
            && let Some(session) = self.session.as_mut()
            && let Err(error) = session.close_text_file(&path)
        {
            self.notify(error.to_string(), true);
            return;
        }
        let active = self.workbench.active().map(str::to_owned);
        self.workbench.close(document_id);
        if self.workbench.active() != active.as_deref() {
            self.activation_generation += 1;
        }
        self.canvases.remove(document_id);
        self.selections.remove(document_id);
        if self.shown.as_deref() == Some(document_id) {
            self.shown = None;
        }
        self.touched();
    }

    // ----- Helpers ---------------------------------------------------------------

    /// Configuration and published components; views are drawn only while a
    /// project is open.
    fn context_project(&self) -> Arc<Project> {
        self.session
            .as_ref()
            .expect("an open project")
            .context()
            .clone()
    }

    fn cleanup(&self, document_id: &str) -> bool {
        self.workbench
            .tabs()
            .iter()
            .find(|tab| tab.document_id == document_id)
            .is_some_and(|tab| tab.cleanup)
    }
}

/// The code editor's widget identity: stable per document, so its text-edit
/// history survives mode and tab switches.
fn editor_id(document_id: &str) -> egui::Id {
    egui::Id::new(("folder-code", document_id))
}

fn text_tab(path: &str) -> String {
    format!("{TEXT_TAB}{path}")
}

fn context_key(document_id: &str, cleanup: bool) -> String {
    format!("{document_id}/{}", if cleanup { "cleanup" } else { "main" })
}

/// What the block library shows for one board: the library panel and, when
/// the panel is not in view, the board's *Add step* popup (KTLO-05d).
fn library_context<'a>(
    project: &'a Project,
    document_id: &'a str,
    model: &Document,
    cleanup: bool,
    editable: bool,
) -> library::Context<'a> {
    // Cleanup steps may use the main graph's results, as on the board.
    let main = cleanup.then(|| graph_of(model, false).nodes.iter());
    let variables = graph_of(model, cleanup)
        .nodes
        .iter()
        .chain(main.into_iter().flatten())
        .filter(|node| node.is_variable())
        .map(|node| (node.id.clone(), node.display_name().to_owned()))
        .collect();
    library::Context {
        project,
        projects: Vec::new(),
        scenario_id: document_id,
        variables,
        parameters: parameters_of(model),
        editing_component: matches!(*model, Document::Component(_)),
        running: false,
        document_locked: !editable,
        show_examples: false,
    }
}

fn graph_of(model: &Document, cleanup: bool) -> &Graph {
    match model {
        Document::Scenario(scenario) if cleanup => &scenario.cleanup,
        Document::Scenario(scenario) => &scenario.graph,
        Document::Component(component) => &component.graph,
    }
}

fn with_graph(model: &Document, cleanup: bool, graph: Graph) -> Document {
    let mut next = model.clone();
    match &mut next {
        Document::Scenario(scenario) if cleanup => scenario.cleanup = graph,
        Document::Scenario(scenario) => scenario.graph = graph,
        Document::Component(component) => component.graph = graph,
    }
    next
}

/// Scenario parameters, or the arguments of a component.
fn parameters_of(model: &Document) -> Vec<String> {
    match model {
        Document::Scenario(scenario) => scenario.parameters.keys().cloned().collect(),
        Document::Component(component) => component
            .inputs
            .iter()
            .map(|port| port.name.clone())
            .collect(),
    }
}

fn kind_label(kind: DocumentKind) -> &'static str {
    match kind {
        DocumentKind::Scenario => "Scenario",
        DocumentKind::Component => "Component",
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn count(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// One-based line and column of a byte offset, counting characters.
fn line_column(text: &str, offset: usize) -> (usize, usize) {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let before = &text[..offset];
    let line = before.matches('\n').count() + 1;
    let column = before
        .rsplit('\n')
        .next()
        .map_or(0, |line| line.chars().count())
        + 1;
    (line, column)
}

/// A kept selection that no longer fits the text moves to its end.
fn clamp_selection(text: &str, selection: &TextSelection) -> TextSelection {
    let clamp = |offset: usize| {
        let mut offset = offset.min(text.len());
        while !text.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    };
    TextSelection {
        anchor: clamp(selection.anchor),
        caret: clamp(selection.caret),
    }
}

fn centred_hint(ui: &mut egui::Ui, text: &str) {
    ui.centered_and_justified(|ui| widgets::hint(ui, text));
}

/// Names a painted decoration so that a test can measure where it is, and
/// hides it from assistive technology: a sleeping dog says nothing a screen
/// reader needs, and KTLO-20 asks that only the hint be announced. AccessKit's
/// `hidden` is what `aria-hidden` is to a decorative picture.
fn name_decoration(ui: &mut egui::Ui, rect: egui::Rect, name: &'static str) {
    let response = ui.allocate_rect(rect, egui::Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Image, true, name));
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_hidden());
}

/// The hint under the sleeping dog, the pair centred in the board area
/// (KTLO-20). The drawing is decoration, named so that a screen reader says
/// what the empty board shows and a test can measure it. A board with no room
/// for the pair keeps the hint alone, as the board's other hints are, and says
/// so: a board that draws no dog has no loop to keep going.
fn hint_with_dog(ui: &mut egui::Ui, text: &str, motion: dog::Motion) -> bool {
    let area = ui.available_rect_before_wrap();
    let gap = theme::SPACE_MD;
    // The hint's own height, so that the pair is centred on the board's centre
    // whatever the text and the display scale do to it.
    let line = ui
        .painter()
        .layout_no_wrap(
            text.to_owned(),
            egui::FontId::proportional(theme::FONT_LABEL),
            theme::MUTED,
        )
        .size()
        .y;
    let size = dog::size_for(ui.ctx().content_rect().height());
    let room = egui::vec2(area.width() - 2.0 * gap, area.height() - 3.0 * gap - line);
    let fit = (room.x / size.x).min(room.y / size.y).min(1.0);
    if fit <= 0.0 {
        centred_hint(ui, text);
        return false;
    }
    // The board is named whether a document fills it or not.
    name_decoration(ui, area, "Diagram board");
    let size = size * fit;
    let top = area.center().y - (size.y + gap + line) / 2.0;
    let drawing =
        egui::Rect::from_center_size(egui::pos2(area.center().x, top + size.y / 2.0), size);
    dog::paint(ui.painter(), drawing, motion);
    name_decoration(ui, drawing, "Sleeping dog");
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(egui::Rect::from_min_size(
                egui::pos2(area.left(), top + size.y + gap),
                egui::vec2(area.width(), line),
            ))
            .layout(egui::Layout::top_down(egui::Align::Center)),
        |ui| widgets::hint(ui, text),
    );
    true
}

fn quoted(name: &str) -> String {
    serde_json::to_string(name).expect("a string serialises")
}

fn scenario_template(name: &str) -> String {
    format!("scenario {} {{\n}}\n", quoted(name))
}

fn component_template(name: &str) -> String {
    format!(
        "component {} {} {{\n}}\n",
        component_handle(name),
        quoted(name)
    )
}

/// A component's name in code: the name's words in UpperCamelCase.
fn component_handle(name: &str) -> String {
    let handle: String = name
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut characters = word.chars();
            let first = characters.next().expect("non-empty").to_ascii_uppercase();
            std::iter::once(first).chain(characters).collect::<String>()
        })
        .collect();
    match handle.chars().next() {
        Some(first) if first.is_ascii_alphabetic() => handle,
        Some(_) => format!("Component{handle}"),
        None => "Component".into(),
    }
}

/// A file name from a display name: lower-case words joined by hyphens.
fn file_stem(name: &str) -> String {
    let stem = name
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join("-");
    if stem.is_empty() {
        "untitled".into()
    } else {
        stem
    }
}

#[cfg(test)]
mod tests;
