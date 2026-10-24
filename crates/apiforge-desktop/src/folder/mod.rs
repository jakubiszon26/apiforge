//! The folder project window (ADR 0012): Code, Diagram and Split over one
//! `ProjectSession`.
//!
//! The session owns every document's source, model and model history. This
//! window owns only views: tabs, each document's text-edit history (kept by its
//! stable editor identity), caret, scroll, board selection and diagram camera.
//! Switching a mode or a tab therefore never saves, closes or resets a document.
//! Folder projects are opened explicitly with `--folder`; the normal startup
//! path still opens the workspace.

mod analysis;
mod chrome;
mod code_view;
mod diagram;
mod explorer;
mod file_prompts;
mod placement;
mod publication;
mod runs;
mod text_view;
mod watcher;
mod workbench;

use crate::icons::Icon;
use crate::{
    board_edit::{Board, Edit},
    canvas::{self, Canvas, Command},
    code, dialogs,
    inspector::{self, Inspector},
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
    AnalysisState, DeletePreview, Document, DocumentKind, EventKind, ExternalConflict,
    ExternalOutcome, ModelState, Origin, ProjectSession, Registration, RegistryState, Unavailable,
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
use workbench::{Mode, Workbench};

/// Edits typed into the code view; the view recognises its own changes.
const CODE: Origin = Origin(1);
/// Graphical edits, undo and redo of the model, and reverts.
const DIAGRAM: Origin = Origin(2);
/// Unsaved work is written to local recovery after this much quiet.
const RECOVERY_DELAY: Duration = Duration::from_secs(1);
/// Split keeps the inspector beside code and diagram only on wide windows.
const SPLIT_INSPECTOR_WIDTH: f32 = 1440.0;
/// A tab shows a registered document, by its ID, or a project text file, by
/// its path behind this prefix. Documents are resolved first, so a tab key
/// cannot shadow one.
const TEXT_TAB: &str = "text:";

enum Prompt {
    Run {
        prepared: Box<apiforge_project::PreparedRun>,
        live: bool,
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
    analyser: Analyser,
    watcher: Watcher,
    explorer: ExplorerState,
    /// File dialogs running on their own threads.
    picked: Vec<Receiver<Picked>>,
    /// The last session event this window has followed.
    seen: u64,
    code: code::CodeEditor,
    /// The document the code editor last showed; its caret is restored on entry.
    shown: Option<String>,
    scroll_restore: Option<f32>,
    /// The block whose statement the code view last revealed.
    revealed: Option<String>,
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
}

struct Clipboard {
    context: String,
    fragment: Graph,
    pastes: u32,
}

/// A frame's copy of what one document shows, so views can change the window
/// while they read it.
struct Snapshot {
    path: String,
    kind: DocumentKind,
    text: String,
    diagnostics: Vec<Diagnostic>,
    analysis: AnalysisState,
    model: Option<Document>,
    model_state: ModelState,
    unavailable: Option<Unavailable>,
    editable: bool,
    held: bool,
    dirty: bool,
    layout_dirty: bool,
    conflict: Option<ExternalConflict>,
}

impl FolderApp {
    pub fn open(context: &egui::Context, root: PathBuf) -> Self {
        theme::apply(context);
        let mut app = Self {
            ui_context: context.clone(),
            root,
            session: None,
            uninitialised: false,
            survey: Vec::new(),
            new_name: String::new(),
            open_error: None,
            workbench: Workbench::restore(None, |_| false),
            saved_view: None,
            analyser: Analyser::new(Some(context.clone())),
            watcher: Watcher::new(Some(context.clone())),
            explorer: ExplorerState::default(),
            picked: Vec::new(),
            seen: 0,
            code: code::CodeEditor::default(),
            shown: None,
            scroll_restore: None,
            revealed: None,
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
            preferences: Preferences::load(),
            runs: runs::Runs::default(),
        };
        match ProjectSession::open_with(&app.root, Arc::new(CardPlacer)) {
            Ok(session) => app.install(session),
            Err(apiforge_project::Error::NotAProject) => {
                app.uninitialised = true;
                match ProjectSession::survey(&app.root) {
                    Ok(surveys) => {
                        app.survey = surveys
                            .into_iter()
                            .map(|survey| SurveyRow {
                                path: survey.path,
                                include: true,
                                kind: survey.declaration.map(|declaration| declaration.kind),
                            })
                            .collect()
                    }
                    Err(error) => app.open_error = Some(error.to_string()),
                }
                app.new_name = app
                    .root
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
            }
            Err(error) => app.open_error = Some(error.to_string()),
        }
        app
    }

    fn install(&mut self, session: ProjectSession) {
        let project_id = session.manifest().id.clone();
        let state = session
            .store()
            .load_workspace_state(&project_id)
            .ok()
            .flatten();
        let first_open = state.is_none();
        self.workbench = Workbench::restore(state.clone(), |id| {
            session.document(id).is_some() || id.starts_with(TEXT_TAB)
        });
        self.saved_view = state;
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
                self.workbench.open(document.id());
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
        if session.is_read_only() {
            self.notice = Some(Notice {
                text: "Another window is editing this project. This window is read-only: \
                       changes cannot be saved."
                    .into(),
                error: true,
            });
        }
        self.seen = session.last_sequence();
        self.uninitialised = false;
        self.session = Some(session);
        self.open_run_journal();
        self.reload_field_drafts();
        self.restore_text_tabs();
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

    pub fn has_document(&self) -> bool {
        self.session.is_some()
    }

    fn new_canvas(&self) -> Canvas {
        let mut canvas = Canvas::default();
        canvas.wire_style = self.preferences.wire_style;
        canvas.bridges = self.preferences.bridges;
        canvas
    }

    fn snapshot(&self, document_id: &str) -> Option<Snapshot> {
        let session = self.session.as_ref()?;
        let document = session.document(document_id)?;
        let held = session
            .recovered_drafts()
            .iter()
            .any(|(_, draft)| draft.document_id == document_id);
        Some(Snapshot {
            path: document.entry().path.as_str().to_owned(),
            kind: document.entry().kind,
            text: document.text().to_owned(),
            diagnostics: document.diagnostics().to_vec(),
            analysis: document.analysis(),
            model: document.model().cloned(),
            model_state: document.model_state(),
            unavailable: document.unavailable(),
            editable: document.accepts_model_edits() && !held,
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

    // ----- Frame -----------------------------------------------------------------

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.poll_run();
        if self.close_requested {
            self.close_requested = false;
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let Some(session) = self.session.as_mut() else {
            self.start_screen(ui);
            return;
        };
        let now = Instant::now();
        if let Some(wait) = self.analyser.poll(session, now) {
            ui.ctx().request_repaint_after(wait);
        }
        self.watcher
            .focus(ui.input(|input| input.viewport().focused.unwrap_or(true)));
        let (outcome, wait) = self.watcher.poll(session, now);
        ui.ctx().request_repaint_after(wait);
        self.follow_events();
        if let Some(outcome) = outcome {
            self.follow_external(outcome);
        }
        self.follow_dialogs();
        if self.prompt.is_none() {
            self.shortcuts(ui.ctx());
        }

        egui::Panel::top("folder-header")
            .frame(
                egui::Frame::new()
                    .fill(theme::WHITE)
                    .inner_margin(egui::Margin::symmetric(16, 8)),
            )
            .show(ui, |ui| self.header(ui));
        egui::Panel::top("folder-tabs")
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(8, 4)),
            )
            .show(ui, |ui| {
                self.tab_strip(ui);
                self.notices(ui);
            });
        let active = self.workbench.active().map(str::to_owned);
        self.run_report(ui, active.as_deref());
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
                        self.explorer_panel(ui);
                    });
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(theme::WHITE))
                    .show(ui, |ui| self.main_code_area(ui, active.as_deref()));
            }
            Mode::Diagram => {
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
                if width >= SPLIT_INSPECTOR_WIDTH {
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
                ui.label(
                    RichText::new("Open a folder project")
                        .size(theme::FONT_TITLE)
                        .family(theme::bold_family()),
                );
                ui.add_space(theme::SPACE_SM);
                widgets::hint(ui, self.root.display().to_string());
                ui.add_space(theme::SPACE_LG);
                if let Some(error) = &self.open_error {
                    widgets::error_box(ui, std::slice::from_ref(error));
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
                widgets::field_label(ui, "Project name");
                ui.add(egui::TextEdit::singleline(&mut self.new_name).desired_width(320.0));
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
                                        egui::ComboBox::from_id_salt(("survey-kind", &row.path))
                                            .selected_text(match row.kind {
                                                Some(kind) => kind_label(kind),
                                                None => "Choose a kind",
                                            })
                                            .show_ui(ui, |ui| {
                                                for kind in [
                                                    DocumentKind::Scenario,
                                                    DocumentKind::Component,
                                                ] {
                                                    ui.selectable_value(
                                                        &mut row.kind,
                                                        Some(kind),
                                                        kind_label(kind),
                                                    );
                                                }
                                            });
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
        match ProjectSession::initialise_with(
            &self.root,
            self.new_name.trim(),
            &registrations,
            Arc::new(CardPlacer),
        ) {
            Ok(session) => self.install(session),
            Err(error) => self.open_error = Some(error.to_string()),
        }
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

    /// Reports what an external change did; a reloaded project may have lost
    /// documents, so their tabs close.
    fn follow_external(&mut self, outcome: ExternalOutcome) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        if outcome.project_reloaded {
            let gone: Vec<String> = self
                .workbench
                .tabs()
                .iter()
                .filter(|tab| session.document(&tab.document_id).is_none())
                .map(|tab| tab.document_id.clone())
                .collect();
            for document_id in gone {
                self.close_tab(&document_id);
            }
            self.shown = None;
            self.reload_field_drafts();
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
        let shortcut = |modifiers, key| {
            context
                .input_mut(|input| input.consume_shortcut(&KeyboardShortcut::new(modifiers, key)))
        };
        if shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::S) {
            self.save_all();
        } else if shortcut(Modifiers::COMMAND, Key::S) {
            self.save_active();
        }
        if shortcut(Modifiers::COMMAND, Key::E) {
            self.workbench.toggle();
            self.touched();
        }
        if shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::Tab) {
            self.workbench.cycle(false);
            self.touched();
        } else if shortcut(Modifiers::COMMAND, Key::Tab) {
            self.workbench.cycle(true);
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
        if undo || redo {
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

    fn step_model_history(&mut self, document_id: &str, undo: bool) {
        self.component_action(document_id, publication::Action::History { undo });
    }

    // ----- Saving ----------------------------------------------------------------

    /// Ctrl+S: the active document's exact text, errors included, plus dirty
    /// configuration. Without an active document, configuration alone.
    fn save_active(&mut self) {
        match self.workbench.active().map(str::to_owned) {
            Some(active) => {
                if let Err(message) = self.save_document(&active) {
                    self.notify(message, true);
                }
            }
            None => self.save_configuration(),
        }
    }

    fn save_document(&mut self, document_id: &str) -> Result<(), String> {
        if let Some(path) = self.text_of(document_id).map(str::to_owned) {
            return self.save_text(&path);
        }
        let session = self.session.as_mut().ok_or_else(String::new)?;
        let name = session
            .document(document_id)
            .map(|document| file_name(document.entry().path.as_str()).to_owned())
            .unwrap_or_default();
        match session.save(document_id) {
            Ok(_) => {
                self.touched();
                Ok(())
            }
            Err(error) => Err(format!("Could not save {name}: {error}")),
        }
    }

    fn save_configuration(&mut self) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        if let Err(error) = session.save_configuration() {
            self.notify(
                format!("Could not save the project settings: {error}"),
                true,
            );
        }
    }

    /// Ctrl+Shift+S: each document with unsaved work in its own transaction,
    /// then configuration. Every failure is reported; none stops the others.
    fn save_all(&mut self) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let mut pending: Vec<String> = session
            .documents()
            .iter()
            .filter(|document| document.is_dirty() || document.is_layout_dirty())
            .map(|document| document.id().to_owned())
            .collect();
        pending.extend(
            session
                .text_files()
                .iter()
                .filter(|file| file.is_dirty())
                .map(|file| text_tab(file.path())),
        );
        let mut failures = Vec::new();
        for document_id in &pending {
            if let Err(message) = self.save_document(document_id) {
                failures.push(message);
            }
        }
        if self
            .session
            .as_ref()
            .is_some_and(ProjectSession::is_manifest_dirty)
        {
            self.save_configuration();
        }
        if failures.is_empty() {
            if !pending.is_empty() {
                let count = pending.len();
                let noun = if count == 1 { "file" } else { "files" };
                self.notify(format!("Saved {count} {noun}."), false);
            }
        } else {
            self.notify(failures.join("\n"), true);
        }
    }

    /// Writes unsaved work to local recovery after a pause, and the view state
    /// with it. Neither ever saves a source file.
    fn protect(&mut self, now: Instant) {
        let Some(changed) = self.changed_at else {
            return;
        };
        let due = changed + RECOVERY_DELAY;
        if now < due {
            self.ui_context.request_repaint_after(due - now);
            return;
        }
        self.changed_at = None;
        let Some(session) = self.session.as_mut() else {
            return;
        };
        if session.is_read_only() {
            return;
        }
        let failure = session.save_recovery().err();
        let _ = session.save_last_valid();
        if let Some(error) = failure {
            self.notify(format!("Unsaved work is not protected: {error}"), true);
        }
        self.save_view();
    }

    fn save_view(&mut self) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        if session.is_read_only() {
            return;
        }
        let ids: Vec<String> = self
            .workbench
            .tabs()
            .iter()
            .map(|tab| tab.document_id.clone())
            .collect();
        for document_id in ids {
            let camera = self
                .canvases
                .get(&document_id)
                .and_then(Canvas::camera)
                .map(|(x, y, zoom)| CameraState { x, y, zoom });
            if let Some(tab) = self.workbench.tab_mut(&document_id)
                && camera.is_some()
            {
                tab.camera = camera;
            }
        }
        let state = self.workbench.state(&session.manifest().id);
        if self.saved_view.as_ref() != Some(&state)
            && session.store().save_workspace_state(&state).is_ok()
        {
            self.saved_view = Some(state);
        }
    }

    // ----- Closing ---------------------------------------------------------------

    /// Closing succeeds once unsaved work is in local recovery; it never saves
    /// a source file. A failure keeps the window open with the reason.
    pub fn prepare_close(&mut self) -> bool {
        self.poll_run();
        if self.runs.handle.is_some() {
            if let Some(handle) = &self.runs.handle {
                handle.cancel();
            }
            return false;
        }
        self.save_view();
        if let Some(session) = self.session.as_mut()
            && !session.is_read_only()
        {
            let _ = session.save_last_valid();
        }
        let Some(session) = self.session.take() else {
            return true;
        };
        match session.close() {
            Ok(()) => {
                self.runs = runs::Runs::default();
                true
            }
            Err((session, error)) => {
                self.session = Some(*session);
                self.close_error = Some(error.to_string());
                false
            }
        }
    }

    pub fn close_error(&mut self) -> Option<String> {
        self.close_error.take()
    }

    pub fn abort_close(&mut self, error: String) {
        self.prompt = Some(Prompt::CloseFailed(error));
    }

    pub fn save_pending(&self) -> bool {
        self.runs.handle.is_some()
            || self
                .runs
                .runner
                .as_ref()
                .is_some_and(|runner| runner.active_runs() > 0)
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
        self.workbench.close(document_id);
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
