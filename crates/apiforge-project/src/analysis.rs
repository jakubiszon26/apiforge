//! Source → model derivation. An `AnalysisInput` is immutable and self-contained,
//! so a worker can run it off the session thread; the session installs the
//! result only while every field of its ticket still matches (ADR 0012).
use crate::layout::Placer;
use apiforge_core::{
    format::{Component, Graph, Project, Scenario},
    model::ErrorPolicy,
};
use apiforge_script::{Diagnostic, Document, LowerContext};
use apiforge_storage::project::{DiagramRecord, DocumentKind, IdentityBase, MAX_SOURCE_BYTES};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AnalysisTicket {
    /// Unique session instance; epochs alone restart at one after reopening.
    pub session_id: String,
    pub session_epoch: u64,
    pub document_id: String,
    pub source_version: u64,
    pub context_generation: u64,
}

/// The definition a source must describe: its registry kind and entity, plus
/// the revision a working component carries until it is published.
#[derive(Clone, Debug)]
pub(crate) struct Target {
    pub kind: DocumentKind,
    pub entity_id: String,
    pub revision: u32,
}

#[derive(Clone)]
pub struct AnalysisInput {
    pub ticket: AnalysisTicket,
    pub(crate) target: Target,
    pub(crate) text: Arc<str>,
    pub(crate) context: Arc<Project>,
    /// The session's previous model of this document, whose identities win.
    pub(crate) base: Option<Document>,
    /// The shared checkpoint, used only when the session has no model yet.
    pub(crate) checkpoint: Option<DiagramRecord>,
    pub(crate) placer: Arc<dyn Placer>,
}

/// How identities and layout of a valid model were obtained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Restored {
    /// The checkpoint describes exactly this source and context.
    Checkpoint,
    /// Matched against the session model or an older checkpoint.
    Reconciled,
    /// No usable identity base: new identities and automatic layout.
    Fresh,
}

#[derive(Clone, Debug)]
pub struct Analysis {
    pub ticket: AnalysisTicket,
    pub diagnostics: Vec<Diagnostic>,
    pub(crate) model: Option<(Document, Restored)>,
}

impl Analysis {
    pub fn is_valid(&self) -> bool {
        self.model.is_some()
    }
    pub fn restored(&self) -> Option<Restored> {
        self.model.as_ref().map(|(_, restored)| *restored)
    }
}

impl AnalysisInput {
    /// Pure and potentially slow: parse, lower with the folder policy, then
    /// reconcile identities. Never touches storage or session state.
    pub fn run(&self) -> Analysis {
        let (diagnostics, model) = derive(
            &self.target,
            &self.text,
            &self.context,
            self.base.as_ref(),
            self.checkpoint.as_ref(),
            self.placer.as_ref(),
        );
        Analysis {
            ticket: self.ticket.clone(),
            diagnostics,
            model,
        }
    }
}

pub(crate) fn derive(
    target: &Target,
    text: &str,
    context: &Project,
    base: Option<&Document>,
    checkpoint: Option<&DiagramRecord>,
    placer: &dyn Placer,
) -> (Vec<Diagnostic>, Option<(Document, Restored)>) {
    if text.len() > MAX_SOURCE_BYTES {
        return (
            vec![Diagnostic::error(
                0..0,
                "The source exceeds the 1 MiB limit",
            )],
            None,
        );
    }
    let tree = apiforge_script::parse(text);
    let empty = empty_document(target);
    let pinned = match base.unwrap_or(&empty) {
        Document::Component(component) => Some(component),
        Document::Scenario(_) => None,
    };
    let lower_context = LowerContext::folder(context);
    let lower_context = match pinned {
        Some(component) => lower_context.with_component(component),
        None => lower_context,
    };
    let lowered = apiforge_script::lower(&tree, &lower_context);
    let mut diagnostics = lowered.diagnostics.clone();
    if lowered.has_errors() {
        return (diagnostics, None);
    }
    let Some(mut document) = lowered.document.clone() else {
        return (diagnostics, None);
    };
    if kind(&document) != target.kind {
        diagnostics.push(Diagnostic::error(
            0..text.len(),
            "This code describes a different kind of document",
        ));
        return (diagnostics, None);
    }
    normalise(&mut document, target);
    let checkpoint = checkpoint.filter(|record| {
        record.identity_base.kind() == target.kind
            && record.identity_base.entity_id() == target.entity_id
            && record.validate_in_context(context).is_ok()
    });
    let lowered_base = identity_base(&document);
    let (identity, restored) = match (base, checkpoint) {
        (Some(base), _) => (base.clone(), Restored::Reconciled),
        (None, Some(record)) if record.describes(text, &lowered_base, context) => {
            let mut exact = document_of(&record.identity_base);
            normalise(&mut exact, target);
            return (diagnostics, Some((exact, Restored::Checkpoint)));
        }
        (None, Some(record)) => (document_of(&record.identity_base), Restored::Reconciled),
        (None, None) => (empty, Restored::Fresh),
    };
    let Ok(reconciled) = apiforge_script::reconcile(&identity, &lowered) else {
        diagnostics.push(Diagnostic::error(
            0..text.len(),
            "This code describes a different document",
        ));
        return (diagnostics, None);
    };
    let mut document = reconciled.document;
    normalise(&mut document, target);
    placer.place(&mut document, &reconciled.unpositioned);
    if apiforge_script::validate_document_model(&document, &LowerContext::folder(context)).is_err()
    {
        diagnostics.push(Diagnostic::error(
            0..text.len(),
            "The definition is not valid in this project",
        ));
        return (diagnostics, None);
    }
    (diagnostics, Some((document, restored)))
}

/// Registry identity is authoritative: lowering and reconciliation may suggest
/// another entity ID or revision, for example after a component handle change.
pub(crate) fn normalise(document: &mut Document, target: &Target) {
    match document {
        Document::Scenario(scenario) => scenario.id.clone_from(&target.entity_id),
        Document::Component(component) => {
            component.id.clone_from(&target.entity_id);
            component.revision = target.revision;
        }
    }
}

pub(crate) fn kind(document: &Document) -> DocumentKind {
    match document {
        Document::Scenario(_) => DocumentKind::Scenario,
        Document::Component(_) => DocumentKind::Component,
    }
}

pub(crate) fn identity_base(document: &Document) -> IdentityBase {
    match document {
        Document::Scenario(scenario) => IdentityBase::Scenario(scenario.clone()),
        Document::Component(component) => IdentityBase::Component(component.clone()),
    }
}

pub(crate) fn document_of(base: &IdentityBase) -> Document {
    match base {
        IdentityBase::Scenario(scenario) => Document::Scenario(scenario.clone()),
        IdentityBase::Component(component) => Document::Component(component.clone()),
    }
}

fn empty_document(target: &Target) -> Document {
    match target.kind {
        DocumentKind::Scenario => Document::Scenario(Scenario {
            id: target.entity_id.clone(),
            name: String::new(),
            description: String::new(),
            graph: Graph::default(),
            cleanup: Graph::default(),
            parameters: Default::default(),
            error_policy: ErrorPolicy::Stop,
        }),
        DocumentKind::Component => Document::Component(Component {
            id: target.entity_id.clone(),
            revision: target.revision,
            handle: String::new(),
            name: String::new(),
            description: String::new(),
            inputs: Vec::new(),
            graph: Graph::default(),
            action: None,
        }),
    }
}
