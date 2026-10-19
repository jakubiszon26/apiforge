//! Editor document state: the workspace, what is being edited, selection and history.
//!
//! This module has no egui dependency. The interface reads from [`Editor`] and
//! changes the document only through its methods, so every edit is validated, can
//! be undone and marks the workspace for saving.

use apiforge_core::{
    format::{Binding, Component, Graph, Project, Scenario, Workspace},
    graph::has_cycle,
    run::Phase,
};
use std::{
    borrow::Cow,
    collections::HashSet,
    time::{Duration, Instant},
};

/// Undo entries kept per editing context.
const HISTORY_LIMIT: usize = 50;
/// Consecutive edits of one node within this interval (typing, dragging) are one
/// undo step.
const MERGE_WINDOW: Duration = Duration::from_millis(650);

/// A component whose inner graph is being edited. Saving creates a new revision;
/// leaving without saving discards the copy.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub definition: Component,
    pub graph: Graph,
}

#[derive(Clone, Debug)]
struct Snapshot {
    graph: Graph,
    /// The cleanup graph, captured with main-graph edits because deleting a main
    /// node also removes cleanup inputs that referenced it.
    cleanup: Option<Graph>,
}

#[derive(Debug, Default)]
struct History {
    past: Vec<Snapshot>,
    future: Vec<Snapshot>,
    merge_key: Option<String>,
    last_edit: Option<Instant>,
}

/// Graph fragment copied on the board, frozen at copy time.
#[derive(Clone, Debug)]
pub struct BlockClipboard {
    pub context: String,
    pub fragment: Graph,
    pub pastes: u32,
}

pub struct Editor {
    workspace: Workspace,
    scenario_id: String,
    phase: Phase,
    drafts: Vec<Draft>,
    pub selection: Vec<String>,
    history: History,
    pub clipboard: Option<BlockClipboard>,
    /// Incremented by every document change; the persistence layer saves on change.
    revision: u64,
}

impl Editor {
    pub fn new(workspace: Workspace) -> Self {
        let scenario_id = active_project(&workspace).scenarios[0].id.clone();
        Self {
            workspace,
            scenario_id,
            phase: Phase::Main,
            drafts: Vec::new(),
            selection: Vec::new(),
            history: History::default(),
            clipboard: None,
            revision: 0,
        }
    }

    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn project(&self) -> &Project {
        active_project(&self.workspace)
    }

    fn project_mut(&mut self) -> &mut Project {
        let id = self.workspace.active_id.clone();
        self.workspace
            .projects
            .iter_mut()
            .find(|project| project.id == id)
            .expect("the active project exists")
    }

    pub fn scenario(&self) -> &Scenario {
        let project = self.project();
        project
            .scenarios
            .iter()
            .find(|scenario| scenario.id == self.scenario_id)
            .unwrap_or(&project.scenarios[0])
    }

    fn scenario_mut(&mut self) -> &mut Scenario {
        let id = self.scenario().id.clone();
        self.project_mut()
            .scenarios
            .iter_mut()
            .find(|scenario| scenario.id == id)
            .expect("the current scenario exists")
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn drafts(&self) -> &[Draft] {
        &self.drafts
    }

    pub fn draft(&self) -> Option<&Draft> {
        self.drafts.last()
    }

    /// The graph shown on the board.
    pub fn graph(&self) -> &Graph {
        match (self.drafts.last(), self.phase) {
            (Some(draft), _) => &draft.graph,
            (None, Phase::Main) => &self.scenario().graph,
            (None, Phase::Cleanup) => &self.scenario().cleanup,
        }
    }

    /// Nodes a binding on the board may reference: cleanup steps can use results of
    /// the main graph.
    pub fn reference_graph(&self) -> Cow<'_, Graph> {
        if self.drafts.is_empty() && self.phase == Phase::Cleanup {
            let scenario = self.scenario();
            let mut graph = scenario.graph.clone();
            graph.nodes.extend(scenario.cleanup.nodes.iter().cloned());
            graph.edges.extend(scenario.cleanup.edges.iter().cloned());
            Cow::Owned(graph)
        } else {
            Cow::Borrowed(self.graph())
        }
    }

    /// Identifies the editing context; history and the block clipboard are scoped to
    /// it.
    pub fn context_key(&self) -> String {
        let drafts: Vec<String> = self
            .drafts
            .iter()
            .map(|draft| format!("{}@{}", draft.definition.id, draft.definition.revision))
            .collect();
        format!(
            "{}:{}:{:?}:{}",
            self.project().id,
            self.scenario().id,
            self.phase,
            drafts.join("/")
        )
    }

    /// Parameters visible to bindings: scenario parameters, or the public arguments
    /// of the component being edited.
    pub fn parameter_names(&self) -> Vec<String> {
        match self.drafts.last() {
            Some(draft) => draft
                .definition
                .inputs
                .iter()
                .map(|port| port.name.clone())
                .collect(),
            None => self.scenario().parameters.keys().cloned().collect(),
        }
    }

    pub fn selected_node(&self) -> Option<&apiforge_core::format::Node> {
        let id = self.selection.first()?;
        self.graph().nodes.iter().find(|node| &node.id == id)
    }

    // ----- Context changes -------------------------------------------------------

    fn context_changed(&mut self) {
        self.history = History::default();
        let graph = self.graph();
        let present: HashSet<&str> = graph.nodes.iter().map(|node| node.id.as_str()).collect();
        let kept: Vec<String> = self
            .selection
            .iter()
            .filter(|id| present.contains(id.as_str()))
            .cloned()
            .collect();
        self.selection = kept;
    }

    pub fn set_phase(&mut self, phase: Phase) {
        if self.phase != phase || !self.drafts.is_empty() {
            self.phase = phase;
            self.drafts.clear();
            self.selection.clear();
            self.context_changed();
        }
    }

    pub fn switch_scenario(&mut self, id: &str) {
        if self
            .project()
            .scenarios
            .iter()
            .any(|scenario| scenario.id == id)
        {
            self.scenario_id = id.to_owned();
            self.phase = Phase::Main;
            self.drafts.clear();
            self.selection.clear();
            self.context_changed();
        }
    }

    pub fn switch_project(&mut self, id: &str) {
        if let Some(project) = self
            .workspace
            .projects
            .iter()
            .find(|project| project.id == id)
        {
            self.scenario_id = project.scenarios[0].id.clone();
            self.workspace.active_id = id.to_owned();
            self.phase = Phase::Main;
            self.drafts.clear();
            self.selection.clear();
            self.context_changed();
            // The active project is part of the saved workspace.
            self.revision += 1;
        }
    }

    /// Opens a copy of a component revision for editing.
    pub fn enter_component(&mut self, definition: Component) {
        let graph = definition.graph.clone();
        self.drafts.push(Draft { definition, graph });
        self.selection.clear();
        self.context_changed();
    }

    /// Leaves the innermost component draft without saving it.
    pub fn leave_component(&mut self) {
        if self.drafts.pop().is_some() {
            self.selection.clear();
            self.context_changed();
        }
    }

    /// Keeps the drafts up to and including `index`, as in a breadcrumb.
    pub fn truncate_drafts(&mut self, index: usize) {
        if index + 1 < self.drafts.len() {
            self.drafts.truncate(index + 1);
            self.selection.clear();
            self.context_changed();
        }
    }

    // ----- Document changes ------------------------------------------------------

    /// Replaces the board's graph as one undoable edit.
    ///
    /// Rejects graphs with cycles. Edits touching a single node within the merge
    /// window form one undo step.
    pub fn apply_graph(&mut self, next: Graph, now: Instant) -> Result<(), String> {
        let current = self.graph();
        if &next == current {
            return Ok(());
        }
        if has_cycle(&next) {
            return Err("This change creates a dependency cycle and was not applied.".into());
        }
        let merge_key = single_node_change(current, &next);
        let mergeable = merge_key.is_some()
            && merge_key == self.history.merge_key
            && self
                .history
                .last_edit
                .is_some_and(|last| now.saturating_duration_since(last) <= MERGE_WINDOW);
        if !mergeable {
            let snapshot = self.snapshot();
            push_bounded(&mut self.history.past, snapshot);
        }
        self.history.future.clear();
        self.history.merge_key = merge_key;
        self.history.last_edit = Some(now);
        self.replace_graph(next, None);
        Ok(())
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            graph: self.graph().clone(),
            cleanup: (self.drafts.is_empty() && self.phase == Phase::Main)
                .then(|| self.scenario().cleanup.clone()),
        }
    }

    fn replace_graph(&mut self, next: Graph, restored_cleanup: Option<Graph>) {
        self.revision += 1;
        if let Some(draft) = self.drafts.last_mut() {
            draft.graph = next;
            return;
        }
        let phase = self.phase;
        let scenario = self.scenario_mut();
        if phase == Phase::Cleanup {
            scenario.cleanup = next;
            return;
        }
        let kept: HashSet<&str> = next.nodes.iter().map(|node| node.id.as_str()).collect();
        let removed: HashSet<String> = scenario
            .graph
            .nodes
            .iter()
            .filter(|node| !kept.contains(node.id.as_str()))
            .map(|node| node.id.clone())
            .collect();
        scenario.graph = next;
        match restored_cleanup {
            Some(cleanup) => scenario.cleanup = cleanup,
            None if !removed.is_empty() => {
                for node in &mut scenario.cleanup.nodes {
                    node.inputs.retain(|_, binding: &mut Binding| {
                        !binding
                            .references()
                            .iter()
                            .any(|reference| removed.contains(reference.node_id))
                    });
                }
            }
            None => {}
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.history.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.history.future.is_empty()
    }

    pub fn undo(&mut self) {
        let Some(previous) = self.history.past.pop() else {
            return;
        };
        let current = self.snapshot();
        self.history.future.insert(0, current);
        self.history.future.truncate(HISTORY_LIMIT);
        self.history.merge_key = None;
        self.replace_graph(previous.graph, previous.cleanup);
        self.prune_selection();
    }

    pub fn redo(&mut self) {
        if self.history.future.is_empty() {
            return;
        }
        let next = self.history.future.remove(0);
        let current = self.snapshot();
        push_bounded(&mut self.history.past, current);
        self.history.merge_key = None;
        self.replace_graph(next.graph, next.cleanup);
        self.prune_selection();
    }

    fn prune_selection(&mut self) {
        let present: HashSet<String> = self
            .graph()
            .nodes
            .iter()
            .map(|node| node.id.clone())
            .collect();
        self.selection.retain(|id| present.contains(id));
    }

    /// Changes project data outside the board graph (settings, environment,
    /// components). Such edits are saved but are not part of the board's undo
    /// history, as in the reference editor.
    pub fn change_project(&mut self, change: impl FnOnce(&mut Project)) {
        change(self.project_mut());
        self.revision += 1;
        // A change may remove the current scenario.
        if !self
            .project()
            .scenarios
            .iter()
            .any(|scenario| scenario.id == self.scenario_id)
        {
            self.scenario_id = self.project().scenarios[0].id.clone();
            self.drafts.clear();
            self.context_changed();
        }
    }

    /// Adds a project and makes it active.
    pub fn add_project(&mut self, project: Project) {
        let id = project.id.clone();
        self.workspace.projects.push(project);
        self.switch_project(&id);
    }

    /// Removes the active project, unless it is the last one.
    pub fn remove_active_project(&mut self) -> bool {
        if self.workspace.projects.len() < 2 {
            return false;
        }
        let id = self.workspace.active_id.clone();
        self.workspace.projects.retain(|project| project.id != id);
        let next = self.workspace.projects[0].id.clone();
        self.switch_project(&next);
        true
    }

    pub fn add_scenario(&mut self, scenario: Scenario) {
        let id = scenario.id.clone();
        self.change_project(|project| project.scenarios.push(scenario));
        self.switch_scenario(&id);
    }
}

fn active_project(workspace: &Workspace) -> &Project {
    workspace
        .projects
        .iter()
        .find(|project| project.id == workspace.active_id)
        .unwrap_or(&workspace.projects[0])
}

fn push_bounded(stack: &mut Vec<Snapshot>, snapshot: Snapshot) {
    stack.push(snapshot);
    if stack.len() > HISTORY_LIMIT {
        stack.remove(0);
    }
}

/// The id of the only changed node, when nothing else (node set, edges) changed.
fn single_node_change(current: &Graph, next: &Graph) -> Option<String> {
    if current.edges != next.edges || current.nodes.len() != next.nodes.len() {
        return None;
    }
    let mut changed = next.nodes.iter().filter(|node| {
        current
            .nodes
            .iter()
            .find(|old| old.id == node.id)
            .is_none_or(|old| old != *node)
    });
    let first = changed.next()?;
    changed.next().is_none().then(|| first.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_core::{
        ImportLimits,
        format::{OrderEdge, Position},
        graph::create_node,
        import_document,
        model::NodeKind,
    };

    fn editor() -> Editor {
        let workspace = import_document(
            include_bytes!("../../../fixtures/compatibility/workspace-v1.json"),
            &ImportLimits::default(),
        )
        .unwrap()
        .into_workspace();
        Editor::new(workspace)
    }

    fn moved(graph: &Graph, index: usize, x: f64) -> Graph {
        let mut graph = graph.clone();
        graph.nodes[index].position.x = x;
        graph
    }

    #[test]
    fn consecutive_edits_of_one_node_merge_into_one_undo_step() {
        let mut editor = editor();
        let original = editor.graph().clone();
        let start = Instant::now();
        editor.apply_graph(moved(&original, 0, 1.0), start).unwrap();
        editor
            .apply_graph(moved(&original, 0, 2.0), start + Duration::from_millis(100))
            .unwrap();
        editor.undo();
        assert_eq!(editor.graph(), &original);
        assert!(!editor.can_undo());
        editor.redo();
        assert_eq!(editor.graph().nodes[0].position.x, 2.0);

        editor
            .apply_graph(
                moved(editor.graph(), 0, 3.0),
                start + Duration::from_secs(5),
            )
            .unwrap();
        editor.undo();
        assert_eq!(editor.graph().nodes[0].position.x, 2.0);
    }

    #[test]
    fn cycles_are_rejected_without_changing_the_document() {
        let mut editor = editor();
        let mut graph = editor.graph().clone();
        let (a, b) = (graph.nodes[0].id.clone(), graph.nodes[1].id.clone());
        for (source, target) in [(&a, &b), (&b, &a)] {
            graph.edges.push(OrderEdge {
                id: format!("{source}-{target}"),
                source: source.clone(),
                target: target.clone(),
                branch: None,
            });
        }
        let revision = editor.revision();
        assert!(editor.apply_graph(graph, Instant::now()).is_err());
        assert_eq!(editor.revision(), revision);
        assert!(!editor.can_undo());
    }

    #[test]
    fn deleting_a_main_node_prunes_cleanup_references_and_undo_restores_them() {
        let mut editor = editor();
        let cleanup_before = editor.scenario().cleanup.clone();
        let referenced: Vec<String> = cleanup_before
            .nodes
            .iter()
            .flat_map(|node| node.inputs.values())
            .flat_map(|binding| binding.references())
            .map(|reference| reference.node_id.to_owned())
            .filter(|id| editor.graph().nodes.iter().any(|node| &node.id == id))
            .collect();
        let target = referenced
            .first()
            .expect("the shop cleanup references the main graph")
            .clone();
        let mut graph = editor.graph().clone();
        graph.nodes.retain(|node| node.id != target);
        graph
            .edges
            .retain(|edge| edge.source != target && edge.target != target);
        for node in &mut graph.nodes {
            node.inputs
                .retain(|_, binding| !binding.references_node(&target));
        }
        editor.apply_graph(graph, Instant::now()).unwrap();
        assert_ne!(editor.scenario().cleanup, cleanup_before);
        editor.undo();
        assert_eq!(editor.scenario().cleanup, cleanup_before);
    }

    #[test]
    fn contexts_have_separate_history_and_selection() {
        let mut editor = editor();
        let node = create_node(NodeKind::Variable, Position { x: 0.0, y: 0.0 }, None);
        let mut graph = editor.graph().clone();
        graph.nodes.push(node.clone());
        editor.apply_graph(graph, Instant::now()).unwrap();
        editor.selection = vec![node.id.clone()];
        let main_key = editor.context_key();
        editor.set_phase(Phase::Cleanup);
        assert_ne!(editor.context_key(), main_key);
        assert!(!editor.can_undo());
        assert!(editor.selection.is_empty());
        assert!(
            editor
                .reference_graph()
                .nodes
                .iter()
                .any(|candidate| candidate.id == node.id)
        );
    }

    #[test]
    fn component_drafts_edit_a_copy() {
        let mut editor = editor();
        let definition = editor.project().components[0].clone();
        editor.enter_component(definition.clone());
        let mut graph = editor.graph().clone();
        graph.nodes[0].name = "Changed".into();
        editor.apply_graph(graph, Instant::now()).unwrap();
        assert_eq!(editor.project().components[0], definition);
        editor.leave_component();
        assert!(editor.draft().is_none());
    }
}
