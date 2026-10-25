//! The test environment: base URL, plain variables and session secrets.
//!
//! The project stores secret names only. Their values live in the application's
//! memory for the session and are returned separately, so they never reach the
//! document, its saved copy or an export.

use super::{Chrome, NameError, Outcome, autofocus, name_errors, new_name_error, submitted};
use crate::icons::Icon;
use crate::{theme, widgets};
use apiforge_core::{format::Environment, js};
use eframe::egui::{self, Ui};
use indexmap::IndexMap;

/// Secret values for the current session, by secret name.
pub type SessionSecrets = IndexMap<String, String>;

/// Names an environment entry cannot take: `baseUrl` is the address itself.
const RESERVED: [&str; 1] = ["baseUrl"];

const NAME_WIDTH: f32 = 170.0;

#[derive(Clone, Debug, PartialEq)]
struct Variable {
    name: String,
    value: String,
}

#[derive(Clone, Debug, PartialEq)]
struct Secret {
    name: String,
    value: String,
    revealed: bool,
}

/// Problems that keep the environment from being saved.
#[derive(Debug, Default, PartialEq)]
struct Problems {
    base_url: Option<&'static str>,
    variables: Vec<Option<NameError>>,
    secrets: Vec<Option<NameError>>,
}

impl Problems {
    fn is_empty(&self) -> bool {
        self.base_url.is_none()
            && self.variables.iter().all(Option::is_none)
            && self.secrets.iter().all(Option::is_none)
    }
}

/// Checks a base URL: empty, or an absolute `http`/`https` address with a host.
pub fn base_url_error(url: &str) -> Option<&'static str> {
    let url = js::trim(url);
    if url.is_empty() {
        return None;
    }
    let Some((scheme, rest)) = url.split_once("://") else {
        return Some("Enter a full address, e.g. http://localhost:3000.");
    };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return Some("The base URL must use HTTP or HTTPS.");
    }
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = host.rsplit_once('@').map_or(host, |(_, host)| host);
    if host.is_empty() || host.starts_with(':') {
        return Some("Add a host after the scheme, e.g. http://localhost:3000.");
    }
    if url.chars().any(char::is_whitespace) {
        return Some("The base URL cannot contain spaces.");
    }
    None
}

/// Edits the environment of a project and the session values of its secrets.
pub struct EnvironmentDialog {
    base_url: String,
    variables: Vec<Variable>,
    secrets: Vec<Secret>,
    new_variable: String,
    new_secret: String,
    focused: bool,
    folder: bool,
    error: Option<String>,
}

impl EnvironmentDialog {
    /// Opens the dialog on a copy of `environment`; `secrets` are the values entered
    /// earlier in this session.
    pub fn new(environment: &Environment, secrets: &SessionSecrets) -> Self {
        Self {
            base_url: environment.base_url.clone(),
            variables: environment
                .variables
                .iter()
                .map(|(name, value)| Variable {
                    name: name.clone(),
                    value: value.clone(),
                })
                .collect(),
            secrets: environment
                .secret_names
                .iter()
                .map(|name| Secret {
                    name: name.clone(),
                    value: secrets.get(name).cloned().unwrap_or_default(),
                    revealed: false,
                })
                .collect(),
            new_variable: String::new(),
            new_secret: String::new(),
            focused: false,
            folder: false,
            error: None,
        }
    }

    pub fn folder(environment: &Environment, secrets: &SessionSecrets) -> Self {
        let mut dialog = Self::new(environment, secrets);
        dialog.folder = true;
        dialog
    }

    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }

    fn names(&self) -> Vec<&str> {
        self.variables
            .iter()
            .map(|variable| variable.name.as_str())
            .chain(self.secrets.iter().map(|secret| secret.name.as_str()))
            .collect()
    }

    /// Variables and secrets share one namespace, since blocks refer to both by name.
    fn problems(&self) -> Problems {
        let mut names = name_errors(&self.names(), &RESERVED);
        let secrets = names.split_off(self.variables.len());
        Problems {
            base_url: base_url_error(&self.base_url),
            variables: names,
            secrets,
        }
    }

    fn new_name_error(&self, name: &str) -> Option<NameError> {
        new_name_error(name, &self.names(), &RESERVED)
    }

    /// The edited environment and the session values of its secrets, if valid.
    fn result(&self) -> Option<(Environment, SessionSecrets)> {
        if !self.problems().is_empty() {
            return None;
        }
        let environment = Environment {
            base_url: js::trim(&self.base_url).to_owned(),
            variables: self
                .variables
                .iter()
                .map(|variable| (js::trim(&variable.name).to_owned(), variable.value.clone()))
                .collect(),
            secret_names: self
                .secrets
                .iter()
                .map(|secret| js::trim(&secret.name).to_owned())
                .collect(),
        };
        let secrets = self
            .secrets
            .iter()
            .map(|secret| (js::trim(&secret.name).to_owned(), secret.value.clone()))
            .collect();
        Some((environment, secrets))
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome<(Environment, SessionSecrets)> {
        let chrome = Chrome {
            id: "environment-dialog",
            eyebrow: "PROJECT SETTINGS",
            title: "Test environment",
            width: 580.0,
            apply_label: if self.folder {
                "Apply environment"
            } else {
                "Save environment"
            },
            note: self.folder.then_some(
                "Public settings are saved with Save all. Secret values stay in this session.",
            ),
        };
        let mut submit = false;
        let action = chrome.show(
            ctx,
            self,
            |ui, dialog| submit = dialog.body(ui),
            |dialog| dialog.problems().is_empty(),
        );
        match action {
            super::Action::Cancel => Outcome::Cancelled,
            super::Action::Apply => self.result().map_or(Outcome::Open, Outcome::Applied),
            super::Action::None if submit => self.result().map_or(Outcome::Open, Outcome::Applied),
            super::Action::None => Outcome::Open,
        }
    }

    /// Draws the form; returns whether Enter was pressed in the base URL field.
    fn body(&mut self, ui: &mut Ui) -> bool {
        let problems = self.problems();
        if let Some(error) = &self.error {
            widgets::field_error(ui, error);
        }
        let field = super::labelled_field(
            ui,
            "Base URL",
            &mut self.base_url,
            "http://localhost:3000",
            problems.base_url,
        );
        autofocus(&field, &mut self.focused);
        let submit = submitted(ui, &field);
        widgets::hint(
            ui,
            "The shared start of the address for requests with a relative path. \
             Requests with a full URL keep their own address.",
        );

        widgets::section(ui, "Variables");
        widgets::hint(
            ui,
            "Plain values that blocks can read by name, e.g. an API version or a tenant.",
        );
        ui.add_space(2.0);
        self.variable_rows(ui, &problems.variables);
        self.add_row(ui, Entry::Variable);

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            widgets::section_inline(ui, "Session secrets");
            ui.add_space(4.0);
            widgets::pill(ui, "IN MEMORY ONLY", theme::WARNING)
                .on_hover_text("Values are never saved to disk or exported");
        });
        ui.add_space(2.0);
        widgets::hint(
            ui,
            "The project keeps only the names. Values last until you close the app \
             and are never saved or exported.",
        );
        ui.add_space(2.0);
        self.secret_rows(ui, &problems.secrets);
        self.add_row(ui, Entry::Secret);
        ui.add_space(4.0);
        widgets::hint(
            ui,
            "To use a secret, set a block argument's source to Environment and pick its name.",
        );
        submit
    }

    fn variable_rows(&mut self, ui: &mut Ui, errors: &[Option<NameError>]) {
        if self.variables.is_empty() {
            widgets::hint(ui, "No variables yet.");
        }
        let mut remove = None;
        for (index, variable) in self.variables.iter_mut().enumerate() {
            let error = errors.get(index).copied().flatten();
            ui.push_id(("variable", index), |ui| {
                ui.horizontal(|ui| {
                    let field = super::text_field(
                        ui,
                        &mut variable.name,
                        "name",
                        NAME_WIDTH,
                        error.is_some(),
                    );
                    widgets::name(ui, field.id, "Variable name");
                    field.on_hover_text("Variable name");
                    let width = ui.available_width() - 32.0;
                    let field = super::text_field(ui, &mut variable.value, "value", width, false);
                    widgets::name(ui, field.id, &format!("Value of {}", variable.name));
                    field.on_hover_text("Value");
                    if ui
                        .add(
                            widgets::icon_button(Icon::Trash, "Remove variable")
                                .compact()
                                .danger(),
                        )
                        .clicked()
                    {
                        remove = Some(index);
                    }
                });
                if let Some(error) = error {
                    widgets::field_error(ui, error.to_string());
                }
            });
        }
        if let Some(index) = remove {
            self.variables.remove(index);
        }
    }

    fn secret_rows(&mut self, ui: &mut Ui, errors: &[Option<NameError>]) {
        if self.secrets.is_empty() {
            widgets::hint(ui, "No secrets yet.");
        }
        let mut remove = None;
        for (index, secret) in self.secrets.iter_mut().enumerate() {
            let error = errors.get(index).copied().flatten();
            ui.push_id(("secret", index), |ui| {
                ui.horizontal(|ui| {
                    let field = super::text_field(
                        ui,
                        &mut secret.name,
                        "name",
                        NAME_WIDTH,
                        error.is_some(),
                    );
                    widgets::name(ui, field.id, "Secret name");
                    field.on_hover_text("Secret name");
                    let width = ui.available_width() - 90.0;
                    let field = ui.add(
                        crate::widgets::text_edit(&mut secret.value)
                            .password(!secret.revealed)
                            .hint_text("Value for this session")
                            .desired_width(width),
                    );
                    widgets::name(ui, field.id, &format!("Value of {}", secret.name));
                    let (icon, toggle) = if secret.revealed {
                        (Icon::EyeOff, "Hide secret")
                    } else {
                        (Icon::Eye, "Show secret")
                    };
                    if ui.add(widgets::icon_button(icon, toggle)).clicked() {
                        secret.revealed = !secret.revealed;
                    }
                    if ui
                        .add(
                            widgets::icon_button(Icon::Trash, "Remove secret")
                                .compact()
                                .danger(),
                        )
                        .clicked()
                    {
                        remove = Some(index);
                    }
                });
                if let Some(error) = error {
                    widgets::field_error(ui, error.to_string());
                }
            });
        }
        if let Some(index) = remove {
            self.secrets.remove(index);
        }
    }

    /// The field and button that add a variable or a secret; Enter adds as well.
    fn add_row(&mut self, ui: &mut Ui, entry: Entry) {
        let (hint, label, field_name) = match entry {
            Entry::Variable => (
                "New variable name, e.g. apiVersion",
                "Add variable",
                "New variable name",
            ),
            Entry::Secret => (
                "New secret name, e.g. apiToken",
                "Add secret",
                "New secret name",
            ),
        };
        let mut name = match entry {
            Entry::Variable => std::mem::take(&mut self.new_variable),
            Entry::Secret => std::mem::take(&mut self.new_secret),
        };
        let error = self.new_name_error(&name);
        let typed = !js::trim(&name).is_empty();
        let mut add = false;
        ui.push_id(("add", label), |ui| {
            ui.horizontal(|ui| {
                let width = (ui.available_width() - 150.0).max(80.0);
                let field = super::text_field(ui, &mut name, hint, width, typed && error.is_some());
                widgets::name(ui, field.id, field_name);
                let clicked = ui
                    .add_enabled(error.is_none(), widgets::action(Icon::Plus, label))
                    .clicked();
                add = error.is_none() && (clicked || submitted(ui, &field));
                if add {
                    field.request_focus();
                }
            });
            if typed && let Some(error) = error {
                widgets::field_error(ui, error.to_string());
            }
        });
        if add {
            let name = js::trim(&name).to_owned();
            match entry {
                Entry::Variable => self.variables.push(Variable {
                    name,
                    value: String::new(),
                }),
                Entry::Secret => self.secrets.push(Secret {
                    name,
                    value: String::new(),
                    revealed: false,
                }),
            }
        } else {
            match entry {
                Entry::Variable => self.new_variable = name,
                Entry::Secret => self.new_secret = name,
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Entry {
    Variable,
    Secret,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::test_support::{press, run_frames};
    use apiforge_core::demos;

    fn environment() -> Environment {
        Environment {
            base_url: "http://localhost:3000".into(),
            variables: IndexMap::from([("tenant".into(), "acme".into())]),
            secret_names: vec!["apiToken".into(), "unused".into()],
        }
    }

    #[test]
    fn base_urls_must_be_absolute_http_addresses() {
        for valid in [
            "",
            "  ",
            "http://localhost:3000",
            "HTTPS://api.example.com/v1",
            "https://user@host",
        ] {
            assert_eq!(base_url_error(valid), None, "{valid}");
        }
        for invalid in [
            "localhost:3000",
            "ftp://host",
            "http://",
            "http:///path",
            "http://:80",
            "http://a b",
        ] {
            assert!(base_url_error(invalid).is_some(), "{invalid}");
        }
    }

    #[test]
    fn secrets_are_returned_separately_and_names_are_trimmed() {
        let secrets = SessionSecrets::from([("apiToken".into(), "s3cret".into())]);
        let mut dialog = EnvironmentDialog::new(&environment(), &secrets);
        dialog.variables[0].name = " tenant ".into();
        dialog.base_url = " http://localhost:4000 ".into();
        let (saved, values) = dialog.result().expect("valid");
        assert_eq!(saved.base_url, "http://localhost:4000");
        assert_eq!(saved.variables["tenant"], "acme");
        assert_eq!(saved.secret_names, ["apiToken", "unused"]);
        assert_eq!(values["apiToken"], "s3cret");
        assert_eq!(values["unused"], "");
    }

    #[test]
    fn variables_and_secrets_share_one_namespace() {
        let mut dialog = EnvironmentDialog::new(&environment(), &SessionSecrets::new());
        dialog.secrets[1].name = "tenant".into();
        let problems = dialog.problems();
        assert_eq!(problems.variables, [Some(NameError::Duplicate)]);
        assert_eq!(problems.secrets, [None, Some(NameError::Duplicate)]);
        assert!(dialog.result().is_none());
        assert_eq!(dialog.new_name_error("baseUrl"), Some(NameError::Reserved));
        assert_eq!(
            dialog.new_name_error("apiToken"),
            Some(NameError::Duplicate)
        );
        dialog.secrets[1].name = "constructor".into();
        assert_eq!(dialog.problems().secrets[1], Some(NameError::Reserved));
        dialog.secrets.remove(1);
        dialog.base_url = "ftp://x".into();
        assert!(dialog.result().is_none());
    }

    #[test]
    fn the_dialog_renders_and_closes_on_escape() {
        for project in demos::demo_projects() {
            let mut dialog = EnvironmentDialog::new(&project.environment, &SessionSecrets::new());
            run_frames(3, |ctx| {
                assert_eq!(dialog.show(ctx), Outcome::Open);
            });
        }
        let ctx = egui::Context::default();
        let mut dialog = EnvironmentDialog::new(&environment(), &SessionSecrets::new());
        let mut outcome = Outcome::Open;
        press(&ctx, egui::Key::Escape, egui::Modifiers::NONE, |ctx| {
            outcome = dialog.show(ctx);
        });
        assert_eq!(outcome, Outcome::Cancelled);
    }

    #[test]
    fn ctrl_enter_applies_a_valid_environment() {
        let ctx = egui::Context::default();
        let mut dialog = EnvironmentDialog::new(&environment(), &SessionSecrets::new());
        let mut outcome = Outcome::Open;
        press(&ctx, egui::Key::Enter, egui::Modifiers::COMMAND, |ctx| {
            outcome = dialog.show(ctx);
        });
        assert!(matches!(outcome, Outcome::Applied((saved, _)) if saved == environment()));
    }
}
