//! Narrow Git checkout rules installed only by explicit project creation.
use crate::Result;
use apiforge_storage::project::{FileState, ProjectChange, ProjectPath, ProjectStore};

const LF_RULES: &[u8] =
    b"*.afs text eol=lf\n/apiforge.project.json text eol=lf\n/.apiforge/diagram.json text eol=lf\n";

pub(crate) fn lf_attributes(store: &ProjectStore) -> Result<Vec<ProjectChange>> {
    let path = ProjectPath::new(".gitattributes")?;
    let snapshot = store.read(std::slice::from_ref(&path))?;
    let current = snapshot.files[&path].as_deref().unwrap_or_default();
    if current.ends_with(LF_RULES) {
        return Ok(Vec::new());
    }
    let mut bytes = current.to_vec();
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        bytes.push(b'\n');
    }
    bytes.extend_from_slice(LF_RULES);
    Ok(vec![ProjectChange::Write {
        expected: snapshot.state(&path).unwrap_or(FileState::Missing),
        path,
        bytes,
    }])
}
