//! Snapshot execution, reports and deliberate interrupted-run review.
use super::*;
use apiforge_core::{
    journal::JournalRun,
    run::{Phase, RunMode, RunResult},
};
use apiforge_project::PreparedRun;
use apiforge_runner::{RunHandle, RunRequest, RunnerService};
use apiforge_storage::JournalStore;

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
    pub exporting: bool,
}

pub(super) struct JournalReview {
    journal: Arc<JournalStore>,
    runs: Vec<JournalRun>,
    legacy: bool,
}

impl FolderApp {
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
            Err(error) => self.notify(format!("Cannot run this scenario: {error}"), true),
        }
    }

    pub(super) fn start_prepared_run(&mut self, prepared: PreparedRun, live: bool) {
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
        let Some(scenario) = project.scenarios.first() else {
            return;
        };
        let scenario_id = scenario.id.clone();
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
                if let apiforge_storage::project::IdentityBase::Scenario(scenario) = prepared.snapshot.document() {
                    ui.label(format!("Scenario: {}", scenario.name));
                }
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
            return None;
        }
        Some(Prompt::Run {
            prepared: Box::new(prepared),
            live,
        })
    }

    pub(super) fn run_report(&mut self, ui: &mut egui::Ui, active: Option<&str>) {
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
                show_policy: matches!(
                    snapshot
                        .as_ref()
                        .and_then(|snapshot| snapshot.model.as_deref()),
                    Some(Document::Scenario(_))
                ),
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

/// Mock-up KTLO-23 (throwaway, never merged). Nothing can run a component
/// today, so the run record below is staged by hand in the shape the runner
/// gives a scenario run; it is not computed.
#[cfg(test)]
impl FolderApp {
    fn mock_document(&self, path: &str) -> String {
        self.session
            .as_ref()
            .unwrap()
            .documents()
            .iter()
            .find(|document| document.entry().path.as_str() == path)
            .map(|document| document.id().to_owned())
            .unwrap()
    }

    fn mock_component(&self, id: &str) -> apiforge_core::format::Component {
        match self.snapshot(id).unwrap().model.as_deref() {
            Some(Document::Component(component)) => component.clone(),
            _ => panic!("{id} is not a component"),
        }
    }

    /// Opens the document at `path` in a tab and makes it active.
    pub(crate) fn mock_open(&mut self, path: &str) {
        let id = self.mock_document(path);
        self.workbench.open(&id);
    }

    /// A finished component run in the report panel, with the `export` step
    /// selected there and on the component's board.
    pub(crate) fn mock_component_run(&mut self, component_path: &str, scenario_path: &str) {
        use apiforge_core::format::OptionalJson;
        use apiforge_core::model::NodeKind;
        use apiforge_core::run::{RequestSnapshot, RunStatus, StepResult};
        use indexmap::IndexMap;
        use serde_json::{Value, json};

        let id = self.mock_document(component_path);
        let component = self.mock_component(&id);
        // A prepared run is only a staleness key here; any document gives one.
        let scenario = self.mock_document(scenario_path);
        let prepared = self
            .session
            .as_mut()
            .unwrap()
            .prepare_run(&scenario)
            .unwrap();
        let product = json!({ "id": 123, "name": "Coffee", "price": 100,
            "promotion": { "active": true, "price": 80 } });
        let cart = json!({ "id": "sample-cart", "productId": 123, "quantity": 1, "total": 80 });
        let item = json!({ "productId": 123, "quantity": 1 });
        let map = |pairs: Vec<(&str, Value)>| -> IndexMap<String, Value> {
            pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()
        };
        let request = |method: &str, path: &str, body: OptionalJson| RequestSnapshot {
            method: method.into(),
            url: format!("{}{path}", apiforge_core::demos::SHOP_BASE_URL),
            headers: [("Content-Type".to_owned(), "application/json".to_owned())]
                .into_iter()
                .collect(),
            body,
        };
        let mut export = None;
        let steps: Vec<StepResult> = component
            .graph
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let mut step = StepResult::new(node, Phase::Main);
                step.duration_ms = [1, 0, 42, 37, 0, 0][index.min(5)];
                match (node.kind, node.handle.as_str()) {
                    (NodeKind::Value, handle)
                        if handle.contains("body") || handle.contains("item") =>
                    {
                        step.inputs = map(vec![("productId", json!(123)), ("quantity", json!(1))]);
                        step.outputs = map(vec![("value", item.clone())]);
                    }
                    (NodeKind::Value, _) => {
                        step.inputs = map(vec![("product", product.clone())]);
                        step.outputs = map(vec![("value", json!(123))]);
                    }
                    (NodeKind::Request, handle) if handle.contains("create") => {
                        step.inputs = map(vec![("body", item.clone())]);
                        step.outputs = map(vec![
                            ("body", cart.clone()),
                            ("status", json!(200)),
                            ("headers", json!({})),
                        ]);
                        step.request = Some(request(
                            "POST",
                            "/carts",
                            OptionalJson::Present(item.clone()),
                        ));
                    }
                    (NodeKind::Request, _) => {
                        step.inputs = map(vec![("id", json!("sample-cart"))]);
                        step.outputs = map(vec![
                            ("body", cart.clone()),
                            ("status", json!(200)),
                            ("headers", json!({})),
                        ]);
                        step.request =
                            Some(request("GET", "/carts/sample-cart", OptionalJson::Missing));
                    }
                    (NodeKind::Export, _) => {
                        step.inputs = map(vec![("value", cart.clone())]);
                        step.outputs = map(vec![("value", cart.clone())]);
                        export = Some(node.id.clone());
                    }
                    _ => {}
                }
                step
            })
            .collect();
        let result = RunResult {
            id: "mock-component-run".into(),
            scenario_id: component.id.clone(),
            mode: RunMode::Simulation,
            started_at: "2026-10-27T09:00:00Z".into(),
            finished_at: Some("2026-10-27T09:00:00.080Z".into()),
            status: RunStatus::Passed,
            steps,
            diagnostic: false,
            snapshot: None,
            error: None,
            notice: None,
        };
        self.runs.reports.insert(id.clone(), (prepared, result));
        self.runs.panel.open = true;
        self.runs.panel.mock_height(380.0);
        if let Some(node) = export {
            self.selections.insert(id, vec![node]);
        }
    }

    /// The Run inputs dialog for the component at `path`, as it opens, or
    /// with `typed` entered into the named inputs.
    pub(crate) fn mock_run_inputs(&mut self, path: &str, typed: &[(&str, &str)]) {
        let id = self.mock_document(path);
        let component = self.mock_component(&id);
        let contracts: Vec<(String, String)> = self
            .context_project()
            .contracts
            .iter()
            .map(|contract| (contract.id.clone(), contract.name.clone()))
            .collect();
        let mut dialog = dialogs::run_inputs::RunInputsDialog::new(
            &component.name,
            &component.inputs,
            &contracts,
        );
        for (name, text) in typed {
            dialog.set(name, text);
        }
        self.prompt = Some(Prompt::MockRunInputs(Box::new(dialog)));
    }
}
