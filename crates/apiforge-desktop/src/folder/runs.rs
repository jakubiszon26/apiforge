//! Snapshot execution, reports and deliberate interrupted-run review.
use super::*;
use apiforge_core::{
    journal::JournalRun,
    run::{Phase, RunMode, RunResult},
};
use apiforge_project::PreparedRun;
use apiforge_runner::{RunHandle, RunRequest, RunTarget, RunnerService};
use apiforge_storage::JournalStore;
use apiforge_storage::project::IdentityBase;

#[derive(Default)]
pub(super) struct Runs {
    pub runner: Option<RunnerService>,
    pub handle: Option<RunHandle>,
    active: Option<PreparedRun>,
    reports: HashMap<String, (PreparedRun, RunResult)>,
    panel: crate::report::ReportPanel,
    journal: Option<Arc<JournalStore>>,
    pending_review: Option<JournalReview>,
    pub secrets: dialogs::SessionSecrets,
    /// The input values last run per component document, for the session
    /// (KTLO-23b).
    inputs: HashMap<String, dialogs::RunInputs>,
    pub exporting: bool,
    /// The active tab and mode the report was last drawn for (KTLO-19).
    view: Option<(Option<String>, Mode)>,
}

pub(super) struct JournalReview {
    journal: Arc<JournalStore>,
    runs: Vec<JournalRun>,
    legacy: bool,
}

impl FolderApp {
    /// The active tab's run results fill the window (KTLO-19).
    pub(super) fn report_maximised(&self, active: Option<&str>) -> bool {
        self.runs.panel.is_maximised()
            && active.is_some_and(|id| self.runs.reports.contains_key(id))
    }

    /// Escape returns maximised run results to their height; the board, which
    /// has its own uses of Escape, is not drawn meanwhile.
    pub(super) fn restore_report_on_escape(&mut self, context: &egui::Context) {
        if self.runs.panel.is_maximised()
            && context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape))
        {
            self.runs.panel.restore();
        }
    }

    pub(super) fn fresh_run(&self, id: &str) -> Option<RunResult> {
        let (prepared, report) = self.runs.reports.get(id)?;
        (!self.session.as_ref()?.run_is_stale(prepared)).then(|| report.clone())
    }

    #[cfg(test)]
    pub(super) fn run_result(&self, id: &str) -> Option<&RunResult> {
        self.runs.reports.get(id).map(|(_, result)| result)
    }

    pub(super) fn open_run_journal(&mut self) {
        let Some(session) = &self.session else { return };
        if session.is_read_only() {
            return;
        }
        let identity = session.storage_identity();
        let store = session.store().clone();
        self.storage_io(
            "Loading run journal…",
            move || JournalStore::open_project(store),
            move |app, result| {
                if !app.session_matches(&identity) {
                    return;
                }
                match result {
                    Ok(Ok(loaded)) => {
                        let journal = Arc::new(loaded.store);
                        if !loaded.interrupted.is_empty() {
                            app.runs.pending_review = Some(JournalReview {
                                journal: journal.clone(),
                                runs: loaded.interrupted,
                                legacy: false,
                            });
                        }
                        if loaded.unreadable {
                            app.notify("An unreadable run journal was preserved for review.", true);
                        }
                        app.runs.journal = Some(journal);
                    }
                    Ok(Err(error)) => {
                        app.notify(format!("Live runs cannot be journalled: {error}"), true)
                    }
                    Err(error) => app.notify(error, true),
                }
            },
        );
    }

    pub(super) fn request_run(&mut self, id: &str, live: bool) {
        if self.watcher.recovery_required {
            self.notify("Resolve project recovery and reload before running.", true);
            return;
        }
        if self.runs.handle.is_some() {
            return;
        }
        let Some(session) = self.session.as_mut() else {
            return;
        };
        if live && !session.is_read_only() && self.runs.journal.is_none() {
            self.notify(
                "The project journal is unavailable. Repair it before sending live requests.",
                true,
            );
            return;
        }
        match session.prepare_run(id) {
            Ok(prepared) if prepared.last_valid || (live && self.runs.journal.is_none()) => {
                self.prompt = Some(Prompt::Run {
                    prepared: Box::new(prepared),
                    live,
                });
            }
            Ok(prepared) => self.start_prepared_run(prepared, live),
            Err(error) => {
                let kind = match session.document(id).map(|document| document.entry().kind) {
                    Some(DocumentKind::Component) => "component",
                    _ => "scenario",
                };
                self.notify(format!("Cannot run this {kind}: {error}"), true)
            }
        }
    }

    pub(super) fn start_prepared_run(&mut self, prepared: PreparedRun, live: bool) {
        self.start_run_with(prepared, live, None);
    }

    /// Starts `prepared`; a component given no `inputs` runs with its
    /// defaults, or first asks for them when some input has none (KTLO-23b).
    fn start_run_with(
        &mut self,
        prepared: PreparedRun,
        live: bool,
        inputs: Option<dialogs::RunInputs>,
    ) {
        if self.watcher.recovery_required {
            self.notify("Resolve project recovery and reload before running.", true);
            return;
        }
        let Some(session) = self.session.as_mut() else {
            return;
        };
        if let Err(error) = session.validate_run(&prepared) {
            self.notify(
                format!("The proposed run changed. Review it again: {error}"),
                true,
            );
            return;
        }
        if self.runs.handle.is_some() {
            return;
        }
        let project = match prepared.project() {
            Ok(project) => project,
            Err(error) => {
                self.notify(error.to_string(), true);
                return;
            }
        };
        // A component runs its working revision on its own (KTLO-23), with
        // the values entered for its inputs over the defaults written in code.
        let (scenario_id, target) = match prepared.snapshot.document() {
            IdentityBase::Scenario(scenario) => (scenario.id.clone(), RunTarget::Scenario),
            IdentityBase::Component(component) => {
                let inputs = match inputs {
                    Some(inputs) => inputs,
                    None if dialogs::RunInputsDialog::needed(&component.inputs) => {
                        let dialog = dialogs::RunInputsDialog::new(
                            &component.name,
                            &component.inputs,
                            &project.contracts,
                            self.runs.inputs.get(prepared.document_id()),
                        );
                        self.prompt = Some(Prompt::RunInputs {
                            prepared: Box::new(prepared),
                            live,
                            dialog: Box::new(dialog),
                        });
                        return;
                    }
                    None => dialogs::RunInputs::new(),
                };
                (
                    component.id.clone(),
                    RunTarget::Component {
                        revision: component.revision,
                        inputs,
                    },
                )
            }
        };
        if self.runs.runner.is_none() {
            let context = self.ui_context.clone();
            match RunnerService::new(Arc::new(move || context.request_repaint())) {
                Ok(runner) => {
                    self.runs.runner = Some(match &self.runs.journal {
                        Some(journal) => runner.with_journal(journal.clone()),
                        None => runner,
                    })
                }
                Err(error) => {
                    self.notify(format!("Cannot start the runner: {error}"), true);
                    return;
                }
            }
        }
        session
            .store()
            .set_known_secrets(self.runs.secrets.values().cloned());
        let request = RunRequest {
            project,
            scenario_id,
            target,
            mode: if live {
                RunMode::Live
            } else {
                RunMode::Simulation
            },
            target_node_id: None,
            single_node_id: None,
            previous_run: None,
            secrets: self.runs.secrets.clone(),
        };
        match self
            .runs
            .runner
            .as_ref()
            .expect("initialised")
            .start(request)
        {
            Ok(handle) => {
                let message = if prepared.last_valid {
                    "Running the confirmed earlier code and configuration."
                } else if prepared.unsaved {
                    "Running unsaved code."
                } else {
                    "Running saved code."
                };
                self.notify(message, false);
                self.runs.active = Some(prepared);
                self.runs.handle = Some(handle);
                self.runs.panel.open = true;
            }
            Err(error) => self.notify(error.to_string(), true),
        }
    }

    pub(super) fn poll_run(&mut self) {
        if let (Some(handle), Some(prepared)) = (&self.runs.handle, &self.runs.active) {
            if let Some(report) = handle.latest() {
                self.runs
                    .reports
                    .insert(prepared.document_id().into(), (prepared.clone(), report));
            }
            if handle.is_finished() {
                if let Some(report) = handle.latest() {
                    self.runs
                        .reports
                        .insert(prepared.document_id().into(), (prepared.clone(), report));
                }
                self.runs.handle = None;
                self.runs.active = None;
            }
        }
        if self.prompt.is_none()
            && let Some(review) = self.runs.pending_review.take()
        {
            self.prompt = Some(Prompt::Journal(Box::new(review)));
        }
    }

    pub(super) fn run_prompt(
        &mut self,
        context: &egui::Context,
        prepared: PreparedRun,
        live: bool,
    ) -> Option<Prompt> {
        let mut run = false;
        let mut cancel = false;
        let response = dialogs::centered_modal(egui::Id::new("folder-run-consent"), context).show(context, |ui| {
            ui.set_width(440.0);
            ui.heading(if prepared.last_valid { "Run the last valid version?" } else { "Run without a journal?" });
            if prepared.last_valid {
                ui.label("The current code or inspector fields contain errors. Run the earlier code with its recorded public configuration and pinned component revisions.");
                ui.label(match prepared.snapshot.document() {
                    IdentityBase::Scenario(scenario) => format!("Scenario: {}", scenario.name),
                    IdentityBase::Component(component) => format!("Component: {}", component.name),
                });
            }
            if live && self.runs.journal.is_none() {
                ui.label("This session cannot record live requests. An interrupted run will leave no journal of its effects on the API.");
            }
            ui.horizontal(|ui| {
                run = ui.add(widgets::primary_button("Run this version")).clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        if cancel || response.should_close() {
            return None;
        }
        if run {
            self.start_prepared_run(prepared, live);
            // A component may ask for its inputs next.
            return self.prompt.take();
        }
        Some(Prompt::Run {
            prepared: Box::new(prepared),
            live,
        })
    }

    pub(super) fn run_inputs_prompt(
        &mut self,
        context: &egui::Context,
        prepared: PreparedRun,
        live: bool,
        mut dialog: dialogs::RunInputsDialog,
    ) -> Option<Prompt> {
        match dialog.show(context) {
            dialogs::Outcome::Open => Some(Prompt::RunInputs {
                prepared: Box::new(prepared),
                live,
                dialog: Box::new(dialog),
            }),
            dialogs::Outcome::Cancelled => None,
            dialogs::Outcome::Applied(inputs) => {
                self.runs
                    .inputs
                    .insert(prepared.document_id().to_owned(), inputs.clone());
                self.start_run_with(prepared, live, Some(inputs));
                self.prompt.take()
            }
        }
    }

    pub(super) fn run_report(&mut self, ui: &mut egui::Ui, active: Option<&str>) {
        // Another tab or mode restores the layout (KTLO-19).
        let view = Some((active.map(str::to_owned), self.workbench.mode));
        if self.runs.view != view {
            self.runs.view = view;
            self.runs.panel.restore();
        }
        let Some(id) = active else { return };
        let Some((prepared, result)) = self.runs.reports.get(id).cloned() else {
            return;
        };
        let stale = self
            .session
            .as_ref()
            .is_none_or(|session| session.run_is_stale(&prepared));
        let snapshot = self.snapshot(id);
        let policy = snapshot
            .as_ref()
            .and_then(|snapshot| match snapshot.model.as_deref() {
                Some(Document::Scenario(scenario)) => Some(scenario.error_policy),
                _ => None,
            })
            .unwrap_or(apiforge_core::model::ErrorPolicy::Stop);
        let selected = self
            .selections
            .get(id)
            .and_then(|ids| ids.first())
            .map(String::as_str);
        let actions = self.runs.panel.show(
            ui,
            &crate::report::Context {
                run: Some(&result),
                selected_node_id: selected,
                error_policy: policy,
                running: self.runs.handle.is_some(),
                stale,
                files_busy: false,
                component: self
                    .session
                    .as_ref()
                    .and_then(|session| session.document(id))
                    .is_some_and(|document| document.entry().kind == DocumentKind::Component),
                document_locked: stale
                    || snapshot.as_ref().is_none_or(|snapshot| !snapshot.editable),
            },
        );
        for action in actions {
            self.folder_report_action(id, &result, stale, action);
        }
    }

    pub(super) fn folder_report_action(
        &mut self,
        id: &str,
        result: &RunResult,
        stale: bool,
        action: crate::report::Action,
    ) {
        use crate::report::Action;
        let Some(snapshot) = self.snapshot(id) else {
            return;
        };
        let Some(model) = snapshot.model else { return };
        match action {
            Action::SelectNode(node) => {
                let cleanup = result
                    .steps
                    .iter()
                    .find(|step| step.node_id == node)
                    .is_some_and(|step| step.phase == Phase::Cleanup);
                if let Some(tab) = self.workbench.tab_mut(id) {
                    tab.cleanup = cleanup;
                }
                self.selections.insert(id.into(), vec![node]);
            }
            Action::ExportReport => {
                if let Ok(text) = serde_json::to_string_pretty(result) {
                    self.export_text("run-report.json".into(), text);
                }
            }
            _ if stale || !snapshot.editable => {}
            Action::SetErrorPolicy(policy) => {
                if let Document::Scenario(mut scenario) = Arc::unwrap_or_clone(model) {
                    scenario.error_policy = policy;
                    self.component_action(
                        id,
                        publication::Action::Model {
                            next: Box::new(Document::Scenario(scenario)),
                            selection: None,
                        },
                    );
                }
            }
            Action::AssertField {
                node_id,
                port,
                path,
                value,
            } => {
                let cleanup = !graph_of(&model, false)
                    .nodes
                    .iter()
                    .any(|node| node.id == node_id);
                let graph = graph_of(&model, cleanup);
                if let Some(node) = graph.nodes.iter().find(|node| node.id == node_id) {
                    let context = self.context_project();
                    let parameters = parameters_of(&model);
                    let board = Board {
                        graph,
                        reference: graph_of(&model, false),
                        project: &context,
                        parameters: &parameters,
                    };
                    self.apply_edit(
                        id,
                        &model,
                        cleanup,
                        Ok(board.push_node(edit::field_assertion(node, &port, &path, &value))),
                    );
                }
            }
            Action::AssignField {
                node_id,
                port,
                path,
            } => {
                let cleanup = !graph_of(&model, false)
                    .nodes
                    .iter()
                    .any(|node| node.id == node_id);
                self.prompt = Some(Prompt::AssignResult {
                    document_id: id.into(),
                    cleanup,
                    dialog: Box::new(dialogs::AssignVariableDialog::new(
                        graph_of(&model, cleanup),
                        &node_id,
                        &port,
                        &path,
                    )),
                });
            }
        }
    }

    pub(super) fn assign_result_prompt(
        &mut self,
        context: &egui::Context,
        id: String,
        cleanup: bool,
        mut dialog: dialogs::AssignVariableDialog,
    ) -> Option<Prompt> {
        match dialog.show(context) {
            dialogs::Outcome::Cancelled => return None,
            dialogs::Outcome::Applied(assignment) => {
                let snapshot = self.snapshot(&id)?;
                if !snapshot.editable {
                    return None;
                }
                let model = snapshot.model?;
                match edit::assign_response_variable(
                    graph_of(&model, cleanup),
                    &self.context_project(),
                    &dialog.source_id,
                    &dialog.port,
                    &dialog.path,
                    &assignment.name,
                    assignment.target_id.as_deref(),
                ) {
                    Ok(assigned) => {
                        self.apply_edit(
                            &id,
                            &model,
                            cleanup,
                            Ok(Edit {
                                graph: assigned.graph,
                                selection: Some(vec![assigned.variable_id]),
                            }),
                        );
                        return self.prompt.take();
                    }
                    Err(error) => dialog.set_error(error.to_string()),
                }
            }
            dialogs::Outcome::Open => {}
        }
        Some(Prompt::AssignResult {
            document_id: id,
            cleanup,
            dialog: Box::new(dialog),
        })
    }

    pub(super) fn review_legacy_runs(&mut self) {
        let Some(data) = dirs::data_dir() else {
            self.notify("No application data directory is available.", true);
            return;
        };
        self.review_legacy_runs_from(&data.join("apiforge"));
    }

    pub(super) fn review_legacy_runs_from(&mut self, data: &std::path::Path) {
        let data = data.to_owned();
        self.working_io(
            "Loading legacy journal…",
            move || {
                (|| {
                    let catalogue = apiforge_project::ProjectCatalogue::load(&data)
                        .map_err(|error| error.to_string())?
                        .ok_or("No migrated legacy journal".to_owned())?;
                    let path = catalogue
                        .legacy_journal
                        .ok_or("No migrated legacy journal".to_owned())?;
                    let owner = apiforge_storage::WorkspaceStore::open_exclusive(
                        path.with_file_name("workspace.json"),
                    )
                    .map_err(|error| error.to_string())?;
                    JournalStore::open(owner).map_err(|error| error.to_string())
                })()
            },
            |app, result| match result {
                Ok(Ok(loaded)) if !loaded.interrupted.is_empty() => {
                    app.prompt = Some(Prompt::Journal(Box::new(JournalReview {
                        journal: Arc::new(loaded.store),
                        runs: loaded.interrupted,
                        legacy: true,
                    })))
                }
                Ok(Ok(_)) => app.notify("There are no unresolved legacy runs.", false),
                Ok(Err(error)) | Err(error) => app.notify(error, true),
            },
        );
    }

    pub(super) fn journal_prompt(
        &mut self,
        context: &egui::Context,
        review: JournalReview,
    ) -> Option<Prompt> {
        let mut handled = false;
        let mut later = false;
        let response = dialogs::centered_modal(egui::Id::new("folder-journal"), context).show(context, |ui| {
            ui.set_width(520.0);
            ui.heading(if review.legacy { "Unresolved legacy runs" } else { "A live run did not finish" });
            ui.label("Check the API and its cleanup before marking these requests as handled.");
            if review.legacy { ui.label("These runs belong to the earlier global journal. They have no project assignment."); }
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for run in &review.runs {
                    ui.label(RichText::new(format!("{} · {}", run.scenario_name, run.started_at)).strong());
                    ui.label(format!("Cleanup: {:?}", run.cleanup));
                    for entry in run.effects() { ui.label(format!("{} {} · {}", entry.method, entry.url, entry.state.label())); }
                    if run.elided > 0 { ui.label(format!("{} earlier requests are omitted.", run.elided)); }
                }
            });
            ui.horizontal(|ui| {
                handled = ui.add(widgets::primary_button("Mark as handled")).clicked();
                later = ui.button("Remind me later").clicked();
            });
        });
        if handled {
            let ids = review
                .runs
                .iter()
                .map(|run| run.run_id.clone())
                .collect::<Vec<_>>();
            let journal = review.journal.clone();
            self.working_io(
                "Acknowledging reviewed runs…",
                move || journal.discard(&ids).map_err(|error| error.to_string()),
                move |app, result| {
                    if let Err(error) = result.and_then(|result| result) {
                        app.notify(format!("Could not acknowledge these runs: {error}"), true);
                        app.prompt = Some(Prompt::Journal(Box::new(review)));
                    }
                },
            );
            return self.prompt.take();
        }
        if later || response.should_close() {
            None
        } else {
            Some(Prompt::Journal(Box::new(review)))
        }
    }
}
