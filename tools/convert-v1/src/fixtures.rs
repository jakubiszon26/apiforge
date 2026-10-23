//! Native fixtures generated from the frozen version-one sources.

use crate::{convert, v1};
use apiforge_core::{
    format::{Graph, Project, Scenario},
    graph::{VALUE_INPUT, component_definition, condition_inputs},
    model::NodeKind,
    template::{self, template_text},
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
    let source_scenario = source
        .scenarios
        .iter()
        .find(|candidate| candidate.id == scenario_id)
        .ok_or("the fixture's scenario does not exist")?;
    let context = Context {
        project: &project,
        scenario,
        source: &source,
        source_scenario,
    };
    if let Some(previous) = request.get_mut("previousRun") {
        convert_report(previous, &context)?;
    }
    request["project"] = serde_json::to_value(&project).map_err(|error| error.to_string())?;
    convert_report(&mut fixture["expected"], &context)?;
    Ok(json_text(&fixture))
}

/// The converted project of a run fixture and its version-one source.
struct Context<'a> {
    project: &'a Project,
    scenario: &'a Scenario,
    source: &'a v1::Project,
    source_scenario: &'a v1::Scenario,
}

/// Adapts a recorded report: each step's `name` becomes its node's handle and label.
/// Steps are looked up in the graph they ran in: main, cleanup, or the component
/// definition of the step that contains them.
fn convert_report(report: &mut Value, context: &Context<'_>) -> Result<(), String> {
    fn convert_steps(
        steps: &mut [Value],
        graphs: Option<(&Graph, &v1::Graph)>,
        context: &Context<'_>,
    ) -> Result<(), String> {
        let (project, scenario) = (context.project, context.scenario);
        for step in steps {
            let Value::Object(fields) = step else {
                return Err("a step is not an object".into());
            };
            let id = fields
                .get("nodeId")
                .and_then(Value::as_str)
                .ok_or("a step without a node identifier")?
                .to_owned();
            let (graph, source_graph) = match graphs {
                Some(graphs) => graphs,
                None if fields.get("phase").and_then(Value::as_str) == Some("cleanup") => {
                    (&scenario.cleanup, &context.source_scenario.cleanup)
                }
                None => (&scenario.graph, &context.source_scenario.graph),
            };
            let source_node = source_graph
                .nodes
                .iter()
                .find(|node| node.id == id)
                .ok_or_else(|| format!("a step of unknown node {id}"))?;
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
            if node.kind == NodeKind::Value {
                // A Value's only input is its expression, whose result it outputs.
                next.insert("kind".into(), Value::String("value".into()));
                let inputs = next
                    .get("outputs")
                    .and_then(|outputs| outputs.get(VALUE_INPUT))
                    .map(|value| Map::from_iter([(VALUE_INPUT.to_owned(), value.clone())]))
                    .unwrap_or_default();
                next.insert("inputs".into(), Value::Object(inputs));
            }
            if node.kind == NodeKind::Condition {
                condition_step(&mut next, source_node);
            }
            if let Some(Value::Array(children)) = next.get_mut("children") {
                let definition = component_definition(project, node)
                    .ok_or_else(|| format!("children of {id} without a component"))?;
                let source_definition = context
                    .source
                    .components
                    .iter()
                    .find(|component| {
                        component.id == definition.id && component.revision == definition.revision
                    })
                    .ok_or_else(|| format!("children of {id} without a source component"))?;
                convert_steps(
                    children,
                    Some((&definition.graph, &source_definition.graph)),
                    context,
                )?;
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
        Some(Value::Array(list)) => convert_steps(list, None, context),
        _ => Ok(()),
    }
}

/// A condition step with positional inputs: check *n* reports `actual<n>`, the
/// recorded input narrowed to the predicate's path, and `expected<n>`. A condition
/// with several checks passes its `actual` input on as `value` and reports no
/// single expected value.
fn condition_step(step: &mut Map<String, Value>, source: &v1::Node) {
    let recorded = step
        .get("inputs")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let checks = source
        .config
        .predicates
        .as_ref()
        .filter(|checks| !checks.is_empty());
    let mut inputs = Map::new();
    match checks {
        Some(checks) => {
            if let Some(value) = recorded.get("actual") {
                inputs.insert(VALUE_INPUT.to_owned(), value.clone());
            }
            for (index, check) in checks.iter().enumerate() {
                let (actual, expected) = condition_inputs(index + 1);
                let lookup = |key: &str| recorded.get(key).map(template_text);
                let path = check.path.clone().unwrap_or_default();
                let mut expanded = String::new();
                let mut last = 0;
                let mut complete = true;
                for token in template::tokens(&path) {
                    expanded.push_str(&path[last..token.range.start]);
                    match lookup(token.key) {
                        Some(text) => expanded.push_str(&text),
                        None => complete = false,
                    }
                    last = token.range.end;
                }
                expanded.push_str(&path[last..]);
                if let Some(value) = recorded
                    .get(&check.input)
                    .filter(|_| complete)
                    .and_then(|value| apiforge_core::path::resolve(value, &expanded))
                {
                    inputs.insert(actual, value.into_owned());
                }
                if let Some(value) = recorded.get(&check.expected) {
                    inputs.insert(expected, value.clone());
                }
            }
            step.remove("expected");
        }
        None => {
            let (actual, expected) = condition_inputs(1);
            for (from, to) in [("actual", actual), ("expected", expected)] {
                if let Some(value) = recorded.get(from) {
                    inputs.insert(to, value.clone());
                }
            }
        }
    }
    step.insert("inputs".into(), Value::Object(inputs));
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
