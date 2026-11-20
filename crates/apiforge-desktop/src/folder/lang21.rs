//! MOCK (LANG-21, #482): the window offers *Update the project* for a
//! project written in an older language. Test builds only. Nothing here
//! converts anything: a scene says what the notice band, the Problems list
//! and one dialog show, and the window draws it with its own widgets.
use super::*;
use std::cell::RefCell;

/// The offer under the tabs.
#[derive(Clone, Default)]
pub(crate) struct Offer {
    pub text: String,
    /// The buttons after the text, in order.
    pub buttons: Vec<String>,
}

/// One line of a dialog's list.
#[derive(Clone, Default)]
pub(crate) struct Row {
    pub text: String,
    /// A second, muted line: what to do.
    pub detail: Option<String>,
    /// A link at the row's end.
    pub link: Option<String>,
}

/// A titled group of a dialog's list: a file.
#[derive(Clone, Default)]
pub(crate) struct Group {
    pub title: String,
    pub rows: Vec<Row>,
}

/// The one dialog of a view.
#[derive(Clone, Default)]
pub(crate) struct Dialog {
    pub heading: String,
    pub before: Vec<String>,
    pub groups: Vec<Group>,
    pub after: Vec<String>,
    pub primary: Option<String>,
    pub secondary: Vec<String>,
    pub width: f32,
}

/// A statement the person places by hand: the file, the text its problem
/// underlines and the message.
#[derive(Clone, Default)]
pub(crate) struct ByHand {
    pub path: String,
    pub needle: String,
    pub message: String,
}

#[derive(Clone, Default)]
pub(crate) struct Scene {
    pub offer: Option<Offer>,
    /// Every `@after` is an error without a fix, as in a language-1 project
    /// read by a build of language 2 (LANG-13, A5).
    pub after_errors: bool,
    pub by_hand: Vec<ByHand>,
    /// The file is shown without problems: a converted project.
    pub converted: bool,
    pub dialog: Option<Dialog>,
}

thread_local! {
    static SCENE: RefCell<Scene> = RefCell::new(Scene::default());
}

pub(crate) fn set(scene: Scene) {
    SCENE.with(|current| *current.borrow_mut() = scene);
}

fn scene() -> Scene {
    SCENE.with(|current| current.borrow().clone())
}

/// LANG-13's message for an `@after`.
pub(crate) const AFTER_REMOVED: &str = "`@after` is not part of the language: the steps of a block run in the order they are written; delete this line";

/// The problems of `path` as the scene has them.
pub(super) fn problems(
    path: &str,
    text: &str,
    diagnostics: &mut Vec<Diagnostic>,
    analysis: &mut AnalysisState,
) {
    let scene = scene();
    if scene.converted {
        diagnostics.clear();
        if *analysis == AnalysisState::Invalid {
            *analysis = AnalysisState::Valid;
        }
        return;
    }
    let mut mock = Vec::new();
    for by_hand in scene.by_hand.iter().filter(|by_hand| by_hand.path == path) {
        if let Some(start) = text.find(&by_hand.needle) {
            mock.push(Diagnostic::error(
                start..start + by_hand.needle.len(),
                by_hand.message.clone(),
            ));
        }
    }
    if scene.after_errors {
        let mut from = 0;
        while let Some(found) = text[from..].find("@after(") {
            let start = from + found;
            let end = text[start..]
                .find(')')
                .map_or(text.len(), |at| start + at + 1);
            mock.push(Diagnostic::error(start..end, AFTER_REMOVED));
            from = end;
        }
    }
    if !mock.is_empty() {
        diagnostics.clear();
        diagnostics.extend(mock);
        *analysis = AnalysisState::Invalid;
    }
}

pub(super) fn has_offer() -> bool {
    scene().offer.is_some()
}

/// The offer, drawn as the band's other notices with a button are.
pub(super) fn offer(ui: &mut egui::Ui) {
    let Some(offer) = scene().offer else {
        return;
    };
    ui.horizontal_wrapped(|ui| {
        ui.colored_label(theme::WARNING, &offer.text);
        for button in &offer.buttons {
            let _ = ui.button(button);
        }
    });
}

/// The dialog, drawn as the hand-over prompt is.
pub(super) fn dialog(context: &egui::Context) {
    let Some(dialog) = scene().dialog else {
        return;
    };
    let list_height = context.content_rect().height() * 0.42;
    dialogs::centered_modal(egui::Id::new("folder-lang21"), context).show(context, |ui| {
        ui.set_width(dialog.width);
        ui.heading(&dialog.heading);
        ui.add_space(theme::SPACE_SM);
        for text in &dialog.before {
            ui.add(egui::Label::new(text).wrap());
            ui.add_space(theme::SPACE_SM);
        }
        if !dialog.groups.is_empty() {
            egui::ScrollArea::vertical()
                .max_height(list_height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for group in &dialog.groups {
                        if !group.title.is_empty() {
                            ui.label(RichText::new(&group.title).strong());
                        }
                        for row in &group.rows {
                            ui.horizontal_top(|ui| {
                                ui.add_space(theme::SPACE_LG);
                                ui.label("•");
                                let link = row.link.as_deref().unwrap_or_default();
                                let room =
                                    ui.available_width() - if link.is_empty() { 0.0 } else { 56.0 };
                                ui.vertical(|ui| {
                                    ui.set_width(room);
                                    ui.add(egui::Label::new(&row.text).wrap());
                                    if let Some(detail) = &row.detail {
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(detail)
                                                    .size(theme::FONT_LABEL)
                                                    .color(theme::MUTED),
                                            )
                                            .wrap(),
                                        );
                                    }
                                });
                                if !link.is_empty() {
                                    let _ = ui.link(link);
                                }
                            });
                        }
                        ui.add_space(theme::SPACE_SM);
                    }
                });
        }
        for text in &dialog.after {
            ui.add(
                egui::Label::new(
                    RichText::new(text)
                        .size(theme::FONT_LABEL)
                        .color(theme::MUTED),
                )
                .wrap(),
            );
            ui.add_space(theme::SPACE_SM);
        }
        ui.add_space(theme::SPACE_SM);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for button in dialog.secondary.iter().rev() {
                    let _ = ui.button(button);
                }
                if let Some(primary) = &dialog.primary {
                    let _ = ui.add(widgets::primary_button(primary));
                }
            });
        });
    });
}
