//! Tries (REQ-17a): requests with no file, sent and changed without making a
//! test of them. A try is held beside the project session and never in it, so
//! that nothing that saves, reloads, hands over or shares the project meets
//! one; it is edited, checked and undone by what edits a scenario
//! ([`EditedText`]).
use crate::{
    Error, Result,
    analysis::{self, Target},
    document::{AnalysisState, EditedText, ModelState},
    language,
    layout::Placer,
    session::{Reading, Written},
};
use apiforge_core::format::Project;
use apiforge_script::{Diagnostic, Document, Severity};
use apiforge_storage::project::DocumentKind;
use std::{
    ops::Deref,
    path::{Path, PathBuf},
    sync::Arc,
};

/// What a try is read in: the project as a scenario of it is read in, and the
/// language it is written in. [`crate::ProjectSession::try_context`] gives it.
#[derive(Clone, Debug)]
pub struct TryContext {
    project: Arc<Project>,
    language: u32,
    folder: PathBuf,
}

impl TryContext {
    pub(crate) fn new(project: Arc<Project>, language: u32, folder: &Path) -> Self {
        Self {
            project,
            language,
            folder: folder.to_owned(),
        }
    }

    fn is_older(&self) -> bool {
        language::is_older(self.language)
    }
}

/// One try: its name, its text and what the text is read as.
#[derive(Clone, Debug, PartialEq)]
pub struct Try {
    id: String,
    name: String,
    edited: EditedText,
}

impl Try {
    pub fn id(&self) -> &str {
        &self.id
    }
    /// What the try is called. Its text does not say it.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// The model is edited, and the history stepped, only while the text
    /// checks, as a document's is.
    pub fn accepts_model_edits(&self) -> bool {
        self.model_state() == ModelState::Current
    }
    pub fn can_undo(&self) -> bool {
        self.accepts_model_edits() && !self.edited.history.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        self.accepts_model_edits() && !self.edited.history.redo.is_empty()
    }

    /// A scenario with an identity of its own.
    fn target(&self, context: &TryContext) -> Target {
        Target {
            kind: DocumentKind::Scenario,
            entity_id: self.id.clone(),
            revision: 0,
            stored: false,
            older: context.is_older().then(|| context.folder.clone()),
        }
    }

    /// What its text is read as in `context`, keeping its identities.
    fn derive(
        &self,
        context: &TryContext,
        placer: &dyn Placer,
    ) -> (Vec<Diagnostic>, usize, Option<Document>) {
        let (diagnostics, omitted, model) = analysis::derive_counted(
            &self.target(context),
            self.text(),
            &context.project,
            self.model(),
            None,
            placer,
        );
        (diagnostics, omitted, model.map(|(model, _)| model))
    }

    fn analyse(&mut self, context: &TryContext, placer: &dyn Placer) {
        let (diagnostics, omitted, model) = self.derive(context, placer);
        self.edited.install_analysis(diagnostics, omitted, model);
    }

    fn step(&mut self, context: &TryContext, placer: &dyn Placer, undo: bool) -> Result<bool> {
        if !self.accepts_model_edits() {
            return Err(Error::ModelLocked);
        }
        let read = Reading {
            target: &self.target(context),
            context: &context.project,
            placer,
        };
        Ok(self.edited.step(&read, undo)?.is_some())
    }
}

impl Deref for Try {
    type Target = EditedText;
    fn deref(&self) -> &EditedText {
        &self.edited
    }
}

/// The tries of one window, in the order they were made. The set belongs to
/// its caller; only the text of a try leaves it, and the project a run needs.
#[derive(Clone)]
pub struct TrySet {
    tries: Vec<Try>,
    placer: Arc<dyn Placer>,
}

impl std::fmt::Debug for TrySet {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_list().entries(&self.tries).finish()
    }
}

/// Equal when they hold the same tries: a placer is how a set reads, not what
/// it holds.
impl PartialEq for TrySet {
    fn eq(&self, other: &Self) -> bool {
        self.tries == other.tries
    }
}

impl TrySet {
    /// An empty set that places blocks as the caller's session does.
    pub fn new(placer: Arc<dyn Placer>) -> Self {
        Self {
            tries: Vec::new(),
            placer,
        }
    }

    /// A new try, last in the set, with its text read in `context`.
    pub fn create(&mut self, name: &str, text: &str, context: &TryContext) -> Result<String> {
        if text.len() > apiforge_storage::project::MAX_SOURCE_BYTES {
            return Err(Error::SourceTooLarge);
        }
        let id = uuid::Uuid::new_v4().to_string();
        let mut held = Try {
            id: id.clone(),
            name: name.into(),
            edited: EditedText::new(text.into()),
        };
        held.analyse(context, self.placer.as_ref());
        self.tries.push(held);
        Ok(id)
    }

    pub fn get(&self, id: &str) -> Option<&Try> {
        self.tries.iter().find(|held| held.id == id)
    }

    pub fn list(&self) -> &[Try] {
        &self.tries
    }

    pub fn rename(&mut self, id: &str, name: &str) -> Result<()> {
        self.held(id)?.0.name = name.into();
        Ok(())
    }

    pub fn discard(&mut self, id: &str) -> Result<()> {
        self.held(id)?;
        self.tries.retain(|held| held.id != id);
        Ok(())
    }

    /// Replaces a try's text and reads it at once. Gives its source version.
    pub fn edit_source(&mut self, id: &str, text: &str, context: &TryContext) -> Result<u64> {
        let (held, placer) = self.held(id)?;
        if held.edited.take_source(text)? {
            held.analyse(context, placer);
        }
        Ok(held.source_version())
    }

    /// As [`crate::ProjectSession::apply_model_edit`], for a try.
    pub fn apply_model_edit(
        &mut self,
        id: &str,
        model: Document,
        context: &TryContext,
    ) -> Result<()> {
        let (held, placer) = self.held(id)?;
        if !held.accepts_model_edits() {
            return Err(Error::ModelLocked);
        }
        let read = Reading {
            target: &held.target(context),
            context: &context.project,
            placer,
        };
        held.edited.apply_edit(&read, model, Written::Rebuilt)?;
        Ok(())
    }

    pub fn undo(&mut self, id: &str, context: &TryContext) -> Result<bool> {
        let (held, placer) = self.held(id)?;
        held.step(context, placer, true)
    }

    pub fn redo(&mut self, id: &str, context: &TryContext) -> Result<bool> {
        let (held, placer) = self.held(id)?;
        held.step(context, placer, false)
    }

    /// As [`crate::ProjectSession::accept_comment_loss`], for a try.
    pub fn accept_comment_loss(&mut self, id: &str) -> Result<()> {
        self.held(id)?.0.edited.accept_comment_loss()
    }

    /// Reads every try again in a context that changed: a publication, a
    /// reload, a project taken back.
    pub fn refresh(&mut self, context: &TryContext) {
        for held in &mut self.tries {
            held.edited.analysis = AnalysisState::Pending;
            held.analyse(context, self.placer.as_ref());
        }
    }

    /// The project a run of the try needs: the context with the try as its
    /// one scenario. The try is checked in the context given, whatever it was
    /// last read in; a project in an older language is refused as
    /// [`crate::ProjectSession::prepare_run`] refuses it.
    pub fn run_project(&self, id: &str, context: &TryContext) -> Result<Project> {
        let held = self.get(id).ok_or(Error::UnknownTry)?;
        if context.is_older() {
            return Err(language::older_problem(context.language, &context.folder));
        }
        let (diagnostics, _, model) = held.derive(context, self.placer.as_ref());
        let Some(Document::Scenario(scenario)) = model else {
            return Err(diagnostics
                .into_iter()
                .find(|diagnostic| diagnostic.severity == Severity::Error)
                .map_or(Error::InvalidModel, |diagnostic| Error::TryDoesNotCheck {
                    diagnostic: Box::new(diagnostic),
                }));
        };
        let mut project = Project::clone(&context.project);
        project.scenarios = vec![scenario];
        Ok(project)
    }

    /// The text of a try, to register with
    /// [`crate::ProjectSession::create_document`]. The try stays until it is
    /// discarded: a registration can be refused.
    pub fn into_source(&self, id: &str) -> Result<String> {
        Ok(self.get(id).ok_or(Error::UnknownTry)?.text().to_owned())
    }

    /// A try to edit, with the set's placer.
    fn held(&mut self, id: &str) -> Result<(&mut Try, &dyn Placer)> {
        let held = self
            .tries
            .iter_mut()
            .find(|held| held.id == id)
            .ok_or(Error::UnknownTry)?;
        Ok((held, self.placer.as_ref()))
    }
}
