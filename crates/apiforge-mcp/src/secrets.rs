//! The server's known secret values (MCP-00, "Secrets and the project
//! boundary"): `APIFORGE_SECRET_<NAME>` from the environment and the
//! `--secrets` file, read again at each call. No value leaves the server: a
//! run's report is redacted by the runner, a text that contains one is
//! refused (`secret_in_text`) and a saved source that contains one is
//! withheld (`secret_in_source`).
use crate::{
    error::{ErrorCode, ToolError},
    executor,
};
use indexmap::IndexMap;
use std::{ffi::OsString, path::PathBuf, sync::Arc};

/// Where the server reads its secrets from.
#[derive(Clone, Default)]
pub struct Secrets {
    /// The `--secrets` file, an object of names and text values.
    file: Option<PathBuf>,
    /// The environment `APIFORGE_SECRET_<NAME>` is read from; the process
    /// environment when `None`. A test seam.
    environment: Option<Arc<Vec<(OsString, OsString)>>>,
}

/// Names the source, never a value.
impl std::fmt::Debug for Secrets {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Secrets")
            .field("file", &self.file)
            .field("environment", &self.environment.as_ref().map(|_| "given"))
            .finish()
    }
}

impl Secrets {
    /// The process environment and, when given, the `--secrets` file.
    pub fn new(file: Option<PathBuf>) -> Self {
        Self {
            file,
            environment: None,
        }
    }

    /// `environment` instead of the process environment: a test seam.
    pub fn with_environment(file: Option<PathBuf>, environment: Vec<(OsString, OsString)>) -> Self {
        Self {
            file,
            environment: Some(Arc::new(environment)),
        }
    }

    /// The secrets by name, read now. A name in both takes its value from
    /// the environment. The error never quotes the file's content.
    pub fn read(&self) -> Result<IndexMap<String, String>, ToolError> {
        match &self.environment {
            Some(environment) => {
                executor::secrets_from(self.file.as_deref(), environment.iter().cloned())
            }
            None => executor::secrets(self.file.as_deref()),
        }
        .map_err(|message| ToolError::new(ErrorCode::ProjectUnreadable, message))
    }

    /// The known values, read now; empty values are not secrets.
    pub fn values(&self) -> Result<Vec<String>, ToolError> {
        Ok(self
            .read()?
            .into_values()
            .filter(|value| !value.is_empty())
            .collect())
    }
}

/// The 1-based line of the first known value in `text`, if one is there.
pub fn secret_line(_text: &str, _values: &[String]) -> Option<usize> {
    unimplemented!("MCP-06")
}

/// Refuses a saved source that contains a known value: `secret_in_source`
/// with the line, never a modified text.
pub fn withhold(_text: &str, _values: &[String]) -> Result<(), ToolError> {
    unimplemented!("MCP-06")
}
