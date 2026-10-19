//! The application window: layout, global shortcuts and applying user actions.

use crate::{
    canvas::{self, Canvas, Command},
    dialogs::{self, Outcome},
    editor::{BlockClipboard, Editor},
    files,
    inspector::{self, Inspector},
    library::{self, Library},
    persistence::{SaveStatus, Saver},
    theme, widgets,
};
use apiforge_core::{
    edit,
    format::{Binding, Graph, Position, Workspace},
    graph::{create_operator_node, delete_nodes, position_after},
    model::NodeKind,
    run::{Phase, RunMode},
};
use apiforge_storage::WorkspaceStore;
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, RichText};
use std::{collections::HashSet, time::Instant};

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

pub struct App {
    editor: Editor,
    saver: Option<Saver>,
    saved_revision: u64,
    canvas: Canvas,
    library: Library,
    inspector: Inspector,
    notice: Option<String>,
    mode: RunMode,
    secrets: dialogs::SessionSecrets,
    dialog: Option<Dialog>,
    files: files::Files,
    report_open: bool,
    report_height: f32,
}

impl App {
    pub fn new(context: &egui::Context, source: Source) -> Self {
        theme::apply(context);
        let mut notice = None;
        let (workspace, saver) = match source {
            Source::Session(workspace) => (workspace, None),
            Source::Storage(store) => {
                let workspace = match store.load() {
                    Ok(Some(loaded)) => {
                        notice = loaded.recovered;
                        loaded.workspace
                    }
                    Ok(None) => default_workspace(),
                    Err(error) => {
                        notice = Some(format!(
                            "{error}. A new workspace was opened; the unreadable file is kept until you save changes."
                        ));
                        default_workspace()
                    }
                };
                let repaint = context.clone();
                let saver = Saver::spawn(store, move || repaint.request_repaint())
                    .map_err(|error| notice = Some(format!("Cannot start saving: {error}")))
                    .ok();
                (workspace, saver)
            }
        };
        Self {
            editor: Editor::new(workspace),
            saver,
            saved_revision: 0,
            canvas: Canvas::default(),
            library: Library::default(),
            inspector: Inspector::default(),
            notice,
            mode: RunMode::Simulation,
            secrets: dialogs::SessionSecrets::new(),
            dialog: None,
            files: files::Files::default(),
            report_open: true,
            report_height: 240.0,
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
        if let Some(result) = self.files.poll() {
            self.file_result(result);
        }
        self.shortcuts(ui.ctx());

        egui::Panel::left("sidebar")
            .exact_size(268.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::WHITE)
                    .inner_margin(egui::Margin::symmetric(16, 14)),
            )
            .show(ui, |ui| self.sidebar(ui));
        egui::Panel::top("topbar")
            .exact_size(48.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::WHITE)
                    .inner_margin(egui::Margin::symmetric(24, 8)),
            )
            .show(ui, |ui| self.topbar(ui));
        egui::Panel::top("scenario-header")
            .frame(
                egui::Frame::new()
                    .fill(theme::WHITE)
                    .inner_margin(egui::Margin {
                        left: 24,
                        right: 24,
                        top: 14,
                        bottom: 12,
                    }),
            )
            .show(ui, |ui| self.scenario_header(ui));
        egui::Panel::top("editor-bar")
            .exact_size(44.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::WHITE)
                    .inner_margin(egui::Margin::symmetric(24, 6)),
            )
            .show(ui, |ui| self.editor_bar(ui));
        if self.editor.draft().is_none() {
            self.report(ui);
        }
        if self.editor.selected_node().is_some() {
            egui::Panel::right("inspector")
                .default_size(340.0)
                .size_range(300.0..=520.0)
                .frame(egui::Frame::new().fill(theme::WHITE).inner_margin(18))
                .show(ui, |ui| self.inspector(ui));
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.board(ui));
        self.dialogs(ui.ctx());
        self.save_if_changed();
    }

    // ----- Panels ----------------------------------------------------------------

    fn sidebar(&mut self, ui: &mut egui::Ui) {
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
            running: false,
        };
        let actions = self.library.show(ui, &context);
        drop(graph);

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            let status = self.saver.as_ref().map(Saver::status);
            let (text, color) = match &status {
                None => ("Session only — not saved".to_owned(), theme::MUTED),
                Some(SaveStatus::Failed(error)) => (format!("Save failed: {error}"), theme::DANGER),
                Some(status) => (status.label().to_owned(), theme::MUTED),
            };
            ui.label(RichText::new(format!("●  {text}")).size(10.5).color(color));
            ui.add_space(4.0);
            let base = &self.editor.project().environment.base_url;
            let host = base.split_once("://").map_or(base.as_str(), |(_, rest)| {
                rest.split('/').next().unwrap_or(rest)
            });
            let environment = egui::Button::new(
                RichText::new(format!(
                    "ENVIRONMENT\n{}",
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
                self.editor.switch_project(&id);
                self.secrets.clear();
            }
            library::Action::NewProject => self.dialog = Some(Dialog::NewProject(String::new())),
            library::Action::SwitchScenario(id) => self.editor.switch_scenario(&id),
            library::Action::NewScenario => self.dialog = Some(Dialog::NewScenario(String::new())),
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
            return;
        }
        self.editor.add_project(build());
        self.secrets.clear();
        self.notice = Some(notice.into());
    }

    fn topbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            let project = &self.editor.project().name;
            let location = if self.editor.draft().is_some() {
                "Component editor"
            } else {
                "Scenarios"
            };
            ui.label(
                RichText::new(format!("{project}   ›   "))
                    .size(11.5)
                    .color(theme::MUTED),
            );
            ui.label(RichText::new(location).size(11.5).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button("Settings")
                    .on_hover_text("Project and scenario settings")
                    .clicked()
                {
                    self.dialog = Some(Dialog::Settings(dialogs::SettingsDialog::new(
                        self.editor.project(),
                        &self.editor.scenario().id,
                    )));
                }
                ui.separator();
                if ui
                    .add_enabled(!self.files.busy(), egui::Button::new("Export"))
                    .on_hover_text("Export the project as JSON (without secret values)")
                    .clicked()
                {
                    let project = apiforge_storage::public_project(self.editor.project());
                    self.files.export(project);
                }
                if ui
                    .add_enabled(!self.files.busy(), egui::Button::new("Import"))
                    .on_hover_text("Import a project or workspace JSON file")
                    .clicked()
                {
                    self.files.import();
                }
            });
        });
    }

    fn scenario_header(&mut self, ui: &mut egui::Ui) {
        let draft = self.editor.draft().cloned();
        let scenario = self.editor.scenario().clone();
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 330.0).max(200.0);
            ui.allocate_ui_with_layout(
                egui::vec2(width, 70.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    let eyebrow = match &draft {
                        Some(draft) => format!(
                            "COMPONENT · REVISION {} · WORKING COPY   ·   {} steps",
                            draft.definition.revision,
                            draft.graph.nodes.len()
                        ),
                        None => format!(
                            "API REGRESSION TEST   ·   {} steps · {} after test",
                            scenario.graph.nodes.len(),
                            scenario.cleanup.nodes.len()
                        ),
                    };
                    ui.label(RichText::new(eyebrow).size(10.0).color(theme::MUTED));
                    let title = draft.as_ref().map_or(scenario.name.as_str(), |draft| {
                        draft.definition.name.as_str()
                    });
                    ui.label(RichText::new(title).size(22.0).family(theme::bold_family()));
                    let description = draft
                        .as_ref()
                        .map_or(scenario.description.as_str(), |draft| {
                            draft.definition.description.as_str()
                        });
                    let description = if description.is_empty() {
                        "Build the scenario from requests, data and checks."
                    } else {
                        description
                    };
                    ui.add(
                        egui::Label::new(RichText::new(description).size(11.5).color(theme::MUTED))
                            .truncate(),
                    );
                },
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(draft) = &draft {
                    if ui
                        .add(widgets::primary_button("Save revision"))
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
                    if ui.button("← Back without saving").clicked() {
                        self.editor.leave_component();
                    }
                    return;
                }
                if ui
                    .add(widgets::primary_button("▶  Run test"))
                    .on_hover_text("Run the whole scenario")
                    .clicked()
                {
                    self.notice = Some("The runner is not connected yet.".into());
                }
                ui.add_space(6.0);
                ui.selectable_value(&mut self.mode, RunMode::Live, "API")
                    .on_hover_text("Send real requests to the configured API");
                ui.selectable_value(&mut self.mode, RunMode::Simulation, "Simulation")
                    .on_hover_text("Use saved sample responses; no requests are sent");
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
                            if ui.small_button("✕").on_hover_text("Dismiss").clicked() {
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
                let phase = self.editor.phase();
                if ui
                    .selectable_label(phase == Phase::Main, "Scenario")
                    .clicked()
                {
                    self.editor.set_phase(Phase::Main);
                }
                if ui
                    .selectable_label(phase == Phase::Cleanup, format!("After test  {cleanup}"))
                    .on_hover_text("Cleanup steps that run after the test, also after errors")
                    .clicked()
                {
                    self.editor.set_phase(Phase::Cleanup);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(self.editor.can_redo(), egui::Button::new("↷"))
                    .on_hover_text("Redo (Ctrl+Shift+Z)")
                    .clicked()
                {
                    self.editor.redo();
                }
                if ui
                    .add_enabled(self.editor.can_undo(), egui::Button::new("↶"))
                    .on_hover_text("Undo (Ctrl+Z)")
                    .clicked()
                {
                    self.editor.undo();
                }
                ui.label(
                    RichText::new(match self.mode {
                        RunMode::Simulation => "Sample data · no requests",
                        RunMode::Live => "Live requests to the API",
                    })
                    .size(11.0)
                    .color(theme::MUTED),
                );
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
        egui::Panel::bottom("report")
            .resizable(self.report_open)
            .default_size(if self.report_open {
                self.report_height
            } else {
                40.0
            })
            .size_range(if self.report_open {
                120.0..=620.0
            } else {
                40.0..=40.0
            })
            .frame(
                egui::Frame::new()
                    .fill(theme::WHITE)
                    .inner_margin(egui::Margin::symmetric(24, 8)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let arrow = if self.report_open { "▾" } else { "▸" };
                    if ui
                        .add(
                            egui::Button::new(format!(
                                "{arrow}  {} run",
                                if self.report_open { "Hide" } else { "Show" }
                            ))
                            .frame(false),
                        )
                        .clicked()
                    {
                        self.report_open = !self.report_open;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        widgets::hint(ui, "Results and data of every step");
                    });
                });
                if self.report_open {
                    self.report_height = ui.available_height() + 40.0;
                    ui.separator();
                    ui.vertical_centered(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new("Results appear after you run the scenario.")
                                .color(theme::MUTED),
                        );
                    });
                }
            });
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        let Some(node) = self.editor.selected_node().cloned() else {
            return;
        };
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            if ui
                .small_button("✕")
                .on_hover_text("Close the configuration")
                .clicked()
            {
                self.editor.selection.clear();
            }
        });
        let reference = self.editor.reference_graph().into_owned();
        let parameters = self.editor.parameter_names();
        let actions = {
            let context = inspector::Context {
                node: &node,
                graph: &reference,
                board: self.editor.graph(),
                project: self.editor.project(),
                parameters: &parameters,
                run: None,
                step: None,
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
        let commands = {
            let input = canvas::Input {
                graph: self.editor.graph(),
                reference_graph: &reference,
                project: self.editor.project(),
                selection: &self.editor.selection,
                steps: None,
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
                match edit::duplicate_selection(self.editor.graph(), &selected) {
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
        if context.egui_wants_keyboard_input() || self.dialog.is_some() {
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
                    self.editor.change_project(|project| {
                        project.name = change.project_name;
                        project.contracts = change.contracts;
                        if let Some(scenario) = project
                            .scenarios
                            .iter_mut()
                            .find(|scenario| scenario.id == change.scenario_id)
                        {
                            scenario.name = change.scenario_name;
                            scenario.description = change.description;
                            scenario.error_policy = change.error_policy;
                            scenario.parameters = change.parameters;
                        }
                    });
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
        egui::Modal::new(egui::Id::new("new-item")).show(context, |ui| {
            ui.set_width(380.0);
            ui.label(RichText::new(title).size(17.0).family(theme::bold_family()));
            ui.add_space(8.0);
            ui.label("Name");
            let field = ui.add(
                egui::TextEdit::singleline(name)
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
                        .add_enabled(ready, widgets::primary_button("Create →"))
                        .clicked()
                        || (ready
                            && field.lost_focus()
                            && ui.input(|input| input.key_pressed(Key::Enter)))
                    {
                        create = true;
                    }
                    if ui.button("Cancel").clicked() {
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
                }
                Some(Dialog::NewScenario(name)) => {
                    self.editor
                        .add_scenario(apiforge_core::demos::empty_scenario(name.trim()));
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
