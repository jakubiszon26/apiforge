//! Folder commands share the desktop's stable loader and folder analysis policy.
//! Only committed source is loaded: recovery and last-valid cache are excluded.
use apiforge_mcp::folder::{check_document as check_saved, select as select_path};
use apiforge_project::{Origin, ProjectSession};
use std::{ffi::OsString, path::PathBuf};

pub const USAGE: &str = "apiforge check --folder <directory> [--file <relative.afs>]\napiforge print --folder <directory> --file <relative.afs>\napiforge apply --folder <directory> --file <relative.afs> <source.afs>";

struct Args {
    command: String,
    folder: PathBuf,
    file: Option<String>,
    source: Option<PathBuf>,
}

fn arguments(args: &[OsString]) -> Option<Args> {
    let command = args.first()?.to_str()?;
    if !matches!(command, "check" | "print" | "apply") {
        return None;
    }
    let (mut folder, mut file, mut source) = (None, None, None);
    let mut rest = args[1..].iter();
    while let Some(arg) = rest.next() {
        match arg.to_str() {
            Some("--folder") if folder.is_none() => folder = Some(PathBuf::from(rest.next()?)),
            Some("--file") if file.is_none() => file = Some(rest.next()?.to_str()?.to_owned()),
            Some(value) if value.starts_with('-') => return None,
            _ if source.is_none() => source = Some(PathBuf::from(arg)),
            _ => return None,
        }
    }
    if (command != "check" && file.is_none()) || (command == "apply") != source.is_some() {
        return None;
    }
    Some(Args {
        command: command.into(),
        folder: folder?,
        file,
        source,
    })
}

pub fn run(args: &[OsString]) -> u8 {
    let Some(args) = arguments(args) else {
        eprintln!("{USAGE}");
        return 2;
    };
    match execute(args) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn execute(args: Args) -> Result<(), String> {
    let mut session = if args.command == "apply" {
        ProjectSession::open_saved_exclusive(&args.folder)
    } else {
        ProjectSession::open_saved_read_only(&args.folder)
    }
    .map_err(|error| error.to_string())?;
    let selected = args
        .file
        .as_deref()
        .map(|path| select(&session, path))
        .transpose()?;
    if args.command == "apply" {
        let id = selected.as_ref().expect("apply requires --file");
        let text = super::source(args.source.as_ref().expect("apply requires source"))?;
        session
            .edit_source(id, &text, Origin::SESSION)
            .map_err(|e| e.to_string())?;
        session.settle(id).map_err(|e| e.to_string())?;
        check_document(&session, id)?;
        session.save(id).map_err(|e| e.to_string())?;
        return Ok(());
    }
    if args.command == "print" {
        let id = selected.as_ref().expect("print requires --file");
        check_document(&session, id)?;
        let model = session
            .document(id)
            .and_then(|document| document.model())
            .ok_or("No valid definition")?;
        let text = apiforge_script::print_folder_document(session.context(), model).map_err(
            |diagnostics| {
                super::diagnostics(&diagnostics);
                "Cannot print definition"
            },
        )?;
        print!("{text}");
        return Ok(());
    }
    let mut valid = true;
    for document in session
        .documents()
        .iter()
        .filter(|document| selected.as_deref().is_none_or(|id| document.id() == id))
    {
        valid &= check_document(&session, document.id()).is_ok();
    }
    if valid {
        Ok(())
    } else {
        Err("Saved project source has errors".into())
    }
}

/// Prints the document's diagnostics in the CLI's stderr format.
fn check_document(session: &ProjectSession, id: &str) -> Result<(), String> {
    let (document, valid) = check_saved(session, id).map_err(|error| error.message)?;
    super::diagnostics(document.diagnostics());
    if !valid {
        return Err("Saved source has no valid definition".into());
    }
    Ok(())
}

pub(super) fn select(session: &ProjectSession, path: &str) -> Result<String, String> {
    select_path(session, path).map_err(|error| error.message)
}

pub(super) fn execute_run(args: super::run::Args) -> Result<apiforge_core::run::RunStatus, String> {
    let mut session = if args.live {
        ProjectSession::open_saved_exclusive(&args.path)
    } else {
        ProjectSession::open_saved_read_only(&args.path)
    }
    .map_err(|error| error.to_string())?;
    let id = session
        .documents()
        .iter()
        .find(|document| {
            document.entry().kind == apiforge_project::DocumentKind::Scenario
                && (document.entry().entity_id == args.scenario
                    || document.entry().path.as_str() == args.scenario)
        })
        .map(|document| document.id().to_owned())
        .ok_or("No scenario has this registered path or ID")?;
    let snapshot = session
        .current_snapshot(&id)
        .map_err(|error| error.to_string())?;
    let apiforge_storage::project::IdentityBase::Scenario(scenario) = snapshot.document() else {
        return Err("Expected a scenario".into());
    };
    let mut project = snapshot.context().clone();
    project.scenarios.push(scenario.clone());
    let secrets = apiforge_mcp::executor::secrets(args.secrets.as_deref())?;
    session.store().set_known_secrets(secrets.values().cloned());
    let journal = if args.live {
        Some(
            apiforge_storage::JournalStore::open_project(session.store().clone())
                .map_err(|error| error.to_string())?,
        )
    } else {
        None
    };
    let request = apiforge_runner::RunRequest {
        project,
        scenario_id: scenario.id.clone(),
        target: apiforge_runner::RunTarget::Scenario,
        mode: if args.live {
            apiforge_core::run::RunMode::Live
        } else {
            apiforge_core::run::RunMode::Simulation
        },
        target_node_id: None,
        single_node_id: None,
        previous_run: None,
        secrets,
    };
    super::run::execute_request(request, journal, args.json)
}
