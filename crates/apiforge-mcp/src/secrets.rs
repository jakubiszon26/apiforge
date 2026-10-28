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

/// The 1-based line of the first known value in `text`, if one is there. A
/// value is found as written and in its JSON-escaped form, as the storage's
/// staging check finds it.
pub fn secret_line(text: &str, values: &[String]) -> Option<usize> {
    values
        .iter()
        .filter(|value| !value.is_empty())
        .flat_map(|value| {
            let escaped = serde_json::to_string(value)
                .ok()
                .map(|quoted| quoted[1..quoted.len() - 1].to_owned());
            [Some(value.clone()), escaped]
        })
        .flatten()
        .filter_map(|form| text.find(&form))
        .min()
        .map(|offset| 1 + text[..offset].matches('\n').count())
}

/// Refuses a saved source that contains a known value: `secret_in_source`
/// with the line, never a modified text.
pub fn withhold(text: &str, values: &[String]) -> Result<(), ToolError> {
    match secret_line(text, values) {
        None => Ok(()),
        Some(line) => Err(ToolError::new(
            ErrorCode::SecretInSource,
            format!(
                "The saved source contains the value of a known secret on line {line}; it is withheld and the value is not repeated. Replace the value with the secret's name, env.name"
            ),
        )
        .with_details(serde_json::json!({ "line": line }))),
    }
}

/// A text with every known value replaced by `[REDACTED]`, for what the
/// server reports from a file it did not write, such as the run journal: the
/// runner's redactor, so a value is masked raw and in the encoded forms a URL
/// gives it, the longest first.
pub fn redact(text: &str, values: &[String]) -> String {
    let mut redactor = apiforge_runner::redact::Redactor::new();
    for value in values {
        redactor.add(value);
    }
    redactor.redact_str(text)
}

/// Refuses a text argument that carries a known value: `secret_in_text`,
/// the value never repeated.
pub fn refuse_in_text(text: &str, values: &[String]) -> Result<(), ToolError> {
    match secret_line(text, values) {
        None => Ok(()),
        Some(_) => Err(ToolError::new(
            ErrorCode::SecretInText,
            "The text contains the value of a known secret; nothing was done and the value is not repeated. Name the secret instead: env.name",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_is_found_on_its_line_raw_or_escaped() {
        let values = vec!["s\"q".to_owned(), String::new()];
        assert_eq!(secret_line("a\nb s\"q\n", &values), Some(2));
        assert_eq!(secret_line("a\nb\nc s\\\"q", &values), Some(3));
        assert_eq!(secret_line("nothing here", &values), None);
        assert_eq!(secret_line("", &[String::new()]), None);
    }

    #[test]
    fn redaction_masks_encoded_forms() {
        let values = vec!["a b&c".to_owned()];
        assert_eq!(
            redact("?q=a%20b%26c&r=a+b%26c", &values),
            "?q=[REDACTED]&r=[REDACTED]"
        );
    }

    #[test]
    fn redaction_masks_the_longest_value_first() {
        let values = vec!["abc".to_owned(), "abcdef".to_owned()];
        assert_eq!(
            redact("x abcdef y abc", &values),
            "x [REDACTED] y [REDACTED]"
        );
    }

    #[test]
    fn the_debug_form_names_no_value() {
        let secrets = Secrets::with_environment(
            None,
            vec![("APIFORGE_SECRET_A".into(), "visible-value".into())],
        );
        assert!(!format!("{secrets:?}").contains("visible-value"));
    }
}
