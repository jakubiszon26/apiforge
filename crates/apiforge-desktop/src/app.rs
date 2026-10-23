//! The application window: layout, global shortcuts and applying user actions.

use crate::icons::Icon;
use crate::{
    canvas::{self, Canvas, Command},
    dialogs::{self, Outcome},
    editor::{BlockClipboard, Editor},
    files,
    inspector::{self, Inspector},
    library::{self, Library},
    navigator::Navigator,
    persistence::{SaveStatus, Saver},
    report::{self, ReportPanel},
    theme, widgets,
};
use apiforge_core::{
    edit,
    format::{Binding, Graph, Position, Project, Workspace},
    graph::{create_operator_node, delete_nodes, position_after},
    model::NodeKind,
    run::{Phase, RunMode, RunResult, RunStatus, StepResult},
};
use apiforge_runner::{RunHandle, RunRequest, RunnerService};
use apiforge_storage::WorkspaceStore;
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
}

impl App {
    pub fn new(context: &egui::Context, source: Source) -> Self {
        theme::apply(context);
        let mut notice = None;
        let (workspace, saver) = match source {
            Source::Session(workspace) => (workspace, None),
            Source::Storage(store) => {
                let (workspace, writable) = match store.load() {
                    Ok(Some(loaded)) => {
                        notice = loaded.recovered;
                        (loaded.workspace, true)
                    }
                    Ok(None) => (default_workspace(), true),
                    Err(error) => {
                        notice = Some(format!(
                            "{error}. A temporary workspace was opened with autosave disabled to protect the existing files. Use Export to keep changes from this session."
                        ));
                        (default_workspace(), false)
                    }
                };
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
        let repaint = context.clone();
        let runner = RunnerService::new(Arc::new(move || repaint.request_repaint()))
            .map_err(|error| {
                if notice.is_none() {
                    notice = Some(format!("Cannot start the runner: {error}"));
                }
            })
            .ok();
        Self {
            editor: Editor::new(workspace),
            saver,
            saved_revision: 0,
            canvas: Canvas::default(),
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
        }
    }

    /// Selects a node of the current board by id or name (used by screenshots).
    pub fn select_node(&mut self, key: &str) {
        if let Some(node) = self
            .editor
            .graph()
            .nodes
            .iter()
            .find(|node| node.id == key || node.name == key)
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
        if !self.closing {
            return None;
        }
        match self.saver.as_ref().map(Saver::status) {
            Some(SaveStatus::Failed(error)) => Some(error),
            _ => None,
        }
    }

    pub fn abort_close_after_save_error(&mut self, error: String) {
        self.closing = false;
        self.notice = Some(format!(
            "Your latest changes could not be saved: {error}. The window is staying open. Fix the storage problem and choose Retry save."
        ));
    }

    fn retry_save(&mut self) {
        if let Some(saver) = &self.saver {
            saver.save(self.editor.workspace());
            saver.flush();
            self.notice = None;
        }
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
        if self.closing || self.is_running() || self.editor.draft().is_some() {
            return;
        }
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
        if self.editor.revision() != self.saved_revision {
            self.saved_revision = self.editor.revision();
            if let Some(saver) = &self.saver {
                saver.save(self.editor.workspace());
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
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.board(ui));
        self.dialogs(ui.ctx());
        if let Some(id) = self.navigator.show(ui.ctx(), self.editor.graph()) {
            self.editor.selection = vec![id.clone()];
            self.inspector_collapsed = false;
            self.canvas.reveal(id);
        }
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
                .filter(|node| node.kind == NodeKind::Variable)
                .map(|node| (node.id.clone(), node.name.clone()))
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
            library::Action::AddNode { kind, component } => {
                let position = position_after(self.editor.graph(), 100.0);
                self.add_node(kind, component, position, None);
            }
            library::Action::AddOperator(operator) => {
                let node = create_operator_node(operator, self.canvas.visible_center());
                self.push_node(node);
            }
            library::Action::AddVariable => {
                let position = position_after(self.editor.graph(), 100.0);
                self.add_node(NodeKind::Variable, None, position, None);
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
                ui.add_enabled_ui(!running, |ui| {
                    ui.selectable_value(&mut self.mode, RunMode::Live, "API")
                        .on_hover_text("Send real requests to the configured API");
                    ui.selectable_value(&mut self.mode, RunMode::Simulation, "Simulation")
                        .on_hover_text("Use saved sample responses; no requests are sent");
                });
            });
        });
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
            match action {
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
                    }
                }
                self.apply_graph(graph);
            }
            Command::AddNode {
                kind,
                component,
                position,
                source,
            } => self.add_node(kind, component, position, source),
            Command::AddOperator { operator, position } => {
                self.push_node(create_operator_node(operator, position))
            }
            Command::SetLiteral { node, input, value } => {
                self.set_input(&node, &input, Binding::literal(value))
            }
            Command::Bind {
                node,
                input,
                binding,
            } => self.set_input(&node, &input, binding),
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
            } => {
                let result = edit::connect(
                    self.editor.graph(),
                    self.editor.project(),
                    &source,
                    &source_port,
                    &target,
                    &target_input,
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

    fn set_input(&mut self, node: &str, input: &str, binding: Binding) {
        let mut graph = self.editor.graph().clone();
        if let Some(target) = graph
            .nodes
            .iter_mut()
            .find(|candidate| candidate.id == node)
        {
            target.inputs.insert(input.to_owned(), binding);
            self.apply_graph(graph);
        }
    }

    fn add_node(
        &mut self,
        kind: NodeKind,
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
            kind,
            position,
            definition.as_ref(),
            source,
        ) {
            Ok(added) => {
                self.apply_graph(added.graph);
                self.editor.selection = vec![added.node_id];
            }
            Err(error) => self.canvas.set_error(error.to_string()),
        }
    }

    fn push_node(&mut self, node: apiforge_core::format::Node) {
        let id = node.id.clone();
        let mut graph = self.editor.graph().clone();
        graph.nodes.push(node);
        self.apply_graph(graph);
        self.editor.selection = vec![id];
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
        if self.dialog.is_some() || self.navigator.is_open() || self.closing {
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
        version: 1,
        active_id: projects[0].id.clone(),
        projects,
    }
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
            vec![egui::Event::Text(target.name.clone())],
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
                .name
                .contains(" changed ")
        );
    }

    #[test]
    fn unreadable_startup_documents_are_protected_from_autosave() {
        for bytes in [b"{ truncated".as_slice(), br#"{"version":2}"#.as_slice()] {
            let directory = tempfile::tempdir().unwrap();
            let store = WorkspaceStore::at(directory.path().join("workspace.json"));
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
    fn close_keeps_unsaved_changes_open_and_retry_persists_them() {
        let directory = tempfile::tempdir().unwrap();
        let blocker = directory.path().join("blocked-parent");
        let store = WorkspaceStore::at(blocker.join("workspace.json"));
        store.save(&default_workspace()).unwrap();
        let mut app = App::new(&egui::Context::default(), Source::Storage(store.clone()));
        std::fs::remove_file(store.path()).unwrap();
        std::fs::remove_dir(&blocker).unwrap();
        std::fs::write(&blocker, b"not a directory").unwrap();
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

        std::fs::remove_file(&blocker).unwrap();
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
        let mut cleanup =
            apiforge_core::graph::create_node(NodeKind::Request, Position { x: 0.0, y: 0.0 }, None);
        cleanup.config.url = Some("/cleanup".into());
        project.scenarios[0].cleanup.nodes.push(cleanup);
        let workspace = Workspace {
            version: 1,
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
        graph.nodes[0].name = "Edited name".into();
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
                    project.scenarios[0]
                        .graph
                        .nodes
                        .push(apiforge_core::graph::create_node(
                            NodeKind::Variable,
                            Position {
                                x: index as f64 * 320.0,
                                y: 800.0,
                            },
                            None,
                        ));
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
