//! Board commands as graph edits.
//!
//! Each function takes the graph being edited with what surrounds it and returns
//! the next graph, plus the selection that should follow, or a message for the
//! board. Nothing here owns a document, so the workspace window and folder
//! projects apply the same edits through their own histories.

use crate::canvas;
use apiforge_core::{
    block::Block,
    edit,
    format::{Binding, Graph, Node, Position, Project, Proxy, ProxySource},
    graph::{FALSE_PORT, ORDER_PORT, TRUE_PORT, delete_nodes},
};
use eframe::egui;
use std::collections::{HashMap, HashSet};

/// The graph on the board and everything its cards are drawn from.
pub struct Board<'a> {
    pub graph: &'a Graph,
    /// Nodes that bindings may reference (main graph nodes on the cleanup board).
    pub reference: &'a Graph,
    pub project: &'a Project,
    pub parameters: &'a [String],
    /// The other graph of a scenario, which shares its handles: cleanup on the
    /// main board, main on the cleanup board. `None` where the caller keeps
    /// handles apart itself, or for a component.
    pub sibling: Option<&'a Graph>,
    /// The sizes the board draws its cards at (LAY-16).
    pub density: canvas::CardDensity,
    /// The board can fold its checks into their subjects (LAY-11): Tidy up
    /// of the whole board gives shape D (LAY-06), every card that is not a
    /// spine step hung under the spine step it reads last, the checks in
    /// the same places whether they are folded or not, so that showing them
    /// needs no second Tidy up.
    pub satellites: bool,
    /// The lines the board draws, which Tidy up of the whole board makes
    /// room for (LAY-03).
    pub lines: canvas::Lines,
}

/// A graph edit and the selection that should follow it.
pub struct Edit {
    pub graph: Graph,
    pub selection: Option<Vec<String>>,
}

/// A paste: the edit that adds the copies, and the references among them the
/// board could not keep, each left unconnected.
pub struct Paste {
    pub edit: Edit,
    pub unresolved: Vec<edit::Unresolved>,
}

/// How often Tidy up asks a board's lines what room they want, and how
/// often it may then widen a gap a line is still short of (LAY-03).
const ASKING_ROUNDS: usize = 4;
const ROOM_ROUNDS: usize = 24;

/// What each gap between two columns lacks for the chips and the counts
/// that stand in it (LAY-37), by the left column's place in a row, in the 20
/// px steps the gaps grow by (LAY-03).
///
/// A chip stands left of its socket and a count right of its port, each in
/// up to [`canvas::LANE`]; where lines run down the gap past it, it may
/// stand beyond them, its stub across them. So each needs a stretch of the
/// gap that wide between the lines that pass it, short of a line's gap from
/// the lane of a count or a chip facing it across the gap, level with it.
fn lane_room(
    hanging: &apiforge_layout::Hanging<'_>,
    plan: &canvas::shape::Plan,
    placed: &[apiforge_layout::Point],
    lanes: &[canvas::Lane],
    routes: &[apiforge_layout::Route],
    index: impl Fn(&str) -> Option<usize>,
) -> Vec<(usize, f32)> {
    let step = apiforge_layout::GAP;
    let per = hanging.rows.columns();
    // A stub, the room kept from a line, and half a chip's height, at 100 %.
    let (stub, clear, half) = (10.0, 4.0, 10.0);
    let wide = canvas::LANE - stub - 2.0;
    // The row and the column of each card's stack.
    let place = |node: usize| {
        let spine = plan
            .spine
            .iter()
            .position(|step| *step == node)
            .or_else(|| {
                plan.satellites
                    .iter()
                    .find(|(satellite, _)| *satellite == node)
                    .map(|(_, spine)| *spine)
            })?;
        Some((spine / per, spine % per))
    };
    // The heights of the chips, or of the counts, of each stack.
    let sides = |input: bool| -> HashMap<(usize, usize), Vec<f32>> {
        let mut sides: HashMap<(usize, usize), Vec<f32>> = HashMap::new();
        for lane in lanes.iter().filter(|lane| lane.input == input) {
            if let Some(at) = index(&lane.node).and_then(place) {
                sides.entry(at).or_default().push(lane.anchor.y);
            }
        }
        sides
    };
    let (tagged, counted) = (sides(true), sides(false));
    // A chip and a count face each other across a gap only where they are
    // level: a chip of a card hung low in a stack (LAY-31) does not face the
    // count at its neighbour's port up at the step.
    let level = |side: &HashMap<(usize, usize), Vec<f32>>, at: (usize, usize), y: f32| {
        side.get(&at).is_some_and(|heights| {
            heights
                .iter()
                .any(|other| (other - y).abs() < 2.0 * half + step)
        })
    };
    // The left and right edges of the spine step at a row and column.
    let spine = |row: usize, column: usize| {
        let step = *plan
            .spine
            .get(row * per + column)
            .filter(|_| column < per)?;
        Some((
            placed[step].x,
            placed[step].x + hanging.blocks[step].size.width,
        ))
    };
    let segments: Vec<(apiforge_layout::Point, apiforge_layout::Point)> = routes
        .iter()
        .filter(|route| route.complete)
        .flat_map(apiforge_layout::Route::segments)
        .filter(|(from, to)| (from.x - to.x).abs() < 0.01 && (from.y - to.y).abs() > 0.01)
        .collect();
    let mut short: HashMap<usize, f32> = HashMap::new();
    for lane in lanes {
        let Some(node) = index(&lane.node) else {
            continue;
        };
        let Some((row, column)) = place(node) else {
            continue;
        };
        let size = hanging.blocks[node].size;
        // The gap, and the stretch of it the chip or count may take.
        let (gap, from, to) = if lane.input {
            let Some((_, left)) = column.checked_sub(1).and_then(|before| spine(row, before))
            else {
                continue;
            };
            let facing = level(&counted, (row, column - 1), lane.anchor.y);
            let from = left + if facing { canvas::LANE + step } else { clear };
            (column - 1, from, placed[node].x - stub + clear)
        } else {
            let Some((right, _)) = spine(row, column + 1) else {
                continue;
            };
            let facing = level(&tagged, (row, column + 1), lane.anchor.y);
            let to = right - if facing { canvas::LANE + step } else { clear };
            (column, placed[node].x + size.width + stub - clear, to)
        };
        // The lines that run past it, and the widest stretch between them.
        let (top, bottom) = (lane.anchor.y - half, lane.anchor.y + half);
        let mut lines: Vec<f32> = segments
            .iter()
            .filter(|(at, _)| at.x > from - clear && at.x < to + clear)
            .filter(|(at, other)| at.y.min(other.y) < bottom && at.y.max(other.y) > top)
            .map(|(at, _)| at.x)
            .collect();
        lines.sort_by(f32::total_cmp);
        let ends = std::iter::once(from - clear)
            .chain(lines)
            .chain(std::iter::once(to + clear));
        let widest = ends
            .clone()
            .zip(ends.skip(1))
            .map(|(low, high)| high - low - 2.0 * clear)
            .fold(f32::MIN, f32::max);
        if wide - widest > 0.5 {
            let more = short.entry(gap).or_default();
            *more = more.max(wide - widest);
        }
    }
    short
        .into_iter()
        .map(|(gap, more)| (gap, (more / step).ceil() * step))
        .collect()
}

/// The space kept between pasted cards and the cards they clear.
const PASTE_GAP: f32 = 40.0;

impl From<Graph> for Edit {
    fn from(graph: Graph) -> Self {
        Self {
            graph,
            selection: None,
        }
    }
}

impl Board<'_> {
    /// Final positions after a drag. A block placed by hand is pinned: automatic
    /// layout treats it as an obstacle until the next tidy up. A proxy is always
    /// where it was put.
    pub fn moved(&self, positions: Vec<(String, Position)>) -> Graph {
        let mut graph = self.graph.clone();
        for (id, position) in positions {
            if let Some(node) = graph.nodes.iter_mut().find(|node| node.id == id) {
                node.position = position;
                node.auto_placed = false;
            } else if let Some(proxy) = graph.proxies.iter_mut().find(|proxy| proxy.id == id) {
                proxy.position = position;
            }
        }
        graph
    }

    /// Lays out the selection, or the whole board when nothing is selected.
    ///
    /// It clears the pins of everything it moves: after a tidy up the layout owns
    /// those positions again. Blocks outside the selection are pinned for the
    /// layout, so the tidied blocks keep clear of them. `None` when fewer than
    /// two blocks would move.
    pub fn tidy_up(&self, selection: &[String]) -> Option<Graph> {
        let moving = |node: &Node| selection.is_empty() || selection.contains(&node.id);
        if self.graph.nodes.iter().filter(|node| moving(node)).count() < 2 {
            return None;
        }
        // A chip goes with its first reader (LAY-02): the chips of the blocks
        // that move are placed again after them, the others stay obstacles.
        let follows = |proxy: &Proxy| {
            first_reader(self.graph, &proxy.source)
                .is_some_and(|reader| moving(&self.graph.nodes[reader.node]))
        };
        let staying = Graph {
            proxies: self
                .graph
                .proxies
                .iter()
                .filter(|proxy| !follows(proxy))
                .cloned()
                .collect(),
            ..Graph::default()
        };
        // Shape D (LAY-06) hangs every block that is not a spine step under
        // one, only when the whole board is laid out; a board without a
        // spine step hangs its foldable checks off their subjects (LAY-11).
        // The chips then follow as below.
        let mut tidied = if self.satellites && selection.is_empty() {
            self.tidy_in_rows()
                .unwrap_or_else(|| self.tidy_with_satellites(&staying.proxies))
        } else {
            let blocks: Vec<apiforge_layout::Block> = self
                .graph
                .nodes
                .iter()
                .map(|node| {
                    let block = self.card_block(node);
                    if moving(node) { block } else { block.pinned() }
                })
                // The chips come after the blocks, so a block keeps its index and the
                // laid-out positions still line up with the nodes.
                .chain(self.chip_obstacles_of(&staying.proxies, self.graph))
                .collect();
            let placed = apiforge_layout::tidy(&apiforge_layout::Graph::new(
                &blocks,
                &layout_edges(self.graph),
            ));
            let mut tidied = self.graph.clone();
            for (node, position) in tidied.nodes.iter_mut().zip(placed) {
                if !moving(node) {
                    continue;
                }
                node.position = Position {
                    x: f64::from(position.x),
                    y: f64::from(position.y),
                };
                node.auto_placed = true;
            }
            tidied
        };
        // A Tidy up of the whole board lays out every chip with its reader,
        // however far it was left (LAY-02 criterion 5). A Tidy up of a
        // selection takes along only the chips of the readers it moved.
        let moved: HashSet<String> = tidied
            .nodes
            .iter()
            .zip(&self.graph.nodes)
            .filter(|(after, before)| selection.is_empty() || after.position != before.position)
            .map(|(node, _)| node.id.clone())
            .collect();
        chips_beside_their_readers(
            &mut tidied,
            self.reference,
            self.project,
            self.parameters,
            self.density,
            |id| moved.contains(id),
        );
        Some(tidied)
    }

    /// The whole board in shape D (LAY-06): the spine in the engine's order
    /// in rows of five columns, every other block hung under the spine step
    /// it reads last ([`canvas::shape::plan`]). Folded or not, the checks get
    /// the same places, so showing them needs no second Tidy up. `None` for
    /// a board without a spine step.
    fn tidy_in_rows(&self) -> Option<Graph> {
        let plan = canvas::shape::plan(self.graph, self.sibling)?;
        let blocks: Vec<apiforge_layout::Block> = self
            .graph
            .nodes
            .iter()
            .map(|node| self.card_block(node))
            .collect();
        let hanging = apiforge_layout::Hanging {
            blocks: &blocks,
            spine: &plan.spine,
            satellites: &plan.satellites,
            rows: apiforge_layout::Rows::default(),
        };
        let laid = |placed: &[apiforge_layout::Point]| {
            let mut tidied = self.graph.clone();
            for (node, position) in tidied.nodes.iter_mut().zip(placed) {
                node.position = Position {
                    x: f64::from(position.x),
                    y: f64::from(position.y),
                };
                node.auto_placed = true;
            }
            tidied
        };
        // Folded checks keep their places and are in no line's way.
        let hidden: std::collections::BTreeSet<usize> = if self.lines.folded {
            let index = |id: &str| self.graph.nodes.iter().position(|node| node.id == id);
            canvas::collapsible(self.graph, self.sibling)
                .iter()
                .filter(|candidate| index(&candidate.anchor).is_some())
                .filter_map(|candidate| index(&candidate.check))
                .collect()
        } else {
            Default::default()
        };
        // What the router is given for the board laid out with `room`.
        let drawn = |placed: &[apiforge_layout::Point]| {
            let mut tidied = laid(placed);
            chips_beside_their_readers(
                &mut tidied,
                self.reference,
                self.project,
                self.parameters,
                self.density,
                |_| true,
            );
            canvas::resting(&canvas::Drawn {
                graph: &tidied,
                reference: self.reference,
                later: self.sibling,
                project: self.project,
                parameters: self.parameters,
                density: self.density,
                lines: self.lines,
            })
        };
        // The room the lines need (LAY-03). First what they ask for: each
        // wire routed alone takes the corridor it wants, and a corridor
        // several want is made wide enough for all of them. The routes move
        // a little as the gaps grow, so it is asked again, a few times.
        let mut room = apiforge_layout::Room::default();
        let gap = apiforge_layout::GAP;
        let alone = apiforge_layout::Routing::sizing();
        for _ in 0..ASKING_ROUNDS {
            let (placed, rows) = apiforge_layout::hang_with(&hanging, &room);
            let ((cards, wires), _) = drawn(&placed);
            // A step and the cards hung under it are one group, which the
            // lines of other cards go round (LAY-31).
            let groups = apiforge_layout::groups(&cards);
            let wanted: Vec<apiforge_layout::Route> = wires
                .iter()
                .flat_map(|wire| {
                    apiforge_layout::route_grouped(&cards, &groups, &[*wire], &alone, &[])
                })
                .collect();
            if !apiforge_layout::make_room(&hanging, &placed, &rows, &wanted, gap, &mut room) {
                break;
            }
        }
        // Then what is left: the board is routed as it will be drawn, and
        // each place where a line still runs within the gap widens the gap
        // it lies in, as does each place where a chip or a count has no room
        // beside its card clear of the lines (LAY-37). Inside a group a line
        // need not keep the gap, so a breach there widens nothing (LAY-31).
        // Gaps only grow, so this ends; the limit is for a board whose lines
        // no room separates.
        for _ in 0..ROOM_ROUNDS {
            let (placed, rows) = apiforge_layout::hang_with(&hanging, &room);
            let ((cards, wires), lanes) = drawn(&placed);
            let groups = apiforge_layout::groups(&cards);
            let routes = apiforge_layout::route_grouped(
                &cards,
                &groups,
                &wires,
                &apiforge_layout::Routing::default(),
                &[],
            );
            let found = apiforge_layout::packed_outside_groups(&routes, &cards, &groups, gap);
            let lines = !found.is_empty()
                && apiforge_layout::widen(
                    &hanging, &placed, &rows, &found, &hidden, gap, &mut room,
                );
            let chips = lane_room(&hanging, &plan, &placed, &lanes, &routes, |id| {
                self.graph.nodes.iter().position(|node| node.id == id)
            })
            .into_iter()
            .fold(false, |grew, (column, more)| {
                if room.columns.len() <= column {
                    room.columns.resize(column + 1, 0.0);
                }
                room.columns[column] += more;
                grew || more > 0.0
            });
            if !(lines || chips) {
                break;
            }
        }
        let (placed, _) = apiforge_layout::hang_with(&hanging, &room);
        Some(laid(&placed))
    }

    /// The whole board laid out with its foldable checks as satellites
    /// (LAY-11): the other cards first, as Tidy up lays out a board, with a
    /// wire to or from such a check counted as its subject's; then each
    /// check in the engine's order at the free place nearest what it reads
    /// (`layered::place`), which moves nothing laid out before it. Folded or
    /// not, the checks get the same places, so the board shown with its
    /// checks is the board Tidy up gives with them.
    fn tidy_with_satellites(&self, staying: &[Proxy]) -> Graph {
        let hung = canvas::collapsible(self.graph, self.sibling);
        let anchor: HashMap<&str, &str> = hung
            .iter()
            .map(|candidate| (candidate.check.as_str(), candidate.anchor.as_str()))
            .collect();
        let index_of = |id: &str| self.graph.nodes.iter().position(|node| node.id == id);
        let rest: Vec<usize> = (0..self.graph.nodes.len())
            .filter(|index| !anchor.contains_key(self.graph.nodes[*index].id.as_str()))
            .collect();
        let local = |index: usize| rest.iter().position(|kept| *kept == index);
        let edges: Vec<apiforge_layout::Edge> = apiforge_core::graph::wires(self.graph)
            .iter()
            .filter_map(|wire| {
                let end =
                    |id: &str| index_of(anchor.get(id).copied().unwrap_or(id)).and_then(local);
                let (source, target) = (end(&wire.source)?, end(&wire.target)?);
                (source != target).then(|| apiforge_layout::Edge::new(source, target))
            })
            .collect();
        let blocks: Vec<apiforge_layout::Block> = rest
            .iter()
            .map(|index| self.card_block(&self.graph.nodes[*index]))
            .chain(self.chip_obstacles_of(staying, self.graph))
            .collect();
        let placed = apiforge_layout::tidy(&apiforge_layout::Graph::new(&blocks, &edges));
        let mut tidied = self.graph.clone();
        for (index, position) in rest.iter().zip(placed) {
            let node = &mut tidied.nodes[*index];
            node.position = Position {
                x: f64::from(position.x),
                y: f64::from(position.y),
            };
            node.auto_placed = true;
        }
        if hung.is_empty() {
            return tidied;
        }
        // Every card laid out is an obstacle; the checks are placed in turn.
        let fresh: Vec<usize> = hung
            .iter()
            .filter_map(|candidate| index_of(&candidate.check))
            .collect();
        let blocks: Vec<apiforge_layout::Block> = tidied
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let block = self.card_block(node);
                if fresh.contains(&index) {
                    block
                } else {
                    block.pinned()
                }
            })
            .chain(self.chip_obstacles_of(staying, &tidied))
            .collect();
        let placed = apiforge_layout::place(
            &apiforge_layout::Graph::new(&blocks, &layout_edges(&tidied)),
            &fresh,
        );
        for (index, position) in fresh.iter().zip(placed) {
            let node = &mut tidied.nodes[*index];
            node.position = Position {
                x: f64::from(position.x),
                y: f64::from(position.y),
            };
            node.auto_placed = true;
        }
        tidied
    }

    pub fn add_node(
        &self,
        block: Block,
        component: Option<(String, u32)>,
        position: Position,
        source: Option<(String, String)>,
    ) -> Result<Edit, String> {
        let definition = component.as_ref().and_then(|(id, revision)| {
            self.project
                .components
                .iter()
                .find(|candidate| &candidate.id == id && candidate.revision == *revision)
        });
        let source = source
            .as_ref()
            .map(|(node_id, port)| edit::OutputRef { node_id, port });
        let added = edit::add_node(self.graph, block, position, definition, source)
            .map_err(|error| error.to_string())?;
        let mut graph = self.free_slot_for(added.graph, &added.node_id);
        if block == Block::Export
            && let Some(node) = graph.nodes.iter_mut().find(|node| node.id == added.node_id)
            && let Some(x) = self.right_of_the_flow(node.position.x)
        {
            node.position.x = x;
            node.auto_placed = true;
        }
        Ok(Edit {
            graph,
            selection: Some(vec![added.node_id]),
        })
    }

    /// *Add step* on the board's bar in the folder window (DIAG-06a): a step
    /// goes directly after the step [`edit::add_step_after`] names for the
    /// selection, in its braces, as one from that step's *Next* does; with
    /// nothing selected, and any other block, it is added as the library
    /// adds it, last.
    pub fn add_step(
        &self,
        block: Block,
        component: Option<(String, u32)>,
        position: Position,
        selection: &[String],
    ) -> Result<Edit, String> {
        let selected: Vec<&str> = selection.iter().map(String::as_str).collect();
        let anchor = (apiforge_core::chain::is_step(block.kind())
            && block.kind() != apiforge_core::model::NodeKind::Start)
            .then(|| edit::add_step_after(self.graph, &selected))
            .flatten();
        match anchor {
            Some(anchor) => {
                let near = self
                    .graph
                    .nodes
                    .iter()
                    .find(|node| node.id == anchor)
                    .map_or(position, |node| Position {
                        x: node.position.x + 340.0,
                        y: node.position.y + 20.0,
                    });
                let source = (anchor.to_owned(), ORDER_PORT.to_owned());
                self.add_node(block, component, near, Some(source))
            }
            None => self.add_node(block, component, position, None),
        }
    }

    /// A wire from a step's *Next*, or from an `if`'s *Yes* or *No*, dropped
    /// on a step of the board (DIAG-06a): the step is put at that place,
    /// with what goes with it, or the move is refused with the board's
    /// sentence. Returns the moved blocks beside the graph.
    pub fn place(
        &self,
        source: &str,
        source_port: &str,
        target: &str,
    ) -> Result<(Graph, Vec<String>), String> {
        let at = match source_port {
            TRUE_PORT => edit::Place::First(source, apiforge_core::model::Branch::True),
            FALSE_PORT => edit::Place::First(source, apiforge_core::model::Branch::False),
            _ => edit::Place::After(source),
        };
        edit::place(self.graph, self.project, target, at).map_err(|error| error.to_string())
    }

    /// An *Export* passes values out of a component, so it is a sink: the
    /// left edge one gap right of the rightmost card or chip, when `x` is not
    /// already there (UX-21). Being right of everything on the board, it
    /// overlaps nothing; the layout's free slot keeps the height it chose.
    /// `None` on a board without cards, where there is no flow to follow.
    fn right_of_the_flow(&self, x: f64) -> Option<f64> {
        /// The gap *Add step* leaves after a card, as the menu places a step
        /// 340 right of a 280 wide card.
        const GAP: f64 = 60.0;
        let cards = self
            .graph
            .nodes
            .iter()
            .map(|node| {
                canvas::Card::build(node, self.reference, self.project, self.density)
                    .rect()
                    .max
                    .x
            })
            .reduce(f32::max)?;
        let chips = self
            .chip_obstacles(self.graph)
            .iter()
            .map(|chip| chip.rect().max.x)
            .fold(cards, f32::max);
        let edge = f64::from(chips) + GAP;
        (x < edge).then_some(edge)
    }

    /// Adds a node and selects it, keeping it clear of occupied positions.
    pub fn push_node(&self, node: Node) -> Edit {
        let id = node.id.clone();
        let mut graph = self.graph.clone();
        graph.nodes.push(node);
        Edit {
            graph: self.free_slot_for(graph, &id),
            selection: Some(vec![id]),
        }
    }

    /// Adds a check on `subject` and selects it, in the braces the subject
    /// stands in (#433), keeping it clear of occupied positions.
    pub fn push_check(&self, subject: &str, check: Node) -> Edit {
        let id = check.id.clone();
        Edit {
            graph: self.free_slot_for(edit::add_check(self.graph, subject, check), &id),
            selection: Some(vec![id]),
        }
    }

    /// Sets a slot of a node: a whole input, or the binding at `path` inside it.
    /// `Ok(None)` when the node does not exist.
    pub fn set_input(
        &self,
        node: &str,
        input: &str,
        path: &[String],
        binding: Binding,
    ) -> Result<Option<Graph>, String> {
        let mut graph = self.graph.clone();
        let Some(target) = graph
            .nodes
            .iter_mut()
            .find(|candidate| candidate.id == node)
        else {
            return Ok(None);
        };
        let next = match target.inputs.get(input) {
            Some(current) => current.replace_at(path, binding),
            None => path.is_empty().then_some(binding),
        };
        let next = next.ok_or_else(|| edit::EditError::InvalidOperandPath.to_string())?;
        target.inputs.insert(input.to_owned(), next);
        Ok(Some(graph))
    }

    /// Deletes blocks, proxies and wires. One selection can hold blocks and
    /// proxies; deleting a variable takes its proxies with it.
    pub fn delete(&self, ids: &[String], wires: &[String]) -> Graph {
        let ids: HashSet<&str> = ids.iter().map(String::as_str).collect();
        let wires: HashSet<&str> = wires.iter().map(String::as_str).collect();
        edit::remove_proxies(
            &edit::remove_connections(&delete_nodes(self.graph, &ids), &wires),
            &ids,
        )
    }

    /// What deleting the blocks `ids` would do in the folder window
    /// (DIAG-06e): what goes, who would be left reading it, and whether an
    /// `if` with statements in its braces asks first. The graph beside this
    /// one (a scenario's cleanup) is read for readers too. `needs_start` is
    /// true on the main flow of a scenario.
    pub fn deletion(&self, ids: &[String], needs_start: bool) -> edit::Deletion {
        let selection: HashSet<&str> = ids.iter().map(String::as_str).collect();
        let beside: Vec<&Graph> = self.sibling.into_iter().collect();
        edit::delete_preview(self.graph, &beside, &selection, needs_start)
    }

    /// The folder window's delete, by `deletion`'s answer (DIAG-06e): its
    /// statements, the wires `wires` and the proxies among `ids`. No input of
    /// a statement that stays is changed, so call it only for a deletion
    /// without a refusal. The legacy window keeps [`Board::delete`].
    pub fn delete_checked(
        &self,
        deletion: &edit::Deletion,
        ids: &[String],
        wires: &[String],
    ) -> Graph {
        let ids: HashSet<&str> = ids.iter().map(String::as_str).collect();
        let wires: HashSet<&str> = wires.iter().map(String::as_str).collect();
        edit::remove_proxies(
            &edit::remove_connections(&edit::apply_deletion(self.graph, deletion), &wires),
            &ids,
        )
    }

    /// Places a proxy and selects it, which shows at once what it stands for and
    /// what now reads it from here.
    pub fn add_proxy(
        &self,
        source: apiforge_core::format::ProxySource,
        position: Position,
    ) -> Result<Edit, String> {
        let graph =
            edit::add_proxy(self.graph, source, position).map_err(|error| error.to_string())?;
        let placed = graph.proxies.last().map(|proxy| proxy.id.clone());
        Ok(Edit {
            graph,
            selection: Some(placed.into_iter().collect()),
        })
    }

    pub fn connect(
        &self,
        source: &str,
        source_port: &str,
        target: &str,
        target_input: &str,
        target_path: &[String],
    ) -> Result<Graph, String> {
        edit::connect(
            self.graph,
            self.project,
            source,
            source_port,
            target,
            target_input,
            target_path,
        )
        .map_err(|error| error.to_string())
    }

    /// The selected blocks as a fragment to paste, or `None` when the
    /// selection holds no block (a variable chip alone is not copied).
    pub fn copy(&self, selection: &[String]) -> Option<edit::Fragment> {
        let selected: HashSet<&str> = selection.iter().map(String::as_str).collect();
        let fragment = edit::copy_selection(self.graph, self.reference, &selected);
        (!fragment.graph.nodes.is_empty()).then_some(fragment)
    }

    /// Pastes a copied fragment onto this board (UX-34): references between
    /// the copies follow them, references to blocks this board can read (the
    /// main graph's outputs on the cleanup board) are kept, and the rest are
    /// left unconnected and returned. The copies keep their arrangement and
    /// land clear of every card and chip, where they were copied or at the
    /// first free place right of or below it.
    pub fn paste(&self, fragment: &edit::Fragment) -> Result<Paste, String> {
        let sibling: Vec<&Graph> = self.sibling.into_iter().collect();
        let target = edit::PasteTarget {
            graph: self.graph,
            visible: &[self.reference],
            reserved: &sibling,
            parameters: Some(self.parameters),
            project: self.project,
        };
        let pasted = edit::paste_selection(&target, fragment, &Position { x: 0.0, y: 0.0 })
            .map_err(|error| error.to_string())?;
        Ok(Paste {
            edit: Edit {
                graph: self.land_clear(pasted.graph, &pasted.ids),
                selection: Some(pasted.ids),
            },
            unresolved: pasted.unresolved,
        })
    }

    /// Moves the blocks `ids` together, by the first offset that leaves each
    /// of their cards clear of every other card and chip: none, then one
    /// width of the group to the right, one height down, two to the right,
    /// two down, and so on.
    fn land_clear(&self, mut graph: Graph, ids: &[String]) -> Graph {
        let moving = |node: &Node| ids.contains(&node.id);
        let rect = |node: &Node| {
            canvas::Card::build(node, self.reference, self.project, self.density).rect()
        };
        let pasted: Vec<egui::Rect> = graph
            .nodes
            .iter()
            .filter(|node| moving(node))
            .map(rect)
            .collect();
        let Some(group) = pasted.iter().copied().reduce(egui::Rect::union) else {
            return graph;
        };
        let fixed: Vec<egui::Rect> = graph
            .nodes
            .iter()
            .filter(|node| !moving(node))
            .map(rect)
            .chain(graph.proxies.iter().map(|proxy| {
                canvas::Chip::build(proxy, &graph, self.project, self.parameters).rect()
            }))
            .collect();
        let clear = |shift: egui::Vec2| {
            pasted.iter().all(|card| {
                let card = card.translate(shift).expand(PASTE_GAP / 2.0);
                fixed.iter().all(|other| !card.intersects(*other))
            })
        };
        let (across, down) = (group.width() + PASTE_GAP, group.height() + PASTE_GAP);
        // The places to the right do not overlap one another and a card or
        // chip can block at most two of them, so one of these is free.
        let steps = 2 * fixed.len() + 1;
        let shift = std::iter::once(egui::Vec2::ZERO)
            .chain((1..=steps).flat_map(|step| {
                let step = step as f32;
                [egui::vec2(across * step, 0.0), egui::vec2(0.0, down * step)]
            }))
            .find(|shift| clear(*shift))
            .unwrap_or_else(|| egui::vec2(across * (steps + 1) as f32, 0.0));
        for node in graph.nodes.iter_mut().filter(|node| moving(node)) {
            node.position.x += f64::from(shift.x);
            node.position.y += f64::from(shift.y);
        }
        graph
    }

    /// Moves a freshly added block to the nearest free slot when the position it
    /// was given is already occupied, and leaves everything else where it is.
    ///
    /// A block that keeps its requested position stays pinned, because that
    /// position is the one the user pointed at; a block the layout had to move is
    /// marked as automatically placed, so a later tidy up owns it.
    pub fn free_slot_for(&self, mut graph: Graph, node_id: &str) -> Graph {
        let Some(fresh) = graph.nodes.iter().position(|node| node.id == node_id) else {
            return graph;
        };
        let blocks: Vec<apiforge_layout::Block> = graph
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let block = self.card_block(node);
                if index == fresh {
                    block
                } else {
                    block.pinned()
                }
            })
            .chain(self.chip_obstacles(&graph))
            .collect();
        let placed = apiforge_layout::place(
            &apiforge_layout::Graph::new(&blocks, &layout_edges(&graph)),
            &[fresh],
        );
        if let Some(position) = placed.first() {
            let node = &mut graph.nodes[fresh];
            let moved = (position.x - node.position.x as f32).abs() > 0.5
                || (position.y - node.position.y as f32).abs() > 0.5;
            if moved {
                node.position = Position {
                    x: f64::from(position.x),
                    y: f64::from(position.y),
                };
                node.auto_placed = true;
            }
        }
        graph
    }

    /// Places new variables under the steps whose values they hold (LAY-25),
    /// one under the other, each moved to the nearest free slot the layout
    /// finds from there, as an added block is: the gaps between the rows
    /// stay free for the wires. Nothing else moves.
    pub fn place_under_sources(&self, mut graph: Graph, ids: &[String]) -> Graph {
        /// The space between a step and the variable under it.
        const GAP: f64 = 24.0;
        let mut bottoms: HashMap<String, f64> = HashMap::new();
        for id in ids {
            let Some(source) = graph
                .nodes
                .iter()
                .find(|node| &node.id == id)
                .and_then(|node| node.inputs.get(apiforge_core::graph::VALUE_INPUT))
                .and_then(|binding| match binding {
                    Binding::Reference { node_id, .. } => Some(node_id.clone()),
                    _ => None,
                })
                .and_then(|source| graph.nodes.iter().find(|node| node.id == source))
                .cloned()
            else {
                continue;
            };
            let card = |node: &Node| {
                canvas::Card::build(node, self.reference, self.project, self.density).rect()
            };
            let top = bottoms
                .get(&source.id)
                .copied()
                .unwrap_or_else(|| f64::from(card(&source).bottom()))
                + GAP;
            if let Some(node) = graph.nodes.iter_mut().find(|node| &node.id == id) {
                node.position = Position {
                    x: source.position.x,
                    y: top.round(),
                };
            }
            graph = self.free_slot_for(graph, id);
            if let Some(node) = graph.nodes.iter().find(|node| &node.id == id) {
                bottoms.insert(source.id.clone(), f64::from(card(node).bottom()));
            }
        }
        graph
    }

    fn card_block(&self, node: &Node) -> apiforge_layout::Block {
        let rect = canvas::Card::build(node, self.reference, self.project, self.density).rect();
        apiforge_layout::Block::new(
            apiforge_layout::Size::new(rect.width(), rect.height()),
            apiforge_layout::Point::new(node.position.x as f32, node.position.y as f32),
        )
    }

    /// The proxies of a graph as pinned obstacles, so that automatic placement
    /// keeps clear of the chips the user put on the board.
    fn chip_obstacles(&self, graph: &Graph) -> Vec<apiforge_layout::Block> {
        self.chip_obstacles_of(&graph.proxies, graph)
    }

    /// `proxies` of `graph` as pinned obstacles.
    fn chip_obstacles_of(&self, proxies: &[Proxy], graph: &Graph) -> Vec<apiforge_layout::Block> {
        proxies
            .iter()
            .map(|proxy| {
                let rect = canvas::Chip::build(proxy, graph, self.project, self.parameters).rect();
                apiforge_layout::Block::new(
                    apiforge_layout::Size::new(rect.width(), rect.height()),
                    apiforge_layout::Point::new(rect.min.x, rect.min.y),
                )
                .pinned()
            })
            .collect()
    }
}

/// The block that first reads what a proxy stands for, in document order, and
/// the input row it reads it in.
pub(crate) struct Reader {
    pub node: usize,
    pub input: String,
    pub path: Vec<String>,
}

/// The first block of `graph`, in document order, with an input that reads
/// what `source` stands for, a field of it included.
pub(crate) fn first_reader(graph: &Graph, source: &ProxySource) -> Option<Reader> {
    let reads = |leaf: &Binding| match (source, leaf) {
        (ProxySource::Variable { node_id }, Binding::Reference { node_id: read, .. }) => {
            read == node_id
        }
        (ProxySource::Parameter { name }, Binding::Parameter { name: read, .. })
        | (ProxySource::Environment { name }, Binding::Environment { name: read }) => read == name,
        _ => false,
    };
    graph.nodes.iter().enumerate().find_map(|(node, block)| {
        block.inputs.iter().find_map(|(input, binding)| {
            let mut found = None;
            binding.for_each_leaf(&mut |leaf, path| {
                if found.is_none() && reads(leaf) {
                    found = Some(path.to_vec());
                }
            });
            found.map(|path| Reader {
                node,
                input: input.clone(),
                path,
            })
        })
    })
}

/// The space a chip placed beside its reader keeps from every card and chip.
const CHIP_CLEARANCE: f32 = 12.0;
/// How far from its reader's input a chip is looked for before it stays put.
const CHIP_SEARCH: f32 = 720.0;

/// Places each proxy whose first reader `moved` says has moved beside that
/// reader (LAY-02, #168): left of the input that reads it, level with that
/// input, or at the nearest place around it clear of every card and chip.
/// A proxy that stays but now lies on a card, its reader's or another's, is
/// moved the least that clears it: beside its reader when it has one. Every
/// other proxy stays where it is and is kept clear of.
pub(crate) fn chips_beside_their_readers(
    graph: &mut Graph,
    reference: &Graph,
    project: &Project,
    parameters: &[String],
    density: canvas::CardDensity,
    moved: impl Fn(&str) -> bool,
) {
    if graph.proxies.is_empty() {
        return;
    }
    let cards: Vec<canvas::Card> = graph
        .nodes
        .iter()
        .map(|node| canvas::Card::build(node, reference, project, density))
        .collect();
    let chips: Vec<egui::Rect> = graph
        .proxies
        .iter()
        .map(|proxy| canvas::Chip::build(proxy, graph, project, parameters).rect())
        .collect();
    let on_a_card = |chip: &egui::Rect| {
        cards
            .iter()
            .any(|card| chip.intersects(card.rect()) && chip.intersect(card.rect()).area() > 0.0)
    };
    // Where each proxy that moves should go, before it is kept clear.
    let wanted: Vec<Option<egui::Pos2>> = graph
        .proxies
        .iter()
        .zip(&chips)
        .map(|(proxy, chip)| {
            let reader = first_reader(graph, &proxy.source);
            let follows = reader
                .as_ref()
                .is_some_and(|reader| moved(&graph.nodes[reader.node].id));
            if !follows && !on_a_card(chip) {
                return None;
            }
            Some(match reader {
                Some(reader) => {
                    let card = &cards[reader.node];
                    let level = card
                        .input_anchor(&reader.input, &reader.path)
                        .map_or(card.rect().center().y, |anchor| anchor.y);
                    egui::pos2(
                        card.rect().min.x - 2.0 * CHIP_CLEARANCE - chip.width(),
                        level - chip.height() / 2.0,
                    )
                }
                None => chip.min,
            })
        })
        .collect();
    if wanted.iter().all(Option::is_none) {
        return;
    }
    let mut obstacles: Vec<egui::Rect> = cards
        .iter()
        .map(canvas::Card::rect)
        .chain(
            chips
                .iter()
                .zip(&wanted)
                .filter(|(_, wanted)| wanted.is_none())
                .map(|(chip, _)| *chip),
        )
        .collect();
    // Candidate offsets on a grid, nearest first, built once for every chip.
    let step = CHIP_CLEARANCE;
    let reach = (CHIP_SEARCH / step) as i32;
    let mut offsets: Vec<(i32, i32)> = (-reach..=reach)
        .flat_map(|x| (-reach..=reach).map(move |y| (x, y)))
        .collect();
    offsets.sort_by_key(|(x, y)| (x * x + y * y, x.abs(), *y));
    for (index, wanted) in wanted.iter().enumerate() {
        let Some(wanted) = *wanted else { continue };
        let size = chips[index].size();
        let clear = |at: egui::Pos2| {
            let chip = egui::Rect::from_min_size(at, size).expand(CHIP_CLEARANCE);
            obstacles
                .iter()
                .all(|other| !chip.intersects(*other) || chip.intersect(*other).area() == 0.0)
        };
        let Some(at) = offsets
            .iter()
            .map(|(x, y)| wanted + egui::vec2(*x as f32 * step, *y as f32 * step))
            .find(|at| clear(*at))
        else {
            obstacles.push(chips[index]);
            continue;
        };
        let proxy = &mut graph.proxies[index];
        proxy.position = Position {
            x: f64::from(at.x).round(),
            y: f64::from(at.y).round(),
        };
        obstacles.push(egui::Rect::from_min_size(at, size));
    }
}

fn layout_edges(graph: &Graph) -> Vec<apiforge_layout::Edge> {
    let index_of = |id: &str| graph.nodes.iter().position(|node| node.id == id);
    apiforge_core::graph::wires(graph)
        .iter()
        .filter_map(|wire| {
            Some(apiforge_layout::Edge::new(
                index_of(&wire.source)?,
                index_of(&wire.target)?,
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_core::format::{Proxy, ProxySource};

    /// UX-21: an *Export* lands one gap right of the rightmost card or chip,
    /// so a chip beyond the cards is not covered.
    #[test]
    fn an_export_lands_right_of_the_cards_and_the_chips() {
        let project = apiforge_core::demos::shop_project();
        let mut value = Block::Value.create(Position { x: 0.0, y: 0.0 }, None);
        value.id = "value".into();
        let graph = Graph {
            nodes: vec![value],
            edges: Vec::new(),
            proxies: vec![Proxy {
                id: "chip".into(),
                source: ProxySource::Parameter { name: "id".into() },
                position: Position { x: 900.0, y: 300.0 },
            }],
        };
        let parameters = vec!["id".to_owned()];
        let board = Board {
            density: canvas::CardDensity::Full,
            satellites: false,
            lines: canvas::Lines::default(),
            sibling: None,
            graph: &graph,
            reference: &graph,
            project: &project,
            parameters: &parameters,
        };
        let chip = board.chip_obstacles(&graph)[0].rect();
        let at = Position { x: 0.0, y: 300.0 };
        let added = board.add_node(Block::Export, None, at, None).unwrap();
        let export = added.graph.nodes.last().unwrap();
        assert_eq!(export.position.x, f64::from(chip.max.x) + 60.0);
    }
}
