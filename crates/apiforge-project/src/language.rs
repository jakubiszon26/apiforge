//! The language version a project's files are written in (LANG-15, LANG-00
//! section 2). A build reads one language, its own: a project in a newer
//! one is not opened, and a project in an older one opens for reading and
//! editing but does not run until `apiforge fix` has raised it.
use crate::{Error, Result};
use apiforge_core::format::{Contract, Environment};
use apiforge_storage::project::ProjectManifest;
use std::path::Path;

pub use apiforge_script::LANGUAGE_VERSION;

/// How a project's language compares with the build's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Support {
    Same,
    /// The project is older: it opens, `check` reports it and it does not run.
    Older,
    /// The project is newer: it is not opened.
    Newer,
}

/// The rule, with both numbers as arguments.
pub fn support(project: u32, build: u32) -> Support {
    match project.cmp(&build) {
        std::cmp::Ordering::Less => Support::Older,
        std::cmp::Ordering::Equal => Support::Same,
        std::cmp::Ordering::Greater => Support::Newer,
    }
}

/// A new manifest, for this build's language.
pub(crate) fn new_manifest(
    id: String,
    name: String,
    environment: Environment,
    contracts: Vec<Contract>,
) -> ProjectManifest {
    ProjectManifest::new(LANGUAGE_VERSION, id, name, environment, contracts)
}

/// Refuses a project written in a newer language than this build's: the
/// check a caller makes before it takes a project's writer lock.
pub fn refuse_newer(manifest: &ProjectManifest) -> Result<()> {
    match support(manifest.language(), LANGUAGE_VERSION) {
        Support::Newer => Err(Error::NewerLanguage {
            project: manifest.language(),
            build: LANGUAGE_VERSION,
        }),
        Support::Same | Support::Older => Ok(()),
    }
}

/// The problem of a project at `folder` written in an older language than
/// this build's: `check` reports it and a run is refused with it.
pub(crate) fn older(manifest: &ProjectManifest, folder: &Path) -> Option<Error> {
    (support(manifest.language(), LANGUAGE_VERSION) == Support::Older).then(|| {
        Error::OlderLanguage {
            project: manifest.language(),
            build: LANGUAGE_VERSION,
            folder: folder.to_owned(),
        }
    })
}

/// The folder as a command line argument: quoted when it has a space. A
/// quoted folder loses its trailing separators, so that a Windows shell
/// does not read `\"` as an escaped quote (`"C:\My Project\"`).
pub(crate) fn argument(folder: &Path) -> String {
    let folder = folder.display().to_string();
    if folder.contains(char::is_whitespace) {
        format!("\"{}\"", folder.trim_end_matches(['\\', '/']))
    } else {
        folder
    }
}

/// The error on an `@after` line of a project at `folder` still in an
/// older language (LANG-20): the conversion is the project's, by the
/// command the run refusal names, never a line deleted by hand.
pub fn after_in_older(folder: &Path) -> String {
    format!(
        "`@after` is not part of language {LANGUAGE_VERSION}. Update the project with apiforge fix --folder {}; do not delete the line by hand.",
        argument(folder)
    )
}

/// The last sentence of a refused conversion.
pub const NOTHING_WRITTEN: &str = "Nothing was written. Change these by hand and run apiforge fix again, or restore a source from Git: one written or edited on the board before the project was converted is judged like any other.";

/// What `apiforge fix --folder` found in one source of the manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceReport {
    /// The registered path.
    pub path: String,
    pub outcome: SourceOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceOutcome {
    /// Already written as language 2 reads it.
    Unchanged,
    /// Converted, or convertible under `--check`: what the fix removes and
    /// writes, in source order.
    Converted(Vec<Change>),
    /// The statements in the way.
    Blocked(Vec<Problem>),
    /// The file cannot be read: missing, not text, over the source limit, or
    /// it does not parse.
    Unreadable(Problem),
}

/// One edit of the conversion, at its line of the source as it was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub line: usize,
    /// The text removed, trimmed; empty when nothing is.
    pub removed: String,
    /// The text written, trimmed; empty when nothing is.
    pub written: String,
}

/// Something the person changes by hand before the project converts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// One-based; 0 for a whole file.
    pub line: usize,
    pub column: usize,
    /// The first line of the statement, trimmed; empty for a whole file.
    pub statement: String,
    pub reason: String,
    pub todo: String,
}

/// What the command found in every source of the manifest, in its order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixReport {
    pub sources: Vec<SourceReport>,
}

impl FixReport {
    /// Whether every source converts: the project is converted whole.
    pub fn convertible(&self) -> bool {
        self.sources.iter().all(|source| {
            matches!(
                source.outcome,
                SourceOutcome::Unchanged | SourceOutcome::Converted(_)
            )
        })
    }
}

/// What `apiforge fix --folder` does with a project.
pub enum Planned {
    /// The project is already in this build's language: nothing to do.
    Current,
    Plan(FixPlan),
}

/// The judgement of every source, the commit it makes and what it read.
pub struct FixPlan {
    report: FixReport,
}

impl FixPlan {
    pub fn report(&self) -> &FixReport {
        &self.report
    }

    /// Writes every converted source and the raised manifest in one commit,
    /// or nothing.
    pub fn commit(self) -> Result<FixReport> {
        Err(Error::NotConvertible)
    }
}

/// Reads the project at `folder` and judges every source of its manifest.
/// With `check` the project is opened read-only; otherwise as its writer.
pub fn plan_fix(folder: &Path, check: bool) -> Result<Planned> {
    let _ = (folder, check);
    Err(Error::NotConvertible)
}

/// The store's refusal of the conversion's commit, as the command says it.
pub fn commit_refusal(error: apiforge_storage::project::ProjectError, root: &Path) -> Error {
    let _ = root;
    Error::Storage(error)
}

#[cfg(test)]
mod tests {
    use super::argument;
    use std::path::Path;

    #[test]
    fn a_folder_argument_is_quoted_only_with_a_space_and_never_ends_in_a_separator() {
        assert_eq!(argument(Path::new("/work/shop")), "/work/shop");
        assert_eq!(argument(Path::new("C:\\work\\shop")), "C:\\work\\shop");
        assert_eq!(
            argument(Path::new("/home/me/My Project")),
            "\"/home/me/My Project\""
        );
        assert_eq!(
            argument(Path::new("C:\\My Project\\")),
            "\"C:\\My Project\""
        );
        assert_eq!(
            argument(Path::new("/home/me/My Project/")),
            "\"/home/me/My Project\""
        );
        assert_eq!(
            argument(Path::new("C:\\My Project\\\\")),
            "\"C:\\My Project\""
        );
    }
}
