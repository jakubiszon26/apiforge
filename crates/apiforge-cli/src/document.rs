//! Workspace-aware commands. Mutations share the desktop's exclusive session,
//! external-change checks, backup rotation and deliberate recovery rules.
use apiforge_core::{ImportLimits, format::*, import_document};
use apiforge_script::{Document, LowerContext, lower, parse, print_scenario, reconcile};
use apiforge_storage::WorkspaceStore;
use std::{
    ffi::OsString,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

pub const USAGE: &str = "apiforge check --workspace <doc.json> [--scenario <name|id>] [<file.afs>]\napiforge print --workspace <doc.json> --scenario <name|id>\napiforge apply --workspace <doc.json> --scenario <name|id> <file.afs>";
struct Args {
    command: String,
    path: PathBuf,
    scenario: Option<String>,
    source: Option<PathBuf>,
}
fn arguments(args: &[OsString]) -> Option<Args> {
    let command = args.first()?.to_str()?;
    if !matches!(command, "check" | "print" | "apply") {
        return None;
    }
    let mut path = None;
    let mut scenario = None;
    let mut source = None;
    let mut rest = args[1..].iter();
    while let Some(arg) = rest.next() {
        if arg == "--workspace" && path.is_none() {
            path = Some(PathBuf::from(rest.next()?));
        } else if arg == "--scenario" && scenario.is_none() {
            scenario = Some(rest.next()?.to_str()?.to_owned());
        } else if arg.to_str().is_some_and(|s| s.starts_with('-')) || source.is_some() {
            return None;
        } else {
            source = Some(PathBuf::from(arg));
        }
    }
    if (command == "print" && (scenario.is_none() || source.is_some()))
        || (command == "apply" && (scenario.is_none() || source.is_none()))
    {
        return None;
    }
    Some(Args {
        command: command.into(),
        path: path?,
        scenario,
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
        Err(message) => {
            eprintln!("{message}");
            1
        }
    }
}
fn read_document(path: &Path) -> Result<apiforge_core::ValidatedDocument, String> {
    let limits = ImportLimits::default();
    let file = File::open(path).map_err(|e| format!("Cannot read document: {e}"))?;
    let mut bytes = Vec::new();
    file.take(limits.max_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Cannot read document: {e}"))?;
    // Import diagnostics can contain user-defined names in paths. Keep the CLI
    // message static so malformed documents never echo credential literals.
    import_document(&bytes, &limits)
        .map_err(|_| "Invalid or unsupported workspace/project document".into())
}
fn scenario_index(project: &Project, selector: &str) -> Result<usize, String> {
    if let Some(index) = project.scenarios.iter().position(|s| s.id == selector) {
        return Ok(index);
    }
    let matches: Vec<_> = project
        .scenarios
        .iter()
        .enumerate()
        .filter(|(_, s)| s.name == selector)
        .map(|(i, _)| i)
        .collect();
    match matches.as_slice() {
        [index] => Ok(*index),
        _ => Err("Scenario name is missing or ambiguous; use its ID".into()),
    }
}
fn execute(args: Args) -> Result<(), String> {
    let store = if args.command == "apply" {
        WorkspaceStore::open_exclusive(&args.path)
    } else {
        WorkspaceStore::open_read_only(&args.path)
    }
    .map_err(|e| e.to_string())?;
    let loaded = store
        .load()
        .map_err(|_| "Cannot load document; check its format and recovery state".to_owned())?
        .ok_or("Workspace/project file does not exist")?;
    if loaded.recovered.is_some() {
        return Err(
            "Backup recovery must be explicitly completed before using this document".into(),
        );
    }
    let envelope = read_document(&args.path)?;
    if envelope.workspace() != &loaded.workspace {
        return Err("Document changed while loading; reload and try again".into());
    }
    let standalone = envelope.is_standalone_project();
    let mut workspace = loaded.workspace;
    let project_index = workspace
        .projects
        .iter()
        .position(|p| p.id == workspace.active_id)
        .ok_or("Active project does not exist")?;
    let project = &workspace.projects[project_index];
    let selected = args
        .scenario
        .as_deref()
        .map(|s| scenario_index(project, s))
        .transpose()?;
    if args.command == "print" {
        let text = print_scenario(project, &project.scenarios[selected.unwrap()]).map_err(|d| {
            super::diagnostics(&d);
            "Cannot print scenario"
        })?;
        print!("{text}");
        return Ok(());
    }
    let Some(path) = args.source else {
        for (index, scenario) in project
            .scenarios
            .iter()
            .enumerate()
            .filter(|(i, _)| selected.is_none_or(|s| s == *i))
        {
            let _ = index;
            // The checked projection also resolves workspace names; import
            // validation alone deliberately allows unresolved environments.
            let source = print_scenario(project, scenario).map_err(|d| {
                super::diagnostics(&d);
                "Scenario has diagnostics"
            })?;
            let checked = lower(&parse(&source), &LowerContext::new(project));
            super::diagnostics(&checked.diagnostics);
            if checked.has_errors() {
                return Err("Scenario has diagnostics".into());
            }
        }
        return Ok(());
    };
    let text = super::source(&path)?;
    let tree = parse(&text);
    let lowered = lower(&tree, &LowerContext::new(project));
    super::diagnostics(&lowered.diagnostics);
    if lowered.has_errors() {
        return Err("Source has diagnostics; document was preserved".into());
    }
    if args.command == "check" {
        return Ok(());
    }
    let index = selected.unwrap();
    let current = Document::Scenario(project.scenarios[index].clone());
    let mut result = reconcile(&current, &lowered).map_err(str::to_owned)?;
    let Document::Scenario(scenario) = &mut result.document else {
        return Err("Expected a scenario declaration".into());
    };
    // M2 will replace this simple placement with the shared layout engine. Keep
    // existing positions and place only new nodes beyond the board's right edge.
    for (new, old) in [
        (&mut scenario.graph, &project.scenarios[index].graph),
        (&mut scenario.cleanup, &project.scenarios[index].cleanup),
    ] {
        let mut next_x = old
            .nodes
            .iter()
            .map(|n| n.position.x)
            .reduce(f64::max)
            .map_or(0.0, |x| x + 340.0);
        for node in &mut new.nodes {
            if result.unpositioned.contains(&node.id) {
                node.position = Position { x: next_x, y: 0.0 };
                node.auto_placed = true;
                next_x += 340.0;
            }
        }
    }
    // A semantic no-op must not rotate backups or rewrite existing layout/order.
    if apiforge_script::semantic(&current, project)
        == apiforge_script::semantic(&result.document, project)
    {
        return Ok(());
    }
    let Document::Scenario(scenario) = result.document else {
        unreachable!()
    };
    workspace.projects[project_index].scenarios[index] = scenario;
    apiforge_core::validate_workspace(&workspace)
        .map_err(|_| "Resulting document is invalid; original was preserved")?;
    if standalone {
        store.save_project(&workspace.projects[0])
    } else {
        store.save(&workspace)
    }
    .map_err(|e| e.to_string())?;
    Ok(())
}
