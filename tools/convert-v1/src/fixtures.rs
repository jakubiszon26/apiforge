//! Native fixtures generated from the frozen version-one sources.

use crate::{convert, v1};
use serde_json::Value;
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
    request["project"] =
        serde_json::to_value(convert::project(&source)).map_err(|error| error.to_string())?;
    Ok(json_text(&fixture))
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
