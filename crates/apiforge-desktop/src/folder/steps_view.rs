//! *Steps* in the folder window (REQ-01a): the choice between the list and
//! the board, kept with the project's view on this computer, and the list of
//! the active scenario.

use super::*;
use crate::steps::{
    list::{self, Action, List, Results},
    model::Model,
};

/// The list's state, and its rows for the model and the project they were
/// read from.
#[derive(Default)]
pub(super) struct StepsView {
    list: List,
    rows: Option<(Arc<Document>, Arc<Project>, Arc<Model>)>,
    /// The check last clicked in the list of a document, and the step the
    /// board marks for it once it folds the check into one.
    marks: HashMap<String, (String, Option<String>)>,
}

/// The gap between the header and the summary, the tabs and the switch.
const GAP: f32 = 12.0;

impl FolderApp {
    /// Whether the active tab shows *Steps*: a scenario in Diagram, while
    /// the choice is *Steps*. Code and Split are as without it.
    pub(super) fn steps_shown(&self, active: Option<&str>) -> bool {
        self.steps_offered(active) && self.workbench.panels.steps
    }

    /// Whether *Steps | Canvas* is offered: for a scenario, in Diagram.
    fn steps_offered(&self, active: Option<&str>) -> bool {
        self.workbench.mode == Mode::Diagram
            && active
                .and_then(|id| self.shared_model(id))
                .is_some_and(|model| matches!(*model, Document::Scenario(_)))
    }

    fn choose_steps(&mut self, steps: bool) {
        self.workbench.panels.steps = steps;
        self.touched();
    }

    /// The rows of `model`, read again only when it or the project changed.
    fn step_rows(&mut self, model: &Arc<Document>) -> Arc<Model> {
        let project = self.context_project();
        if let Some((document, context, rows)) = &self.steps.rows
            && Arc::ptr_eq(document, model)
            && Arc::ptr_eq(context, &project)
        {
            return rows.clone();
        }
        let (main, cleanup) = scenario_of(model);
        let rows = Arc::new(Model::of(main, cleanup, &project));
        self.steps.rows = Some((model.clone(), project, rows.clone()));
        rows
    }

    /// The header and the list of `document_id`, in place of its board.
    pub(super) fn steps_area(
        &mut self,
        ui: &mut egui::Ui,
        document_id: &str,
        header: &diagram::Header<'_>,
        model: &Arc<Document>,
    ) {
        let rows = self.step_rows(model);
        Self::steps_header(ui, header, &rows.summary());
        // Back from the board, which marked the step it folds the check into:
        // the mark of the list is where it was.
        if let Some((check, Some(step))) = self.steps.marks.get(document_id)
            && self
                .selections
                .get(document_id)
                .is_some_and(|ids| ids.iter().eq([step]))
        {
            let check = check.clone();
            self.selections
                .insert(document_id.to_owned(), vec![check.clone()]);
            self.steps
                .marks
                .insert(document_id.to_owned(), (check, None));
        }
        let selected = self
            .selections
            .get(document_id)
            .and_then(|ids| ids.first())
            .cloned();
        let mut state = std::mem::take(&mut self.steps.list);
        let run = self.fresh_report(document_id);
        let results = run
            .as_ref()
            .map(|(run, ends)| Results::new(&run.steps, ends, &rows));
        let action = state.show(
            ui,
            document_id,
            &rows,
            results.as_ref(),
            selected.as_deref(),
        );
        self.steps.list = state;
        match action {
            Some(Action::Select { id, cleanup }) => {
                if self.cleanup(document_id) != cleanup {
                    self.touched();
                }
                // The row clicked is the row marked, also for a check the
                // board folds into another step.
                if let Some(tab) = self.workbench.tab_mut(document_id) {
                    tab.cleanup = cleanup;
                }
                self.select_step(document_id, id.clone());
                self.steps.marks.insert(document_id.to_owned(), (id, None));
            }
            Some(Action::Clear) => {
                self.selections.remove(document_id);
            }
            None => {}
        }
    }

    /// Before the board is drawn: a check clicked in the list is marked by the
    /// board's rule, on the step it folds it into, if it does.
    pub(super) fn mark_on_board(&mut self, document_id: &str, model: &Document, cleanup: bool) {
        let Some((check, None)) = self.steps.marks.get(document_id) else {
            return;
        };
        let check = check.clone();
        if !self
            .selections
            .get(document_id)
            .is_some_and(|ids| ids.iter().eq([&check]))
        {
            self.steps.marks.remove(document_id);
            return;
        }
        let step = self.canvases.get(document_id).and_then(|canvas| {
            canvas.folded_into(graph_of(model, cleanup), later_of(model, cleanup), &check)
        });
        if let Some(step) = step {
            self.show_checks(document_id, step.clone());
            self.steps
                .marks
                .insert(document_id.to_owned(), (check, Some(step)));
        }
    }

    /// A step selected by hand elsewhere takes the mark of the list.
    pub(super) fn forget_mark(&mut self, document_id: &str) {
        self.steps.marks.remove(document_id);
    }

    /// Whether a row of the list has the keyboard, which no field then has.
    pub(super) fn steps_keyboard(&self) -> bool {
        self.steps_shown(self.workbench.active()) && self.steps.list.has_keyboard()
    }

    /// The section a step is added to from outside the board: in *Steps* the
    /// selected row's, Main while nothing is selected; else the board's.
    pub(super) fn add_section(&mut self, document_id: &str, model: &Arc<Document>) -> bool {
        if !self.steps_shown(Some(document_id)) {
            return self.cleanup(document_id);
        }
        let rows = self.step_rows(model);
        self.selections
            .get(document_id)
            .and_then(|ids| ids.first())
            .is_some_and(|id| rows.in_cleanup(id))
    }

    /// The header of *Steps*: the board's own name and kind, and the summary.
    fn steps_header(ui: &mut egui::Ui, header: &diagram::Header<'_>, summary: &str) {
        let (band, _) = ui.allocate_exact_size(
            egui::vec2(
                ui.available_width(),
                theme::CONTROL_HEIGHT + 2.0 * canvas_padding::Y,
            ),
            egui::Sense::hover(),
        );
        ui.painter().rect_filled(band, 0.0, theme::WHITE);
        ui.painter().hline(
            band.x_range(),
            band.bottom(),
            egui::Stroke::new(1.0, theme::BORDER),
        );
        let row = egui::Rect::from_min_max(
            band.left_top() + egui::vec2(canvas_padding::LEFT, canvas_padding::Y),
            band.right_bottom() - egui::vec2(canvas_padding::RIGHT, canvas_padding::Y),
        );
        let font = egui::FontId::proportional(theme::FONT_META);
        let galley = ui
            .painter()
            .layout_no_wrap(summary.to_owned(), font, theme::MUTED);
        let alone = row.width();
        let with_summary = alone - galley.size().x - GAP;
        let fit = header.fit(
            ui,
            canvas::HeaderRoom {
                alone,
                with_summary,
            },
        );
        header.show(ui.ctx(), ui.id(), row, &fit, &mut false);
        if fit.width <= with_summary {
            let place = egui::Rect::from_min_size(
                egui::pos2(
                    row.right() - galley.size().x,
                    row.center().y - galley.size().y / 2.0,
                ),
                galley.size(),
            );
            ui.painter().galley(place.min, galley, theme::MUTED);
            ui.interact(place, ui.id().with("steps-summary"), egui::Sense::hover())
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, summary));
        }
    }

    /// Draws *Steps | Canvas* at the right of the tabs' row, in the same
    /// place in both views and at every width (the owner, 2026-11-24).
    /// Returns the width it takes from the tabs.
    pub(super) fn strip_switch(&mut self, ui: &mut egui::Ui) -> f32 {
        if !self.steps_offered(self.workbench.active()) {
            return 0.0;
        }
        let size = list::switch_size(ui);
        let strip = ui.available_rect_before_wrap();
        // Over the right end of the header row below, level with the tabs.
        let margin = (canvas_padding::RIGHT - theme::SPACE_SM).max(0.0);
        let at = egui::Rect::from_center_size(
            egui::pos2(
                strip.right() - margin - size.x / 2.0,
                strip.top() + theme::CONTROL_HEIGHT / 2.0,
            ),
            size,
        );
        let mut place = ui.new_child(egui::UiBuilder::new().max_rect(at));
        if let Some(steps) = list::switch(&mut place, self.workbench.panels.steps) {
            self.choose_steps(steps);
        }
        size.x + margin + GAP
    }
}

/// The paddings of the board's header row.
mod canvas_padding {
    pub const LEFT: f32 = 18.0;
    pub const RIGHT: f32 = 14.0;
    pub const Y: f32 = 4.0;
}
