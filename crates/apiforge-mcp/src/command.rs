//! `apiforge run --folder … --live`: a live run of a saved scenario from the
//! command line (MCP-15). The run's body is a library function so that the
//! `apiforge` binary prints its report and a desktop test runs the same path
//! as a process of its own.
use crate::executor::{self, CancellationToken, DEADLINE, RunOutcome};
use apiforge_core::journal::JournalRun;
use apiforge_project::{DocumentKind, Holder, ProjectSession};
use apiforge_storage::project::IdentityBase;
use std::path::Path;

/// The command's name as the person typed it, in the holder file and in the
/// request to a window.
pub const CLIENT: &str = "run --live";

/// Runs the saved scenario `scenario` (a registered path or an entity ID) of
/// the folder project at `folder` live, journalled, and returns what
/// happened. `before` hears what an earlier run left unfinished and whether
/// the journal was unreadable, before this run adds to it.
pub fn run_live(
    folder: &Path,
    scenario: &str,
    secrets: Option<&Path>,
    before: &mut dyn FnMut(&[JournalRun], bool),
) -> Result<RunOutcome, String> {
    let mut session = ProjectSession::open_saved_exclusive_as(folder, &Holder::command("run"))
        .map_err(|error| error.to_string())?;
    let id = select(&session, scenario)?;
    let snapshot = session
        .current_snapshot(&id)
        .map_err(|error| error.to_string())?;
    let IdentityBase::Scenario(chosen) = snapshot.document() else {
        return Err("Expected a scenario".into());
    };
    let mut project = snapshot.context().clone();
    project.scenarios.push(chosen.clone());
    let secrets = executor::secrets(secrets)?;
    session.store().set_known_secrets(secrets.values().cloned());
    let journal = apiforge_storage::JournalStore::open_project(session.store().clone())
        .map_err(|error| error.to_string())?;
    before(&journal.interrupted, journal.unreadable);
    let request = apiforge_runner::RunRequest {
        project,
        scenario_id: chosen.id.clone(),
        target: apiforge_runner::RunTarget::Scenario,
        mode: apiforge_core::run::RunMode::Live,
        target_node_id: None,
        single_node_id: None,
        previous_run: None,
        secrets,
    };
    executor::execute(request, Some(journal), CancellationToken::new(), DEADLINE)
}

/// The registered scenario named by its path or entity ID.
fn select(session: &ProjectSession, scenario: &str) -> Result<String, String> {
    session
        .documents()
        .iter()
        .find(|document| {
            document.entry().kind == DocumentKind::Scenario
                && (document.entry().entity_id == scenario
                    || document.entry().path.as_str() == scenario)
        })
        .map(|document| document.id().to_owned())
        .ok_or_else(|| "No scenario has this registered path or ID".to_owned())
}
