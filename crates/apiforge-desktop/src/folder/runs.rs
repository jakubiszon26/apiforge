//! Snapshot execution, reports and deliberate interrupted-run review.
use super::*;
use apiforge_core::{
    journal::JournalRun,
    run::{CauseEnds, Phase, RunMode, RunResult, RunStatus},
};
use apiforge_project::PreparedRun;
use apiforge_runner::{RunHandle, RunRequest, RunTarget, RunnerService};
use apiforge_storage::JournalStore;
use apiforge_storage::project::IdentityBase;
use std::cell::Cell;
use std::hash::{DefaultHasher, Hash, Hasher};

#[derive(Default)]
pub(super) struct Runs {
    pub runner: Option<RunnerService>,
    pub handle: Option<RunHandle>,
    active: Option<PreparedRun>,
    reports: HashMap<String, Report>,
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

/// A document's latest run and the checks added from its results since.
struct Report {
    prepared: PreparedRun,
    result: RunResult,
    /// The cause ends of `result`'s steps, resolved when the result changes
    /// rather than every frame (LAY-19).
    ends: Arc<CauseEnds>,
    /// The run before, while it described the document as it is: the board
    /// shows its results, muted, for the steps this run has not reached
    /// (LAY-07).
    previous: Option<RunResult>,
    /// In the order they were added; an undone check stays until another
    /// check replaces it, so that redo finds it again.
    checks: Vec<AddedCheck>,
    /// The document's text at a source version: its hash, and whether it is
    /// the run's own source, so a frame does not read the same text again.
    text: Cell<Option<(u64, u64, bool)>>,
}

impl Report {
    fn new(prepared: PreparedRun, result: RunResult, previous: Option<RunResult>) -> Self {
        Self {
            prepared,
            ends: Arc::new(CauseEnds::of(&result.steps)),
            result,
            previous,
            checks: Vec::new(),
            text: Cell::new(None),
        }
    }
}

/// A check added from a report: its field, and the hash of the document's
/// text right after the check's edit applied (UX-01). Never persisted.
struct AddedCheck {
    field: crate::report::CheckedField,
    source_after: u64,
}

/// A check whose edit is on its way to the document (UX-35): applied at
/// once, or after the person confirms the comment prompt, it is counted
/// only if its card was added.
#[derive(Clone)]
pub(super) struct PendingCheck {
    field: crate::report::CheckedField,
    /// Whether the card goes to the cleanup graph.
    cleanup: bool,
    /// The graph's card count before the edit.
    before: usize,
    /// The checks counted when the edit was asked for; later ones are
    /// replaced, as after an undo.
    checks: usize,
}

/// How a document's report relates to the document now (UX-01).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Freshness {
    /// Nothing changed since the run.
    Fresh,
    /// The first `n` checks added from the results are the only change:
    /// the results stay usable and the checks run on the next run.
    ChecksOnly(usize),
    /// Something else changed.
    Stale,
}

impl Freshness {
    fn checks(self) -> usize {
        match self {
            Freshness::ChecksOnly(count) => count,
            Freshness::Fresh | Freshness::Stale => 0,
        }
    }
}

fn text_hash(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// Why no live request leaves an observing window (MCP-08a).
pub(super) const LIVE_PAUSED: &str =
    "Live requests are not sent from this window while another process holds the project.";

/// Keeps the latest report of `prepared`'s run. Another run starts a new
/// report, and with it a new count of checks, keeping the report it replaces
/// as the previous run when `keep`; the same run's progress keeps the checks
/// added from it meanwhile.
fn publish(
    reports: &mut HashMap<String, Report>,
    prepared: &PreparedRun,
    result: RunResult,
    keep: bool,
) {
    match reports.get_mut(prepared.document_id()) {
        Some(report) if report.result.id == result.id => {
            report.ends = Arc::new(CauseEnds::of(&result.steps));
            report.result = result;
        }
        _ => {
            let previous = reports
                .remove(prepared.document_id())
                .filter(|_| keep)
                .map(|report| report.result);
            reports.insert(
                prepared.document_id().into(),
                Report::new(prepared.clone(), result, previous),
            );
        }
    }
}

/// The run before `run` that the board may show beside it: one of the same
/// mode, as the engine reuses previous results only for the same mode
/// (LAY-07).
pub(super) fn previous_of<'a>(
    run: &RunResult,
    previous: Option<&'a RunResult>,
) -> Option<&'a RunResult> {
    previous.filter(|previous| previous.mode == run.mode)
}

/// The hover of the header's label while a run of unsaved code runs (UX-04).
const UNSAVED_HOVER: &str = "The run uses the code in the editor, which is not saved yet.";
/// The hover while a run of the last confirmed code runs (UX-04).
const EARLIER_HOVER: &str =
    "The editor's code has errors; the run uses the last confirmed code and configuration.";

impl FolderApp {
    /// What the header says beside *Cancel run* while a run runs (UX-04):
    /// the text and its hover, or nothing when no run runs. On the run's
    /// own tab it says *code*; on any other it names the file that runs.
    pub(super) fn running_label(
        &self,
        active: Option<&str>,
    ) -> Option<(String, Option<&'static str>)> {
        self.runs.handle.as_ref()?;
        let prepared = self.runs.active.as_ref()?;
        let (what, hover) = if prepared.last_valid {
            ("earlier", Some(EARLIER_HOVER))
        } else if prepared.unsaved {
            ("unsaved", Some(UNSAVED_HOVER))
        } else {
            ("saved", None)
        };
        let id = prepared.document_id();
        let subject = if active == Some(id) {
            "code"
        } else {
            self.session
                .as_ref()
                .and_then(|session| session.document(id))
                .map_or("code", |document| file_name(document.entry().path.as_str()))
        };
        Some((format!("Running {what} {subject}"), hover))
    }

    /// The active tab's run results fill the window (KTLO-19), so nothing
    /// else is drawn. At any other height the views stay drawn above them
    /// (UX-36).
    pub(super) fn report_maximised(&self, active: Option<&str>) -> bool {
        self.runs.panel.is_maximised()
            && active.is_some_and(|id| self.runs.reports.contains_key(id))
    }

    /// The run results panel's heights and drag, for a test's failure message.
    #[cfg(test)]
    pub(super) fn report_state(&self) -> String {
        self.runs.panel.debug_state()
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

    /// Cuts the document's last run back to the steps `keep` accepts, still
    /// running: a run part of the way through, for tests (LAY-11).
    #[cfg(test)]
    pub(super) fn debug_cut_run(
        &mut self,
        id: &str,
        keep: impl Fn(&apiforge_core::run::StepResult) -> bool,
    ) {
        if let Some(report) = self.runs.reports.get_mut(id) {
            report.result.steps.retain(|step| keep(step));
            report.result.status = apiforge_core::run::RunStatus::Running;
            report.ends = Arc::new(CauseEnds::of(&report.result.steps));
        }
    }

    /// Changes the document's last run in place, as another engine or an
    /// older report would have written it, for tests (LAY-19).
    #[cfg(test)]
    pub(super) fn debug_edit_run(&mut self, id: &str, edit: impl FnOnce(&mut RunResult)) {
        if let Some(report) = self.runs.reports.get_mut(id) {
            edit(&mut report.result);
            report.ends = Arc::new(CauseEnds::of(&report.result.steps));
        }
    }

    /// The cause ends of the document's last run, shared with the report
    /// (LAY-19).
    pub(super) fn run_ends(&self, id: &str) -> Option<Arc<CauseEnds>> {
        self.runs
            .reports
            .get(id)
            .map(|report| Arc::clone(&report.ends))
    }

    /// The document's report while it is fresh or only checks were added
    /// from it (UX-01), for the board's and the code's marks.
    pub(super) fn fresh_run(&self, id: &str) -> Option<RunResult> {
        let report = self.runs.reports.get(id)?;
        (self.freshness(report) != Freshness::Stale).then(|| report.result.clone())
    }

    /// The run before the document's run that is on, while it is on: the
    /// board shows its results for the steps not reached yet (LAY-07).
    pub(super) fn previous_run(&self, id: &str) -> Option<&RunResult> {
        let report = self.runs.reports.get(id)?;
        (report.result.status == RunStatus::Running && self.freshness(report) != Freshness::Stale)
            .then(|| previous_of(&report.result, report.previous.as_ref()))
            .flatten()
    }

    /// Fresh while the run's own checks are clear; otherwise, when nothing
    /// but the text changed, the text says how: the run's own source after
    /// every check was undone, or the text right after one of the checks.
    fn freshness(&self, report: &Report) -> Freshness {
        let Some(session) = &self.session else {
            return Freshness::Stale;
        };
        if !session.run_is_stale(&report.prepared) {
            return Freshness::Fresh;
        }
        if report.checks.is_empty() || session.run_is_stale_beyond_text(&report.prepared) {
            return Freshness::Stale;
        }
        let Some(document) = session.document(report.prepared.document_id()) else {
            return Freshness::Stale;
        };
        let version = document.source_version();
        let (hash, run_source) = match report.text.get() {
            Some((cached, hash, run_source)) if cached == version => (hash, run_source),
            _ => {
                let text = document.text();
                let hash = text_hash(text);
                let run_source = report.prepared.snapshot.matches_source(text);
                report.text.set(Some((version, hash, run_source)));
                (hash, run_source)
            }
        };
        if let Some(index) = report
            .checks
            .iter()
            .rposition(|check| check.source_after == hash)
        {
            Freshness::ChecksOnly(index + 1)
        } else if run_source {
            Freshness::Fresh
        } else {
            Freshness::Stale
        }
    }

    #[cfg(test)]
    pub(super) fn run_result(&self, id: &str) -> Option<&RunResult> {
        self.runs.reports.get(id).map(|report| &report.result)
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

    /// Lets go of the journal and of the runner that writes to it: both
    /// hold the writer's store, which a hand-over gives up (MCP-08b).
    pub(super) fn close_run_journal(&mut self) {
        self.runs.runner = None;
        self.runs.journal = None;
    }

    pub(super) fn request_run(&mut self, id: &str, live: bool) {
        if live && self.observing() {
            self.notify(LIVE_PAUSED, true);
            return;
        }
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
        // A confirmation shown before this window began observing.
        if live && self.observing() {
            self.notify(LIVE_PAUSED, true);
            return;
        }
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
                // What runs is said in the header for the run's life, not
                // in the band (UX-04, #152).
                self.runs.active = Some(prepared);
                self.runs.handle = Some(handle);
                self.runs.panel.open = true;
            }
            Err(error) => self.notify(error.to_string(), true),
        }
    }

    pub(super) fn poll_run(&mut self) {
        if let (Some(handle), Some(prepared)) = (&self.runs.handle, &self.runs.active) {
            // The report a new run replaces stays as its previous run while
            // it describes the document as it is (LAY-07).
            let keep = self
                .runs
                .reports
                .get(prepared.document_id())
                .is_some_and(|report| self.freshness(report) != Freshness::Stale);
            if let Some(report) = handle.latest() {
                publish(&mut self.runs.reports, prepared, report, keep);
            }
            if handle.is_finished() {
                if let Some(report) = handle.latest() {
                    publish(&mut self.runs.reports, prepared, report, keep);
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
        if !self.runs.reports.contains_key(id) {
            return;
        }
        let snapshot = self.snapshot(id);
        // The marks are the document's as it stands (DIAG-12).
        let checks = snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.model.as_ref())
            .map(|model| self.check_index(model));
        let Some(report) = self.runs.reports.get(id) else {
            return;
        };
        let freshness = self.freshness(report);
        let result = report.result.clone();
        let ends = Arc::clone(&report.ends);
        let checked: Vec<_> = report.checks[..freshness.checks()]
            .iter()
            .map(|check| check.field.clone())
            .collect();
        let stale = freshness == Freshness::Stale;
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
        // The run's own revision: what it executed, whatever changed since.
        let executed = &report.prepared.snapshot;
        let (main, cleanup) = match executed.document() {
            IdentityBase::Scenario(scenario) => (&scenario.graph, Some(&scenario.cleanup)),
            IdentityBase::Component(component) => (&component.graph, None),
        };
        let graphs = crate::report::RunGraphs {
            main,
            cleanup,
            project: executed.context(),
        };
        // Tree or JSON is kept with the project's view on this computer
        // (REP-08).
        self.runs
            .panel
            .set_json_view(self.workbench.panels.results_json);
        self.runs
            .panel
            .adopt_list_width(self.preferences.run_list_width);
        let actions = self.runs.panel.show(
            ui,
            &crate::report::Context {
                run: Some(&result),
                ends: Some(&ends),
                selected_node_id: selected,
                error_policy: policy,
                running: self.runs.handle.is_some(),
                stale,
                checked: &checked,
                files_busy: false,
                component: self
                    .session
                    .as_ref()
                    .and_then(|session| session.document(id))
                    .is_some_and(|document| document.entry().kind == DocumentKind::Component),
                document_locked: stale
                    || snapshot.as_ref().is_none_or(|snapshot| !snapshot.editable),
                graphs: Some(graphs),
                checks: checks.as_deref(),
            },
        );
        let json = self.runs.panel.json_view();
        if json != self.workbench.panels.results_json {
            self.workbench.panels.results_json = json;
            self.touched();
        }
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
        if let Action::RememberListWidth(width) = action {
            // Remembered on this computer, for every project (REP-07).
            self.preferences.run_list_width = Some(width);
            let preferences = self.preferences.clone();
            self.storage_io(
                "Saving preferences…",
                move || preferences.save(crate::preferences::Preference::RunListWidth),
                |_, _| {},
            );
            return;
        }
        let Some(snapshot) = self.snapshot(id) else {
            return;
        };
        let Some(model) = snapshot.model else { return };
        match action {
            Action::SelectNode(node) => {
                // The run says where its step ran; a check added since the
                // run, chosen in a field's menu (DIAG-12), is where the
                // document holds it.
                let cleanup = match result.steps.iter().find(|step| step.node_id == node) {
                    Some(step) => step.phase == Phase::Cleanup,
                    None => cleanup_holds(&model, &node),
                };
                // A check folded into its subject is found there: the row
                // selects the subject and opens its checks (LAY-11).
                self.select_on_board(id, &model, node, cleanup);
            }
            Action::ExportReport => {
                if let Ok(text) = serde_json::to_string_pretty(result) {
                    self.export_text("run-report.json".into(), text);
                }
            }
            Action::RememberListWidth(_) => {}
            _ if stale || !snapshot.editable => {}
            Action::SetErrorPolicy(policy) => {
                if let Document::Scenario(mut scenario) = Arc::unwrap_or_clone(model) {
                    scenario.error_policy = policy;
                    self.component_action(
                        id,
                        publication::Action::Model {
                            next: Box::new(Document::Scenario(scenario)),
                            selection: None,
                            check: None,
                            moved: Vec::new(),
                            placed: false,
                        },
                    );
                }
            }
            Action::AssertField { .. } | Action::AssignField { .. } => {
                self.field_action(id, action);
            }
        }
    }

    /// A check from the menu *Add a check* (DIAG-08) or *Assign to
    /// variable* on a field of a step's data: the run's results or, before
    /// any run, the step's sample (DIAG-09). Neither needs a run: the graph
    /// that holds the step tells
    /// the cleanup from the main flow. A check is counted against the
    /// document's report only while that report is not stale (UX-01).
    pub(super) fn field_action(&mut self, id: &str, action: crate::report::Action) {
        use crate::report::Action;
        if let Action::RememberListWidth(width) = action {
            // Remembered on this computer, for every project (REP-07).
            self.preferences.run_list_width = Some(width);
            let preferences = self.preferences.clone();
            self.storage_io(
                "Saving preferences…",
                move || preferences.save(crate::preferences::Preference::RunListWidth),
                |_, _| {},
            );
            return;
        }
        let Some(snapshot) = self.snapshot(id) else {
            return;
        };
        if !snapshot.editable {
            return;
        }
        let Some(model) = snapshot.model else { return };
        match action {
            Action::AssertField {
                node_id,
                port,
                path,
                check,
            } => {
                let cleanup = !graph_of(&model, false)
                    .nodes
                    .iter()
                    .any(|node| node.id == node_id);
                let graph = graph_of(&model, cleanup);
                let before = graph.nodes.len();
                let checks = self
                    .runs
                    .reports
                    .get(id)
                    .map(|report| self.freshness(report))
                    .filter(|freshness| *freshness != Freshness::Stale)
                    .map(Freshness::checks);
                if let Some(node) = graph.nodes.iter().find(|node| node.id == node_id) {
                    let context = self.context_project();
                    let parameters = parameters_of(&model);
                    let board = Board {
                        graph,
                        reference: &reference_of(&model, cleanup),
                        project: &context,
                        parameters: &parameters,
                        sibling: sibling_of(&model, cleanup),
                        density: self.density(),
                        satellites: true,
                        lines: lines_of(&self.preferences),
                    };
                    let pushed =
                        board.push_check(&node_id, edit::field_check(node, &port, &path, &check));
                    // The check is counted when its edit applies, which may
                    // first ask about unattached comments (UX-35).
                    self.component_action(
                        id,
                        publication::Action::Model {
                            next: Box::new(with_graph(&model, cleanup, pushed.graph)),
                            selection: pushed.selection,
                            check: checks.map(|checks| {
                                Box::new(PendingCheck {
                                    field: crate::report::CheckedField {
                                        node_id,
                                        port,
                                        path,
                                    },
                                    cleanup,
                                    before,
                                    checks,
                                })
                            }),
                            moved: Vec::new(),
                            placed: false,
                        },
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
            _ => {}
        }
    }

    /// Remembers a check added from the document's report when its edit
    /// applied, the text right after it as the key (UX-01). A check after an
    /// undone one replaces it, as a new edit clears what redo would bring
    /// back.
    pub(super) fn record_check(&mut self, id: &str, check: PendingCheck) {
        let Some(document) = self
            .session
            .as_ref()
            .and_then(|session| session.document(id))
        else {
            return;
        };
        let added = document
            .model()
            .is_some_and(|model| graph_of(model, check.cleanup).nodes.len() > check.before);
        let source_after = text_hash(document.text());
        if added && let Some(report) = self.runs.reports.get_mut(id) {
            report.checks.truncate(check.checks);
            report.checks.push(AddedCheck {
                field: check.field,
                source_after,
            });
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
        #[cfg(test)]
        if let Some(data) = self.legacy_data.clone() {
            self.review_legacy_runs_from(&data);
            return;
        }
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
