//! Project configuration stays dirty until an explicit manifest/source save.
//! The person's consent to the agent's live requests is not configuration:
//! it applies at once and is kept outside the project (MCP-09).
use super::*;

impl FolderApp {
    /// Whether the person allows an MCP agent's live requests in the open
    /// project (MCP-09). Off for a project never set.
    pub(super) fn live_allowed(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(|session| self.consent.live(&session.manifest().id))
    }

    /// The person ticked or unticked the box: kept at once, and said to the
    /// agent's server over the channel.
    fn set_live_allowed(&mut self, live: bool) {
        let Some(session) = &self.session else { return };
        let project = session.manifest().id.clone();
        self.consent.set(&project, live);
        self.say_consent();
    }

    /// The channel says the open project's consent to every server it is
    /// connected to, and to each that connects; only a change is sent. A
    /// change another window made is followed within a second.
    pub(super) fn say_consent(&mut self) {
        self.consent.follow();
        self.handover.channel.set_consent(self.live_allowed());
    }

    /// The *Agent* section of the open project's settings as it stands.
    fn agent_section(&self) -> dialogs::AgentSection {
        dialogs::AgentSection {
            live: self.live_allowed(),
            running: self.handover.agent_runs_live(),
            base_url: self
                .session
                .as_ref()
                .map(|session| session.manifest().environment.base_url.clone())
                .unwrap_or_default(),
        }
    }

    pub(super) fn open_project_settings(&mut self) {
        let Some(session) = &self.session else { return };
        let dialog =
            dialogs::SettingsDialog::project(session.context()).with_agent(self.agent_section());
        // Observing, the settings are shown with every control disabled but
        // the agent's toggle (MCP-08a, MCP-09); a read-only window that is
        // not observing does not open them.
        let dialog = match (session.is_read_only(), self.observing()) {
            (false, _) => dialog,
            (true, true) => dialog.read_only(),
            (true, false) => return,
        };
        self.prompt = Some(Prompt::Settings(Box::new(dialog)));
    }

    pub(super) fn open_environment(&mut self) {
        let Some(session) = &self.session else { return };
        let dialog =
            dialogs::EnvironmentDialog::folder(&session.manifest().environment, &self.runs.secrets);
        // Observing, only the session's secret values can change (MCP-08a).
        let dialog = if self.observing() {
            dialog.public_read_only()
        } else {
            dialog
        };
        self.prompt = Some(Prompt::Environment(Box::new(dialog)));
    }

    pub(super) fn project_settings_prompt(
        &mut self,
        context: &egui::Context,
        mut dialog: dialogs::SettingsDialog,
    ) -> Option<Prompt> {
        // The window may have begun observing since the dialog opened.
        let read_only = self
            .session
            .as_ref()
            .is_some_and(ProjectSession::is_read_only);
        dialog.set_read_only(read_only);
        dialog.set_agent(self.agent_section());
        let outcome = dialog.show(context);
        // The toggle applies at once, whatever else the dialog does.
        if let Some(live) = dialog.take_agent_change() {
            self.set_live_allowed(live);
        }
        match outcome {
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
        let observing = self.observing();
        dialog.set_public_read_only(observing);
        match dialog.show(context) {
            dialogs::Outcome::Cancelled => return None,
            dialogs::Outcome::Applied((environment, secrets)) => {
                let session = self.session.as_ref()?;
                // Observing, only the secret values apply: the public part
                // is the holder's, as last followed, not the dialog's copy.
                let environment = if observing {
                    session.manifest().environment.clone()
                } else {
                    environment
                };
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
