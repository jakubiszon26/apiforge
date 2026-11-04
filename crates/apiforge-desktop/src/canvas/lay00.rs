//! LAY-00 mock-up: what the switches in `crate::mock_lay00` change on the
//! board this frame. Throwaway branch `mock/lay-00-cards`.
use super::paint::CardMock;
use crate::mock_lay00::{self as mock, Aggregate, Flags};
use crate::theme;
use apiforge_core::{block::Block, format::Graph, graph::wires, model::NodeKind, run::StepResult};
use eframe::egui::Color32;
use std::collections::{HashMap, HashSet};

pub struct Focus {
    pub selected: String,
    pub ringed: HashSet<String>,
    pub wire_color: Option<Color32>,
    pub accent_aggregate: bool,
}

pub struct Frame {
    pub flags: Flags,
    pub hidden: HashSet<String>,
    pub sequence: HashMap<String, usize>,
    pub checks_of: HashMap<String, Vec<String>>,
    pub focus: Option<Focus>,
    pub ends: HashMap<String, (String, String)>,
}

impl Frame {
    pub fn new(graph: &Graph, selection: &[String]) -> Self {
        let flags = mock::get();
        let hidden = if flags.collapsed {
            mock::folded(graph)
        } else {
            HashSet::new()
        };
        let focus =
            match selection {
                [selected] if flags.focus => graph
                    .nodes
                    .iter()
                    .find(|node| &node.id == selected)
                    .map(|node| {
                        let (producers, consumers) = mock::neighbours(graph, &node.id);
                        if mock::is_check(node.kind) {
                            Focus {
                                selected: node.id.clone(),
                                ringed: producers.into_iter().collect(),
                                wire_color: Some(theme::block_color(Block::Comparison)),
                                accent_aggregate: false,
                            }
                        } else {
                            Focus {
                                selected: node.id.clone(),
                                ringed: producers.into_iter().chain(consumers).collect(),
                                wire_color: None,
                                accent_aggregate: matches!(
                                    node.kind,
                                    NodeKind::Request | NodeKind::Component
                                ),
                            }
                        }
                    }),
                _ => None,
            };
        Self {
            hidden,
            sequence: mock::sequence(graph),
            checks_of: mock::checks_of(graph),
            focus,
            ends: wires(graph)
                .into_iter()
                .map(|wire| (wire.id, (wire.source, wire.target)))
                .collect(),
            flags,
        }
    }

    pub fn ringed(&self, id: &str) -> bool {
        self.focus
            .as_ref()
            .is_some_and(|focus| focus.ringed.contains(id))
    }

    /// Colour, alpha and width of a wire.
    pub fn wire(&self, id: &str) -> (Option<Color32>, f32, Option<f32>) {
        let Some(focus) = &self.focus else {
            return (None, 1.0, None);
        };
        let lit = self.ends.get(id).is_some_and(|(source, target)| {
            (source == &focus.selected && focus.ringed.contains(target))
                || (target == &focus.selected && focus.ringed.contains(source))
        });
        if lit {
            (focus.wire_color, 1.0, Some(2.2))
        } else {
            (None, 0.3, None)
        }
    }

    pub fn card(
        &self,
        id: &str,
        step: Option<&StepResult>,
        steps: Option<&[StepResult]>,
    ) -> CardMock {
        let aggregate = self
            .flags
            .collapsed
            .then(|| self.checks_of.get(id))
            .flatten()
            .map(|checks| {
                let aggregate = Aggregate::of(checks, steps);
                let accent = self
                    .focus
                    .as_ref()
                    .is_some_and(|focus| focus.accent_aggregate && focus.selected == id);
                let color = if accent {
                    theme::ACCENT
                } else if aggregate.failed > 0 {
                    theme::DANGER
                } else if aggregate.passed == aggregate.total {
                    theme::SUCCESS
                } else {
                    theme::MUTED
                };
                (aggregate.text(), color, aggregate.failed > 0 || accent)
            });
        let running = self.flags.running.as_deref() == Some(id);
        CardMock {
            sequence: self
                .flags
                .proposed
                .then(|| self.sequence.get(id).copied())
                .flatten(),
            aggregate,
            proposed: self.flags.proposed,
            running,
            pending: self.flags.running.is_some() && !running && step.is_none(),
            dimmed: self
                .focus
                .as_ref()
                .is_some_and(|focus| focus.selected != id && !focus.ringed.contains(id)),
        }
    }
}

/// A tinted band behind the steps of one branch of an `if`.
pub struct Band {
    pub source: String,
    pub branch: apiforge_core::model::Branch,
    pub members: HashSet<String>,
}

/// The branches of each `if`: the steps its branch edges lead to, and every
/// step that depends on them.
pub fn bands(graph: &Graph) -> Vec<Band> {
    let index = apiforge_core::graph::dependency_index(graph);
    let mut result = Vec::new();
    for node in graph.nodes.iter().filter(|node| node.kind == NodeKind::If) {
        for branch in [
            apiforge_core::model::Branch::True,
            apiforge_core::model::Branch::False,
        ] {
            let mut members: HashSet<String> = graph
                .edges
                .iter()
                .filter(|edge| edge.source == node.id && edge.branch == Some(branch))
                .map(|edge| edge.target.clone())
                .collect();
            loop {
                let more: Vec<String> = graph
                    .nodes
                    .iter()
                    .filter(|candidate| {
                        !members.contains(&candidate.id)
                            && index
                                .get(candidate.id.as_str())
                                .is_some_and(|deps| deps.iter().any(|dep| members.contains(*dep)))
                    })
                    .map(|candidate| candidate.id.clone())
                    .collect();
                if more.is_empty() {
                    break;
                }
                members.extend(more);
            }
            if !members.is_empty() {
                result.push(Band {
                    source: node.id.clone(),
                    branch,
                    members,
                });
            }
        }
    }
    result
}

impl Frame {
    pub fn bands(&self, graph: &Graph) -> Vec<Band> {
        if self.flags.bands {
            bands(graph)
        } else {
            Vec::new()
        }
    }

    /// An ordering wire from an `if` into its band: the band's one wire
    /// stands for it.
    pub fn hides_wire(&self, bands: &[Band], id: &str) -> bool {
        self.ends.get(id).is_some_and(|(source, target)| {
            bands
                .iter()
                .any(|band| &band.source == source && band.members.contains(target))
        })
    }
}

pub fn paint_bands(
    painter: &eframe::egui::Painter,
    bands: &[Band],
    cards: &[super::Card],
    camera: super::Camera,
) {
    use apiforge_core::model::Branch;
    use eframe::egui::{Align2, FontId, Stroke, StrokeKind, vec2};
    for band in bands {
        let Some(world) = cards
            .iter()
            .filter(|card| band.members.contains(&card.id))
            .map(super::Card::rect)
            .reduce(eframe::egui::Rect::union)
        else {
            continue;
        };
        let z = camera.zoom;
        let rect = camera.rect(world.expand(12.0));
        let color = match band.branch {
            Branch::True => theme::BRANCH_YES,
            Branch::False => theme::BRANCH_NO,
        };
        painter.rect(
            rect,
            14.0 * z,
            color.gamma_multiply(0.08),
            Stroke::new(1.2, color.gamma_multiply(0.55)),
            StrokeKind::Inside,
        );
        let entry = eframe::egui::pos2(rect.left(), rect.top() + (40.0 * z).max(14.0));
        painter.text(
            entry + vec2(-4.0, -3.0),
            Align2::RIGHT_BOTTOM,
            match band.branch {
                Branch::True => "Yes",
                Branch::False => "No",
            },
            FontId::new((15.0 * z).max(9.0), theme::bold_family()),
            color,
        );
        let port = match band.branch {
            Branch::True => apiforge_core::graph::TRUE_PORT,
            Branch::False => apiforge_core::graph::FALSE_PORT,
        };
        if let Some(source) = cards.iter().find(|card| card.id == band.source)
            && let Some(anchor) = source.output_anchor(port)
        {
            let start = camera.to_screen(anchor);
            // Into the band's left edge, under its label.
            let end = entry;
            super::paint::wire(painter, start, end, true, Some(band.branch), false, z);
        }
    }
}
