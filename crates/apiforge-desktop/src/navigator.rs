//! Keyboard-accessible navigation to blocks on the current board.
//!
//! The navigator only returns an identifier. Selection, camera movement and the
//! document remain responsibilities of the application and canvas respectively.

use crate::theme;
use apiforge_core::format::{Graph, Node};
use eframe::egui::{self, Key, Modifiers, RichText};

#[derive(Default)]
pub struct Navigator {
    open: bool,
    query: String,
    selected: usize,
    focus_query: bool,
}

impl Navigator {
    pub fn open(&mut self) {
        *self = Self {
            open: true,
            focus_query: true,
            ..Self::default()
        };
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn show(&mut self, context: &egui::Context, graph: &Graph) -> Option<String> {
        if !self.open {
            return None;
        }
        let mut chosen = None;
        let modal = crate::dialogs::centered_modal(egui::Id::new("find-step"), context).show(
            context,
            |ui| {
                ui.set_width(420.0);
                ui.label(
                    RichText::new("Find a step")
                        .size(18.0)
                        .family(theme::bold_family()),
                );
                let field = ui.add(
                    crate::widgets::text_edit(&mut self.query)
                        .hint_text("Search by name or block type…")
                        .desired_width(f32::INFINITY),
                );
                if self.focus_query {
                    field.request_focus();
                    self.focus_query = false;
                }
                if field.changed() {
                    self.selected = 0;
                }
                let query = self.query.trim().to_lowercase();
                let matches: Vec<&Node> = graph
                    .nodes
                    .iter()
                    .filter(|node| {
                        node.name.to_lowercase().contains(&query)
                            || node.kind.label().to_lowercase().contains(&query)
                    })
                    .collect();
                self.selected = self.selected.min(matches.len().saturating_sub(1));
                let (previous, next, accept) = ui.input_mut(|input| {
                    (
                        input.consume_key(Modifiers::NONE, Key::ArrowUp),
                        input.consume_key(Modifiers::NONE, Key::ArrowDown),
                        input.consume_key(Modifiers::NONE, Key::Enter),
                    )
                });
                if previous {
                    self.selected = self.selected.saturating_sub(1);
                }
                if next {
                    self.selected = (self.selected + 1).min(matches.len().saturating_sub(1));
                }
                if accept {
                    chosen = matches.get(self.selected).map(|node| node.id.clone());
                }
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        if matches.is_empty() {
                            ui.label(
                                RichText::new("No matching steps on this board.")
                                    .color(theme::MUTED),
                            );
                        }
                        for (index, node) in matches.iter().enumerate() {
                            let response = ui.add_sized(
                                [ui.available_width(), 32.0],
                                egui::Button::selectable(
                                    index == self.selected,
                                    format!("{}   ·   {}", node.name, node.kind.label()),
                                ),
                            );
                            if response.clicked() {
                                chosen = Some(node.id.clone());
                            }
                            if index == self.selected && (previous || next || field.changed()) {
                                response.scroll_to_me(Some(egui::Align::Center));
                            }
                        }
                    });
                ui.separator();
                ui.label(
                    RichText::new("↑ ↓ Choose   ·   Enter Open configuration   ·   Esc Close")
                        .size(11.0)
                        .color(theme::MUTED),
                );
            },
        );
        if chosen.is_some() || modal.should_close() {
            self.open = false;
        }
        chosen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        navigator: &mut Navigator,
        context: &egui::Context,
        graph: &Graph,
        events: Vec<egui::Event>,
    ) -> Option<String> {
        let mut chosen = None;
        let mut output = context.run_ui(
            egui::RawInput {
                events,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1100.0, 800.0),
                )),
                ..Default::default()
            },
            |_| {
                chosen = navigator.show(context, graph);
            },
        );
        output.textures_delta.clear();
        chosen
    }

    fn key(key: Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    #[test]
    fn typing_a_name_and_pressing_enter_opens_that_step() {
        let project = apiforge_core::demos::shop_project();
        let graph = &project.scenarios[0].graph;
        let target = graph.nodes.last().unwrap();
        let context = egui::Context::default();
        theme::apply(&context);
        let mut navigator = Navigator::default();
        navigator.open();
        frame(&mut navigator, &context, graph, vec![]);
        frame(
            &mut navigator,
            &context,
            graph,
            vec![egui::Event::Text(target.name.clone())],
        );
        assert_eq!(
            frame(&mut navigator, &context, graph, vec![key(Key::Enter)]),
            Some(target.id.clone())
        );
        assert!(!navigator.is_open());
    }

    #[test]
    fn arrow_selection_and_escape_work_without_a_pointer() {
        let project = apiforge_core::demos::shop_project();
        let graph = &project.scenarios[0].graph;
        let context = egui::Context::default();
        theme::apply(&context);
        let mut navigator = Navigator::default();
        navigator.open();
        frame(&mut navigator, &context, graph, vec![]);
        frame(&mut navigator, &context, graph, vec![key(Key::ArrowDown)]);
        assert_eq!(
            frame(&mut navigator, &context, graph, vec![key(Key::Enter)]),
            Some(graph.nodes[1].id.clone())
        );
        navigator.open();
        frame(&mut navigator, &context, graph, vec![]);
        assert_eq!(
            frame(&mut navigator, &context, graph, vec![key(Key::Escape)]),
            None
        );
        assert!(!navigator.is_open());
    }
}
