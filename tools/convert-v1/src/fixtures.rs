//! Native fixtures generated from the frozen version-one sources.

use crate::{convert, v1};
use apiforge_core::{
    format::{Graph, Project, Scenario},
    graph::component_definition,
};
use serde_json::{Map, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// The repository root, two levels above this crate.
pub fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Converts a version-one workspace or standalone project and validates the result.
pub fn convert_document(bytes: &[u8]) -> Result<String, String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    let converted = if value.get("projects").is_some() {
        let source: v1::Workspace =
            serde_json::from_value(value).map_err(|error| error.to_string())?;
        let workspace = convert::workspace(&source);
        apiforge_core::validate_workspace(&workspace).map_err(|error| error.to_string())?;
        serde_json::to_value(workspace)
    } else {
        let source: v1::Project =
            serde_json::from_value(value).map_err(|error| error.to_string())?;
        let project = convert::project(&source);
        apiforge_core::validate_project(&project).map_err(|error| error.to_string())?;
        serde_json::to_value(project)
    }
    .map_err(|error| error.to_string())?;
    Ok(json_text(&converted))
}

/// A run fixture with its project converted and its reports adapted.
///
/// The project is not validated: some fixtures hold projects the native
/// validator deliberately rejects (see `run_corpus.rs`).
fn convert_run(bytes: &[u8]) -> Result<String, String> {
    let mut fixture: Value = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    let request = fixture
        .get_mut("request")
        .ok_or("run fixture without a request")?;
    let source: v1::Project = serde_json::from_value(request["project"].take())
        .map_err(|error| format!("request.project: {error}"))?;
    let project = convert::project(&source);
    let scenario_id = request["scenarioId"]
        .as_str()
        .ok_or("run fixture without a scenario")?
        .to_owned();
    let scenario = project
        .scenarios
        .iter()
        .find(|scenario| scenario.id == scenario_id)
        .ok_or("the fixture's scenario does not exist")?;
    if let Some(previous) = request.get_mut("previousRun") {
        convert_report(previous, &project, scenario)?;
    }
    request["project"] = serde_json::to_value(&project).map_err(|error| error.to_string())?;
    convert_report(&mut fixture["expected"], &project, scenario)?;
    Ok(json_text(&fixture))
}

/// Adapts a recorded report: each step's `name` becomes its node's handle and label.
/// Steps are looked up in the graph they ran in: main, cleanup, or the component
/// definition of the step that contains them.
fn convert_report(
    report: &mut Value,
    project: &Project,
    scenario: &Scenario,
) -> Result<(), String> {
    fn convert_steps(
        steps: &mut [Value],
        graph: Option<&Graph>,
        project: &Project,
        scenario: &Scenario,
    ) -> Result<(), String> {
        for step in steps {
            let Value::Object(fields) = step else {
                return Err("a step is not an object".into());
            };
            let id = fields
                .get("nodeId")
                .and_then(Value::as_str)
                .ok_or("a step without a node identifier")?
                .to_owned();
            let graph = match graph {
                Some(graph) => graph,
                None if fields.get("phase").and_then(Value::as_str) == Some("cleanup") => {
                    &scenario.cleanup
                }
                None => &scenario.graph,
            };
            let node = graph
                .nodes
                .iter()
                .find(|node| node.id == id)
                .ok_or_else(|| format!("a step of unknown node {id}"))?;
            let mut next = Map::new();
            for (key, value) in std::mem::take(fields) {
                if key == "name" {
                    next.insert("handle".into(), Value::String(node.handle.clone()));
                    if let Some(label) = &node.label {
                        next.insert("label".into(), Value::String(label.clone()));
                    }
                } else {
                    next.insert(key, value);
                }
            }
            if let Some(Value::Array(children)) = next.get_mut("children") {
                let definition = component_definition(project, node)
                    .ok_or_else(|| format!("children of {id} without a component"))?;
                convert_steps(children, Some(&definition.graph), project, scenario)?;
            }
            *fields = next;
        }
        Ok(())
    }
    // A snapshot holds the scenario's graphs as they ran.
    if let Some(snapshot) = report.get_mut("snapshot") {
        let graph = |value: &mut Value| -> Result<v1::Graph, String> {
            serde_json::from_value(value.take()).map_err(|error| format!("snapshot: {error}"))
        };
        let (main, cleanup) = convert::scenario_graphs(
            &graph(&mut snapshot["graph"])?,
            &graph(&mut snapshot["cleanup"])?,
        );
        snapshot["graph"] = serde_json::to_value(main).map_err(|error| error.to_string())?;
        snapshot["cleanup"] = serde_json::to_value(cleanup).map_err(|error| error.to_string())?;
    }
    match report.get_mut("steps") {
        Some(Value::Array(list)) => convert_steps(list, None, project, scenario),
        _ => Ok(()),
    }
}

fn json_text(value: &Value) -> String {
    let mut text = serde_json::to_string_pretty(value).expect("JSON values serialise");
    text.push('\n');
    text
}

/// Every generated fixture as (path relative to the repository, content).
fn generate(root: &Path) -> Result<Vec<(PathBuf, String)>, String> {
    let sources = root.join("tools/convert-v1/sources");
    let mut files = Vec::new();
    let mut add = |source: PathBuf,
                   target: PathBuf,
                   convert: fn(&[u8]) -> Result<String, String>| {
        let bytes = fs::read(&source).map_err(|error| format!("{}: {error}", source.display()))?;
        let text = convert(&bytes).map_err(|error| format!("{}: {error}", source.display()))?;
        files.push((target, text));
        Ok::<_, String>(())
    };
    for (directory, convert) in [
        (
            "projects",
            convert_document as fn(&[u8]) -> Result<String, String>,
        ),
        ("runs", convert_run),
    ] {
        for source in json_files(&sources.join(directory))? {
            let name = source.file_name().expect("a file name").to_owned();
            add(
                source,
                Path::new("fixtures").join(directory).join(name),
                convert,
            )?;
        }
    }
    add(
        sources.join("workspace-v1.json"),
        PathBuf::from("fixtures/workspace.json"),
        convert_document,
    )?;
    Ok(files)
}

fn json_files(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files: Vec<PathBuf> = fs::read_dir(directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    files.sort();
    Ok(files)
}

pub fn write(root: &Path) -> Result<(), String> {
    for (path, text) in generate(root)? {
        let target = root.join(&path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        fs::write(&target, text).map_err(|error| format!("{}: {error}", target.display()))?;
    }
    Ok(())
}

pub fn check(root: &Path) -> Result<(), String> {
    let stale: Vec<String> = generate(root)?
        .into_iter()
        .filter(|(path, text)| fs::read_to_string(root.join(path)).ok().as_deref() != Some(text))
        .map(|(path, _)| path.display().to_string())
        .collect();
    if stale.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Stale fixtures, regenerate with `cargo run -p apiforge-convert-v1 -- --fixtures`:\n{}",
            stale.join("\n")
        ))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn generated_fixtures_are_up_to_date() {
        if let Err(error) = super::check(&super::repository()) {
            panic!("{error}");
        }
    }
}
