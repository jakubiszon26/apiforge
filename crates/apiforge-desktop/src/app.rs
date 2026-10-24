//! The application window: layout, global shortcuts and applying user actions.

use crate::icons::Icon;
use crate::{
    buffers,
    canvas::{self, Canvas, Command},
    code,
    dialogs::{self, Outcome},
    editor::{BlockClipboard, Editor},
    files,
    inspector::{self, Inspector},
    library::{self, Library},
    navigator::Navigator,
    persistence::{SaveStatus, Saver},
    preferences::Preferences,
    report::{self, ReportPanel},
    theme, widgets,
};
use apiforge_core::{
    block::Block,
    edit,
    format::{Binding, Graph, Position, Project, Workspace},
    graph::{create_operator_node, delete_nodes, position_after},
    journal::{CleanupState, JournalRun},
    run::{Phase, RunMode, RunResult, RunStatus, StepResult},
};
use apiforge_runner::{RunHandle, RunRequest, RunnerService};
use apiforge_storage::{JournalStore, WorkspaceStore};
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, RichText};
use std::{collections::HashSet, sync::Arc, time::Instant};

/// Where the workspace comes from and whether edits are saved.
pub enum Source {
    /// The user's workspace in the data directory, saved automatically.
    Storage(WorkspaceStore),
    /// A document opened for this session only (`--project`), never written back.
    Session(Workspace),
}

enum Dialog {
    NewProject(String),
    NewScenario(String),
    Environment(dialogs::EnvironmentDialog),
    Settings(dialogs::SettingsDialog),
    AssignVariable(dialogs::AssignVariableDialog),
    Component(Box<dialogs::ComponentDialog>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunKind {
    Full,
    UntilSelected,
    SingleSelected,
}

#[derive(Clone, Debug)]
enum StoragePrompt {
    Recovery(String),
    Conflict(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutMode {
    Standard,
    Compact,
}

pub fn layout_mode(available: egui::Vec2) -> LayoutMode {
    if available.x < 1080.0 || available.y < 660.0 {
        LayoutMode::Compact
    } else {
        LayoutMode::Standard
    }
}

pub struct App {
    editor: Editor,
    saver: Option<Saver>,
    saved_revision: u64,
    canvas: Canvas,
    library: Library,
    navigator: Navigator,
    inspector: Inspector,
    inspector_collapsed: bool,
    report: ReportPanel,
    runner: Option<RunnerService>,
    run_handle: Option<RunHandle>,
    run: Option<RunResult>,
    run_fingerprint: Option<Project>,
    notice: Option<String>,
    mode: RunMode,
    secrets: dialogs::SessionSecrets,
    dialog: Option<Dialog>,
    files: files::Files,
    closing: bool,
    storage_message: Option<String>,
    storage_prompt: Option<StoragePrompt>,
    storage_busy: bool,
    /// Keeps the live-run journal of this session; `None` when there is nowhere to
    /// write one, in which case an interrupted live run leaves no record.
    journal: Option<Arc<JournalStore>>,
    /// Live runs a previous process left unfinished, until the user has seen them.
    interrupted: Option<Vec<JournalRun>>,
    /// A run waiting for an answer to "the code has errors; run the last valid
    /// version?", and the answer once it is given.
    run_prompt: Option<RunKind>,
    run_anyway: Option<RunKind>,
    preferences: Preferences,
    /// Writes code drafts beside the workspace; `None` without a writable one.
    store: Option<WorkspaceStore>,
    /// What was last written as drafts, so an unchanged set is not rewritten.
    saved_drafts: Vec<(buffers::Context, String)>,
    /// The text of every context the user has edited as code.
    buffers: buffers::Buffers,
    code: code::CodeEditor,
    /// Whether the code panel is open beside the board.
    code_open: bool,
    /// The block whose statement the panel last revealed, so a selection that
    /// has not changed does not keep pulling the caret back.
    revealed: Option<String>,
}

impl App {
    pub fn new(context: &egui::Context, source: Source) -> Self {
        theme::apply(context);
        let mut notice = None;
        let mut storage_prompt = None;
        let mut storage_message = None;
        let mut journal: Option<Arc<JournalStore>> = None;
        let mut interrupted = None;
        let mut store_handle: Option<WorkspaceStore> = None;
        let mut drafts: Option<apiforge_storage::Drafts> = None;
        let (workspace, saver) = match source {
            Source::Session(workspace) => (workspace, None),
            Source::Storage(store) => {
                let read_only = store.is_read_only();
                if read_only {
                    storage_message = Some("This workspace is open in another API Forge window. Changes here are not saved.".into());
                }
                let (workspace, writable) = match store.load() {
                    Ok(Some(loaded)) => {
                        if let Some(reason) = loaded.recovered {
                            if read_only {
                                notice = Some("A workspace backup is available. Close the other API Forge window and reopen this one to restore it.".into());
                            } else {
                                storage_prompt = Some(StoragePrompt::Recovery(reason));
                            }
                            (default_workspace(), !read_only)
                        } else {
                            (loaded.workspace, !read_only)
                        }
                    }
                    Ok(None) => (default_workspace(), !read_only),
                    Err(apiforge_storage::StorageError::Document(
                        apiforge_storage::DocumentError::Invalid {
                            source: apiforge_core::ImportError::LegacyVersion,
                            ..
                        },
                    )) => match store.archive_legacy_workspace() {
                        Ok(true) => {
                            notice = Some(
                                "Your format version 1 workspace and any backup were preserved as workspace.v1.json and workspace.v1.json.bak. Convert the archived workspace with apiforge-convert-v1 to import it. A new demo workspace was opened.".into(),
                            );
                            (default_workspace(), true)
                        }
                        result => {
                            let reason = match result {
                                Err(error) => error.to_string(),
                                _ => "The workspace changed while it was being archived".into(),
                            };
                            notice = Some(format!(
                                "{reason}. A temporary workspace was opened with autosave disabled to protect the existing files. Use Export to keep changes from this session."
                            ));
                            (default_workspace(), false)
                        }
                    },
                    Err(error) => {
                        notice = Some(format!(
                            "{error}. A temporary workspace was opened with autosave disabled to protect the existing files. Use Export to keep changes from this session."
                        ));
                        (default_workspace(), false)
                    }
                };
                if !writable && storage_message.is_none() {
                    storage_message = Some("Temporary workspace — autosave is disabled. Export to keep changes from this session.".into());
                }
                // The journal and the drafts share the session's writer lock, so
                // both are opened from a clone before the saver takes the store.
                if writable {
                    store_handle = Some(store.clone());
                    match store.load_drafts() {
                        Ok(Some(loaded)) => drafts = Some(loaded.drafts),
                        Ok(None) => {}
                        Err(error) if notice.is_none() => {
                            notice = Some(format!("{error} Code drafts were not restored."));
                        }
                        Err(_) => {}
                    }
                }
                if writable {
                    match JournalStore::open(store.clone()) {
                        Ok(loaded) => {
                            if loaded.unreadable && notice.is_none() {
                                notice = Some("A previous live run left a run journal that could not be read. It was preserved as journal.unreadable.json.".into());
                            }
                            if !loaded.interrupted.is_empty() {
                                interrupted = Some(loaded.interrupted);
                            }
                            journal = Some(Arc::new(loaded.store));
                        }
                        Err(error) if notice.is_none() => {
                            notice = Some(format!(
                                "{error} Live runs in this session are not recorded, so an interrupted run leaves no record of what it sent."
                            ));
                        }
                        Err(_) => {}
                    }
                }
                let repaint = context.clone();
                let saver = writable
                    .then(|| {
                        Saver::spawn(store, move || repaint.request_repaint())
                            .map_err(|error| notice = Some(format!("Cannot start saving: {error}")))
                            .ok()
                    })
                    .flatten();
                (workspace, saver)
            }
        };
        let preferences = Preferences::load();
        let store = store_handle;
        let mut canvas = Canvas::default();
        canvas.wire_style = preferences.wire_style;
        canvas.bridges = preferences.bridges;
        let repaint = context.clone();
        let runner = RunnerService::new(Arc::new(move || repaint.request_repaint()))
            .map_err(|error| {
                if notice.is_none() {
                    notice = Some(format!("Cannot start the runner: {error}"));
                }
            })
            .ok()
            .map(|runner| match &journal {
                Some(journal) => runner.with_journal(Arc::clone(journal) as _),
                None => runner,
            });
        let editor = Editor::new(workspace);
        let buffers = restore_drafts(&editor, drafts.take());
        Self {
            editor,
            saver,
            saved_revision: 0,
            canvas,
            library: Library::default(),
            navigator: Navigator::default(),
            inspector: Inspector::default(),
            inspector_collapsed: false,
            report: ReportPanel::default(),
            runner,
            run_handle: None,
            run: None,
            run_fingerprint: None,
            notice,
            mode: RunMode::Simulation,
            secrets: dialogs::SessionSecrets::new(),
            dialog: None,
            files: files::Files::new(context.clone()),
            closing: false,
            storage_message,
            storage_prompt,
            storage_busy: false,
            journal,
            interrupted,
            run_prompt: None,
            run_anyway: None,
            code_open: preferences.code_panel,
            preferences,
            store,
            saved_drafts: Vec::new(),
            buffers,
            code: code::CodeEditor::default(),
            revealed: None,
        }
    }

    /// Selects a node of the current board by id, handle or label (used by screenshots).
    pub fn select_node(&mut self, key: &str) {
        if let Some(node) =
            self.editor.graph().nodes.iter().find(|node| {
                node.id == key || node.handle == key || node.label.as_deref() == Some(key)
            })
        {
            self.editor.selection = vec![node.id.clone()];
        }
    }

    pub fn has_document(&self) -> bool {
        !self.files.busy()
    }

    /// Requests cancellation once, then keeps polling until cleanup finishes.
    /// The window owns the overall close deadline; this method never waits.
    pub fn prepare_close(&mut self) -> bool {
        self.poll_storage();
        if self.storage_prompt.is_some() || self.storage_busy {
            return false;
        }
        if !self.closing {
            self.save_if_changed();
            self.closing = true;
            self.cancel_run();
            if let Some(saver) = &self.saver {
                saver.flush();
            }
            if self.run_handle.is_some() {
                self.notice = Some("Closing after the active test finishes cleanup…".into());
            }
        }
        self.poll_run();
        self.run_handle.is_none()
            && !self.files.busy()
            && !self.save_pending()
            && self.close_save_error().is_none()
            && self
                .runner
                .as_ref()
                .is_none_or(|runner| runner.active_runs() == 0)
    }

    pub fn save_pending(&self) -> bool {
        self.saver
            .as_ref()
            .is_some_and(|saver| saver.status() == SaveStatus::Pending)
    }

    pub fn close_save_error(&self) -> Option<String> {
        if self.storage_prompt.is_some() {
            return Some(
                "Resolve the workspace file change or choose a temporary session before closing."
                    .into(),
            );
        }
        if !self.closing {
            return None;
        }
        match self.saver.as_ref().map(Saver::status) {
            Some(SaveStatus::Failed(error) | SaveStatus::Conflict(error)) => Some(error),
            _ => None,
        }
    }

    pub fn abort_close_after_save_error(&mut self, error: String) {
        self.closing = false;
        if self.storage_prompt.is_some() {
            self.notice = Some(error);
            return;
        }
        self.notice = Some(format!(
            "Your latest changes could not be saved: {error}. The window is staying open. Fix the storage problem and choose Retry save."
        ));
    }

    fn retry_save(&mut self) {
        if self.storage_prompt.is_some() || self.storage_busy {
            return;
        }
        if let Some(saver) = &self.saver {
            saver.save(self.editor.workspace());
            saver.flush();
            self.notice = None;
        }
    }

    fn poll_storage(&mut self) {
        let loaded = self.saver.as_ref().and_then(Saver::take_loaded);
        if let Some(result) = loaded {
            self.storage_busy = false;
            match result {
                Ok(Some(loaded)) if loaded.recovered.is_some() => {
                    self.storage_prompt = Some(StoragePrompt::Recovery(loaded.recovered.unwrap()));
                }
                Ok(loaded) => {
                    self.clear_run();
                    self.editor =
                        Editor::new(loaded.map_or_else(default_workspace, |l| l.workspace));
                    self.canvas = Canvas::default();
                    self.inspector = Inspector::default();
                    self.inspector_collapsed = false;
                    self.navigator = Navigator::default();
                    self.saved_revision = self.editor.revision();
                    self.storage_prompt = None;
                    self.storage_message = None;
                    self.notice = Some("Workspace reloaded from disk.".into());
                    self.dialog = None;
                    self.secrets = dialogs::SessionSecrets::new();
                }
                Err(message) => self.storage_prompt = Some(StoragePrompt::Conflict(message)),
            }
        }
        if self.storage_prompt.is_none()
            && !self.storage_busy
            && let Some(SaveStatus::Conflict(message)) = self.saver.as_ref().map(Saver::status)
        {
            self.storage_message = Some(
                "Saving is paused because the workspace file changed outside this window.".into(),
            );
            self.storage_prompt = Some(StoragePrompt::Conflict(message));
        }
    }

    fn reload_storage(&mut self, restore: bool) {
        if self.storage_busy || self.is_running() {
            return;
        }
        if let Some(saver) = &self.saver {
            self.storage_busy = true;
            self.notice = None;
            saver.reload(restore);
        }
    }

    fn use_temporary_workspace(&mut self) {
        if self.storage_busy {
            return;
        }
        self.saver = None;
        self.storage_prompt = None;
        self.closing = false;
        self.storage_message = Some(
            "Temporary workspace — changes here are not saved. Export to keep your work.".into(),
        );
    }

    fn storage_dialog(&mut self, context: &egui::Context) {
        let Some(prompt) = self.storage_prompt.clone() else {
            return;
        };
        let recovery = matches!(prompt, StoragePrompt::Recovery(_));
        let (StoragePrompt::Recovery(message) | StoragePrompt::Conflict(message)) = prompt;
        let mut reload = false;
        let mut temporary = false;
        let response = dialogs::centered_modal(egui::Id::new("storage-decision"), context)
            .show(context, |ui| {
                ui.set_width((context.content_rect().width() - 80.0).clamp(320.0, 520.0));
                ui.heading(if recovery { "Restore workspace backup?" } else { "The workspace file changed outside this window" });
                ui.add_space(8.0);
                ui.label(message);
                ui.label(if recovery {
                    "The original file is untouched. Restore the last valid backup, or open a temporary workspace."
                } else {
                    "Saving has stopped. Reloading discards your local changes. You can keep editing in a temporary session and export your work."
                });
                ui.add_space(12.0);
                if self.storage_busy {
                    ui.spinner();
                    ui.label("Reading workspace…");
                } else {
                    reload = ui.add_enabled(self.saver.is_some() && !self.is_running(),
                        widgets::primary_button(if recovery { "Restore backup" } else { "Reload and discard local changes" })).clicked();
                    temporary = ui.button("Continue without saving").clicked();
                    if self.is_running() { ui.label("Wait for the active run to finish before reloading."); }
                }
            });
        if !self.storage_busy
            && response.is_top_modal
            && context.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            temporary = true;
        }
        if reload {
            self.reload_storage(recovery);
        } else if temporary {
            self.use_temporary_workspace();
        }
    }

    /// Asks whether to run the last valid version when the code has errors.
    fn run_with_errors_dialog(&mut self, context: &egui::Context) {
        let Some(kind) = self.run_prompt else {
            return;
        };
        let mut run = false;
        let mut cancel = false;
        let response = dialogs::centered_modal(egui::Id::new("run-with-errors"), context).show(
            context,
            |ui| {
                ui.set_width((context.content_rect().width() - 80.0).clamp(320.0, 460.0));
                ui.heading("The code has errors");
                ui.add_space(8.0);
                ui.label(
                    "A test always runs the saved scenario, not the text in the code panel. The last version without errors is what would run.",
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    run = ui
                        .add(widgets::primary_button("Run the last valid version"))
                        .clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            },
        );
        if response.is_top_modal
            && context.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            cancel = true;
        }
        if run {
            self.run_prompt = None;
            self.run_anyway = Some(kind);
            self.start_run(kind);
        } else if cancel {
            self.run_prompt = None;
        }
    }

    /// Reports live runs that a previous process left unfinished.
    ///
    /// The journal only holds a run that never reported its own outcome, so every
    /// entry here is an effect the tested API may be holding with no report to
    /// explain it. Marking them as handled is the only thing that clears them.
    fn interrupted_runs_dialog(&mut self, context: &egui::Context) {
        let Some(runs) = self.interrupted.clone() else {
            return;
        };
        let mut dismiss = false;
        let mut handled = false;
        let response = dialogs::centered_modal(egui::Id::new("interrupted-runs"), context).show(
            context,
            |ui| {
                ui.set_width((context.content_rect().width() - 80.0).clamp(320.0, 560.0));
                ui.heading(if runs.len() == 1 {
                    "A live run did not finish".to_owned()
                } else {
                    format!("{} live runs did not finish", runs.len())
                });
                ui.add_space(8.0);
                ui.label(
                    "API Forge closed before these runs reported their result. The requests below reached, or may have reached, the tested API, and cleanup may not have run. Check the API before relying on it.",
                );
                ui.add_space(8.0);
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        for run in &runs {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(format!("{} — {}", run.scenario_name, run.started_at))
                                    .strong(),
                            );
                            ui.label(
                                RichText::new(match run.cleanup {
                                    CleanupState::NotStarted => "Cleanup had not started.",
                                    CleanupState::Started => "Cleanup had started; it may not have finished.",
                                })
                                .size(12.0),
                            );
                            let effects: Vec<_> = run.effects().collect();
                            if effects.is_empty() {
                                ui.label(
                                    RichText::new("No request had been sent.").size(12.0).italics(),
                                );
                            }
                            for entry in effects {
                                ui.label(
                                    RichText::new(format!(
                                        "{} {} — {} ({})",
                                        entry.method,
                                        entry.url,
                                        entry.state.label(),
                                        entry.display_name()
                                    ))
                                    .size(12.0),
                                );
                            }
                            if run.elided > 0 {
                                ui.label(
                                    RichText::new(format!(
                                        "{} earlier requests are not listed.",
                                        run.elided
                                    ))
                                    .size(12.0)
                                    .italics(),
                                );
                            }
                        }
                    });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    handled = ui
                        .add(widgets::primary_button("Mark as handled"))
                        .on_hover_text("Remove these runs from the journal")
                        .clicked();
                    dismiss = ui
                        .button("Remind me later")
                        .on_hover_text("Keep the journal and report it again at the next start")
                        .clicked();
                });
            },
        );
        if response.is_top_modal
            && context.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            dismiss = true;
        }
        if handled {
            let ids: Vec<String> = runs.iter().map(|run| run.run_id.clone()).collect();
            if let Some(journal) = &self.journal
                && let Err(error) = journal.discard(&ids)
            {
                self.notice = Some(format!("The run journal could not be updated: {error}"));
            }
            self.interrupted = None;
        } else if dismiss {
            self.interrupted = None;
        }
    }

    /// Lays out the selection, or the whole board when nothing is selected.
    ///
    /// One undo step, and it clears the pins of everything it moves: after a tidy
    /// up the layout owns those positions again. Blocks outside the selection are
    /// pinned for the layout, so the tidied blocks keep clear of them.
    fn tidy_up(&mut self, selection: &[String]) {
        let graph = self.editor.graph().clone();
        let reference = self.editor.reference_graph().into_owned();
        let project = self.editor.project().clone();
        let moving = |node: &apiforge_core::format::Node| {
            selection.is_empty() || selection.contains(&node.id)
        };
        if graph.nodes.iter().filter(|node| moving(node)).count() < 2 {
            return;
        }
        let blocks: Vec<apiforge_layout::Block> = graph
            .nodes
            .iter()
            .map(|node| {
                let rect = canvas::Card::build(node, &reference, &project).rect();
                let block = apiforge_layout::Block::new(
                    apiforge_layout::Size::new(rect.width(), rect.height()),
                    apiforge_layout::Point::new(node.position.x as f32, node.position.y as f32),
                );
                if moving(node) { block } else { block.pinned() }
            })
            .collect();
        let index_of = |id: &str| graph.nodes.iter().position(|node| node.id == id);
        let edges: Vec<apiforge_layout::Edge> = apiforge_core::graph::wires(&graph)
            .iter()
            .filter_map(|wire| {
                Some(apiforge_layout::Edge::new(
                    index_of(&wire.source)?,
                    index_of(&wire.target)?,
                ))
            })
            .collect();
        let placed = apiforge_layout::tidy(&apiforge_layout::Graph::new(&blocks, &edges));
        let mut tidied = graph;
        for (node, position) in tidied.nodes.iter_mut().zip(placed) {
            if !(selection.is_empty() || selection.contains(&node.id)) {
                continue;
            }
            node.position = Position {
                x: f64::from(position.x),
                y: f64::from(position.y),
            };
            node.auto_placed = true;
        }
        self.apply_graph(tidied);
    }

    /// Opens or closes the code panel and remembers the choice.
    fn toggle_code_panel(&mut self) {
        self.code_open = !self.code_open;
        self.preferences.code_panel = self.code_open;
        self.preferences.save();
    }

    /// Applies an inspector action that belongs to the window rather than to the
    /// document: showing a block in the code panel.
    fn inspector_action(&mut self, action: inspector::Action) {
        let inspector::Action::Reveal(handle) = action else {
            return;
        };
        // A request's body lives in the code of its scenario, so that is where
        // the panel shows it.
        self.code_open = true;
        self.preferences.code_panel = true;
        self.preferences.save();
        self.revealed = None;
        if let Some(node) = self
            .editor
            .graph()
            .nodes
            .iter()
            .find(|node| node.handle == handle)
        {
            self.editor.selection = vec![node.id.clone()];
        }
    }

    /// Whether the code panel has the keyboard, which decides whose history
    /// Ctrl+Z walks: the text's or the document's.
    fn code_has_focus(&self, context: &egui::Context) -> bool {
        self.code_open
            && context.memory(|memory| memory.has_focus(egui::Id::new("code-panel-editor")))
    }

    /// What a context's document prints as, for comparing a draft against it.
    fn printed_for(&self, context: &buffers::Context) -> Option<String> {
        let project = self.editor.project();
        let document = match context {
            buffers::Context::Scenario { id, .. } => apiforge_script::Document::Scenario(
                project
                    .scenarios
                    .iter()
                    .find(|scenario| &scenario.id == id)?
                    .clone(),
            ),
            buffers::Context::Component { id, revision, .. } => {
                apiforge_script::Document::Component(
                    project
                        .components
                        .iter()
                        .find(|component| &component.id == id && component.revision == *revision)?
                        .clone(),
                )
            }
        };
        buffers::print(project, &document)
    }

    /// Which document the code panel is showing.
    fn code_context(&self) -> buffers::Context {
        let project = self.editor.project().id.clone();
        match self.editor.draft() {
            Some(draft) => buffers::Context::Component {
                project,
                id: draft.definition.id.clone(),
                revision: draft.definition.revision,
            },
            None => buffers::Context::Scenario {
                project,
                id: self.editor.scenario().id.clone(),
            },
        }
    }

    /// The document the panel's text is printed from and applied to.
    fn code_document(&self) -> apiforge_script::Document {
        match self.editor.draft() {
            Some(draft) => {
                let mut definition = draft.definition.clone();
                definition.graph = draft.graph.clone();
                apiforge_script::Document::Component(definition)
            }
            None => apiforge_script::Document::Scenario(self.editor.scenario().clone()),
        }
    }

    /// The code panel: the scenario or component as text, kept in step with the
    /// board. Typing pauses, then the text becomes a document edit.
    fn code_panel(&mut self, ui: &mut egui::Ui) {
        let context = self.code_context();
        let document = self.code_document();
        let project = self.editor.project().clone();
        let printed = buffers::print(&project, &document);
        ui.horizontal(|ui| {
            widgets::caption(ui, "CODE");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(widgets::icon_button(Icon::Close, "Close the code panel").compact())
                    .clicked()
                {
                    self.code_open = false;
                    self.preferences.code_panel = false;
                    self.preferences.save();
                }
            });
        });
        ui.add_space(4.0);
        let focused = self.code_has_focus(ui.ctx());
        let revision = self.editor.revision();
        let Some(buffer) = self.buffers.text(&context, printed, revision, focused) else {
            widgets::hint(
                ui,
                "This scenario uses something the language cannot write yet, so it has no code view.",
            );
            return;
        };
        let state = buffer.state;
        let stale = buffer.stale;
        let mut text = std::mem::take(&mut buffer.text);
        let diagnostics = buffer.diagnostics.clone();
        if stale {
            self.stale_banner(ui, &context);
        } else if state == buffers::State::Invalid {
            self.code_banner(ui, &context);
        }
        let names = crate::shapes::Names::of_board(
            &project,
            &self.editor.graph().nodes,
            &self.editor.parameter_names(),
            None,
        );
        let steps: &[StepResult] = self.run.as_ref().map_or(&[], |run| run.steps.as_slice());
        let shapes = crate::shapes::BoardShapes {
            project: &project,
            nodes: &self.editor.graph().nodes,
            steps,
        };
        let outcome = egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.code.show(
                    ui,
                    egui::Id::new("code-panel-editor"),
                    &mut text,
                    &code::Context {
                        mode: apiforge_script::complete::Mode::Script,
                        scope: names.scope(&shapes),
                        diagnostics: &diagnostics,
                        line_numbers: true,
                        wrap: false,
                        font_size: 13.0,
                    },
                )
            })
            .inner;
        // The buffer owns the text; the widget only borrowed it for a frame.
        if outcome.changed {
            self.buffers.typed(&context, text.clone(), Instant::now());
        } else if let Some(buffer) = self.buffers.get_mut(&context) {
            buffer.text.clone_from(&text);
        }
        if let Some(cursor) = outcome.cursor
            && let Some(buffer) = self.buffers.get_mut(&context)
        {
            buffer.cursor = cursor;
        }
        self.sync_selection(&text, outcome.cursor);
        self.apply_code(&context, &project, &document);
    }

    /// Keeps the board's selection and the caret pointing at the same block.
    ///
    /// Whoever has the keyboard leads: with the caret in the text, the board
    /// follows it; otherwise a block selected on the board reveals its statement.
    fn sync_selection(&mut self, text: &str, cursor: Option<usize>) {
        if let Some(cursor) = cursor {
            let Some(handle) = apiforge_script::complete::statement_at(text, cursor) else {
                return;
            };
            let selected = self
                .editor
                .graph()
                .nodes
                .iter()
                .find(|node| node.handle == handle)
                .map(|node| node.id.clone());
            if let Some(id) = selected
                && self.editor.selection != [id.clone()]
            {
                self.editor.selection = vec![id];
            }
            self.revealed = None;
            return;
        }
        // The caret is elsewhere, so the board leads.
        let Some(handle) = self
            .editor
            .selection
            .first()
            .and_then(|id| self.editor.graph().nodes.iter().find(|node| &node.id == id))
            .map(|node| node.handle.clone())
        else {
            return;
        };
        if self.revealed.as_deref() == Some(handle.as_str()) {
            return;
        }
        if let Some(range) = apiforge_script::complete::statement_range(text, &handle) {
            self.code.reveal(range.start);
            self.revealed = Some(handle);
        }
    }

    /// The banner for code restored against a document that has moved on since.
    ///
    /// Nothing is thrown away without being asked: the text is shown as it was
    /// written, and the user says whether to keep editing it or go back to the
    /// document.
    fn stale_banner(&mut self, ui: &mut egui::Ui, context: &buffers::Context) {
        egui::Frame::new()
            .fill(theme::ACCENT_SOFT)
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("This code was written before the document changed.")
                        .size(12.0)
                        .color(theme::TEXT),
                );
                ui.horizontal(|ui| {
                    if ui.button("Keep editing it").clicked()
                        && let Some(buffer) = self.buffers.get_mut(context)
                    {
                        buffer.stale = false;
                    }
                    if ui.button("Discard it").clicked() {
                        self.buffers.discard(context);
                    }
                });
            });
        ui.add_space(6.0);
    }

    /// The banner shown while the code has errors and the board is read-only.
    fn code_banner(&mut self, ui: &mut egui::Ui, context: &buffers::Context) {
        let first = self
            .buffers
            .get(context)
            .and_then(|buffer| buffer.errors().next())
            .map(|diagnostic| diagnostic.message.clone())
            .unwrap_or_default();
        egui::Frame::new()
            .fill(theme::ACCENT_SOFT)
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("The code has errors, so the board waits for it.")
                        .size(12.0)
                        .color(theme::DANGER),
                );
                if !first.is_empty() {
                    ui.label(RichText::new(first).size(11.0).color(theme::MUTED));
                }
                if ui.button("Discard code changes").clicked() {
                    self.buffers.discard(context);
                }
            });
        ui.add_space(6.0);
    }

    /// Turns a paused buffer into a document edit.
    fn apply_code(
        &mut self,
        context: &buffers::Context,
        project: &apiforge_core::format::Project,
        document: &apiforge_script::Document,
    ) {
        let now = Instant::now();
        match self.buffers.apply(context, project, document, now) {
            buffers::Apply::Nothing | buffers::Apply::Failed => {}
            buffers::Apply::Scenario(scenario) => {
                let placed = self.place_new_blocks(*scenario, document, project);
                match self.editor.apply_scenario(placed, now) {
                    Ok(()) => self.buffers.applied(context, self.editor.revision()),
                    Err(message) => self.canvas.set_error(message),
                }
            }
            buffers::Apply::Component(component) => {
                match self.editor.apply_component(*component, now) {
                    Ok(()) => self.buffers.applied(context, self.editor.revision()),
                    Err(message) => self.canvas.set_error(message),
                }
            }
        }
    }

    /// Gives the blocks that code introduced a place on the board.
    fn place_new_blocks(
        &self,
        mut scenario: apiforge_core::format::Scenario,
        document: &apiforge_script::Document,
        project: &apiforge_core::format::Project,
    ) -> apiforge_core::format::Scenario {
        let apiforge_script::Document::Scenario(current) = document else {
            return scenario;
        };
        let reference = self.editor.reference_graph().into_owned();
        for (graph, previous) in [
            (&mut scenario.graph, &current.graph),
            (&mut scenario.cleanup, &current.cleanup),
        ] {
            let fresh: Vec<usize> = graph
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, node)| !previous.nodes.iter().any(|old| old.id == node.id))
                .map(|(index, _)| index)
                .collect();
            if fresh.is_empty() {
                continue;
            }
            let blocks: Vec<apiforge_layout::Block> = graph
                .nodes
                .iter()
                .enumerate()
                .map(|(index, node)| {
                    let rect = canvas::Card::build(node, &reference, project).rect();
                    let block = apiforge_layout::Block::new(
                        apiforge_layout::Size::new(rect.width(), rect.height()),
                        apiforge_layout::Point::new(node.position.x as f32, node.position.y as f32),
                    );
                    if fresh.contains(&index) {
                        block
                    } else {
                        block.pinned()
                    }
                })
                .collect();
            let index_of = |id: &str| graph.nodes.iter().position(|node| node.id == id);
            let edges: Vec<apiforge_layout::Edge> = apiforge_core::graph::wires(graph)
                .iter()
                .filter_map(|wire| {
                    Some(apiforge_layout::Edge::new(
                        index_of(&wire.source)?,
                        index_of(&wire.target)?,
                    ))
                })
                .collect();
            let placed =
                apiforge_layout::place(&apiforge_layout::Graph::new(&blocks, &edges), &fresh);
            for (index, position) in fresh.iter().zip(placed) {
                let node = &mut graph.nodes[*index];
                node.position = Position {
                    x: f64::from(position.x),
                    y: f64::from(position.y),
                };
                node.auto_placed = true;
            }
        }
        scenario
    }

    fn is_running(&self) -> bool {
        self.run_handle.is_some()
            || self
                .run
                .as_ref()
                .is_some_and(|run| run.status == RunStatus::Running)
    }

    fn is_run_stale(&self) -> bool {
        self.run.is_some()
            && self.run_fingerprint.as_ref()
                != Some(&apiforge_storage::public_project(self.editor.project()))
    }

    fn clear_run(&mut self) {
        if let Some(handle) = self.run_handle.take() {
            handle.cancel();
        }
        self.run = None;
        self.run_fingerprint = None;
    }

    fn poll_run(&mut self) {
        let Some(handle) = &self.run_handle else {
            return;
        };
        if let Some(latest) = handle.latest() {
            self.run = Some(latest);
        }
        if handle.is_finished() {
            if let Some(latest) = handle.latest() {
                self.run = Some(latest);
            }
            self.run_handle = None;
        }
    }

    fn start_run(&mut self, kind: RunKind) {
        if self.closing
            || self.is_running()
            || self.editor.draft().is_some()
            || self.storage_prompt.is_some()
            || self.storage_busy
        {
            return;
        }
        // A run executes the document, never the buffer. When the code says
        // something else, the user is asked once which one they meant.
        if self.buffers.blocks_the_board(&self.code_context()) && self.run_anyway != Some(kind) {
            self.run_prompt = Some(kind);
            return;
        }
        self.run_anyway = None;
        let selected = (self.editor.phase() == Phase::Main)
            .then(|| self.editor.selected_node().map(|node| node.id.clone()))
            .flatten();
        if kind != RunKind::Full && selected.is_none() {
            return;
        }
        if kind == RunKind::SingleSelected
            && let Some(previous) = &self.run
            && (previous.mode != self.mode
                || self.is_run_stale()
                || previous.scenario_id != self.editor.scenario().id)
        {
            self.notice = Some(
                "Previous data comes from another mode or an earlier configuration. Run up to the selected block to fetch fresh inputs.".into(),
            );
            return;
        }
        let (target_node_id, single_node_id, previous_run) = match kind {
            RunKind::Full => (None, None, None),
            RunKind::UntilSelected => (selected, None, None),
            RunKind::SingleSelected => (None, selected, self.run.clone()),
        };
        let fingerprint = apiforge_storage::public_project(self.editor.project());
        let request = RunRequest {
            project: self.editor.project().clone(),
            scenario_id: self.editor.scenario().id.clone(),
            mode: self.mode,
            target_node_id,
            single_node_id,
            previous_run,
            secrets: self.secrets.clone(),
        };
        let Some(runner) = &self.runner else {
            self.notice = Some("The runner service is not available.".into());
            return;
        };
        match runner.start(request) {
            Ok(handle) => {
                self.notice = None;
                self.report.open = true;
                self.run_fingerprint = Some(fingerprint);
                self.run = handle.latest();
                self.run_handle = Some(handle);
                self.poll_run();
            }
            Err(error) => {
                self.notice = Some(error.to_string());
            }
        }
    }

    fn cancel_run(&mut self) {
        if let Some(handle) = &self.run_handle {
            handle.cancel();
            self.notice = Some(
                "Cancellation requested. Waiting for the current step and cleanup to finish."
                    .into(),
            );
        }
    }

    fn save_if_changed(&mut self) {
        if self.storage_prompt.is_some() || self.storage_busy {
            return;
        }
        if self.editor.revision() != self.saved_revision {
            self.saved_revision = self.editor.revision();
            if let Some(saver) = &self.saver {
                saver.save(self.editor.workspace());
            }
        }
        self.save_drafts();
    }

    /// Writes the unfinished code beside the workspace, so that closing the
    /// window, switching away or a crash does not lose what was typed.
    ///
    /// Only what the document does not already say is written, and only when it
    /// changed: a draft file is rewritten by editing, not by looking.
    fn save_drafts(&mut self) {
        let Some(store) = &self.store else {
            return;
        };
        let project = self.editor.project().id.clone();
        let current: Vec<(buffers::Context, String)> = self
            .buffers
            .drafts()
            .map(|(context, buffer)| (context.clone(), buffer.text.clone()))
            .collect();
        if current == self.saved_drafts {
            return;
        }
        let entries: Vec<apiforge_storage::CodeDraft> = self
            .buffers
            .drafts()
            .filter_map(|(context, buffer)| {
                let (project_id, target) = match context {
                    buffers::Context::Scenario { project, id } => (
                        project.clone(),
                        apiforge_storage::DraftTarget::Scenario { id: id.clone() },
                    ),
                    buffers::Context::Component {
                        project,
                        id,
                        revision,
                    } => (
                        project.clone(),
                        apiforge_storage::DraftTarget::Component {
                            id: id.clone(),
                            revision: *revision,
                        },
                    ),
                };
                if project_id != project {
                    return None;
                }
                // The base is the document as it stands now. A context whose
                // document is gone writes nothing, which is how a deleted
                // scenario loses its draft.
                let printed = self.printed_for(context)?;
                Some(apiforge_storage::CodeDraft {
                    project_id,
                    target,
                    base_hash: apiforge_storage::draft_base_hash(&printed),
                    text: buffer.text.clone(),
                    cursor: buffer.cursor.min(buffer.text.len()),
                })
            })
            .collect();
        // Code names a secret, it never holds one; the store refuses text that
        // does, and the session's own values are what it checks against.
        let secrets: Vec<String> = self.secrets.values().cloned().collect();
        let borrowed: Vec<&str> = secrets.iter().map(String::as_str).collect();
        let drafts = apiforge_storage::Drafts {
            version: 1,
            entries,
        };
        match store.save_drafts(&drafts, &borrowed) {
            Ok(()) => self.saved_drafts = current,
            Err(error) => {
                if self.notice.is_none() {
                    self.notice = Some(format!("Code drafts were not saved: {error}"));
                }
            }
        }
    }

    fn apply_graph(&mut self, graph: Graph) {
        if let Err(message) = self.editor.apply_graph(graph, Instant::now()) {
            self.canvas.set_error(message);
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.poll_run();
        self.poll_storage();
        if self.storage_prompt.is_some() || self.storage_busy {
            ui.disable();
        }
        if let Some(result) = self.files.poll() {
            self.file_result(result);
        }
        self.shortcuts(ui.ctx());

        let compact = layout_mode(ui.ctx().content_rect().size()) == LayoutMode::Compact;
        egui::Panel::left(if compact {
            "sidebar-compact"
        } else {
            "sidebar"
        })
        .exact_size(if compact {
            theme::COMPACT_SIDEBAR_WIDTH
        } else {
            theme::SIDEBAR_WIDTH
        })
        .resizable(false)
        .frame(
            egui::Frame::new()
                .fill(theme::PANEL)
                .inner_margin(if compact {
                    egui::Margin::symmetric(12, 10)
                } else {
                    egui::Margin::symmetric(16, 16)
                }),
        )
        .show(ui, |ui| self.sidebar(ui));
        egui::Panel::top("scenario-header")
            .frame(
                egui::Frame::new()
                    .fill(theme::WHITE)
                    .inner_margin(egui::Margin::symmetric(
                        if compact { 16 } else { 24 },
                        if compact { 6 } else { 8 },
                    )),
            )
            .show(ui, |ui| self.scenario_header(ui));
        egui::Panel::top("editor-bar")
            .exact_size(44.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::WHITE)
                    .inner_margin(egui::Margin::symmetric(
                        if compact {
                            theme::SPACE_LG as i8
                        } else {
                            theme::SPACE_XL as i8
                        },
                        6,
                    )),
            )
            .show(ui, |ui| self.editor_bar(ui));
        if self.editor.draft().is_none() {
            self.report(ui);
        }
        if self.code_open {
            egui::Panel::left("code-panel")
                .default_size(if compact { 320.0 } else { 440.0 })
                .size_range(if compact {
                    260.0..=520.0
                } else {
                    300.0..=760.0
                })
                .frame(
                    egui::Frame::new()
                        .fill(theme::WHITE)
                        .inner_margin(egui::Margin::symmetric(12, 10)),
                )
                .show(ui, |ui| self.code_panel(ui));
        }
        if self.editor.selected_node().is_some() && !self.inspector_collapsed {
            egui::Panel::right(if compact {
                "inspector-compact"
            } else {
                "inspector"
            })
            .default_size(if compact { 272.0 } else { 360.0 })
            .size_range(if compact {
                248.0..=340.0
            } else {
                320.0..=520.0
            })
            .frame(
                egui::Frame::new()
                    .fill(theme::WHITE)
                    .inner_margin(if compact { 12 } else { 16 }),
            )
            .show(ui, |ui| self.inspector(ui));
        }
        let board_waits = self.buffers.blocks_the_board(&self.code_context());
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                // A graph edit cannot be merged into text that does not parse,
                // so the board waits until the code is fixed or discarded.
                if board_waits {
                    ui.disable();
                }
                self.board(ui);
            });
        self.dialogs(ui.ctx());
        if let Some(id) = self.navigator.show(ui.ctx(), self.editor.graph()) {
            self.editor.selection = vec![id.clone()];
            self.inspector_collapsed = false;
            self.canvas.reveal(id);
        }
        self.storage_dialog(ui.ctx());
        self.interrupted_runs_dialog(ui.ctx());
        self.run_with_errors_dialog(ui.ctx());
        self.save_if_changed();
    }

    // ----- Panels ----------------------------------------------------------------

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        let running = self.is_running();
        let project = self.editor.project();
        let graph = self.editor.reference_graph();
        let context = library::Context {
            project,
            projects: self
                .editor
                .workspace()
                .projects
                .iter()
                .map(|project| (project.id.clone(), project.name.clone()))
                .collect(),
            scenario_id: &self.editor.scenario().id,
            variables: graph
                .nodes
                .iter()
                .filter(|node| node.is_variable())
                .map(|node| (node.id.clone(), node.display_name().to_owned()))
                .collect(),
            parameters: self.editor.parameter_names(),
            editing_component: self.editor.draft().is_some(),
            running,
        };
        let actions = self.library.show(ui, &context);
        drop(graph);

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            let status = self.saver.as_ref().map(Saver::status);
            if matches!(&status, Some(SaveStatus::Failed(_)))
                && ui
                    .add(widgets::action(Icon::Refresh, "Retry save").quiet())
                    .clicked()
            {
                self.retry_save();
            }
            let (text, color) = match &status {
                None => ("Session only — not saved".to_owned(), theme::MUTED),
                Some(SaveStatus::Failed(error)) => (format!("Save failed: {error}"), theme::DANGER),
                Some(status) => (status.label().to_owned(), theme::MUTED),
            };
            ui.label(RichText::new(format!("●  {text}")).size(11.0).color(color));
            ui.add_space(4.0);
            let base = &self.editor.project().environment.base_url;
            let host = base.split_once("://").map_or(base.as_str(), |(_, rest)| {
                rest.split('/').next().unwrap_or(rest)
            });
            let environment = egui::Button::new(
                RichText::new(format!(
                    "Environment\n{}",
                    if host.is_empty() {
                        "Configure the environment"
                    } else {
                        host
                    }
                ))
                .size(11.0),
            )
            .min_size(egui::vec2(ui.available_width(), 44.0));
            if ui
                .add(environment)
                .on_hover_text("Base URL, variables and session secrets")
                .clicked()
            {
                self.dialog = Some(Dialog::Environment(dialogs::EnvironmentDialog::new(
                    &self.editor.project().environment,
                    &self.secrets,
                )));
            }
        });

        for action in actions {
            self.library_action(action);
        }
    }

    fn library_action(&mut self, action: library::Action) {
        match action {
            library::Action::SwitchProject(id) => {
                if self.is_running() {
                    return;
                }
                self.editor.switch_project(&id);
                self.secrets.clear();
                self.clear_run();
                self.notice = None;
            }
            library::Action::NewProject => {
                if !self.is_running() {
                    self.dialog = Some(Dialog::NewProject(String::new()));
                }
            }
            library::Action::SwitchScenario(id) => {
                if self.is_running() {
                    return;
                }
                self.editor.switch_scenario(&id);
                self.clear_run();
            }
            library::Action::NewScenario => {
                if !self.is_running() {
                    self.dialog = Some(Dialog::NewScenario(String::new()));
                }
            }
            library::Action::AddNode { block, component } => {
                let position = position_after(self.editor.graph(), 100.0);
                self.add_node(block, component, position, None);
            }
            library::Action::AddOperator(operator) => {
                let node = create_operator_node(operator, self.canvas.visible_center());
                self.push_node(node);
            }
            library::Action::SelectNode(id) => self.editor.selection = vec![id],
            library::Action::OpenExample(example) => self.open_example(example),
        }
    }

    fn open_example(&mut self, example: library::Example) {
        if self.is_running() {
            return;
        }
        let (id, build, notice) = match example {
            library::Example::Workflow => (
                "workflow-poc",
                apiforge_core::demos::workflow_project as fn() -> apiforge_core::format::Project,
                "Start → prices → IF → cart → add products. Choose API and run the test; the No branch does not create a cart.",
            ),
            library::Example::OperatorBlocks => (
                "blocks-poc",
                apiforge_core::demos::blocks_project as fn() -> apiforge_core::format::Project,
                "Example: open a Comparison and edit the nested blocks in its arguments. You can run a simulation or the local API.",
            ),
        };
        if self
            .editor
            .workspace()
            .projects
            .iter()
            .any(|project| project.id == id)
        {
            self.editor.switch_project(id);
            self.secrets.clear();
            self.clear_run();
            return;
        }
        self.editor.add_project(build());
        self.secrets.clear();
        self.clear_run();
        self.notice = Some(notice.into());
    }

    fn project_menu(&mut self, ui: &mut egui::Ui) {
        let running = self.is_running();
        let menu = widgets::action_menu(
            ui,
            widgets::icon_button(Icon::More, "Project actions"),
            |ui| {
                ui.set_min_width(180.0);
                if ui
                    .add_enabled(
                        !self.files.busy() && !running,
                        widgets::action(Icon::Import, "Import project…").quiet(),
                    )
                    .clicked()
                {
                    self.files.import();
                    ui.close();
                }
                if ui
                    .add_enabled(
                        !self.files.busy(),
                        widgets::action(Icon::Export, "Export project…").quiet(),
                    )
                    .clicked()
                {
                    self.files
                        .export(apiforge_storage::public_project(self.editor.project()));
                    ui.close();
                }
            },
        );
        menu.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Project file actions")
        });
        if ui
            .add(widgets::icon_button(Icon::Settings, "Settings"))
            .clicked()
        {
            self.dialog = Some(Dialog::Settings(dialogs::SettingsDialog::new(
                self.editor.project(),
                &self.editor.scenario().id,
            )));
        }
        ui.add_space(4.0);
    }

    fn scenario_header(&mut self, ui: &mut egui::Ui) {
        let draft = self.editor.draft().cloned();
        let scenario = self.editor.scenario().clone();
        let running = self.is_running();
        let compact = layout_mode(ui.ctx().content_rect().size()) == LayoutMode::Compact;
        let project_name = self.editor.project().name.clone();
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 428.0 - ui.spacing().item_spacing.x).max(140.0);
            ui.allocate_ui_with_layout(
                egui::vec2(width, 40.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(width);
                    ui.spacing_mut().item_spacing.y = 2.0;
                    let title = draft.as_ref().map_or(scenario.name.as_str(), |draft| {
                        draft.definition.name.as_str()
                    });
                    let description = draft
                        .as_ref()
                        .map_or(scenario.description.as_str(), |draft| {
                            draft.definition.description.as_str()
                        });
                    ui.add(
                        egui::Label::new(
                            RichText::new(title)
                                .size(if compact { 18.0 } else { theme::FONT_TITLE })
                                .family(theme::bold_family()),
                        )
                        .truncate(),
                    )
                    .on_hover_text(format!("{title}\n{description}"));
                    let context = if draft.is_some() {
                        "Component editor"
                    } else {
                        "Scenarios"
                    };
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("{project_name}  /  {context}"))
                                .size(theme::FONT_META)
                                .color(theme::MUTED),
                        )
                        .truncate(),
                    );
                },
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.project_menu(ui);
                if let Some(draft) = &draft {
                    if ui
                        .add(widgets::action(Icon::Save, "Save revision").primary())
                        .on_hover_text("Save the inner graph as a new revision")
                        .clicked()
                    {
                        let selected: Vec<String> = draft
                            .graph
                            .nodes
                            .iter()
                            .map(|node| node.id.clone())
                            .collect();
                        self.dialog =
                            Some(Dialog::Component(Box::new(dialogs::ComponentDialog::new(
                                &draft.graph,
                                &selected,
                                self.editor.project(),
                                Some(&draft.definition),
                            ))));
                    }
                    if ui
                        .add(widgets::action(Icon::ArrowLeft, "Back without saving").quiet())
                        .clicked()
                    {
                        self.editor.leave_component();
                    }
                    return;
                }
                if running {
                    if ui
                        .add(widgets::action(Icon::Stop, "Cancel run").danger())
                        .on_hover_text("Stop the main phase; cleanup still runs")
                        .clicked()
                    {
                        self.cancel_run();
                    }
                } else {
                    let can_run = !scenario.graph.nodes.is_empty();
                    let can_partial =
                        self.editor.phase() == Phase::Main && self.editor.selected_node().is_some();
                    widgets::action_menu(
                        ui,
                        widgets::icon_button(Icon::ChevronDown, "Run options"),
                        |ui| {
                            ui.set_min_width(220.0);
                            if ui
                                .add_enabled(
                                    can_partial,
                                    widgets::action(Icon::Play, "Run up to selected block").quiet(),
                                )
                                .on_hover_text("Fresh data from preceding steps")
                                .clicked()
                            {
                                self.start_run(RunKind::UntilSelected);
                                ui.close();
                            }
                            let single_hint = if self.run.is_some() {
                                "Use available inputs from the last run"
                            } else {
                                "Current literals and arguments; no run history"
                            };
                            if ui
                                .add_enabled(
                                    can_partial,
                                    widgets::action(Icon::Play, "Run only selected step").quiet(),
                                )
                                .on_hover_text(single_hint)
                                .clicked()
                            {
                                self.start_run(RunKind::SingleSelected);
                                ui.close();
                            }
                        },
                    );
                    if ui
                        .add_enabled(can_run, widgets::action(Icon::Play, "Run test").primary())
                        .on_hover_text("Run the whole scenario")
                        .clicked()
                    {
                        self.start_run(RunKind::Full);
                    }
                }
                ui.add_space(6.0);
                let unrecorded = self
                    .runner
                    .as_ref()
                    .is_some_and(|runner| !runner.is_journalled());
                let mut chose_live = false;
                ui.add_enabled_ui(!running, |ui| {
                    chose_live = ui
                        .selectable_value(&mut self.mode, RunMode::Live, "API")
                        .on_hover_text(if unrecorded {
                            "Send real requests to the configured API. This session cannot record them, so an interrupted run leaves no record."
                        } else {
                            "Send real requests to the configured API"
                        })
                        .clicked();
                    ui.selectable_value(&mut self.mode, RunMode::Simulation, "Simulation")
                        .on_hover_text("Use saved sample responses; no requests are sent");
                });
                if chose_live && unrecorded && self.notice.is_none() {
                    self.notice = Some("Live requests in this session are not recorded in a run journal. If the window closes during a run, there will be no record of what the API was asked to do.".into());
                }
            });
        });
        if let Some(message) = &self.storage_message {
            ui.add_space(6.0);
            ui.label(RichText::new(message).size(12.0).color(theme::DANGER));
        }
        if let Some(notice) = self.notice.clone() {
            ui.add_space(6.0);
            egui::Frame::new()
                .fill(theme::ACCENT_SOFT)
                .corner_radius(8)
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::Label::new(RichText::new(notice).size(12.0)).wrap());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(widgets::icon_button(Icon::Close, "Dismiss notice").compact())
                                .clicked()
                            {
                                self.notice = None;
                            }
                        });
                    });
                });
        }
    }

    fn editor_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            if let Some(index) = self.breadcrumb(ui) {
                self.editor.truncate_drafts(index);
            }
            if self.editor.draft().is_none() {
                let cleanup = self.editor.scenario().cleanup.nodes.len();
                let mut phase = self.editor.phase();
                if widgets::tab_value(ui, &mut phase, Phase::Main, "Scenario").clicked() {
                    self.editor.set_phase(Phase::Main);
                }
                if widgets::tab_value(
                    ui,
                    &mut phase,
                    Phase::Cleanup,
                    format!("After test  {cleanup}"),
                )
                .on_hover_text("Cleanup steps that run after the test, also after errors")
                .clicked()
                {
                    self.editor.set_phase(Phase::Cleanup);
                }
            }
            if ui
                .add(widgets::action(Icon::Search, "Find step").quiet())
                .on_hover_text("Find a block and open its configuration (Ctrl+K)")
                .clicked()
            {
                self.navigator.open();
            }
            if ui
                .add(if self.code_open {
                    widgets::action(Icon::Code, "Code")
                } else {
                    widgets::action(Icon::Code, "Code").quiet()
                })
                .on_hover_text("Edit this scenario as code (Ctrl+E)")
                .clicked()
            {
                self.toggle_code_panel();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.editor.selected_node().is_some() {
                    let toggle_label = if self.inspector_collapsed {
                        "Show Inspector"
                    } else {
                        "Hide Inspector"
                    };
                    if ui
                        .add(widgets::icon_button(Icon::Panel, toggle_label))
                        .clicked()
                    {
                        self.inspector_collapsed = !self.inspector_collapsed;
                    }
                }
                if ui
                    .add_enabled(
                        self.editor.can_redo(),
                        widgets::icon_button(Icon::Redo, "Redo (Ctrl+Shift+Z)"),
                    )
                    .clicked()
                {
                    self.editor.redo();
                }
                if ui
                    .add_enabled(
                        self.editor.can_undo(),
                        widgets::icon_button(Icon::Undo, "Undo (Ctrl+Z)"),
                    )
                    .clicked()
                {
                    self.editor.undo();
                }
            });
        });
    }

    fn breadcrumb(&self, ui: &mut egui::Ui) -> Option<usize> {
        let mut chosen = None;
        for (index, draft) in self.editor.drafts().iter().enumerate() {
            if index > 0 {
                ui.label(RichText::new("›").color(theme::MUTED));
            }
            if ui
                .selectable_label(
                    index + 1 == self.editor.drafts().len(),
                    &draft.definition.name,
                )
                .clicked()
            {
                chosen = Some(index);
            }
        }
        chosen
    }

    fn report(&mut self, ui: &mut egui::Ui) {
        let actions = {
            let selected_id = self.editor.selection.first().map(String::as_str);
            let context = report::Context {
                run: self.run.as_ref(),
                selected_node_id: selected_id,
                error_policy: self.editor.scenario().error_policy,
                running: self.is_running(),
                stale: self.is_run_stale(),
                files_busy: self.files.busy(),
            };
            self.report.show(ui, &context)
        };
        for action in actions {
            self.report_action(action);
        }
    }

    fn report_action(&mut self, action: report::Action) {
        match action {
            report::Action::SelectNode(id) => {
                let Some(phase) = self
                    .run
                    .as_ref()
                    .and_then(|run| run.steps.iter().find(|step| step.node_id == id))
                    .map(|step| step.phase)
                else {
                    return;
                };
                self.editor.set_phase(phase);
                self.editor.selection = vec![id];
            }
            report::Action::AssertField {
                node_id,
                port,
                path,
                value,
            } => {
                if self.is_run_stale() {
                    self.notice = Some(
                        "The graph changed after this run. Run the test again to add a comparison from current data.".into(),
                    );
                    return;
                }
                let target_phase = if self
                    .editor
                    .scenario()
                    .graph
                    .nodes
                    .iter()
                    .any(|node| node.id == node_id)
                {
                    Phase::Main
                } else {
                    Phase::Cleanup
                };
                self.editor.set_phase(target_phase);
                self.add_field_assertion(&node_id, &port, &path, &value);
            }
            report::Action::AssignField {
                node_id,
                port,
                path,
            } => {
                let scenario = self.editor.scenario();
                let target_phase = if scenario.graph.nodes.iter().any(|node| node.id == node_id) {
                    Phase::Main
                } else if scenario.cleanup.nodes.iter().any(|node| node.id == node_id) {
                    Phase::Cleanup
                } else {
                    self.notice = Some("The source block is no longer available.".into());
                    return;
                };
                self.editor.set_phase(target_phase);
                self.dialog = Some(Dialog::AssignVariable(dialogs::AssignVariableDialog::new(
                    self.editor.graph(),
                    &node_id,
                    &port,
                    &path,
                )));
            }
            report::Action::SetErrorPolicy(policy) => {
                let scenario_id = self.editor.scenario().id.clone();
                self.editor.change_project(|project| {
                    if let Some(scenario) = project
                        .scenarios
                        .iter_mut()
                        .find(|scenario| scenario.id == scenario_id)
                    {
                        scenario.error_policy = policy;
                    }
                });
            }
            report::Action::ExportReport => {
                if let Some(run) = self.run.clone() {
                    self.files.export_report(run);
                }
            }
        }
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        let Some(node) = self.editor.selected_node().cloned() else {
            return;
        };
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            if ui
                .add(widgets::icon_button(Icon::Close, "Close Inspector").compact())
                .clicked()
            {
                self.editor.selection.clear();
            }
        });
        let reference = self.editor.reference_graph().into_owned();
        let parameters = self.editor.parameter_names();
        let in_scenario = self.editor.draft().is_none();
        let stale = self.is_run_stale();
        let phase = self.editor.phase();
        let actions = {
            let run = in_scenario.then_some(self.run.as_ref()).flatten();
            let step = (in_scenario && !stale)
                .then(|| {
                    self.run.as_ref().and_then(|run| {
                        run.steps
                            .iter()
                            .find(|step| step.phase == phase && step.node_id == node.id)
                    })
                })
                .flatten();
            let context = inspector::Context {
                node: &node,
                graph: &reference,
                board: self.editor.graph(),
                project: self.editor.project(),
                parameters: &parameters,
                run,
                step,
            };
            self.inspector.show(ui, &context)
        };
        for action in actions {
            if let inspector::Action::Reveal(_) = &action {
                self.inspector_action(action);
                continue;
            }
            match action {
                inspector::Action::Reveal(_) => unreachable!("handled above"),
                inspector::Action::UpdateNode(updated) => {
                    let mut graph = self.editor.graph().clone();
                    if let Some(slot) = graph
                        .nodes
                        .iter_mut()
                        .find(|candidate| candidate.id == updated.id)
                    {
                        *slot = *updated;
                        self.apply_graph(graph);
                    }
                }
                inspector::Action::UpdateGraph(graph) => self.apply_graph(graph),
                inspector::Action::EditComponent => self.edit_component(&node.id),
                inspector::Action::AssertField { port, path, value } => {
                    self.add_field_assertion(&node.id, &port, &path, &value)
                }
                inspector::Action::AssignField { port, path } => {
                    self.dialog = Some(Dialog::AssignVariable(dialogs::AssignVariableDialog::new(
                        self.editor.graph(),
                        &node.id,
                        &port,
                        &path,
                    )));
                }
                inspector::Action::Error(message) => self.canvas.set_error(message),
            }
        }
    }

    fn add_field_assertion(
        &mut self,
        source: &str,
        port: &str,
        path: &str,
        value: &serde_json::Value,
    ) {
        let Some(node) = self
            .editor
            .reference_graph()
            .nodes
            .iter()
            .find(|node| node.id == source)
            .cloned()
        else {
            return;
        };
        let assertion = edit::field_assertion(&node, port, path, value);
        let label = if path.is_empty() { port } else { path };
        self.push_node(assertion);
        self.notice = Some(format!(
            "Added a comparison for “{label}”. Check the expected value or type in its configuration."
        ));
    }

    fn board(&mut self, ui: &mut egui::Ui) {
        let context = self.editor.context_key();
        let reference = self.editor.reference_graph().into_owned();
        let phase = self.editor.phase();
        let steps: Option<Vec<StepResult>> = (self.editor.draft().is_none()
            && !self.is_run_stale())
        .then(|| {
            self.run.as_ref().map(|run| {
                run.steps
                    .iter()
                    .filter(|step| step.phase == phase)
                    .cloned()
                    .collect()
            })
        })
        .flatten();
        let commands = {
            let input = canvas::Input {
                graph: self.editor.graph(),
                reference_graph: &reference,
                project: self.editor.project(),
                selection: &self.editor.selection,
                steps: steps.as_deref(),
                can_paste: self
                    .editor
                    .clipboard
                    .as_ref()
                    .is_some_and(|clipboard| clipboard.context == context),
                context: &context,
            };
            self.canvas.show(ui, &input)
        };
        for command in commands {
            self.board_command(command);
        }
    }

    fn board_command(&mut self, command: Command) {
        match command {
            Command::Select(ids) => self.editor.selection = ids,
            Command::Move(positions) => {
                let mut graph = self.editor.graph().clone();
                for (id, position) in positions {
                    if let Some(node) = graph.nodes.iter_mut().find(|node| node.id == id) {
                        node.position = position;
                        // A block placed by hand is pinned: automatic layout treats
                        // it as an obstacle until the next tidy up.
                        node.auto_placed = false;
                    }
                }
                self.apply_graph(graph);
            }
            Command::TidyUp(selection) => self.tidy_up(&selection),
            Command::SetWireStyle(style) => {
                self.canvas.wire_style = style;
                self.preferences.wire_style = style;
                self.preferences.save();
            }
            Command::SetBridges(bridges) => {
                self.canvas.bridges = bridges;
                self.preferences.bridges = bridges;
                self.preferences.save();
            }
            Command::AddNode {
                block,
                component,
                position,
                source,
            } => self.add_node(block, component, position, source),
            Command::AddOperator { operator, position } => {
                self.push_node(create_operator_node(operator, position))
            }
            Command::SetLiteral {
                node,
                input,
                path,
                value,
            } => self.set_input(&node, &input, &path, Binding::literal(value)),
            Command::Bind {
                node,
                input,
                path,
                binding,
            } => self.set_input(&node, &input, &path, binding),
            Command::Delete { nodes, wires } => {
                let ids: HashSet<&str> = nodes.iter().map(String::as_str).collect();
                let wire_ids: HashSet<&str> = wires.iter().map(String::as_str).collect();
                let graph =
                    edit::remove_connections(&delete_nodes(self.editor.graph(), &ids), &wire_ids);
                self.editor
                    .selection
                    .retain(|id| !ids.contains(id.as_str()));
                self.apply_graph(graph);
            }
            Command::EditComponent(id) => self.edit_component(&id),
            Command::Connect {
                source,
                source_port,
                target,
                target_input,
                target_path,
            } => {
                let result = edit::connect(
                    self.editor.graph(),
                    self.editor.project(),
                    &source,
                    &source_port,
                    &target,
                    &target_input,
                    &target_path,
                );
                match result {
                    Ok(graph) => self.apply_graph(graph),
                    Err(error) => self.canvas.set_error(error.to_string()),
                }
            }
            Command::Copy => self.copy_selection(),
            Command::Paste => self.paste(),
            Command::Duplicate => {
                let selected: HashSet<&str> =
                    self.editor.selection.iter().map(String::as_str).collect();
                if selected.is_empty() {
                    return;
                }
                // Cleanup bindings may read main-graph outputs. Preserve those
                // references exactly as a regular paste does.
                let fragment = edit::copy_selection(self.editor.graph(), &selected);
                let reference = self.editor.reference_graph().into_owned();
                let external: Vec<&str> = reference
                    .nodes
                    .iter()
                    .map(|node| node.id.as_str())
                    .collect();
                match edit::paste_selection(
                    self.editor.graph(),
                    &fragment,
                    &edit::PASTE_OFFSET,
                    &external,
                ) {
                    Ok(pasted) => self.insert_pasted(pasted),
                    Err(error) => self.canvas.set_error(error.to_string()),
                }
            }
            Command::SaveSelectionAsComponent => {
                if !self.editor.selection.is_empty() {
                    self.dialog = Some(Dialog::Component(Box::new(dialogs::ComponentDialog::new(
                        self.editor.graph(),
                        &self.editor.selection,
                        self.editor.project(),
                        None,
                    ))));
                }
            }
        }
    }

    fn copy_selection(&mut self) {
        let selected: HashSet<&str> = self.editor.selection.iter().map(String::as_str).collect();
        let fragment = edit::copy_selection(self.editor.graph(), &selected);
        if fragment.nodes.is_empty() {
            return;
        }
        let count = fragment.nodes.len();
        self.editor.clipboard = Some(BlockClipboard {
            context: self.editor.context_key(),
            fragment,
            pastes: 0,
        });
        self.notice = Some(format!(
            "Copied {count} step(s) with their configuration. Paste with Ctrl+V."
        ));
    }

    fn paste(&mut self) {
        let context = self.editor.context_key();
        let Some(clipboard) = self
            .editor
            .clipboard
            .clone()
            .filter(|clipboard| clipboard.context == context)
        else {
            return;
        };
        let count = f64::from(clipboard.pastes + 1);
        let offset = Position {
            x: edit::PASTE_OFFSET.x * count,
            y: edit::PASTE_OFFSET.y * count,
        };
        let reference = self.editor.reference_graph().into_owned();
        let external: Vec<&str> = reference
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect();
        match edit::paste_selection(self.editor.graph(), &clipboard.fragment, &offset, &external) {
            Ok(pasted) => {
                if let Some(stored) = &mut self.editor.clipboard {
                    stored.pastes += 1;
                }
                self.insert_pasted(pasted);
            }
            Err(error) => self.canvas.set_error(error.to_string()),
        }
    }

    fn insert_pasted(&mut self, pasted: edit::Pasted) {
        let count = pasted.ids.len();
        self.apply_graph(pasted.graph);
        self.editor.selection = pasted.ids;
        self.notice = Some(format!("Pasted {count} step(s) with their configuration."));
    }

    /// Sets a slot of a node: a whole input, or the binding at `path` inside it.
    fn set_input(&mut self, node: &str, input: &str, path: &[String], binding: Binding) {
        let mut graph = self.editor.graph().clone();
        let Some(target) = graph
            .nodes
            .iter_mut()
            .find(|candidate| candidate.id == node)
        else {
            return;
        };
        let next = match target.inputs.get(input) {
            Some(current) => current.replace_at(path, binding),
            None => path.is_empty().then_some(binding),
        };
        match next {
            Some(next) => {
                target.inputs.insert(input.to_owned(), next);
                self.apply_graph(graph);
            }
            None => self
                .canvas
                .set_error(edit::EditError::InvalidOperandPath.to_string()),
        }
    }

    fn add_node(
        &mut self,
        block: Block,
        component: Option<(String, u32)>,
        position: Position,
        source: Option<(String, String)>,
    ) {
        let definition = component.as_ref().and_then(|(id, revision)| {
            self.editor
                .project()
                .components
                .iter()
                .find(|candidate| &candidate.id == id && candidate.revision == *revision)
                .cloned()
        });
        let source = source
            .as_ref()
            .map(|(node_id, port)| edit::OutputRef { node_id, port });
        match edit::add_node(
            self.editor.graph(),
            block,
            position,
            definition.as_ref(),
            source,
        ) {
            Ok(added) => {
                let graph = self.free_slot_for(added.graph, &added.node_id);
                self.apply_graph(graph);
                self.editor.selection = vec![added.node_id];
            }
            Err(error) => self.canvas.set_error(error.to_string()),
        }
    }

    fn push_node(&mut self, node: apiforge_core::format::Node) {
        let id = node.id.clone();
        let mut graph = self.editor.graph().clone();
        graph.nodes.push(node);
        let graph = self.free_slot_for(graph, &id);
        self.apply_graph(graph);
        self.editor.selection = vec![id];
    }

    /// Moves a freshly added block to the nearest free slot when the position it
    /// was given is already occupied, and leaves everything else where it is.
    ///
    /// A block that keeps its requested position stays pinned, because that
    /// position is the one the user pointed at; a block the layout had to move is
    /// marked as automatically placed, so a later tidy up owns it.
    fn free_slot_for(&self, mut graph: Graph, node_id: &str) -> Graph {
        let reference = self.editor.reference_graph().into_owned();
        let project = self.editor.project().clone();
        let Some(fresh) = graph.nodes.iter().position(|node| node.id == node_id) else {
            return graph;
        };
        let blocks: Vec<apiforge_layout::Block> = graph
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let rect = canvas::Card::build(node, &reference, &project).rect();
                let block = apiforge_layout::Block::new(
                    apiforge_layout::Size::new(rect.width(), rect.height()),
                    apiforge_layout::Point::new(node.position.x as f32, node.position.y as f32),
                );
                if index == fresh {
                    block
                } else {
                    block.pinned()
                }
            })
            .collect();
        let index_of = |id: &str| graph.nodes.iter().position(|node| node.id == id);
        let edges: Vec<apiforge_layout::Edge> = apiforge_core::graph::wires(&graph)
            .iter()
            .filter_map(|wire| {
                Some(apiforge_layout::Edge::new(
                    index_of(&wire.source)?,
                    index_of(&wire.target)?,
                ))
            })
            .collect();
        let placed =
            apiforge_layout::place(&apiforge_layout::Graph::new(&blocks, &edges), &[fresh]);
        if let Some(position) = placed.first() {
            let node = &mut graph.nodes[fresh];
            let moved = (position.x - node.position.x as f32).abs() > 0.5
                || (position.y - node.position.y as f32).abs() > 0.5;
            if moved {
                node.position = Position {
                    x: f64::from(position.x),
                    y: f64::from(position.y),
                };
                node.auto_placed = true;
            }
        }
        graph
    }

    fn edit_component(&mut self, node_id: &str) {
        let Some(node) = self
            .editor
            .graph()
            .nodes
            .iter()
            .find(|node| node.id == node_id)
        else {
            return;
        };
        match apiforge_core::graph::component_definition(self.editor.project(), node) {
            Some(definition) => self.editor.enter_component(definition.clone()),
            None => {
                self.notice = Some("The component revision is missing from the project.".into())
            }
        }
    }

    // ----- Shortcuts, dialogs and files ------------------------------------------

    fn shortcuts(&mut self, context: &egui::Context) {
        if self.dialog.is_some()
            || self.navigator.is_open()
            || self.closing
            || self.storage_prompt.is_some()
            || self.storage_busy
        {
            return;
        }
        if context.input_mut(|input| {
            input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::K))
        }) {
            self.navigator.open();
            return;
        }
        if context.egui_wants_keyboard_input() {
            return;
        }
        if self.code_has_focus(context) {
            // The code panel owns its keystrokes: undo there walks the text, not
            // the document. The text edit handles it, so it is left alone.
            return;
        }
        let undo = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
        let redo = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
        let redo_alt = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
        // Check the longer shortcut first: Ctrl+Shift+Z also matches Ctrl+Z.
        if context
            .input_mut(|input| input.consume_shortcut(&redo) || input.consume_shortcut(&redo_alt))
        {
            self.editor.redo();
        } else if context.input_mut(|input| input.consume_shortcut(&undo)) {
            self.editor.undo();
        }
        if context.input_mut(|input| {
            input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::D))
        }) {
            self.board_command(Command::Duplicate);
        }
        if context.input_mut(|input| {
            input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::E))
        }) {
            self.toggle_code_panel();
        }
        let (copy, paste) = context.input(|input| {
            let copy = input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::Copy));
            let paste = input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::Paste(_)));
            (copy, paste)
        });
        if copy && !self.editor.selection.is_empty() {
            self.board_command(Command::Copy);
        }
        if paste {
            self.board_command(Command::Paste);
        }
    }

    fn dialogs(&mut self, context: &egui::Context) {
        match &mut self.dialog {
            None => {}
            Some(Dialog::NewProject(_) | Dialog::NewScenario(_)) => self.new_item_dialog(context),
            Some(Dialog::Environment(dialog)) => match dialog.show(context) {
                Outcome::Open => {}
                Outcome::Cancelled => self.dialog = None,
                Outcome::Applied((environment, secrets)) => {
                    self.editor
                        .change_project(|project| project.environment = environment);
                    self.secrets = secrets;
                    self.dialog = None;
                }
            },
            Some(Dialog::Settings(dialog)) => match dialog.show(context) {
                Outcome::Open => {}
                Outcome::Cancelled => self.dialog = None,
                Outcome::Applied(change) => {
                    self.editor.change_project(|project| change.apply(project));
                    self.dialog = None;
                }
            },
            Some(Dialog::AssignVariable(dialog)) => match dialog.show(context) {
                Outcome::Open => {}
                Outcome::Cancelled => self.dialog = None,
                Outcome::Applied(assignment) => {
                    let source_id = dialog.source_id.clone();
                    let port = dialog.port.clone();
                    let path = dialog.path.clone();
                    match edit::assign_response_variable(
                        self.editor.graph(),
                        self.editor.project(),
                        &source_id,
                        &port,
                        &path,
                        &assignment.name,
                        assignment.target_id.as_deref(),
                    ) {
                        Ok(assigned) => {
                            let label = if path.is_empty() { &port } else { &path };
                            self.apply_graph(assigned.graph);
                            self.editor.selection = vec![assigned.variable_id];
                            self.notice = Some(format!(
                                "Variable “{}” reads {label} from the response on every run.",
                                assignment.name
                            ));
                            self.dialog = None;
                        }
                        Err(error) => {
                            if let Some(Dialog::AssignVariable(dialog)) = &mut self.dialog {
                                dialog.set_error(error.to_string());
                            }
                        }
                    }
                }
            },
            Some(Dialog::Component(dialog)) => {
                let project = self.editor.project().clone();
                match dialog.show(context, &project) {
                    Outcome::Open => {}
                    Outcome::Cancelled => self.dialog = None,
                    Outcome::Applied(saved) => match edit::add_component(
                        &project,
                        &saved.component,
                        saved.revision_of.as_deref(),
                    ) {
                        Ok(next) => {
                            let added = next.components.last().cloned().unwrap_or(saved.component);
                            let revising = saved.revision_of.is_some();
                            self.editor.change_project(|project| *project = next);
                            if revising {
                                self.editor.leave_component();
                            }
                            self.dialog = None;
                            self.notice = Some(format!(
                                "Saved “{}”, revision {}. Existing instances keep their revisions.",
                                added.name, added.revision
                            ));
                        }
                        Err(error) => {
                            if let Some(Dialog::Component(dialog)) = &mut self.dialog {
                                dialog.set_error(error.to_string());
                            }
                        }
                    },
                }
            }
        }
    }

    fn new_item_dialog(&mut self, context: &egui::Context) {
        let Some(dialog) = &mut self.dialog else {
            return;
        };
        let (title, placeholder, hint, name) = match dialog {
            Dialog::NewProject(name) => (
                "New API project",
                "e.g. Platform API",
                "Your own endpoints, components and test environment.",
                name,
            ),
            Dialog::NewScenario(name) => (
                "New scenario",
                "e.g. User registration",
                "Start from an empty board and blocks from the project library.",
                name,
            ),
            _ => return,
        };
        let mut close = false;
        let mut create = false;
        dialogs::centered_modal(egui::Id::new("new-item"), context).show(context, |ui| {
            ui.set_width(380.0);
            ui.label(RichText::new(title).size(17.0).family(theme::bold_family()));
            ui.add_space(8.0);
            ui.label("Name");
            let field = ui.add(
                crate::widgets::text_edit(name)
                    .hint_text(placeholder)
                    .desired_width(f32::INFINITY),
            );
            if !field.has_focus() && name.is_empty() {
                field.request_focus();
            }
            widgets::hint(ui, hint);
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let ready = !name.trim().is_empty();
                    if ui
                        .add_enabled(ready, widgets::action(Icon::Plus, "Create").primary())
                        .clicked()
                        || (ready
                            && field.lost_focus()
                            && ui.input(|input| input.key_pressed(Key::Enter)))
                    {
                        create = true;
                    }
                    if ui
                        .add(widgets::action(Icon::Close, "Cancel").quiet())
                        .clicked()
                    {
                        close = true;
                    }
                });
            });
            if ui.input(|input| input.key_pressed(Key::Escape)) {
                close = true;
            }
        });
        if create {
            match self.dialog.take() {
                Some(Dialog::NewProject(name)) => {
                    let mut project = apiforge_core::demos::empty_project();
                    project.name = name.trim().to_owned();
                    self.editor.add_project(project);
                    self.secrets.clear();
                    self.clear_run();
                }
                Some(Dialog::NewScenario(name)) => {
                    self.editor
                        .add_scenario(apiforge_core::demos::empty_scenario(name.trim()));
                    self.clear_run();
                }
                _ => {}
            }
        } else if close {
            self.dialog = None;
        }
    }

    fn file_result(&mut self, result: files::Outcome) {
        match result {
            files::Outcome::Imported(document) => {
                let mut workspace = document.into_workspace();
                let count = workspace.projects.len();
                let mut first = None;
                for mut project in workspace.projects.drain(..) {
                    if self
                        .editor
                        .workspace()
                        .projects
                        .iter()
                        .any(|existing| existing.id == project.id)
                    {
                        project.id = apiforge_core::ids::new_id("project");
                        project.name = format!("{} (import)", project.name);
                    }
                    first.get_or_insert_with(|| project.id.clone());
                    self.editor.add_project(project);
                }
                if let Some(first) = first {
                    self.editor.switch_project(&first);
                    self.secrets.clear();
                    self.clear_run();
                }
                self.notice = Some(format!("Imported {count} project(s)."));
            }
            files::Outcome::Exported(path) => {
                self.notice = Some(format!("Exported to {}.", path.display()))
            }
            files::Outcome::Failed(message) => self.notice = Some(message),
            files::Outcome::Cancelled => {}
        }
    }
}

/// The workspace of a first start: the shop and ticketing demos, as in the
/// reference editor.
pub fn default_workspace() -> Workspace {
    let projects = vec![
        apiforge_core::demos::shop_project(),
        apiforge_core::demos::tickets_project(),
    ];
    Workspace {
        version: apiforge_core::format::FORMAT_VERSION,
        active_id: projects[0].id.clone(),
        projects,
    }
}

/// Puts the code drafts of the open workspace back into their buffers.
///
/// A draft whose document has changed since it was written is still restored —
/// losing someone's text is worse than showing it against a document that moved —
/// but it is marked stale, and the panel asks what to do with it.
fn restore_drafts(editor: &Editor, drafts: Option<apiforge_storage::Drafts>) -> buffers::Buffers {
    let mut buffers = buffers::Buffers::default();
    let Some(drafts) = drafts else {
        return buffers;
    };
    let project = editor.project();
    for draft in drafts.entries {
        if draft.project_id != project.id {
            continue;
        }
        let (context, printed) = match &draft.target {
            apiforge_storage::DraftTarget::Scenario { id } => {
                let Some(scenario) = project.scenarios.iter().find(|found| &found.id == id) else {
                    continue;
                };
                (
                    buffers::Context::Scenario {
                        project: project.id.clone(),
                        id: id.clone(),
                    },
                    buffers::print(
                        project,
                        &apiforge_script::Document::Scenario(scenario.clone()),
                    ),
                )
            }
            apiforge_storage::DraftTarget::Component { id, revision } => {
                let Some(component) = project
                    .components
                    .iter()
                    .find(|found| &found.id == id && found.revision == *revision)
                else {
                    continue;
                };
                (
                    buffers::Context::Component {
                        project: project.id.clone(),
                        id: id.clone(),
                        revision: *revision,
                    },
                    buffers::print(
                        project,
                        &apiforge_script::Document::Component(component.clone()),
                    ),
                )
            }
        };
        let stale = printed
            .as_deref()
            .map(apiforge_storage::draft_base_hash)
            .is_none_or(|hash| hash != draft.base_hash);
        buffers.restore(
            context,
            buffers::Draft {
                text: draft.text,
                cursor: draft.cursor,
                stale,
            },
        );
    }
    buffers
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_core::model::ErrorPolicy;
    use serde_json::json;
    use std::time::Duration;

    fn wait_until_finished(app: &mut App) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.is_running() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
            app.poll_run();
        }
        app.poll_run();
    }

    fn draw_frame(app: &mut App, context: &egui::Context, events: Vec<egui::Event>) {
        let _ = draw_output(app, context, events);
    }

    fn draw_output(
        app: &mut App,
        context: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1600.0, 1000.0),
                )),
                events,
                ..Default::default()
            },
            |ui| app.ui(ui),
        );
        output.textures_delta.clear();
        output
    }

    fn click_storage_button(app: &mut App, context: &egui::Context, label: &str) {
        for _ in 0..3 {
            draw_frame(app, context, vec![]);
        }
        let output = draw_output(app, context, vec![]);
        let point = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("button text not rendered: {label}"));
        let click = |pressed| egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        draw_frame(
            app,
            context,
            vec![egui::Event::PointerMoved(point), click(true)],
        );
        draw_frame(app, context, vec![click(false)]);
    }

    fn wait_for_storage(app: &mut App) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while (app.save_pending() || app.storage_busy) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
            app.poll_storage();
        }
        app.poll_storage();
        assert!(!app.storage_busy && !app.save_pending());
    }

    #[test]
    fn a_second_window_stays_read_only_and_displays_a_persistent_warning() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspace.json");
        let writer = WorkspaceStore::open_exclusive(&path).unwrap();
        writer.save(&default_workspace()).unwrap();
        let original = std::fs::read(&path).unwrap();
        let context = egui::Context::default();
        let mut app = App::new(
            &context,
            Source::Storage(WorkspaceStore::open_read_only(&path).unwrap()),
        );
        assert!(app.saver.is_none());
        app.notice = None;
        app.editor
            .change_project(|project| project.name = "Local only".into());
        draw_frame(&mut app, &context, vec![]);
        assert!(
            app.storage_message
                .as_deref()
                .unwrap()
                .contains("Changes here are not saved")
        );
        drop(app);
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }

    /// Writes a journal of one interrupted live run beside `path`.
    fn write_interrupted_journal(path: &std::path::Path) {
        use apiforge_core::journal::{EffectState, Journal, JournalEntry};
        let mut run = JournalRun::new("run-7", "scen-1", "Checkout", "2026-10-23T09:00:00.000Z");
        run.record(JournalEntry {
            sequence: 1,
            node_id: "order".into(),
            handle: "order".into(),
            label: Some("Create order".into()),
            phase: Phase::Main,
            method: "POST".into(),
            url: "http://127.0.0.1:4318/demo/shop/carts".into(),
            started_at: "2026-10-23T09:00:01.000Z".into(),
            state: EffectState::InFlight,
            status: None,
            detail: None,
        });
        let mut journal = Journal::default();
        journal.begin(run);
        std::fs::write(
            path.with_file_name("journal.json"),
            serde_json::to_vec_pretty(&journal).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn an_interrupted_live_run_is_reported_and_cleared_only_when_handled() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspace.json");
        let journal_path = directory.path().join("journal.json");
        write_interrupted_journal(&path);
        let context = egui::Context::default();
        let mut app = App::new(
            &context,
            Source::Storage(WorkspaceStore::open_exclusive(&path).unwrap()),
        );
        assert!(app.journal.is_some(), "a writable session owns the journal");
        let runs = app
            .interrupted
            .clone()
            .expect("the interrupted run is read");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].effects().count(), 1);
        draw_frame(&mut app, &context, vec![]);

        // Remind me later keeps the file, so the next start reports it again.
        click_storage_button(&mut app, &context, "Remind me later");
        assert!(app.interrupted.is_none());
        assert!(journal_path.exists());
        drop(app);

        let mut app = App::new(
            &context,
            Source::Storage(WorkspaceStore::open_exclusive(&path).unwrap()),
        );
        assert!(app.interrupted.is_some());
        draw_frame(&mut app, &context, vec![]);
        click_storage_button(&mut app, &context, "Mark as handled");
        assert!(app.interrupted.is_none());
        assert!(!journal_path.exists(), "a handled run leaves the journal");
        assert!(app.notice.is_none());
    }

    #[test]
    fn a_session_without_a_writable_workspace_says_live_runs_are_not_recorded() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspace.json");
        let _writer = WorkspaceStore::open_exclusive(&path).unwrap();
        let context = egui::Context::default();
        // A read-only second window must not write files the first window owns.
        let mut app = App::new(
            &context,
            Source::Storage(WorkspaceStore::open_read_only(&path).unwrap()),
        );
        assert!(app.journal.is_none());
        assert!(app.interrupted.is_none());
        assert!(
            app.runner
                .as_ref()
                .is_some_and(|runner| !runner.is_journalled())
        );
        app.notice = None;
        app.mode = RunMode::Simulation;
        draw_frame(&mut app, &context, vec![]);
        click_storage_button(&mut app, &context, "API");
        assert_eq!(app.mode, RunMode::Live);
        assert!(
            app.notice
                .as_deref()
                .unwrap_or_default()
                .contains("not recorded"),
            "{:?}",
            app.notice
        );
    }

    #[test]
    fn tidy_up_lays_out_the_board_in_one_undo_step_and_clears_the_pins() {
        let context = egui::Context::default();
        let mut app = App::new(&context, Source::Session(default_workspace()));
        // Pile the blocks of the scenario onto one spot and pin them all.
        app.editor.change_project(|project| {
            for node in &mut project.scenarios[0].graph.nodes {
                node.position = Position { x: 400.0, y: 300.0 };
                node.auto_placed = false;
            }
        });
        let before = app.editor.graph().clone();
        assert!(before.nodes.len() >= 3, "the demo scenario has blocks");
        let revision = app.editor.revision();

        app.board_command(Command::TidyUp(vec![]));
        let after = app.editor.graph().clone();
        assert!(
            after.nodes.iter().all(|node| node.auto_placed),
            "tidy up owns the positions it chose"
        );
        let columns: std::collections::BTreeSet<i64> = after
            .nodes
            .iter()
            .map(|node| node.position.x.round() as i64)
            .collect();
        assert!(columns.len() > 1, "the flow is laid out in columns");
        // The board keeps its place: the layout starts where the blocks were.
        let left = after
            .nodes
            .iter()
            .map(|node| node.position.x)
            .fold(f64::INFINITY, f64::min);
        assert_eq!(left, 400.0);
        let places: std::collections::BTreeSet<(i64, i64)> = after
            .nodes
            .iter()
            .map(|node| {
                (
                    node.position.x.round() as i64,
                    node.position.y.round() as i64,
                )
            })
            .collect();
        assert_eq!(
            places.len(),
            after.nodes.len(),
            "no two blocks share a position any more"
        );

        // One undo step brings every block back.
        assert_ne!(app.editor.revision(), revision);
        app.editor.undo();
        let restored = app.editor.graph().clone();
        for (node, original) in restored.nodes.iter().zip(&before.nodes) {
            assert_eq!(node.position, original.position);
            assert!(!node.auto_placed);
        }
    }

    #[test]
    fn tidy_up_with_a_selection_moves_only_the_selected_blocks() {
        let context = egui::Context::default();
        let mut app = App::new(&context, Source::Session(default_workspace()));
        app.editor.change_project(|project| {
            for node in &mut project.scenarios[0].graph.nodes {
                node.position = Position { x: 400.0, y: 300.0 };
            }
        });
        let ids: Vec<String> = app
            .editor
            .graph()
            .nodes
            .iter()
            .map(|node| node.id.clone())
            .collect();
        let selected = ids[..2].to_vec();
        // Put the unselected blocks somewhere definite, so landing on one shows.
        app.editor.change_project(|project| {
            for (index, node) in project.scenarios[0].graph.nodes.iter_mut().enumerate() {
                if index >= 2 {
                    node.position = Position {
                        x: 900.0,
                        y: index as f64 * 40.0,
                    };
                }
            }
        });
        app.board_command(Command::TidyUp(selected.clone()));
        let graph = app.editor.graph().clone();
        for (index, node) in graph.nodes.iter().enumerate() {
            if selected.contains(&node.id) {
                continue;
            }
            assert_eq!(
                node.position,
                Position {
                    x: 900.0,
                    y: index as f64 * 40.0
                },
                "an unselected block stayed where it was"
            );
        }
        // The tidied blocks keep clear of the ones outside the selection.
        let reference = app.editor.reference_graph().into_owned();
        let project = app.editor.project().clone();
        let rect = |node: &apiforge_core::format::Node| {
            canvas::Card::build(node, &reference, &project).rect()
        };
        for node in graph
            .nodes
            .iter()
            .filter(|node| selected.contains(&node.id))
        {
            for other in graph
                .nodes
                .iter()
                .filter(|other| !selected.contains(&other.id))
            {
                assert!(
                    !rect(node).intersects(rect(other)),
                    "a tidied block landed on an unselected one"
                );
            }
        }
    }

    #[test]
    fn moving_a_block_by_hand_pins_it() {
        let context = egui::Context::default();
        let mut app = App::new(&context, Source::Session(default_workspace()));
        let id = app.editor.graph().nodes[0].id.clone();
        app.editor.change_project(|project| {
            project.scenarios[0].graph.nodes[0].auto_placed = true;
        });
        app.board_command(Command::Move(vec![(
            id.clone(),
            Position { x: 120.0, y: 90.0 },
        )]));
        let node = app
            .editor
            .graph()
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap();
        assert_eq!(node.position, Position { x: 120.0, y: 90.0 });
        assert!(!node.auto_placed, "a block moved by hand is pinned");
    }

    #[test]
    fn a_new_block_lands_beside_an_occupied_position_rather_than_on_it() {
        let context = egui::Context::default();
        let mut app = App::new(&context, Source::Session(default_workspace()));
        let occupied = app.editor.graph().nodes[0].position.clone();
        let before = app.editor.graph().nodes.len();
        app.board_command(Command::AddNode {
            block: Block::Variable,
            component: None,
            position: occupied.clone(),
            source: None,
        });
        let graph = app.editor.graph().clone();
        assert_eq!(graph.nodes.len(), before + 1);
        let added = graph.nodes.last().unwrap();
        assert_ne!(
            (added.position.x, added.position.y),
            (occupied.x, occupied.y),
            "the new block was moved out of the way"
        );
        assert!(added.auto_placed, "a block the layout moved is not pinned");
        // Nothing else moved.
        assert_eq!(graph.nodes[0].position, occupied);
    }

    #[test]
    fn the_wire_style_is_remembered_for_the_next_session() {
        let context = egui::Context::default();
        let mut app = App::new(&context, Source::Session(default_workspace()));
        let directory = tempfile::tempdir().unwrap();
        // Keep the user's own preferences file out of the test.
        let path = directory.path().join("preferences.json");
        app.preferences = crate::preferences::Preferences::at(Some(path.clone()));
        assert_eq!(app.canvas.wire_style, canvas::WireStyle::Orthogonal);
        app.board_command(Command::SetWireStyle(canvas::WireStyle::Spline));
        app.board_command(Command::SetBridges(false));
        assert_eq!(app.canvas.wire_style, canvas::WireStyle::Spline);
        assert!(!app.canvas.bridges);
        let stored = crate::preferences::Preferences::at(Some(path));
        assert_eq!(stored.wire_style, canvas::WireStyle::Spline);
        assert!(!stored.bridges);
    }

    /// Types `text` into the code panel and lets the pause before applying pass.
    fn edit_code(app: &mut App, context: &egui::Context, text: &str) {
        let key = app.code_context();
        app.buffers.typed(&key, text.to_owned(), Instant::now());
        std::thread::sleep(crate::buffers::APPLY_DELAY + Duration::from_millis(5));
        let project = app.editor.project().clone();
        let document = app.code_document();
        app.apply_code(&key, &project, &document);
        draw_frame(app, context, vec![]);
    }

    #[test]
    fn the_code_panel_shows_the_scenario_and_applies_what_is_typed() {
        let context = egui::Context::default();
        let mut app = App::new(&context, Source::Session(default_workspace()));
        app.code_open = true;
        draw_frame(&mut app, &context, vec![]);
        let key = app.code_context();
        let printed = app
            .buffers
            .get(&key)
            .map(|buffer| buffer.text.clone())
            .expect("the scenario printed into the panel");
        assert!(printed.starts_with("scenario \""), "{printed}");

        let renamed = printed.replacen("scenario \"", "scenario \"Renamed ", 1);
        edit_code(&mut app, &context, &renamed);
        assert!(
            app.editor.scenario().name.starts_with("Renamed "),
            "the document took the change: {}",
            app.editor.scenario().name
        );
        // One undo step puts the whole scenario back.
        app.editor.undo();
        assert!(!app.editor.scenario().name.starts_with("Renamed "));
    }

    #[test]
    fn a_block_added_in_code_lands_on_the_board_with_a_position() {
        let context = egui::Context::default();
        let mut app = App::new(&context, Source::Session(default_workspace()));
        app.code_open = true;
        draw_frame(&mut app, &context, vec![]);
        let key = app.code_context();
        let printed = app.buffers.get(&key).unwrap().text.clone();
        let closing = printed.rfind('}').unwrap();
        let mut extended = printed.clone();
        extended.insert_str(closing, "  var from_code = 7\n");
        let before: Vec<String> = app
            .editor
            .graph()
            .nodes
            .iter()
            .map(|node| node.id.clone())
            .collect();
        edit_code(&mut app, &context, &extended);

        let graph = app.editor.graph().clone();
        assert_eq!(graph.nodes.len(), before.len() + 1);
        let fresh = graph
            .nodes
            .iter()
            .find(|node| node.handle == "from_code")
            .expect("the new block reached the board");
        assert!(fresh.auto_placed, "the layout chose where it goes");
        let reference = app.editor.reference_graph().into_owned();
        let project = app.editor.project().clone();
        let rect = |node: &apiforge_core::format::Node| {
            canvas::Card::build(node, &reference, &project).rect()
        };
        for other in graph.nodes.iter().filter(|node| node.id != fresh.id) {
            assert!(
                !rect(fresh).intersects(rect(other)),
                "the new block landed on {}",
                other.handle
            );
        }
    }

    #[test]
    fn code_with_errors_holds_the_board_until_it_is_fixed_or_discarded() {
        let context = egui::Context::default();
        let mut app = App::new(&context, Source::Session(default_workspace()));
        app.code_open = true;
        draw_frame(&mut app, &context, vec![]);
        let key = app.code_context();
        let printed = app.buffers.get(&key).unwrap().text.clone();
        let revision = app.editor.revision();

        edit_code(&mut app, &context, &format!("{printed}\n  oops ="));
        assert!(app.buffers.blocks_the_board(&key), "the board waits");
        assert_eq!(
            app.editor.revision(),
            revision,
            "a broken buffer changes no document"
        );
        // The text is not thrown away by a reprint while it is being fixed.
        draw_frame(&mut app, &context, vec![]);
        assert!(app.buffers.get(&key).unwrap().text.ends_with("oops ="));

        app.buffers.discard(&key);
        draw_frame(&mut app, &context, vec![]);
        assert!(!app.buffers.blocks_the_board(&key));
        assert_eq!(app.buffers.get(&key).unwrap().text, printed);
    }

    #[test]
    fn a_board_edit_reprints_the_code_beside_it() {
        let context = egui::Context::default();
        let mut app = App::new(&context, Source::Session(default_workspace()));
        app.code_open = true;
        draw_frame(&mut app, &context, vec![]);
        let key = app.code_context();
        let before = app.buffers.get(&key).unwrap().text.clone();
        let id = app.editor.graph().nodes[0].id.clone();
        app.board_command(Command::Move(vec![(id, Position { x: 640.0, y: 480.0 })]));
        draw_frame(&mut app, &context, vec![]);
        // Layout is not code, so moving a block changes nothing in the text.
        assert_eq!(app.buffers.get(&key).unwrap().text, before);

        app.editor.change_project(|project| {
            project.scenarios[0].description = "Edited on the board".into();
        });
        draw_frame(&mut app, &context, vec![]);
        let after = app.buffers.get(&key).unwrap().text.clone();
        assert!(
            after.contains("Edited on the board"),
            "the panel followed the document: {after}"
        );
    }

    #[test]
    fn unfinished_code_survives_closing_and_comes_back() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspace.json");
        let context = egui::Context::default();
        let broken = {
            let mut app = App::new(
                &context,
                Source::Storage(WorkspaceStore::open_exclusive(&path).unwrap()),
            );
            app.code_open = true;
            draw_frame(&mut app, &context, vec![]);
            let key = app.code_context();
            let printed = app.buffers.get(&key).unwrap().text.clone();
            let broken = format!("{printed}\n  half_written =");
            edit_code(&mut app, &context, &broken);
            assert!(app.buffers.blocks_the_board(&key));
            app.save_drafts();
            wait_for_storage(&mut app);
            drop(app);
            broken
        };
        assert!(directory.path().join("drafts.json").exists());

        // A new window finds the text where it was left.
        let mut app = App::new(
            &context,
            Source::Storage(WorkspaceStore::open_exclusive(&path).unwrap()),
        );
        let key = app.code_context();
        let restored = app
            .buffers
            .get(&key)
            .expect("the draft came back")
            .text
            .clone();
        assert_eq!(restored, broken);
        app.code_open = true;
        draw_frame(&mut app, &context, vec![]);
        assert!(
            app.buffers.blocks_the_board(&key),
            "the restored draft still has its error"
        );
    }

    #[test]
    fn a_draft_written_against_an_older_document_comes_back_marked_stale() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspace.json");
        let context = egui::Context::default();
        {
            let mut app = App::new(
                &context,
                Source::Storage(WorkspaceStore::open_exclusive(&path).unwrap()),
            );
            app.code_open = true;
            draw_frame(&mut app, &context, vec![]);
            let key = app.code_context();
            let printed = app.buffers.get(&key).unwrap().text.clone();
            edit_code(&mut app, &context, &format!("{printed}\n  half ="));
            app.save_drafts();
            // The document moves on after the draft was written.
            app.editor.change_project(|project| {
                project.scenarios[0].description = "Changed afterwards".into();
            });
            wait_for_storage(&mut app);
            app.save_if_changed();
            wait_for_storage(&mut app);
        }
        let app = App::new(
            &context,
            Source::Storage(WorkspaceStore::open_exclusive(&path).unwrap()),
        );
        let key = app.code_context();
        let buffer = app.buffers.get(&key).expect("the draft came back");
        assert!(buffer.stale, "the user is asked what to do with it");
        assert!(buffer.text.ends_with("half ="), "nothing was thrown away");
    }

    #[test]
    fn a_draft_never_holds_a_secret() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspace.json");
        let context = egui::Context::default();
        let mut app = App::new(
            &context,
            Source::Storage(WorkspaceStore::open_exclusive(&path).unwrap()),
        );
        app.secrets.insert("apiToken".into(), "s3cret-value".into());
        app.code_open = true;
        draw_frame(&mut app, &context, vec![]);
        let key = app.code_context();
        let printed = app.buffers.get(&key).unwrap().text.clone();
        // Text that spells a secret out is refused rather than written.
        edit_code(&mut app, &context, &format!("{printed}\n// s3cret-value"));
        app.notice = None;
        app.save_drafts();
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("not saved")),
            "{:?}",
            app.notice
        );
        let written =
            std::fs::read_to_string(directory.path().join("drafts.json")).unwrap_or_default();
        assert!(!written.contains("s3cret-value"));
    }

    #[test]
    fn running_with_broken_code_asks_before_it_runs_the_saved_version() {
        let context = egui::Context::default();
        let mut app = App::new(&context, Source::Session(default_workspace()));
        app.code_open = true;
        draw_frame(&mut app, &context, vec![]);
        let key = app.code_context();
        let printed = app.buffers.get(&key).unwrap().text.clone();
        edit_code(&mut app, &context, &format!("{printed}\n  broken ="));

        app.start_run(RunKind::Full);
        assert!(app.run_prompt.is_some(), "the question is asked first");
        assert!(app.run_handle.is_none(), "and nothing ran yet");
        draw_frame(&mut app, &context, vec![]);
        click_storage_button(&mut app, &context, "Run the last valid version");
        assert!(app.run_prompt.is_none());
        wait_until_finished(&mut app);
        let run = app.run.clone().expect("the saved scenario ran");
        assert_ne!(run.status, RunStatus::Running);
        // Still broken, so the next run asks again.
        app.start_run(RunKind::Full);
        assert!(app.run_prompt.is_some());
    }

    #[test]
    fn the_caret_and_the_board_point_at_the_same_block() {
        let context = egui::Context::default();
        let mut app = App::new(&context, Source::Session(default_workspace()));
        app.code_open = true;
        draw_frame(&mut app, &context, vec![]);
        let key = app.code_context();
        let text = app.buffers.get(&key).unwrap().text.clone();
        let handle = app.editor.graph().nodes[1].handle.clone();
        let id = app.editor.graph().nodes[1].id.clone();

        // The caret leads: putting it in a statement selects that block.
        let at = apiforge_script::complete::statement_range(&text, &handle)
            .expect("the block is written in the code")
            .start
            + 1;
        app.sync_selection(&text, Some(at));
        assert_eq!(app.editor.selection, vec![id.clone()]);

        // With the caret away, a selection on the board reveals its statement.
        app.editor.selection = vec![app.editor.graph().nodes[0].id.clone()];
        app.revealed = None;
        app.sync_selection(&text, None);
        assert_eq!(
            app.revealed.as_deref(),
            Some(app.editor.graph().nodes[0].handle.as_str())
        );
        // It is revealed once, not pulled back on every frame.
        app.sync_selection(&text, None);
        assert!(app.revealed.is_some());
    }

    #[test]
    fn opening_a_request_in_code_shows_it_in_the_panel() {
        let context = egui::Context::default();
        let mut app = App::new(&context, Source::Session(default_workspace()));
        let node = app.editor.graph().nodes[0].clone();
        assert!(!app.code_open);
        let directory = tempfile::tempdir().unwrap();
        app.preferences =
            crate::preferences::Preferences::at(Some(directory.path().join("preferences.json")));

        app.inspector_action(inspector::Action::Reveal(node.handle.clone()));
        assert!(app.code_open, "the panel opened");
        assert_eq!(app.editor.selection, vec![node.id.clone()]);
        draw_frame(&mut app, &context, vec![]);
        let key = app.code_context();
        let text = app.buffers.get(&key).unwrap().text.clone();
        assert!(
            apiforge_script::complete::statement_range(&text, &node.handle).is_some(),
            "the block is written in the code the panel shows"
        );
        assert_eq!(app.revealed.as_deref(), Some(node.handle.as_str()));
    }

    /// The session the plan describes, start to finish: type in code, edit on the
    /// board, undo, break the code, switch away and back, close and reopen, fix
    /// it. Nothing typed is lost, and whenever the text is valid the document
    /// says the same thing.
    #[test]
    fn a_whole_editing_session_keeps_the_text_and_the_document_in_step() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspace.json");
        let context = egui::Context::default();
        let mut app = App::new(
            &context,
            Source::Storage(WorkspaceStore::open_exclusive(&path).unwrap()),
        );
        app.code_open = true;
        draw_frame(&mut app, &context, vec![]);
        let key = app.code_context();

        // 1. Type in the code: a new block arrives on the board.
        let printed = app.buffers.get(&key).unwrap().text.clone();
        let closing = printed.rfind('}').unwrap();
        let mut typed = printed.clone();
        typed.insert_str(closing, "  var from_code = 7\n");
        edit_code(&mut app, &context, &typed);
        assert!(
            app.editor
                .graph()
                .nodes
                .iter()
                .any(|node| node.handle == "from_code")
        );

        // 2. Edit on the board: the code follows.
        app.editor.change_project(|project| {
            project.scenarios[0].description = "Edited on the board".into();
        });
        draw_frame(&mut app, &context, vec![]);
        assert!(
            app.buffers
                .get(&key)
                .unwrap()
                .text
                .contains("Edited on the board")
        );

        // 3. Undo the document edit: the block from the code goes with it.
        app.editor.undo();
        draw_frame(&mut app, &context, vec![]);

        // 4. Break the code: the document stands still and the board waits.
        let valid = app.buffers.get(&key).unwrap().text.clone();
        let revision = app.editor.revision();
        edit_code(&mut app, &context, &format!("{valid}\n  broken ="));
        assert!(app.buffers.blocks_the_board(&key));
        assert_eq!(app.editor.revision(), revision);

        // 5. Switch scenario and back: the broken text is still there.
        let other = {
            let scenario = apiforge_core::format::Scenario {
                id: "second".into(),
                name: "Second".into(),
                description: String::new(),
                graph: Graph {
                    nodes: vec![],
                    edges: vec![],
                },
                cleanup: Graph {
                    nodes: vec![],
                    edges: vec![],
                },
                parameters: Default::default(),
                error_policy: ErrorPolicy::Stop,
            };
            app.editor.add_scenario(scenario);
            "second".to_owned()
        };
        draw_frame(&mut app, &context, vec![]);
        app.editor
            .switch_scenario(&app.editor.scenario().id.clone());
        app.editor.switch_scenario(&other);
        draw_frame(&mut app, &context, vec![]);
        let first = match &key {
            buffers::Context::Scenario { id, .. } => id.clone(),
            buffers::Context::Component { id, .. } => id.clone(),
        };
        app.editor.switch_scenario(&first);
        draw_frame(&mut app, &context, vec![]);
        assert!(
            app.buffers.get(&key).unwrap().text.ends_with("broken ="),
            "the unfinished text survived the detour"
        );

        // 6. Close and reopen: it is still there.
        app.save_drafts();
        wait_for_storage(&mut app);
        drop(app);
        let mut app = App::new(
            &context,
            Source::Storage(WorkspaceStore::open_exclusive(&path).unwrap()),
        );
        app.code_open = true;
        draw_frame(&mut app, &context, vec![]);
        let key = app.code_context();
        assert!(app.buffers.get(&key).unwrap().text.ends_with("broken ="));

        // 7. Fix it: the board is released and the document matches the text.
        let fixed = app
            .buffers
            .get(&key)
            .unwrap()
            .text
            .replace("\n  broken =", "");
        edit_code(&mut app, &context, &fixed);
        assert!(!app.buffers.blocks_the_board(&key));
        let document = app.code_document();
        let project = app.editor.project().clone();
        let printed = buffers::print(&project, &document).expect("the document prints");
        let lowered = apiforge_script::lower(
            &apiforge_script::parse(&app.buffers.get(&key).unwrap().text),
            &apiforge_script::LowerContext::document(&project),
        );
        assert!(!lowered.has_errors());
        assert_eq!(
            apiforge_script::semantic(&lowered.document.unwrap(), &project),
            apiforge_script::semantic(
                &apiforge_script::lower(
                    &apiforge_script::parse(&printed),
                    &apiforge_script::LowerContext::document(&project),
                )
                .document
                .unwrap(),
                &project
            ),
            "the text and the document say the same thing"
        );
    }

    #[test]
    fn renaming_a_block_in_code_keeps_where_it_sits() {
        let context = egui::Context::default();
        let mut app = App::new(&context, Source::Session(default_workspace()));
        app.code_open = true;
        draw_frame(&mut app, &context, vec![]);
        let key = app.code_context();
        let text = app.buffers.get(&key).unwrap().text.clone();
        // A Value block: its handle is its own, not a component's name as well.
        let node = app
            .editor
            .graph()
            .nodes
            .iter()
            .find(|node| node.kind == apiforge_core::model::NodeKind::Value)
            .expect("the demo has a value block")
            .clone();
        let renamed = text.replace(&node.handle, "renamed_block");
        edit_code(&mut app, &context, &renamed);

        let found = app
            .editor
            .graph()
            .nodes
            .iter()
            .find(|found| found.handle == "renamed_block")
            .expect("the block was renamed, not replaced");
        assert_eq!(found.id, node.id, "it is the same block");
        assert_eq!(found.position, node.position, "and it did not move");
    }

    #[test]
    fn the_code_panel_remembers_whether_it_was_open() {
        let context = egui::Context::default();
        let mut app = App::new(&context, Source::Session(default_workspace()));
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preferences.json");
        app.preferences = crate::preferences::Preferences::at(Some(path.clone()));
        assert!(!app.code_open);
        app.toggle_code_panel();
        assert!(app.code_open);
        assert!(crate::preferences::Preferences::at(Some(path)).code_panel);
    }

    #[test]
    fn saving_a_component_revision_keeps_the_export_added_on_its_board() {
        let context = egui::Context::default();
        let mut app = App::new(&context, Source::Session(default_workspace()));
        let initial = app.editor.project().components[0].clone();
        app.editor.enter_component(initial.clone());
        let mut graph = app.editor.graph().clone();
        let mut export = Block::Export.create(Position { x: 900.0, y: 300.0 }, None);
        export.handle = "additional_output".into();
        export.note = Some("Added on the component board".into());
        export
            .inputs
            .insert("value".into(), Binding::literal(json!(99)));
        let export_id = export.id.clone();
        graph.nodes.push(export);
        app.apply_graph(graph);
        let graph = app.editor.graph();
        let ids = graph
            .nodes
            .iter()
            .map(|node| node.id.clone())
            .collect::<Vec<_>>();
        app.dialog = Some(Dialog::Component(Box::new(dialogs::ComponentDialog::new(
            graph,
            &ids,
            app.editor.project(),
            Some(&initial),
        ))));
        for _ in 0..3 {
            draw_frame(&mut app, &context, vec![]);
        }
        draw_frame(&mut app, &context, vec![command_key(Key::Enter)]);
        assert!(app.dialog.is_none());
        let saved = app
            .editor
            .project()
            .components
            .iter()
            .find(|component| {
                component.id == initial.id && component.revision == initial.revision + 1
            })
            .unwrap();
        let exported = saved
            .graph
            .nodes
            .iter()
            .find(|node| node.handle == "additional_output")
            .unwrap();
        assert_eq!(exported.id, export_id);
        assert_eq!(exported.inputs["value"], Binding::literal(json!(99)));
        assert_eq!(
            exported.note.as_deref(),
            Some("Added on the component board")
        );
        assert!(app.editor.project().components.contains(&initial));
        assert!(app.editor.draft().is_none());
    }

    #[test]
    fn startup_restores_a_backup_only_after_clicking_the_restore_button() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspace.json");
        let store = WorkspaceStore::open_exclusive(&path).unwrap();
        let mut original = default_workspace();
        original.projects[0].name = "Recovered project".into();
        store.save(&original).unwrap();
        store.save(&original).unwrap();
        std::fs::write(&path, b"{ broken").unwrap();
        let context = egui::Context::default();
        let mut app = App::new(&context, Source::Storage(store.clone()));
        assert!(matches!(
            app.storage_prompt,
            Some(StoragePrompt::Recovery(_))
        ));
        assert_ne!(app.editor.project().name, "Recovered project");
        draw_frame(&mut app, &context, vec![]);
        assert_eq!(std::fs::read(&path).unwrap(), b"{ broken");
        click_storage_button(&mut app, &context, "Restore backup");
        assert!(app.storage_busy);
        wait_for_storage(&mut app);
        assert!(app.storage_prompt.is_none());
        assert_eq!(app.editor.project().name, "Recovered project");
        assert!(store.load().unwrap().unwrap().recovered.is_none());
    }

    #[test]
    fn escape_declines_recovery_without_touching_either_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspace.json");
        let store = WorkspaceStore::open_exclusive(&path).unwrap();
        store.save(&default_workspace()).unwrap();
        store.save(&default_workspace()).unwrap();
        let backup_path = directory.path().join("workspace.json.bak");
        let backup = std::fs::read(&backup_path).unwrap();
        std::fs::write(&path, b"{ broken").unwrap();
        let context = egui::Context::default();
        let mut app = App::new(&context, Source::Storage(store));
        draw_frame(&mut app, &context, vec![]);
        draw_frame(
            &mut app,
            &context,
            vec![egui::Event::Key {
                key: Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert!(app.storage_prompt.is_none());
        assert!(app.saver.is_none());
        assert!(app.storage_message.is_some());
        assert!(app.prepare_close());
        drop(app);
        assert_eq!(std::fs::read(path).unwrap(), b"{ broken");
        assert_eq!(std::fs::read(backup_path).unwrap(), backup);
    }

    #[test]
    fn external_changes_show_a_conflict_and_reload_discards_only_local_edits() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspace.json");
        let store = WorkspaceStore::open_exclusive(&path).unwrap();
        store.save(&default_workspace()).unwrap();
        let context = egui::Context::default();
        let mut app = App::new(&context, Source::Storage(store.clone()));
        let mut external = default_workspace();
        external.projects[0].name = "Changed on disk".into();
        let bytes = serde_json::to_vec(&external).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        app.editor
            .change_project(|project| project.name = "Local edit".into());
        app.save_if_changed();
        app.saver.as_ref().unwrap().flush();
        wait_for_storage(&mut app);
        assert!(matches!(
            app.storage_prompt,
            Some(StoragePrompt::Conflict(_))
        ));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert!(!app.prepare_close());
        assert!(app.close_save_error().is_some());
        click_storage_button(&mut app, &context, "Reload and discard local changes");
        wait_for_storage(&mut app);
        assert!(app.storage_prompt.is_none());
        assert_eq!(app.editor.project().name, "Changed on disk");
        app.editor
            .change_project(|project| project.name = "New edit after reload".into());
        app.save_if_changed();
        app.saver.as_ref().unwrap().flush();
        wait_for_storage(&mut app);
        assert_eq!(
            store.load().unwrap().unwrap().workspace.projects[0].name,
            "New edit after reload"
        );
    }

    fn command_key(key: Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            },
        }
    }

    #[test]
    fn keyboard_navigation_duplicate_and_undo_work_through_the_window() {
        let context = egui::Context::default();
        let mut app = App::new(&context, Source::Session(default_workspace()));
        let target = app.editor.graph().nodes[1].clone();
        let original = app.editor.graph().clone();
        draw_frame(&mut app, &context, vec![]);
        draw_frame(&mut app, &context, vec![command_key(Key::K)]);
        assert!(app.navigator.is_open());
        draw_frame(
            &mut app,
            &context,
            vec![egui::Event::Text(target.display_name().to_owned())],
        );
        draw_frame(
            &mut app,
            &context,
            vec![egui::Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert_eq!(app.editor.selection, vec![target.id]);
        assert_eq!(
            app.editor.graph(),
            &original,
            "navigation must not edit the document"
        );
        draw_frame(&mut app, &context, vec![]);
        draw_frame(&mut app, &context, vec![]);
        draw_frame(&mut app, &context, vec![command_key(Key::D)]);
        assert_eq!(app.editor.graph().nodes.len(), original.nodes.len() + 1);
        draw_frame(&mut app, &context, vec![command_key(Key::Z)]);
        assert_eq!(app.editor.graph(), &original);
    }

    #[test]
    fn pasting_into_an_inspector_text_field_does_not_paste_blocks() {
        let context = egui::Context::default();
        let mut app = App::new(&context, Source::Session(default_workspace()));
        let id = app.editor.graph().nodes[1].id.clone();
        app.select_node(&id);
        app.board_command(Command::Copy);
        let count = app.editor.graph().nodes.len();
        draw_frame(&mut app, &context, vec![]);
        let field_id = egui::Id::new(("inspector", &id)).with("name");
        let rect = context
            .read_response(field_id)
            .expect("name field is rendered")
            .rect;
        let click = |pressed| egui::Event::PointerButton {
            pos: rect.center(),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        draw_frame(
            &mut app,
            &context,
            vec![egui::Event::PointerMoved(rect.center()), click(true)],
        );
        draw_frame(&mut app, &context, vec![click(false)]);
        draw_frame(
            &mut app,
            &context,
            vec![egui::Event::Paste(" changed ".into())],
        );
        assert_eq!(app.editor.graph().nodes.len(), count);
        assert!(
            app.editor
                .selected_node()
                .unwrap()
                .display_name()
                .contains(" changed ")
        );
    }

    #[test]
    fn unreadable_startup_documents_are_protected_from_autosave() {
        for bytes in [b"{ truncated".as_slice(), br#"{"version":2}"#.as_slice()] {
            let directory = tempfile::tempdir().unwrap();
            let store =
                WorkspaceStore::open_exclusive(directory.path().join("workspace.json")).unwrap();
            std::fs::write(store.path(), bytes).unwrap();
            let mut app = App::new(&egui::Context::default(), Source::Storage(store.clone()));
            assert!(app.saver.is_none());
            assert!(app.notice.as_deref().unwrap().contains("autosave disabled"));
            app.editor
                .change_project(|project| project.name = "Temporary edits".into());
            app.save_if_changed();
            drop(app);
            assert_eq!(std::fs::read(store.path()).unwrap(), bytes);
        }
    }

    #[test]
    fn legacy_startup_archives_originals_and_allows_saving_a_fresh_workspace() {
        let directory = tempfile::tempdir().unwrap();
        let store =
            WorkspaceStore::open_exclusive(directory.path().join("workspace.json")).unwrap();
        let legacy = include_bytes!("../../../tools/convert-v1/sources/workspace-v1.json");
        std::fs::write(store.path(), legacy).unwrap();
        std::fs::write(directory.path().join("workspace.json.bak"), legacy).unwrap();
        let mut app = App::new(&egui::Context::default(), Source::Storage(store.clone()));
        assert!(app.saver.is_some());
        assert!(
            app.notice
                .as_deref()
                .unwrap()
                .contains("apiforge-convert-v1")
        );
        assert!(app.notice.as_deref().unwrap().contains("workspace.v1.json"));
        app.editor
            .change_project(|project| project.name = "New v2 workspace".into());
        app.save_if_changed();
        drop(app);
        assert_eq!(
            store.load().unwrap().unwrap().workspace.projects[0].name,
            "New v2 workspace"
        );
        for name in ["workspace.v1.json", "workspace.v1.json.bak"] {
            assert_eq!(std::fs::read(directory.path().join(name)).unwrap(), legacy);
        }
    }

    #[test]
    fn a_legacy_archive_conflict_disables_autosave_and_keeps_the_original() {
        let directory = tempfile::tempdir().unwrap();
        let store =
            WorkspaceStore::open_exclusive(directory.path().join("workspace.json")).unwrap();
        let legacy = include_bytes!("../../../tools/convert-v1/sources/workspace-v1.json");
        std::fs::write(store.path(), legacy).unwrap();
        let archive = directory.path().join("workspace.v1.json");
        std::fs::write(&archive, b"earlier archive").unwrap();
        let mut app = App::new(&egui::Context::default(), Source::Storage(store.clone()));
        assert!(app.saver.is_none());
        assert!(app.notice.as_deref().unwrap().contains("autosave disabled"));
        app.editor
            .change_project(|project| project.name = "Temporary edits".into());
        app.save_if_changed();
        drop(app);
        assert_eq!(std::fs::read(store.path()).unwrap(), legacy);
        assert_eq!(std::fs::read(archive).unwrap(), b"earlier archive");
    }

    #[test]
    fn close_keeps_unsaved_changes_open_and_retry_persists_them() {
        let directory = tempfile::tempdir().unwrap();
        let blocker = directory.path().join("workspace.json.bak");
        let store =
            WorkspaceStore::open_exclusive(directory.path().join("workspace.json")).unwrap();
        store.save(&default_workspace()).unwrap();
        let mut app = App::new(&egui::Context::default(), Source::Storage(store.clone()));
        std::fs::create_dir(&blocker).unwrap();
        app.editor
            .change_project(|project| project.name = "Must survive close".into());
        assert!(!app.prepare_close());
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.save_pending() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!app.prepare_close());
        let error = app.close_save_error().expect("failed I/O must block close");
        app.abort_close_after_save_error(error);
        assert!(!app.closing);
        assert!(app.notice.as_deref().unwrap().contains("staying open"));
        assert_eq!(app.editor.project().name, "Must survive close");

        std::fs::remove_dir(&blocker).unwrap();
        app.retry_save();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !app.prepare_close() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(app.prepare_close());
        assert_eq!(
            store.load().unwrap().unwrap().workspace.projects[0].name,
            "Must survive close"
        );
    }

    #[test]
    fn duplicating_cleanup_preserves_main_references_and_can_be_undone() {
        let ctx = egui::Context::default();
        let mut app = App::new(&ctx, Source::Session(default_workspace()));
        app.editor.set_phase(Phase::Cleanup);
        let original = app.editor.graph().clone();
        let node = original
            .nodes
            .iter()
            .find(|node| {
                node.inputs
                    .values()
                    .flat_map(Binding::references)
                    .any(|reference| {
                        app.editor
                            .scenario()
                            .graph
                            .nodes
                            .iter()
                            .any(|main| main.id == reference.node_id)
                    })
            })
            .expect("shop cleanup reads a main-graph response");
        let inputs = node.inputs.clone();
        app.editor.selection = vec![node.id.clone()];

        app.board_command(Command::Duplicate);

        assert_eq!(app.editor.graph().nodes.len(), original.nodes.len() + 1);
        let duplicate = app.editor.selected_node().unwrap();
        assert_ne!(duplicate.id, node.id);
        assert_eq!(duplicate.inputs, inputs);
        app.editor.undo();
        assert_eq!(app.editor.graph(), &original);
        app.editor.redo();
        assert_eq!(app.editor.graph().nodes.len(), original.nodes.len() + 1);
    }

    #[test]
    fn preparing_close_waits_for_active_cleanup_without_blocking() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            sync::mpsc,
        };

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (entered, cleanup_started) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = [0; 2048];
            assert!(stream.read(&mut request).unwrap() > 0);
            entered.send(()).unwrap();
            released.recv_timeout(Duration::from_secs(5)).unwrap();
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}").unwrap();
        });

        let mut project = apiforge_core::demos::empty_project();
        project.environment.base_url = format!("http://{address}");
        let mut cleanup = apiforge_core::graph::create_node(
            apiforge_core::model::NodeKind::Request,
            Position { x: 0.0, y: 0.0 },
            None,
        );
        cleanup.inputs.insert(
            apiforge_core::graph::URL_INPUT.into(),
            Binding::literal(serde_json::json!("/cleanup")),
        );
        project.scenarios[0].cleanup.nodes.push(cleanup);
        let workspace = Workspace {
            version: apiforge_core::format::FORMAT_VERSION,
            active_id: project.id.clone(),
            projects: vec![project],
        };
        let mut app = App::new(&egui::Context::default(), Source::Session(workspace));
        app.mode = RunMode::Live;
        app.start_run(RunKind::Full);
        cleanup_started
            .recv_timeout(Duration::from_secs(5))
            .unwrap();

        assert!(
            !app.prepare_close(),
            "an active cleanup must keep the window open"
        );
        // Repeated frames cannot abort cleanup or start another run.
        assert!(!app.prepare_close());
        app.start_run(RunKind::Full);
        release.send(()).unwrap();
        wait_until_finished(&mut app);
        assert!(app.prepare_close());
        assert!(
            app.run
                .as_ref()
                .unwrap()
                .steps
                .iter()
                .any(|step| step.phase == Phase::Cleanup
                    && step.status == apiforge_core::run::StepStatus::Passed)
        );
        server.join().unwrap();
    }

    #[test]
    fn simulation_run_completes_and_detects_staleness_on_edit() {
        let ctx = egui::Context::default();
        let mut app = App::new(&ctx, Source::Session(default_workspace()));
        app.start_run(RunKind::Full);
        wait_until_finished(&mut app);

        let run = app.run.as_ref().expect("run report should be present");
        assert_eq!(run.status, RunStatus::Passed);
        assert!(!app.is_run_stale());

        // Single-step run with compatible previous_run succeeds.
        let target_id = app.editor.scenario().graph.nodes[1].id.clone();
        app.editor.selection = vec![target_id.clone()];
        app.start_run(RunKind::SingleSelected);
        wait_until_finished(&mut app);
        let single_run = app.run.as_ref().expect("single step report");
        assert!(single_run.diagnostic);
        let main_steps: Vec<_> = single_run
            .steps
            .iter()
            .filter(|step| step.phase == Phase::Main)
            .collect();
        assert_eq!(main_steps.len(), 1);
        assert_eq!(main_steps[0].node_id, target_id);

        // Editing the graph marks the report as stale and blocks stale assertions/single-step reuse.
        let mut graph = app.editor.graph().clone();
        graph.nodes[0].label = Some("Edited name".into());
        app.apply_graph(graph);
        assert!(app.is_run_stale());

        let count_before = app.editor.graph().nodes.len();
        app.report_action(report::Action::AssertField {
            node_id: target_id.clone(),
            port: "body".into(),
            path: "status".into(),
            value: json!(200),
        });
        assert_eq!(app.editor.graph().nodes.len(), count_before);
        assert!(
            app.notice
                .as_deref()
                .unwrap_or_default()
                .contains("changed after this run")
        );
    }

    #[test]
    fn report_actions_switch_phase_and_add_assertions() {
        let ctx = egui::Context::default();
        let mut app = App::new(&ctx, Source::Session(default_workspace()));
        app.start_run(RunKind::Full);
        wait_until_finished(&mut app);

        let cleanup_id = app.editor.scenario().cleanup.nodes[0].id.clone();
        app.report_action(report::Action::SelectNode(cleanup_id.clone()));
        assert_eq!(app.editor.phase(), Phase::Cleanup);
        assert_eq!(app.editor.selection, vec![cleanup_id.clone()]);

        let cleanup_before = app.editor.scenario().cleanup.nodes.len();
        app.editor.set_phase(Phase::Main);
        app.report_action(report::Action::AssertField {
            node_id: cleanup_id,
            port: "status".into(),
            path: String::new(),
            value: json!(200),
        });
        assert_eq!(app.editor.phase(), Phase::Cleanup);
        assert_eq!(
            app.editor.scenario().cleanup.nodes.len(),
            cleanup_before + 1
        );

        app.report_action(report::Action::SetErrorPolicy(ErrorPolicy::Stop));
        assert_eq!(app.editor.scenario().error_policy, ErrorPolicy::Stop);
    }

    #[test]
    fn compact_layout_mode_and_960x540_frame_render_cleanly() {
        assert_eq!(layout_mode(egui::vec2(960.0, 540.0)), LayoutMode::Compact);
        assert_eq!(layout_mode(egui::vec2(1280.0, 800.0)), LayoutMode::Standard);

        let ctx = egui::Context::default();
        let mut app = App::new(&ctx, Source::Session(default_workspace()));
        let first_id = app.editor.graph().nodes[0].id.clone();
        app.editor.selection = vec![first_id];

        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(960.0, 540.0),
            )),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input.take(), |ui| {
            app.ui(ui);
        });
        output.textures_delta.clear();
        assert!(!app.inspector_collapsed);
    }
    #[test]
    fn delete_does_not_reach_the_board_behind_a_modal() {
        let context = egui::Context::default();
        let mut app = App::new(&context, Source::Session(default_workspace()));
        app.editor.selection = vec![app.editor.graph().nodes[1].id.clone()];
        let count = app.editor.graph().nodes.len();
        draw_frame(&mut app, &context, vec![]);
        app.dialog = Some(Dialog::Settings(dialogs::SettingsDialog::new(
            app.editor.project(),
            &app.editor.scenario().id,
        )));
        for _ in 0..3 {
            draw_frame(&mut app, &context, vec![]);
        }
        context.memory_mut(|memory| {
            if let Some(id) = memory.focused() {
                memory.surrender_focus(id);
            }
        });
        draw_frame(
            &mut app,
            &context,
            vec![egui::Event::Key {
                key: Key::Delete,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert_eq!(app.editor.graph().nodes.len(), count);
    }
    #[test]
    fn report_grip_gets_keyboard_focus_above_the_canvas_and_drags() {
        let context = egui::Context::default();
        let mut app = App::new(&context, Source::Session(default_workspace()));
        app.report.open = true;
        for _ in 0..3 {
            draw_frame(&mut app, &context, vec![]);
        }
        let (id, rect, before) = app.report.debug_resize_handle();
        let click = |pressed, pos| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        draw_frame(
            &mut app,
            &context,
            vec![
                egui::Event::PointerMoved(rect.center()),
                click(true, rect.center()),
            ],
        );
        draw_frame(&mut app, &context, vec![click(false, rect.center())]);
        assert!(context.memory(|memory| memory.has_focus(id)));
        draw_frame(
            &mut app,
            &context,
            vec![egui::Event::Key {
                key: Key::ArrowUp,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        draw_frame(&mut app, &context, vec![]);
        let (_, rect, after_key) = app.report.debug_resize_handle();
        assert!(after_key > before);
        draw_frame(
            &mut app,
            &context,
            vec![
                egui::Event::PointerMoved(rect.center()),
                click(true, rect.center()),
            ],
        );
        let target = rect.center() - egui::vec2(0.0, 40.0);
        draw_frame(&mut app, &context, vec![egui::Event::PointerMoved(target)]);
        draw_frame(&mut app, &context, vec![click(false, target)]);
        draw_frame(&mut app, &context, vec![]);
        assert!(
            app.report.debug_resize_handle().2 > after_key,
            "dragging the actual grip must resize the panel"
        );
    }
    #[test]
    fn compact_header_preserves_workspace_height_even_with_a_long_scenario() {
        for size in [
            egui::vec2(960.0, 540.0),
            egui::vec2(1280.0, 800.0),
            egui::vec2(1600.0, 1000.0),
        ] {
            let context = egui::Context::default();
            let mut app = App::new(&context, Source::Session(default_workspace()));
            app.editor.change_project(|project| {
                project.scenarios[0].name =
                    "Nowy scenariusz – a deliberately long scenario title".repeat(3);
                project.scenarios[0].description =
                    "A long description belongs in settings and the title tooltip. ".repeat(100);
                for index in 0..12 {
                    let mut node = Block::Variable.create(
                        Position {
                            x: index as f64 * 320.0,
                            y: 800.0,
                        },
                        None,
                    );
                    node.handle = format!("variable_{index}");
                    project.scenarios[0].graph.nodes.push(node);
                }
            });
            app.editor.selection = vec![app.editor.graph().nodes[0].id.clone()];
            for _ in 0..4 {
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                        ..Default::default()
                    },
                    |ui| app.ui(ui),
                );
                output.textures_delta.clear();
            }
            assert!(
                app.canvas.debug_viewport().top() <= 104.0,
                "Header took too much workspace at {size:?}: {:?}",
                app.canvas.debug_viewport()
            );
        }
    }
}
