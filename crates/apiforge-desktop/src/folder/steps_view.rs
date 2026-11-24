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
    /// The board's header row had no room for *Steps | Canvas*, which stands
    /// at the right of the tabs above it.
    above: bool,
}

/// The gap between the header, the summary and the switch.
const GAP: f32 = 12.0;

impl FolderApp {
    /// Whether the active tab shows *Steps*: a scenario in Diagram, while
    /// the choice is *Steps*. Code and Split are as without it.
    pub(super) fn steps_shown(&self, active: Option<&str>) -> bool {
        self.steps_offered(active) && self.workbench.panels.steps
    }

    /// Whether the header offers *Steps | Canvas*: a scenario's, in Diagram.
    pub(super) fn steps_offered(&self, active: Option<&str>) -> bool {
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
        if self.steps_header(ui, header, &rows.summary()) {
            self.choose_steps(false);
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
            .map(|(run, ends)| Results::new(&run.steps, ends));
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
                // As a row of Run results selects it: a folded check on its
                // step.
                self.select_on_board(document_id, model, id, cleanup);
            }
            Some(Action::Clear) => {
                self.selections.remove(document_id);
            }
            None => {}
        }
    }

    /// The header of *Steps*; whether *Canvas* was chosen.
    fn steps_header(&self, ui: &mut egui::Ui, header: &diagram::Header<'_>, summary: &str) -> bool {
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
        let switch = list::switch_size(ui);
        let at = egui::Rect::from_center_size(
            egui::pos2(row.right() - switch.x / 2.0, row.center().y),
            switch,
        );
        let font = egui::FontId::proportional(theme::FONT_META);
        let galley = ui
            .painter()
            .layout_no_wrap(summary.to_owned(), font, theme::MUTED);
        let alone = row.width() - switch.x - GAP;
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
                    at.left() - GAP - galley.size().x,
                    row.center().y - galley.size().y / 2.0,
                ),
                galley.size(),
            );
            ui.painter().galley(place.min, galley, theme::MUTED);
            ui.interact(place, ui.id().with("steps-summary"), egui::Sense::hover())
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, summary));
        }
        let mut place = ui.new_child(egui::UiBuilder::new().max_rect(at));
        list::switch(&mut place, true) == Some(false)
    }

    /// Whether the board's header row has no room for *Steps | Canvas*, as it
    /// was drawn in this frame.
    pub(super) fn set_switch_above(&mut self, above: bool) {
        if self.steps.above != above {
            self.steps.above = above;
            self.ui_context.request_repaint();
        }
    }

    /// Draws *Steps | Canvas* at the right of the tabs while the board's
    /// header row under them has no room for it. Returns the width it takes
    /// from the tabs.
    pub(super) fn strip_switch(&mut self, ui: &mut egui::Ui) -> f32 {
        let shown = self.steps.above
            && !self.workbench.panels.steps
            && self.steps_offered(self.workbench.active());
        if !shown {
            return 0.0;
        }
        let size = list::switch_size(ui);
        let strip = ui.available_rect_before_wrap();
        // Over the tools of the board's header row, level with the tabs.
        let margin = (canvas_padding::RIGHT - theme::SPACE_SM).max(0.0);
        let at = egui::Rect::from_center_size(
            egui::pos2(
                strip.right() - margin - size.x / 2.0,
                strip.top() + theme::CONTROL_HEIGHT / 2.0,
            ),
            size,
        );
        let mut place = ui.new_child(egui::UiBuilder::new().max_rect(at));
        let chosen = list::switch(&mut place, false);
        self.steps_chosen(chosen);
        size.x + margin + GAP
    }

    /// *Steps | Canvas* chosen on the board's header.
    pub(super) fn steps_chosen(&mut self, steps: Option<bool>) {
        if let Some(steps) = steps {
            self.choose_steps(steps);
        }
    }
}

/// The width the board's header takes with *Steps | Canvas* at its right,
/// before the steps summary and the tools; `None` when the row has no room
/// for it beside a header of `full`, of which nothing is given up for it.
pub(super) fn beside(ui: &egui::Ui, full: f32, room: canvas::HeaderRoom) -> Option<f32> {
    // A point to spare keeps rounding from pushing the summary out.
    let width = room.with_summary - 1.0;
    (full + GAP + list::switch_size(ui).x <= width).then_some(width)
}

/// Draws *Steps | Canvas* at the right of `row`, the board's header as wide
/// as [`beside`] says. Returns what a click chose.
pub(super) fn board_switch(ui: &egui::Ui, row: egui::Rect) -> Option<bool> {
    egui::Area::new(ui.id().with("steps-canvas-switch"))
        .fixed_pos(row.right_center())
        .pivot(egui::Align2::RIGHT_CENTER)
        .fade_in(false)
        .show(ui.ctx(), |ui| list::switch(ui, false))
        .inner
}

/// The paddings of the board's header row.
mod canvas_padding {
    pub const LEFT: f32 = 18.0;
    pub const RIGHT: f32 = 14.0;
    pub const Y: f32 = 4.0;
}
