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
                match session.edit_configuration(
                    |manifest| {
                        manifest.name = change.project_name;
                        manifest.contracts = change.contracts;
                    },
                    DIAGRAM,
                ) {
                    Ok(()) => {
                        self.remember_project();
                        self.notify(
                            "Project settings have been applied. Use Save all to save them.",
                            false,
                        );
                        self.touched();
                        return None;
                    }
                    Err(error) => dialog.set_error(format!("Cannot apply these settings: {error}")),
                }
            }
            dialogs::Outcome::Open => {}
        }
        Some(Prompt::Settings(Box::new(dialog)))
    }

    pub(super) fn apply_environment(
        &mut self,
        environment: apiforge_core::format::Environment,
        secrets: dialogs::SessionSecrets,
    ) -> Result<(), String> {
        let session = self.session.as_mut().ok_or("No project is open")?;
        let public_changed = session.manifest().environment != environment;
        if public_changed {
            if session.is_read_only() {
                return Err("This project's public settings are read-only. Session secret values can still be entered.".into());
            }
            session
                .edit_configuration(|manifest| manifest.environment = environment, DIAGRAM)
                .map_err(|error| error.to_string())?;
        } else if self.runs.secrets != secrets {
            session.invalidate_runtime_inputs(DIAGRAM);
        }
        session.store().set_known_secrets(secrets.values().cloned());
        self.runs.secrets = secrets;
        self.touched();
        Ok(())
    }

    pub(super) fn environment_prompt(
        &mut self,
        context: &egui::Context,
        mut dialog: dialogs::EnvironmentDialog,
    ) -> Option<Prompt> {
        match dialog.show(context) {
            dialogs::Outcome::Cancelled => return None,
            dialogs::Outcome::Applied((environment, secrets)) => {
                match self.apply_environment(environment, secrets) {
                    Ok(()) => {
                        self.notify("Environment applied. Public changes remain unsaved; secret values stay in this session.", false);
                        return None;
                    }
                    Err(error) => dialog.set_error(error),
                }
            }
            dialogs::Outcome::Open => {}
        }
        Some(Prompt::Environment(Box::new(dialog)))
    }
}
