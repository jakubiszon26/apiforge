//! Project configuration stays dirty until an explicit manifest/source save.
use super::*;

impl FolderApp {
    pub(super) fn open_project_settings(&mut self) {
        let Some(session) = &self.session else { return };
        if session.is_read_only() {
            return;
        }
        self.prompt = Some(Prompt::Settings(Box::new(
            dialogs::SettingsDialog::project(session.context()),
        )));
    }

    pub(super) fn open_environment(&mut self) {
        let Some(session) = &self.session else { return };
        self.prompt = Some(Prompt::Environment(Box::new(
            dialogs::EnvironmentDialog::folder(&session.manifest().environment, &self.runs.secrets),
        )));
    }

    pub(super) fn project_settings_prompt(
        &mut self,
        context: &egui::Context,
        mut dialog: dialogs::SettingsDialog,
    ) -> Option<Prompt> {
        match dialog.show(context) {
            dialogs::Outcome::Cancelled => return None,
            dialogs::Outcome::Applied(change) => {
                let session = self.session.as_mut()?;
                if session.is_read_only() {
                    return None;
                }
                self.working_project(
                    "Applying project settings…",
                    move |session| {
                        session.edit_configuration(
                            |manifest| {
                                manifest.name = change.project_name;
                                manifest.contracts = change.contracts;
                            },
                            DIAGRAM,
                        )
                    },
                    move |app, result| match result {
                        Ok(()) => {
                            app.remember_project();
                            app.notify(
                                "Project settings have been applied. Use Save all to save them.",
                                false,
                            );
                            app.touched();
                        }
                        Err(error) => {
                            dialog.set_error(format!("Cannot apply these settings: {error}"));
                            app.prompt = Some(Prompt::Settings(Box::new(dialog)));
                        }
                    },
                );
                return self.prompt.take();
            }
            dialogs::Outcome::Open => {}
        }
        Some(Prompt::Settings(Box::new(dialog)))
    }

    /// Public changes are validated on the worker; values-only changes remain
    /// session-local and invalidate running/prepared inputs immediately.
    pub(super) fn apply_environment(
        &mut self,
        environment: apiforge_core::format::Environment,
        secrets: dialogs::SessionSecrets,
    ) -> Result<(), String> {
        let session = self.session.as_ref().ok_or("No project is open")?;
        if session.manifest().environment != environment {
            if session.is_read_only() {
                return Err("This project's public settings are read-only. Session secret values can still be entered.".into());
            }
            self.queue_environment(environment, secrets, None);
        } else {
            if self.runs.secrets != secrets {
                self.session
                    .as_mut()
                    .expect("checked above")
                    .invalidate_runtime_inputs(DIAGRAM);
            }
            self.install_session_secrets(secrets);
        }
        Ok(())
    }

    fn install_session_secrets(&mut self, secrets: dialogs::SessionSecrets) {
        if let Some(session) = &self.session {
            session.store().set_known_secrets(secrets.values().cloned());
        }
        self.runs.secrets = secrets;
        self.touched();
    }

    fn queue_environment(
        &mut self,
        environment: apiforge_core::format::Environment,
        secrets: dialogs::SessionSecrets,
        failed: Option<dialogs::EnvironmentDialog>,
    ) {
        self.working_project("Applying environment…", move |session| {
            session.edit_configuration(|manifest| manifest.environment = environment, DIAGRAM)
        }, move |app, result| match result {
            Ok(()) => {
                app.install_session_secrets(secrets);
                app.notify("Environment applied. Public changes remain unsaved; secret values stay in this session.", false);
            }
            Err(error) => {
                if let Some(mut dialog) = failed {
                    dialog.set_error(error.to_string());
                    app.prompt = Some(Prompt::Environment(Box::new(dialog)));
                } else { app.notify(format!("Cannot apply the environment: {error}"), true); }
            }
        });
    }

    pub(super) fn environment_prompt(
        &mut self,
        context: &egui::Context,
        mut dialog: dialogs::EnvironmentDialog,
    ) -> Option<Prompt> {
        match dialog.show(context) {
            dialogs::Outcome::Cancelled => return None,
            dialogs::Outcome::Applied((environment, secrets)) => {
                let session = self.session.as_ref()?;
                if session.manifest().environment != environment && !session.is_read_only() {
                    self.queue_environment(environment, secrets, Some(dialog));
                    return self.prompt.take();
                }
                match self.apply_environment(environment, secrets) {
                    Ok(()) => {
                        self.notify("Environment applied. Public changes remain unsaved; secret values stay in this session.", false);
                        return self.prompt.take();
                    }
                    Err(error) => dialog.set_error(error),
                }
            }
            dialogs::Outcome::Open => {}
        }
        Some(Prompt::Environment(Box::new(dialog)))
    }
}
