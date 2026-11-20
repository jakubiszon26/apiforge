//! The language version a project's files are written in (LANG-15, LANG-00
//! section 2). A build reads one language, its own: a project in a newer
//! one is not opened, and a project in an older one opens for reading and
//! editing but does not run until `apiforge fix` has raised it.
use crate::{Error, FixedPlacer, Result};
use apiforge_core::format::{Contract, Environment};
use apiforge_script::{Blocked, Judged, LowerContext};
use apiforge_storage::project::{
    FileState, Holder, MANIFEST_PATH, ProjectChange, ProjectError, ProjectManifest, ProjectPath,
    ProjectStore, ReadSet,
};
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
    store: ProjectStore,
    report: FixReport,
    /// The converted sources and the raised manifest.
    changes: Vec<ProjectChange>,
    /// Every document of the manifest and the manifest, as they were read.
    read: ReadSet,
}

impl FixPlan {
    pub fn report(&self) -> &FixReport {
        &self.report
    }

    /// Writes every converted source and the raised manifest in one commit
    /// of the project store, or nothing: a source that does not convert, a
    /// file read that changed since, or the store's limits refuse it.
    pub fn commit(self) -> Result<FixReport> {
        if !self.report.convertible() {
            return Err(Error::NotConvertible);
        }
        self.store
            .commit_with_read_set(self.changes, &self.read)
            .map_err(|error| commit_refusal(error, self.store.root()))?;
        Ok(self.report)
    }
}

/// Reads the project at `folder` and judges every source of its manifest by
/// its 0.1.0 reading (LANG-20). The manifest is read first without a lock: a
/// project of a newer language is refused, one of this build's language is
/// `Current`. With `check` the project stays read-only and nothing is ever
/// written, `.apiforge/local/` included; otherwise it is opened as its
/// writer, as `apply` opens it, and a project held by someone else is
/// refused.
pub fn plan_fix(folder: &Path, check: bool) -> Result<Planned> {
    let probe = ProjectStore::open_read_only(folder)?;
    let Some(manifest) = read_manifest(&probe)? else {
        return Err(Error::NotAProject);
    };
    refuse_newer(&manifest.0)?;
    if support(manifest.0.language(), LANGUAGE_VERSION) == Support::Same {
        return Ok(Planned::Current);
    }
    let store = if check {
        probe
    } else {
        drop(probe);
        ProjectStore::open_exclusive_as(folder, &Holder::command("fix"))?
    };
    // The manifest and the published revisions in one stable read, under
    // the lock when there is one: what the sources are judged against.
    let manifest_path = ProjectPath::new(MANIFEST_PATH)?;
    let Some((manifest, _)) = read_manifest(&store)? else {
        return Err(Error::NotAProject);
    };
    let mut paths = vec![manifest_path.clone()];
    paths.extend(manifest.revisions.iter().map(|entry| entry.path.clone()));
    let snapshot = store.read(&paths)?;
    let manifest_bytes = snapshot.files[&manifest_path]
        .clone()
        .ok_or(Error::NotAProject)?;
    let manifest = ProjectManifest::from_bytes(&manifest_bytes)?;
    refuse_newer(&manifest)?;
    if support(manifest.language(), LANGUAGE_VERSION) == Support::Same {
        return Ok(Planned::Current);
    }
    for entry in &manifest.revisions {
        let state = snapshot.state(&entry.path);
        if !matches!(state, Some(FileState::File { sha256, .. }) if sha256 == entry.source_hash) {
            return Err(ProjectError::InvalidMetadata.into());
        }
    }
    let context = crate::context::resolve(&manifest, &snapshot, None, &FixedPlacer)?;
    let lower_context = LowerContext::folder(&context);
    let mut read = ReadSet::new();
    read.insert(manifest_path.clone(), FileState::of(&manifest_bytes));
    let mut changes = vec![];
    let mut sources = vec![];
    for entry in &manifest.documents {
        // Each source on its own: one that cannot be read is named.
        let bytes = match store.read(std::slice::from_ref(&entry.path)) {
            Ok(mut snapshot) => snapshot.files.remove(&entry.path).flatten(),
            Err(ProjectError::Limit) => {
                sources.push(SourceReport {
                    path: entry.path.as_str().to_owned(),
                    outcome: SourceOutcome::Unreadable(whole_file(TOO_LARGE)),
                });
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        read.insert(
            entry.path.clone(),
            bytes.as_deref().map_or(FileState::Missing, FileState::of),
        );
        let outcome = match bytes.as_deref().map(std::str::from_utf8) {
            None => SourceOutcome::Unreadable(whole_file(MISSING)),
            Some(Err(_)) => SourceOutcome::Unreadable(whole_file(NOT_TEXT)),
            Some(Ok(text)) => {
                let (outcome, converted) = judge(text, &lower_context);
                if let Some(converted) = converted {
                    changes.push(ProjectChange::Write {
                        path: entry.path.clone(),
                        expected: FileState::of(text.as_bytes()),
                        bytes: converted.into_bytes(),
                    });
                }
                outcome
            }
        };
        sources.push(SourceReport {
            path: entry.path.as_str().to_owned(),
            outcome,
        });
    }
    let mut raised = manifest.clone();
    (raised.format_version, raised.language_version) = (2, Some(LANGUAGE_VERSION));
    changes.push(ProjectChange::Write {
        path: manifest_path,
        expected: FileState::of(&manifest_bytes),
        bytes: raised.to_public_bytes()?,
    });
    Ok(Planned::Plan(FixPlan {
        store,
        report: FixReport { sources },
        changes,
        read,
    }))
}

/// The manifest of `store` and its bytes, or `None` without one.
fn read_manifest(store: &ProjectStore) -> Result<Option<(ProjectManifest, Vec<u8>)>> {
    let path = ProjectPath::new(MANIFEST_PATH)?;
    let Some(bytes) = store
        .read(std::slice::from_ref(&path))?
        .files
        .remove(&path)
        .flatten()
    else {
        return Ok(None);
    };
    Ok(Some((ProjectManifest::from_bytes(&bytes)?, bytes)))
}

const MISSING: &str = "The file is missing.";
const NOT_TEXT: &str = "The file is not UTF-8 text.";
const TOO_LARGE: &str = "The file is over the 1 MiB limit of a source.";
const REPAIR: &str = "Repair it, or restore it from Git.";

fn whole_file(reason: &str) -> Problem {
    Problem {
        line: 0,
        column: 0,
        statement: String::new(),
        reason: reason.to_owned(),
        todo: REPAIR.to_owned(),
    }
}

/// A message as a sentence of the report: one final full stop.
fn sentence(message: &str) -> String {
    format!("{}.", message.trim_end_matches('.'))
}

/// The judgement of one source, and its converted text when it changes.
fn judge(text: &str, context: &LowerContext<'_>) -> (SourceOutcome, Option<String>) {
    let position = |offset: usize| apiforge_script::line_column(text, offset);
    match apiforge_script::judge_language_1(text, context) {
        Judged::Convertible {
            text: converted, ..
        } if converted == text => (SourceOutcome::Unchanged, None),
        Judged::Convertible {
            text: converted,
            edits,
        } => {
            let changes = edits
                .iter()
                .map(|edit| Change {
                    line: position(edit.range.start).0,
                    removed: text[edit.range.clone()].trim().to_owned(),
                    written: edit.replacement.trim().to_owned(),
                })
                .collect();
            (SourceOutcome::Converted(changes), Some(converted))
        }
        Judged::Unparsable(error) => {
            let (line, column) = position(error.range.start);
            (
                SourceOutcome::Unreadable(Problem {
                    line,
                    column,
                    statement: String::new(),
                    reason: format!("The file does not parse: {}", sentence(&error.message)),
                    todo: REPAIR.to_owned(),
                }),
                None,
            )
        }
        Judged::Blocked(blockers) => {
            let problems = blockers
                .into_iter()
                .map(|blocker| {
                    let (line, column) = position(blocker.range.start);
                    let statement = if blocker.handle.is_empty() {
                        String::new()
                    } else {
                        text[blocker.range.clone()]
                            .lines()
                            .next()
                            .unwrap_or_default()
                            .trim()
                            .to_owned()
                    };
                    let (reason, todo) = match blocker.reason {
                        Blocked::RunsNowhere { above } => (
                            "This step runs nowhere today: nothing orders it after `start`."
                                .to_owned(),
                            format!(
                                "Delete it, or write `@after({})` on it; apiforge fix then removes that line.",
                                above.as_deref().unwrap_or("<the step it follows>")
                            ),
                        ),
                        Blocked::NotInPlace { after } => (
                            if after.is_empty() {
                                "This statement is not written where it runs today: it runs first."
                                    .to_owned()
                            } else {
                                format!(
                                    "This statement is not written where it runs today: it runs after {}.",
                                    after
                                        .iter()
                                        .map(|handle| format!("`{handle}`"))
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                )
                            },
                            "Move its lines there.".to_owned(),
                        ),
                        Blocked::Invalid(error) => {
                            (sentence(&error.message), "Repair it.".to_owned())
                        }
                    };
                    Problem {
                        line,
                        column,
                        statement,
                        reason,
                        todo,
                    }
                })
                .collect();
            (SourceOutcome::Blocked(problems), None)
        }
    }
}

/// The store's refusal of the conversion's commit, as the command says it:
/// its limits are the project's size; a file that changed since it was read
/// is named by its path in the project.
pub fn commit_refusal(error: ProjectError, root: &Path) -> Error {
    match error {
        ProjectError::Limit => Error::TooLargeToConvert,
        ProjectError::Conflict { path } => Error::ChangedWhileConverting {
            path: path.strip_prefix(root).map_or_else(
                |_| path.display().to_string(),
                |relative| {
                    relative
                        .components()
                        .map(|part| part.as_os_str().to_string_lossy())
                        .collect::<Vec<_>>()
                        .join("/")
                },
            ),
        },
        error => Error::Storage(error),
    }
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
