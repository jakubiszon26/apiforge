//! Which documents are open as tabs, which one is active and what the main area
//! shows. Pure state, so the mode and tab rules are tested without a window.
//! Switching a mode or a tab never saves, closes or resets a document.

use apiforge_storage::project::{PanelSizes, TabState, TextSelection, WorkspaceState};
use std::collections::BTreeSet;

pub use apiforge_storage::project::ViewMode as Mode;

pub const DEFAULT_PANELS: PanelSizes = PanelSizes {
    documents: 232.0,
    split_code: 520.0,
    diagnostics: 132.0,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Workbench {
    pub mode: Mode,
    tabs: Vec<TabState>,
    active: Option<String>,
    pub panels: PanelSizes,
    /// Explorer folders shown expanded.
    pub expanded: BTreeSet<String>,
}

impl Workbench {
    /// The folder's last view, without tabs of documents that no longer exist.
    /// A project without one starts in Code.
    pub fn restore(state: Option<WorkspaceState>, exists: impl Fn(&str) -> bool) -> Self {
        let Some(state) = state else {
            return Self {
                mode: Mode::Code,
                tabs: Vec::new(),
                active: None,
                panels: DEFAULT_PANELS,
                expanded: BTreeSet::new(),
            };
        };
        let tabs: Vec<TabState> = state
            .tabs
            .into_iter()
            .filter(|tab| exists(&tab.document_id))
            .collect();
        let active = state
            .active
            .filter(|active| tabs.iter().any(|tab| &tab.document_id == active))
            .or_else(|| tabs.first().map(|tab| tab.document_id.clone()));
        Self {
            mode: state.mode,
            tabs,
            active,
            panels: state.panels.unwrap_or(DEFAULT_PANELS),
            expanded: state.expanded.into_iter().collect(),
        }
    }

    pub fn state(&self, project_id: &str) -> WorkspaceState {
        WorkspaceState {
            mode: self.mode,
            tabs: self.tabs.clone(),
            active: self.active.clone(),
            panels: Some(self.panels.clone()),
            expanded: self.expanded.iter().cloned().collect(),
            ..WorkspaceState::new(project_id)
        }
    }

    pub fn tabs(&self) -> &[TabState] {
        &self.tabs
    }

    pub fn active(&self) -> Option<&str> {
        self.active.as_deref()
    }

    pub fn active_tab(&self) -> Option<&TabState> {
        let active = self.active.as_deref()?;
        self.tabs.iter().find(|tab| tab.document_id == active)
    }

    pub fn tab_mut(&mut self, document_id: &str) -> Option<&mut TabState> {
        self.tabs
            .iter_mut()
            .find(|tab| tab.document_id == document_id)
    }

    /// Ctrl+E: Code and Diagram swap; Split goes to Code.
    pub fn toggle(&mut self) {
        self.mode = match self.mode {
            Mode::Code => Mode::Diagram,
            Mode::Diagram | Mode::Split => Mode::Code,
        };
    }

    /// Activates a document, opening its tab after the active one when it has
    /// none. One tab per document.
    pub fn open(&mut self, document_id: &str) {
        if !self.tabs.iter().any(|tab| tab.document_id == document_id) {
            let at = self
                .active
                .as_deref()
                .and_then(|active| self.tabs.iter().position(|tab| tab.document_id == active))
                .map_or(self.tabs.len(), |index| index + 1);
            self.tabs.insert(
                at,
                TabState {
                    document_id: document_id.to_owned(),
                    selection: TextSelection {
                        anchor: 0,
                        caret: 0,
                    },
                    scroll: 0.0,
                    cleanup: false,
                    camera: None,
                },
            );
        }
        self.active = Some(document_id.to_owned());
    }

    /// Opens a tab without activating it: a restored view keeps the document
    /// that was active when it was saved.
    pub fn ensure(&mut self, document_id: &str) {
        if self.tabs.iter().any(|tab| tab.document_id == document_id) {
            return;
        }
        self.tabs.push(TabState {
            document_id: document_id.to_owned(),
            selection: TextSelection {
                anchor: 0,
                caret: 0,
            },
            scroll: 0.0,
            cleanup: false,
            camera: None,
        });
    }

    /// Closes a tab; the tab to its right becomes active, else the one to its left.
    pub fn close(&mut self, document_id: &str) {
        let Some(index) = self
            .tabs
            .iter()
            .position(|tab| tab.document_id == document_id)
        else {
            return;
        };
        self.tabs.remove(index);
        if self.active.as_deref() == Some(document_id) {
            self.active = self
                .tabs
                .get(index)
                .or_else(|| index.checked_sub(1).and_then(|left| self.tabs.get(left)))
                .map(|tab| tab.document_id.clone());
        }
    }

    /// Ctrl+Tab and Ctrl+Shift+Tab, wrapping around.
    pub fn cycle(&mut self, forward: bool) {
        let count = self.tabs.len();
        let Some(index) = self
            .active
            .as_deref()
            .and_then(|active| self.tabs.iter().position(|tab| tab.document_id == active))
        else {
            return;
        };
        let next = if forward {
            (index + 1) % count
        } else {
            (index + count - 1) % count
        };
        self.active = Some(self.tabs[next].document_id.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opened(ids: &[&str]) -> Workbench {
        let mut workbench = Workbench::restore(None, |_| true);
        for id in ids {
            workbench.open(id);
        }
        workbench
    }

    fn order(workbench: &Workbench) -> Vec<&str> {
        workbench
            .tabs()
            .iter()
            .map(|tab| tab.document_id.as_str())
            .collect()
    }

    #[test]
    fn ctrl_e_swaps_code_and_diagram_and_leaves_split_for_code() {
        let mut workbench = opened(&[]);
        assert_eq!(workbench.mode, Mode::Code, "a new project starts in Code");
        workbench.toggle();
        assert_eq!(workbench.mode, Mode::Diagram);
        workbench.toggle();
        assert_eq!(workbench.mode, Mode::Code);
        workbench.mode = Mode::Split;
        workbench.toggle();
        assert_eq!(workbench.mode, Mode::Code);
    }

    #[test]
    fn one_tab_per_document_opened_beside_the_active_one() {
        let mut workbench = opened(&["a", "b", "c"]);
        assert_eq!(order(&workbench), ["a", "b", "c"]);
        workbench.open("a");
        assert_eq!(order(&workbench), ["a", "b", "c"], "no duplicate tab");
        workbench.open("d");
        assert_eq!(order(&workbench), ["a", "d", "b", "c"]);
        assert_eq!(workbench.active(), Some("d"));
    }

    #[test]
    fn closing_activates_the_right_neighbour_then_the_left() {
        let mut workbench = opened(&["a", "b", "c"]);
        workbench.open("b");
        workbench.close("b");
        assert_eq!(workbench.active(), Some("c"));
        workbench.close("c");
        assert_eq!(workbench.active(), Some("a"));
        workbench.close("a");
        assert_eq!(workbench.active(), None);
        workbench.close("missing");
    }

    #[test]
    fn closing_an_inactive_tab_keeps_the_active_one() {
        let mut workbench = opened(&["a", "b", "c"]);
        workbench.close("a");
        assert_eq!(workbench.active(), Some("c"));
        assert_eq!(order(&workbench), ["b", "c"]);
    }

    #[test]
    fn cycling_wraps_in_both_directions() {
        let mut workbench = opened(&["a", "b", "c"]);
        workbench.cycle(true);
        assert_eq!(workbench.active(), Some("a"));
        workbench.cycle(false);
        assert_eq!(workbench.active(), Some("c"));
        workbench.cycle(false);
        assert_eq!(workbench.active(), Some("b"));
    }

    #[test]
    fn the_saved_view_returns_without_documents_that_are_gone() {
        let mut workbench = opened(&["a", "b", "c"]);
        workbench.mode = Mode::Split;
        workbench.tab_mut("b").unwrap().selection = TextSelection {
            anchor: 3,
            caret: 1,
        };
        workbench.open("c");
        let state = workbench.state("project");
        let restored = Workbench::restore(Some(state.clone()), |id| id != "c");
        assert_eq!(restored.mode, Mode::Split, "the folder's last mode");
        assert_eq!(order(&restored), ["a", "b"]);
        assert_eq!(restored.active(), Some("a"), "the active document is gone");
        assert_eq!(restored.tabs()[1].selection, state.tabs[1].selection);
        assert_eq!(Workbench::restore(Some(state), |_| true), workbench);
    }
}
