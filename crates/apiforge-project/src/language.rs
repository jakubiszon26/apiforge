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
