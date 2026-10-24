//! Inspector drafts share the workspace draft store and its secret checks.
use apiforge_storage::{CodeDraft, DraftTarget, Drafts, draft_base_hash};
use eframe::egui::{self, Id, Ui};

#[derive(Clone, Default)]
struct Registry {
    owner: Option<(String, DraftTarget)>,
    entries: Vec<CodeDraft>,
}

fn key() -> Id {
    Id::new("inspector-drafts")
}

pub fn restore(context: &egui::Context, drafts: Option<&Drafts>) {
    context.data_mut(|data| {
        data.insert_temp(
            key(),
            Registry {
                owner: None,
                entries: drafts
                    .into_iter()
                    .flat_map(|drafts| &drafts.entries)
                    .filter(|draft| matches!(draft.target, DraftTarget::Field { .. }))
                    .cloned()
                    .collect(),
            },
        )
    });
}

pub fn set_owner(context: &egui::Context, owner: &crate::buffers::Context) {
    let owner = match owner {
        crate::buffers::Context::Scenario { project, id } => {
            (project.clone(), DraftTarget::Scenario { id: id.clone() })
        }
        crate::buffers::Context::Component {
            project,
            id,
            revision,
        } => (
            project.clone(),
            DraftTarget::Component {
                id: id.clone(),
                revision: *revision,
            },
        ),
    };
    context.data_mut(|data| {
        let mut registry = data.get_temp::<Registry>(key()).unwrap_or_default();
        registry.owner = Some(owner);
        data.insert_temp(key(), registry);
    });
}

pub fn entries(context: &egui::Context) -> Vec<CodeDraft> {
    context.data_mut(|data| data.get_temp::<Registry>(key()).unwrap_or_default().entries)
}

fn target(registry: &Registry, id: Id) -> Option<(String, DraftTarget)> {
    let (project, owner) = registry.owner.as_ref()?;
    Some((
        project.clone(),
        DraftTarget::Field {
            owner: Box::new(owner.clone()),
            widget: id.value(),
        },
    ))
}

/// Restore invalid text. An external field edit requires an explicit choice.
pub fn text(ui: &mut Ui, id: Id, source: &str) -> Option<(String, bool)> {
    let mut registry = ui
        .data_mut(|data| data.get_temp::<Registry>(key()))
        .unwrap_or_default();
    let (project, target) = target(&registry, id)?;
    let index = registry
        .entries
        .iter()
        .position(|draft| draft.project_id == project && draft.target == target)?;
    let hash = draft_base_hash(source);
    let mut conflict = registry.entries[index].base_hash != hash;
    let mut discard = false;
    if conflict {
        ui.colored_label(
            crate::theme::WARNING,
            "This field changed while it had unfinished text.",
        );
        ui.horizontal_wrapped(|ui| {
            if ui.button("Keep unfinished text").clicked() {
                registry.entries[index].base_hash = hash;
                conflict = false;
            }
            discard = ui.button("Use current field").clicked();
        });
    }
    let text = if discard {
        registry.entries.remove(index);
        Some((source.to_owned(), false))
    } else {
        Some((registry.entries[index].text.clone(), conflict))
    };
    ui.data_mut(|data| data.insert_temp(key(), registry));
    text
}

/// Retain only unfinished text. Valid fields already live in the document.
pub fn store(ui: &mut Ui, id: Id, source: &str, text: &str, invalid: bool) {
    ui.data_mut(|data| {
        let mut registry = data.get_temp::<Registry>(key()).unwrap_or_default();
        let Some((project_id, target)) = target(&registry, id) else {
            return;
        };
        let previous = registry
            .entries
            .iter()
            .find(|draft| draft.project_id == project_id && draft.target == target)
            .cloned();
        registry
            .entries
            .retain(|draft| draft.project_id != project_id || draft.target != target);
        if invalid {
            registry.entries.push(CodeDraft {
                project_id,
                target,
                base_hash: previous
                    .map_or_else(|| draft_base_hash(source), |draft| draft.base_hash),
                text: text.to_owned(),
                cursor: text.len(),
            });
        }
        data.insert_temp(key(), registry);
    });
}
