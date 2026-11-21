//! The board: an interactive view of one graph.
//!
//! The board reads the document and returns [`Command`]s; the application applies
//! them through the editor, which validates every change. Positions being dragged,
//! wires being drawn and the camera are view state only.

mod collapse;
mod edge;
mod layout;
mod menu;
mod paint;
mod readers;
mod resting;
mod routing;
pub mod shape;
mod strip;
mod trace;
mod variables;

pub use collapse::collapsible;
#[cfg(test)]
pub(crate) use edge::{DrawnEdge, Side};
pub use layout::{Card, CardDensity, Chip, parse_inline};
#[cfg(test)]
pub(crate) use paint::debug_overview_line;
#[cfg(test)]
pub(crate) use paint::{BadgeState, DrawnBadge, DrawnText, NUMBER_MIN, TextKind};
pub use readers::WireMode;
pub use resting::{Drawn, LANE, Lane, Lines, resting};
pub use routing::WireStyle;
pub use strip::Pill as InputView;

use crate::icons::Icon;
use crate::{dnd::Payload, theme, widgets};
use apiforge_core::{
    binding::data_edge_id,
    block::Block,
    edit::{self, SharedValue, proxy_binding},
    format::{Binding, Graph, Port, Position, Project, ProxySource},
    graph::{ORDER_PORT, URL_INPUT, WireKind, relations, sequence, wires},
    model::{Branch, NodeKind},
    operator::Operator,
    run::{StepResult, StepStatus},
};
use eframe::egui::{
    self, Align2, Color32, FontId, Key, Modifiers, PointerButton, Pos2, Rect, Sense, Stroke,
    StrokeKind, Vec2, vec2,
};
use layout::{BoardOverlays, HintPlacement, OverlayMeasures, RowValue};
use menu::ActionMenu;
use std::collections::{HashMap, HashSet};

/// The farthest zoom: low enough that Fit shows every card of the reference
/// boards at 960 x 540, `shop-50` needing 0.069 (LAY-09, R3). A larger board
/// is still cut at this limit.
const MIN_ZOOM: f32 = 0.05;
const MAX_ZOOM: f32 = 1.6;
/// Below this zoom, inline fields are drawn as text and edited in the inspector.
const INLINE_EDIT_ZOOM: f32 = 0.8;
const SOCKET_HIT: f32 = 10.0;
/// Why *Copy* and *Duplicate* are disabled with only chips selected (UX-34).
const CHIP_NOT_COPIED: &str =
    "Select a step to copy or duplicate. A variable chip shows a value; it is not copied.";

/// A change requested on the board.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Select(Vec<String>),
    /// Select a component's input in the INPUTS strip, or clear it (UX-31).
    /// Selecting an input selects no block.
    SelectInput(Option<String>),
    /// Open the *New input* form for a component (UX-32): *+ Add input* in
    /// the INPUTS strip.
    AddInput,
    /// Open or close the component's settings in the Inspector (UX-33):
    /// *Component settings* in the board's header row.
    ComponentSettings,
    /// Final positions after a drag.
    Move(Vec<(String, Position)>),
    Connect {
        source: String,
        source_port: String,
        target: String,
        target_input: String,
        /// Slot path inside the target input; empty for the whole input.
        target_path: Vec<String>,
    },
    AddNode {
        block: Block,
        component: Option<(String, u32)>,
        position: Position,
        source: Option<(String, String)>,
    },
    /// A step from the Library dropped on a line of a chain (DIAG-06c): it
    /// goes after the line's source, by its port (*Next*, *Yes* or *No*).
    AddOnLine {
        block: Block,
        component: Option<(String, u32)>,
        position: Position,
        after: (String, String),
    },
    /// *Move out of the branch* on a check, value, variable or export in
    /// the braces of an `if` (DIAG-06d): it is written after the `if`.
    MoveOutOfBranch(String),
    /// A block from *Add step* on the board's bar: a step goes after the
    /// selection, in its braces; with nothing selected, and for any other
    /// block, last (DIAG-06a). The legacy window adds it last.
    AddStep {
        block: Block,
        component: Option<(String, u32)>,
        position: Position,
    },
    AddOperator {
        operator: Operator,
        position: Position,
    },
    SetLiteral {
        node: String,
        input: String,
        path: Vec<String>,
        value: serde_json::Value,
    },
    Bind {
        node: String,
        input: String,
        path: Vec<String>,
        binding: Binding,
    },
    Delete {
        /// Blocks and proxies, by identifier.
        ids: Vec<String>,
        wires: Vec<String>,
    },
    /// Show a variable, parameter or environment value in another place.
    AddProxy {
        source: ProxySource,
        position: Position,
    },
    EditComponent(String),
    SaveSelectionAsComponent,
    Copy,
    Paste,
    Duplicate,
    /// Lay out the selection, or the whole board when nothing is selected.
    TidyUp(Vec<String>),
    /// Change how wires are drawn. A user setting, not a document edit.
    SetWireStyle(WireStyle),
    /// *Wires* (LAY-23): every data wire, or chips at the readers of a value
    /// many steps read. A user setting that moves no card and no camera.
    SetWires(WireMode),
    SetBridges(bool),
    /// Switch *Compact cards* (LAY-16): a user setting, which the board
    /// follows with a Tidy up.
    SetCompactCards(bool),
    /// *Checks* on the bar (LAY-11, LAY-30): fold every check, or draw
    /// every one; a user setting that moves no card and no camera, and
    /// forgets each step's own choice.
    SetChecksCollapsed(bool),
    /// The arrow at the end of a step's checks line (LAY-30): show or fold
    /// that step's checks only, against the board's setting.
    ToggleChecksHere(String),
    /// Select a step and open the Inspector's *Checks* list: a click on the
    /// aggregate in its footer (LAY-11).
    ShowChecks(String),
    /// Select a block and open the Inspector on one of its inputs: a
    /// double-click on the input's row (LAY-16). With a `path`, at the
    /// editor of that field of the input, the section holding it open:
    /// *Change source* on a kept statement (LAY-42).
    InspectInput {
        node: String,
        input: String,
        path: Option<Vec<String>>,
    },
    /// Save values in variables (LAY-25), in one edit: each read through
    /// the variable that holds it or a new one after its step. `first` is
    /// the connection that brought the proposal, made in the same edit.
    SaveValues {
        values: Vec<edit::ValueRef>,
        first: Option<Box<Command>>,
    },
    /// *Save shared values as variables* in the right-click menu: every
    /// value of `Input::shared` (LAY-25).
    SaveSharedValues,
    /// The first jump to a card out of view by its edge chip (LAY-15): the
    /// window keeps what *Back* restores beside the board, the Inspector.
    Jumped,
    /// *Back* (LAY-15): select these again, and restore what the window
    /// kept at the first jump.
    Back(Vec<String>),
}

/// Where a carried wire may land on a card, and how near it must come: within
/// `reach` of the socket on screen, or anywhere in `area`.
struct DropSlot {
    name: String,
    path: Vec<String>,
    anchor: Pos2,
    area: Option<Rect>,
    reach: f32,
}

impl DropSlot {
    fn new(name: &str, path: &[String], anchor: Pos2, reach: f32) -> Self {
        Self {
            name: name.to_owned(),
            path: path.to_vec(),
            anchor,
            area: None,
            reach,
        }
    }
}

/// What is being carried to an input: a data wire, or a value from no step
/// (`None`): a proxy's wire or a library variable.
struct Carried(Option<String>);

/// A wire carried within this distance of a compact card shows its hidden
/// inputs (decision 2 = D).
const TRAY_NEAR: f32 = 120.0;

/// What a compact card's trays show this frame: view state, which neither
/// the card's box, the routing nor Tidy up sees.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Reveal {
    inputs: bool,
    outputs: bool,
}

impl Reveal {
    fn of(
        card: &Card,
        camera: Camera,
        selected: bool,
        carried: Option<&Carried>,
        pointer: Option<Pos2>,
    ) -> Self {
        if card.compact.is_none() {
            return Self::default();
        }
        let near = carried.is_some_and(|Carried(source)| source.as_ref() != Some(&card.id))
            && pointer.is_some_and(|pointer| {
                camera
                    .rect(card.rect())
                    .expand(TRAY_NEAR * camera.zoom)
                    .contains(pointer)
            });
        Self {
            inputs: selected || near,
            outputs: selected,
        }
    }
}

/// The board command for a block library action: a block or operator added at
/// `position`, unconnected, or a variable selected. Workspace navigation has no
/// board command.
pub fn library_command(action: crate::library::Action, position: Position) -> Option<Command> {
    use crate::library::Action;
    match action {
        Action::AddNode { block, component } => Some(Command::AddNode {
            block,
            component,
            position,
            source: None,
        }),
        Action::AddOperator(operator) => Some(Command::AddOperator { operator, position }),
        Action::SelectNode(id) => Some(Command::Select(vec![id])),
        _ => None,
    }
}

/// Everything the board needs to draw one frame.
pub struct Input<'a> {
    pub graph: &'a Graph,
    /// Nodes that bindings may reference (main graph nodes on the cleanup board).
    pub reference_graph: &'a Graph,
    /// A graph whose steps may read this board's, as Cleanup reads Main: a
    /// check one of them reads is never folded (LAY-11). `None` elsewhere.
    pub later: Option<&'a Graph>,
    pub project: &'a Project,
    /// Parameters of the scenario, or arguments of the component being edited.
    pub parameters: &'a [String],
    pub selection: &'a [String],
    pub steps: Option<&'a [StepResult]>,
    /// The run's cause ends, resolved once per run (LAY-19); `None`
    /// resolves them from `steps`.
    pub ends: Option<&'a apiforge_core::run::CauseEnds>,
    /// Where a run that is on has got to (LAY-07).
    pub progress: Progress<'a>,
    pub can_paste: bool,
    /// Changing context resets the camera and transient state.
    pub context: &'a str,
    /// Whether a block library panel is beside the board, so the empty-board
    /// hint can point at it (KTLO-05b, criterion 9).
    pub library_in_view: bool,
    /// The size of a header the caller draws at the left of the board's header
    /// row (KTLO-12), which puts the steps summary beside the toolbar and drops
    /// it when there is no room; `None` keeps the summary at the left.
    pub header_leading: Option<Vec2>,
    /// The library's Variables and Operators for the *Add step* popup, when no
    /// library panel is in view (KTLO-05d).
    pub library: Option<&'a crate::library::Context<'a>>,
    /// The board is shown for reading: blocks can be selected, the camera
    /// moves, nothing is added, moved, wired or changed. An observing folder
    /// window shows it so (MCP-08a); a disabled board needs nothing more.
    pub read_only: bool,
    /// The ID of the component whose board this is: *Add step* offers
    /// *Export* and neither *Start* nor *Data condition* (UX-21, decision 12),
    /// nor the component itself (UX-22). `None` on a scenario's board.
    pub component: Option<&'a str>,
    /// A component's declared inputs, shown in the INPUTS strip under the
    /// header row (UX-31); `None` for a board without the strip.
    pub inputs: Option<&'a [Port]>,
    /// The input selected in the strip: the steps that read it are ringed.
    pub selected_input: Option<&'a str>,
    /// The name typed in the *New input* form while it is open (UX-32): the
    /// strip shows the input as being added.
    pub adding_input: Option<&'a str>,
    /// *Component settings* in the header row (UX-33); `None` on a board
    /// without it, a scenario's.
    pub settings: Option<SettingsButton>,
    /// How the window is arranged around the board, such as the folder
    /// window's mode. A change fits a camera the person has not moved again,
    /// as a resized window does; a panel opening or closing beside the
    /// board does not (UX-18).
    pub arrangement: &'a str,
    /// The graphs a value saved in a variable is read in: a scenario's Main
    /// and Cleanup, or a component's graph. `None` where the board proposes
    /// no variable (LAY-25).
    pub scenario: Option<(&'a Graph, Option<&'a Graph>)>,
    /// The values of the document worth a variable, as
    /// `edit::shared_values` lists them (LAY-25).
    pub shared: &'a [SharedValue],
    /// The board moves statements across braces (DIAG-06d): the menu of a
    /// check, value, variable or export in a branch offers *Move out of the
    /// branch*. The folder window's boards; the legacy window has no such
    /// move.
    pub moves: bool,
}

/// Where a run of the board's document has got to while it is on (LAY-07):
/// the running step is marked, the steps not reached yet are muted and show
/// their previous result as such. The default is no run on.
#[derive(Clone, Copy, Debug, Default)]
pub struct Progress<'a> {
    /// A run is on; a step with no result in it has not been reached.
    pub on: bool,
    /// The step the runner announced and has not finished (LAY-22).
    pub running: Option<&'a str>,
    /// The results of the run before, of this board's phase.
    pub previous: Option<&'a [StepResult]>,
}

/// The state of the *Component settings* button in a component board's
/// header row (UX-33, COMP-00 Q4).
#[derive(Clone, Copy, Debug)]
pub struct SettingsButton {
    /// The settings are open in the Inspector: the button is pressed.
    pub open: bool,
    /// Whether the Inspector has room beside the board; without it the
    /// button is disabled and says why.
    pub available: bool,
    /// The width the caller's header wants at the left of the row. The
    /// button keeps its label while the row has room for both, and is an
    /// icon with a tooltip when it has not.
    pub header_width: f32,
}

/// *Checks* on the board's bar (LAY-30): every check card drawn or folded.
pub const CHECKS_SWITCH: &str = "Checks";
/// The board's width from which *Checks* carries its word (LAY-30).
const CHECKS_LABEL_FROM: f32 = 1000.0;
/// The button's name, also its label when the header row has room.
pub const SETTINGS: &str = "Component settings";
const SETTINGS_HOVER: &str = "Component settings: name, description, action, inputs and outputs";
const SETTINGS_UNAVAILABLE: &str =
    "The window is too narrow for the Inspector beside the code. Widen it, or switch to Diagram.";

/// How the toolbar shows *Component settings*.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum SettingsShape {
    None,
    Icon,
    Labelled,
}

/// The width a caller's header may take at the left of the board's header row.
#[derive(Clone, Copy, Debug)]
pub struct HeaderRoom {
    /// With nothing else beside the toolbar.
    pub alone: f32,
    /// With the steps summary still shown between the header and the toolbar.
    pub with_summary: f32,
}

/// Everything the board draws: the blocks' cards and the proxies' chips.
#[derive(Clone, Copy)]
struct Shapes<'a> {
    cards: &'a [Card],
    chips: &'a [Chip],
    /// What else Fit shows whole: the chips and the counts at the cards
    /// (LAY-23).
    extents: &'a [Rect],
}

#[derive(Clone, Copy, Debug)]
struct Camera {
    zoom: f32,
    pan: Vec2,
}

impl Camera {
    fn to_screen(self, world: Pos2) -> Pos2 {
        (world.to_vec2() * self.zoom + self.pan).to_pos2()
    }

    fn to_world(self, screen: Pos2) -> Pos2 {
        ((screen.to_vec2() - self.pan) / self.zoom).to_pos2()
    }

    fn rect(self, world: Rect) -> Rect {
        Rect::from_min_max(self.to_screen(world.min), self.to_screen(world.max))
    }
}

/// Of `now`, the pan along one axis that shows a card whole in the focus,
/// what a change of the board added to `before`, the pan it needed in the
/// last frame's focus. A card the change uncovered, or left as it was, is
/// not panned: a card the person left partly outside stays so (#324).
fn newly_hidden(now: f32, before: f32) -> f32 {
    if before * now <= 0.0 {
        now
    } else if now.abs() > before.abs() {
        now - before
    } else {
        0.0
    }
}

/// How far a span on screen moves, the least, to lie inside `room`; one
/// larger than the room moves its start to the room's start.
fn least_shift(span: egui::Rangef, room: egui::Rangef) -> f32 {
    if span.span() > room.span() || span.min < room.min {
        room.min - span.min
    } else if span.max > room.max {
        room.max - span.max
    } else {
        0.0
    }
}

#[derive(Clone, Debug)]
enum Drag {
    /// Moving the selected cards and proxies; `offset` is in world units.
    Nodes { ids: Vec<String>, offset: Vec2 },
    /// Drawing a wire from an output or a proxy.
    Wire { start: WireStart, from: Pos2 },
    /// Rubber-band selection from a screen position.
    Select { from: Pos2, additive: bool },
}

/// What a dragged wire comes from.
#[derive(Clone, Debug)]
enum WireStart {
    /// An output of a block: dropping it on an input connects the two.
    Output { node: String, port: String },
    /// A proxy: dropping it on an input binds the value it stands for, exactly as
    /// dragging the variable out of the panel does.
    Proxy { binding: Binding },
}

#[derive(Clone, Debug)]
struct InlineEdit {
    node: String,
    input: String,
    path: Vec<String>,
    text: String,
    error: Option<String>,
}

pub struct Canvas {
    camera: Camera,
    context: String,
    fit_pending: bool,
    camera_moved_by_user: bool,
    reveal_pending: Option<String>,
    /// The board size the camera was last fitted to, while nothing has
    /// panned it since: only such a camera is fitted again when the board's
    /// own header settles.
    fitted_for: Option<Vec2>,
    /// The selected card last kept in view, so that a newly selected or
    /// added card is panned into view once (UX-18).
    kept_in_view: Option<String>,
    /// The last frame's focus where it lies now on screen, after a change of
    /// the board that kept the camera; `None` after one that fitted it. A
    /// change pans the selected card only for what it newly hides (#324).
    previous_focus: Option<Rect>,
    /// The window's size and arrangement in the last frame: only their
    /// change fits the board again (UX-18).
    last_window: Vec2,
    last_arrangement: String,
    /// The blocks just pasted, to show once the board is drawn (UX-34).
    pasted_pending: Vec<String>,
    /// What a wire of the order carried from a *Next* with a successor will
    /// do, shown at the pointer (DIAG-06c).
    caption: Option<String>,
    /// Where a step carried from the Library lands (DIAG-06c).
    drop_mark: Option<DropMark>,
    /// The step after the source of a wire of the order being carried from
    /// its *Next*, whose line fades (DIAG-06c).
    successor: Option<String>,
    /// The lines of the chains the last frame drew, by source and port, on
    /// the board: where a step carried from the Library lands (DIAG-06c).
    chain_lines: Vec<(String, String, Vec<Pos2>)>,
    last_size: Vec2,
    last_focus: Option<Rect>,
    last_viewport: Rect,
    drag: Option<Drag>,
    menu: Option<ActionMenu>,
    /// The choice at an input where a far value was dropped (LAY-25).
    offer: Option<variables::Offer>,
    /// The menu of a port's `+ var` (LAY-25).
    port_menu: Option<variables::PortMenu>,
    selected_wires: Vec<String>,
    /// What tracing keeps between frames (LAY-14).
    trace: trace::Memory,
    /// The edge chips' unfolded edges and *Back* (LAY-15).
    edges: edge::Memory,
    inline: Option<InlineEdit>,
    error: Option<String>,
    router: routing::Router,
    /// Routes the wires of chips brought back by pointing or selecting, apart
    /// from the others, so that the wires drawn keep their routes (LAY-23).
    revealed_router: routing::Router,
    /// Which data wires are drawn, from the user's settings (LAY-23).
    wire_mode: WireMode,
    /// The port whose count was clicked: every wire of it is drawn until Esc
    /// or a click on the board (LAY-23).
    shown_port: Option<(String, String)>,
    /// How wires are drawn, from the user's settings.
    pub wire_style: WireStyle,
    /// Whether a crossing is drawn as a bridge, from the user's settings.
    pub bridges: bool,
    /// What a request card's box holds, from the user's settings (LAY-16).
    density: CardDensity,
    /// Whether the toolbar offers *Compact cards* and *Checks*.
    pub density_switch: bool,
    /// Whether check cards with a subject are folded into its footer, from
    /// the user's settings (LAY-11).
    collapsed: bool,
    /// The steps, by handle, whose own choice differs from the board's
    /// (LAY-30): their checks hang under them while the board folds them,
    /// and fold while it draws them. The window keeps the choice per
    /// document on this computer.
    here: HashSet<String>,
    /// The cards of the last frame, in board coordinates.
    #[cfg(test)]
    cards: Vec<Card>,
    /// The lines each compact card wrote in the last frame.
    #[cfg(test)]
    lines: Vec<(String, Vec<String>)>,
    /// The blocks the last frame ringed for the selected input.
    #[cfg(test)]
    ringed: Vec<String>,
    /// What the last frame's cards wrote in their source rows: the card's
    /// name and the row's text (UX-16).
    #[cfg(test)]
    sources: Vec<(String, String)>,
    /// Each card's whole rectangle on screen in the last frame, unclipped.
    #[cfg(test)]
    on_screen: Vec<(String, Rect)>,
    /// The checks the last frame folded into a subject (LAY-11).
    #[cfg(test)]
    hidden: Vec<String>,
    /// The subjects the last frame marked red for a failed check, and each
    /// subject's checks line (LAY-11).
    #[cfg(test)]
    marked: Vec<String>,
    #[cfg(test)]
    aggregates: Vec<(String, String)>,
    /// The wires the last frame drew.
    #[cfg(test)]
    wires: Vec<DrawnWire>,
    /// The chips and the reader counts the last frame drew, and the
    /// statement it showed for a pointed chip or input (LAY-23).
    #[cfg(test)]
    tags: Vec<DrawnTag>,
    #[cfg(test)]
    counts: Vec<DrawnCount>,
    /// The `+ var` marks and the choice at a drop the last frame drew
    /// (LAY-25).
    #[cfg(test)]
    var_marks: Vec<DrawnMark>,
    #[cfg(test)]
    offer_rect: Option<Rect>,
    #[cfg(test)]
    statement: Option<String>,
    /// What the last frame gave the router: rectangles and wires.
    #[cfg(test)]
    geometry: (Vec<apiforge_layout::Rect>, Vec<apiforge_layout::Wire>),
    /// What each card drew in the last frame: its texts, band and signal
    /// border (LAY-09).
    #[cfg(test)]
    drawn: Vec<(String, paint::Drawn)>,
    /// The ports the last frame ringed for the selected step (LAY-10).
    #[cfg(test)]
    ringed_ports: Vec<(String, Pos2)>,
    /// The minimap and the zoom bar of the last frame, on screen.
    #[cfg(test)]
    chrome: (Option<Rect>, Rect),
}

/// A wire as the last frame drew it: its id, its source and target cards,
/// and the label of an `@after` redrawn from a folded check's subject.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DrawnWire {
    pub id: String,
    pub source: Option<String>,
    pub target: String,
    pub label: Option<String>,
    /// An ordering wire, which the board draws dashed.
    pub dashed: bool,
    /// A line of the chain of steps (LAY-45): an ordering wire that is no
    /// branch of an `if`.
    pub chain: bool,
    /// The route in board coordinates, empty when it was not drawn.
    pub points: Vec<Pos2>,
    /// The crossings of wires drawn before it, where it draws a bridge.
    pub bridges: usize,
    /// Where it draws them, on the board (LAY-45).
    pub bridged_at: Vec<Pos2>,
    /// A wire of a chip, drawn because its chip or input is pointed at or
    /// its reader or source is selected (LAY-23).
    pub revealed: bool,
    /// Where its label was drawn, on the board.
    pub label_box: Option<Rect>,
    /// A wire of the selected step, at full strength (LAY-10).
    pub lit: bool,
    /// Drawn at a quarter of its strength while a step is selected.
    pub dimmed: bool,
    /// The line a wire carried from its source's *Next* would replace,
    /// drawn faded while it is carried (DIAG-06c).
    pub faded: bool,
}

/// Where a step carried from the Library lands (DIAG-06c): after the source
/// of the nearest line of a chain, through its port (*Next*, *Yes* or
/// *No*), or behind the last step of the main flow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DropMark {
    Line { source: String, port: String },
    End { last: String },
}

/// A chip at a reader as the last frame drew it (LAY-23).
#[cfg(test)]
#[derive(Clone, Debug)]
pub(crate) struct DrawnTag {
    pub tag: readers::Tag,
    /// What it said at this zoom.
    pub shown: String,
    /// Its box on screen and on the board, and its text size on screen.
    pub rect: Rect,
    pub world: Rect,
    pub font: f32,
}

/// A `+ var` mark as the last frame drew it, on screen (LAY-25).
#[cfg(test)]
#[derive(Clone, Debug)]
pub(crate) struct DrawnMark {
    pub source: String,
    #[allow(dead_code)]
    pub port: String,
    pub rect: Rect,
}

/// The count at a source's port as the last frame drew it (LAY-23).
#[cfg(test)]
#[derive(Clone, Debug)]
pub(crate) struct DrawnCount {
    pub count: readers::Count,
    pub world: Rect,
}

/// The switch of one step's checks (LAY-30), as the Inspector's list offers
/// it: *Expand checks here* while the board folds checks, *Collapse checks
/// here* while it draws them; `chosen` while the step's own choice differs
/// from the board's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChecksHere {
    pub board_folds: bool,
    pub chosen: bool,
}

/// What the board does with its checks in one frame (LAY-11, LAY-30).
#[derive(Default)]
struct Folding {
    /// The checks not drawn, each with the subject it folds into and its
    /// name.
    folded: HashMap<String, (String, String)>,
    /// The checks a step shows under it while the board folds the others,
    /// each with that step and its name: their `@after` is drawn from the
    /// step, as when they are folded, and no order or branch line is drawn
    /// into them, so that opening them reroutes no line.
    hung: HashMap<String, (String, String)>,
}

impl Default for Canvas {
    fn default() -> Self {
        Self {
            camera: Camera {
                zoom: 1.0,
                pan: Vec2::ZERO,
            },
            context: String::new(),
            fit_pending: true,
            camera_moved_by_user: false,
            reveal_pending: None,
            fitted_for: None,
            kept_in_view: None,
            previous_focus: None,
            last_window: Vec2::ZERO,
            last_arrangement: String::new(),
            pasted_pending: Vec::new(),
            caption: None,
            drop_mark: None,
            successor: None,
            chain_lines: Vec::new(),
            last_size: Vec2::ZERO,
            last_focus: None,
            last_viewport: Rect::NOTHING,
            router: routing::Router::default(),
            revealed_router: routing::Router::default(),
            wire_mode: WireMode::All,
            shown_port: None,
            wire_style: WireStyle::default(),
            bridges: true,
            density: CardDensity::default(),
            density_switch: true,
            collapsed: false,
            here: HashSet::new(),
            drag: None,
            menu: None,
            offer: None,
            port_menu: None,
            selected_wires: Vec::new(),
            trace: trace::Memory::default(),
            edges: edge::Memory::default(),
            inline: None,
            error: None,
            #[cfg(test)]
            ringed: Vec::new(),
            #[cfg(test)]
            sources: Vec::new(),
            #[cfg(test)]
            on_screen: Vec::new(),
            #[cfg(test)]
            cards: Vec::new(),
            #[cfg(test)]
            lines: Vec::new(),
            #[cfg(test)]
            hidden: Vec::new(),
            #[cfg(test)]
            marked: Vec::new(),
            #[cfg(test)]
            aggregates: Vec::new(),
            #[cfg(test)]
            wires: Vec::new(),
            #[cfg(test)]
            tags: Vec::new(),
            #[cfg(test)]
            counts: Vec::new(),
            #[cfg(test)]
            #[cfg(test)]
            var_marks: Vec::new(),
            #[cfg(test)]
            offer_rect: None,
            #[cfg(test)]
            statement: None,
            #[cfg(test)]
            #[cfg(test)]
            geometry: (Vec::new(), Vec::new()),
            #[cfg(test)]
            drawn: Vec::new(),
            #[cfg(test)]
            ringed_ports: Vec::new(),
            #[cfg(test)]
            chrome: (None, Rect::NOTHING),
        }
    }
}

impl Canvas {
    #[cfg(test)]
    pub(crate) fn debug_viewport(&self) -> Rect {
        self.last_viewport
    }

    /// The board position a drop at `screen` lands on, as `handle_drop` maps it.
    #[cfg(test)]
    pub(crate) fn debug_board_position(&self, screen: Pos2) -> Position {
        let world = ((screen - self.last_viewport.min) - self.camera.pan) / self.camera.zoom;
        Position {
            x: f64::from(world.x).round(),
            y: f64::from(world.y).round(),
        }
    }

    /// Opens the *Add step* popup near the board's top-left corner, as the
    /// toolbar's button does, for tests that audit the popup.
    #[cfg(test)]
    pub(crate) fn debug_open_menu(&mut self) {
        let at = self.last_viewport.min + vec2(16.0, 48.0);
        let camera = self.screen_camera(self.last_viewport);
        self.menu = Some(ActionMenu::bar(at, camera.to_world(at)));
    }

    #[cfg(test)]
    pub(crate) fn debug_menu_open(&self) -> bool {
        self.menu.is_some()
    }

    /// Whether a board rectangle was wholly in view in the last frame.
    #[cfg(test)]
    pub(crate) fn debug_in_view(&self, world: Rect) -> bool {
        self.last_focus.is_some_and(|focus| {
            focus.contains_rect(self.screen_camera(self.last_viewport).rect(world))
        })
    }

    /// The blocks the last frame ringed for the selected input (UX-31).
    #[cfg(test)]
    pub(crate) fn debug_ringed(&self) -> Vec<String> {
        self.ringed.clone()
    }

    /// What the last frame's cards wrote in their source rows (UX-16).
    #[cfg(test)]
    pub(crate) fn debug_sources(&self) -> Vec<(String, String)> {
        self.sources.clone()
    }

    /// The camera's zoom in the last frame.
    #[cfg(test)]
    pub(crate) fn debug_zoom(&self) -> f32 {
        self.camera.zoom
    }

    /// The part of the board the camera fits and keeps cards in.
    #[cfg(test)]
    pub(crate) fn debug_focus(&self) -> Rect {
        self.last_focus.unwrap_or(Rect::NOTHING)
    }

    /// A card's whole rectangle on screen in the last frame, unclipped by
    /// the board, the panels beside it or the window.
    #[cfg(test)]
    pub(crate) fn debug_card_rect(&self, id: &str) -> Option<Rect> {
        self.on_screen
            .iter()
            .find(|(card, _)| card == id)
            .map(|(_, rect)| *rect)
    }

    /// Every card's whole rectangle on screen in the last frame.
    #[cfg(test)]
    pub(crate) fn debug_cards(&self) -> &[(String, Rect)] {
        &self.on_screen
    }

    /// A card's box on the board in the last frame.
    #[cfg(test)]
    pub(crate) fn debug_world(&self, id: &str) -> Option<Rect> {
        self.cards.iter().find(|card| card.id == id).map(Card::rect)
    }

    /// The lines a compact card wrote in the last frame.
    #[cfg(test)]
    pub(crate) fn debug_lines(&self, id: &str) -> Vec<String> {
        self.lines
            .iter()
            .find(|(card, _)| card == id)
            .map(|(_, lines)| lines.clone())
            .unwrap_or_default()
    }

    #[cfg(test)]
    fn debug_drawn(&self, id: &str) -> Option<&paint::Drawn> {
        self.drawn
            .iter()
            .find(|(card, _)| card == id)
            .map(|(_, drawn)| drawn)
    }

    /// The texts the card `id` wrote in the last frame, with their sizes on
    /// screen (LAY-09).
    #[cfg(test)]
    pub(crate) fn debug_texts(&self, id: &str) -> Vec<DrawnText> {
        self.debug_drawn(id)
            .map(|drawn| drawn.texts.clone())
            .unwrap_or_default()
    }

    /// The badge the card `id` drew in the last frame (LAY-07).
    #[cfg(test)]
    pub(crate) fn debug_badge(&self, id: &str) -> Option<DrawnBadge> {
        self.debug_drawn(id)?.badge.clone()
    }

    /// Whether the card `id` had the selection's frame in the last frame.
    #[cfg(test)]
    pub(crate) fn debug_selected(&self, id: &str) -> bool {
        self.debug_drawn(id).is_some_and(|drawn| drawn.selected)
    }

    /// Whether the card `id` was drawn in the last frame.
    #[cfg(test)]
    pub(crate) fn debug_is_drawn(&self, id: &str) -> bool {
        self.debug_drawn(id).is_some()
    }

    /// Whether the card `id` was dimmed in the last frame (LAY-10).
    #[cfg(test)]
    pub(crate) fn debug_dimmed(&self, id: &str) -> bool {
        self.debug_drawn(id).is_some_and(|drawn| drawn.dimmed)
    }

    /// The colour over the card `id` that dimmed it in the last frame.
    #[cfg(test)]
    pub(crate) fn debug_veil(&self, id: &str) -> Option<Color32> {
        self.debug_drawn(id)?.veil
    }

    /// The colour of the halo around the card `id` in the last frame.
    #[cfg(test)]
    pub(crate) fn debug_ring(&self, id: &str) -> Option<Color32> {
        self.debug_drawn(id)?.ring
    }

    /// Whether the card `id` drew the selection's handles.
    #[cfg(test)]
    pub(crate) fn debug_handles(&self, id: &str) -> bool {
        self.debug_drawn(id).is_some_and(|drawn| drawn.handles)
    }

    /// The ports ringed in the last frame: the card and the port on screen.
    #[cfg(test)]
    pub(crate) fn debug_ringed_ports(&self) -> Vec<(String, Pos2)> {
        self.ringed_ports.clone()
    }

    /// The header row of the card `id`, on screen in the last frame
    /// (LAY-15).
    #[cfg(test)]
    pub(crate) fn debug_header(&self, id: &str) -> Option<Rect> {
        let card = self.cards.iter().find(|card| card.id == id)?;
        Some(
            self.screen_camera(self.last_viewport)
                .rect(card.header_rect()),
        )
    }

    /// The minimap of the last frame, on screen, where the board has room
    /// for one.
    #[cfg(test)]
    pub(crate) fn debug_minimap(&self) -> Option<Rect> {
        self.chrome.0
    }

    /// The zoom bar of the last frame, on screen.
    #[cfg(test)]
    pub(crate) fn debug_zoom_bar(&self) -> Rect {
        self.chrome.1
    }

    /// A box on the board, on screen in the last frame.
    #[cfg(test)]
    pub(crate) fn debug_to_screen(&self, world: Rect) -> Rect {
        self.screen_camera(self.last_viewport).rect(world)
    }

    /// The colour of the card's 2 px border for a failed or blocked step.
    #[cfg(test)]
    pub(crate) fn debug_signal(&self, id: &str) -> Option<Color32> {
        self.debug_drawn(id)?.signal
    }

    /// The colour of a compact card's band.
    #[cfg(test)]
    pub(crate) fn debug_band(&self, id: &str) -> Option<Color32> {
        self.debug_drawn(id)?.band
    }

    /// The camera's pan and zoom.
    #[cfg(test)]
    pub(crate) fn debug_camera(&self) -> (Vec2, f32) {
        (self.camera.pan, self.camera.zoom)
    }

    #[cfg(test)]
    pub(crate) fn debug_compact(&self) -> bool {
        self.density == CardDensity::Compact
    }

    /// Centres the card `id` at `zoom`, as a person scrolls and zooms there.
    #[cfg(test)]
    pub(crate) fn debug_show(&mut self, id: &str, zoom: f32) {
        let (Some(rect), Some(focus)) = (self.debug_world(id), self.last_focus) else {
            return;
        };
        self.camera.zoom = zoom;
        self.camera.pan =
            (focus.center() - self.last_viewport.min) - rect.center().to_vec2() * zoom;
        self.camera_moved_by_user = true;
        self.fit_pending = false;
    }

    /// Zooms to `zoom` about the middle of the board's view, as the zoom
    /// buttons do.
    #[cfg(test)]
    pub(crate) fn debug_zoom_to(&mut self, zoom: f32) {
        let viewport = self.last_viewport;
        let center = self.last_focus.unwrap_or(viewport).center();
        self.zoom_at(viewport, center, zoom / self.camera.zoom);
        self.fit_pending = false;
    }

    /// Moves the camera by `offset`, as a person drags the board.
    #[cfg(test)]
    pub(crate) fn debug_pan(&mut self, offset: Vec2) {
        self.camera.pan += offset;
        self.camera_moved_by_user = true;
        self.fit_pending = false;
    }

    /// The row an input of the card `id` is shown on, on screen.
    #[cfg(test)]
    pub(crate) fn debug_input_row(&self, id: &str, input: &str) -> Option<Rect> {
        let card = self.cards.iter().find(|card| card.id == id)?;
        let index = card.inputs.iter().position(|row| row.name == input)?;
        let row = card.input_row(index);
        (row != Rect::NOTHING).then(|| self.screen_camera(self.last_viewport).rect(row))
    }

    /// The socket of an input of the card `id`, on screen.
    #[cfg(test)]
    pub(crate) fn debug_input_socket(&self, id: &str, input: &str) -> Option<Pos2> {
        let card = self.cards.iter().find(|card| card.id == id)?;
        let anchor = card.input_anchor(input, &[])?;
        Some(self.screen_camera(self.last_viewport).to_screen(anchor))
    }

    /// The checks the last frame folded into a subject.
    #[cfg(test)]
    pub(crate) fn debug_hidden(&self) -> &[String] {
        &self.hidden
    }

    /// The subjects the last frame marked red for a failed check.
    #[cfg(test)]
    pub(crate) fn debug_marked(&self) -> &[String] {
        &self.marked
    }

    /// The checks line of a subject in the last frame, legible or not.
    #[cfg(test)]
    pub(crate) fn debug_aggregate(&self, id: &str) -> Option<String> {
        self.aggregates
            .iter()
            .find(|(subject, _)| subject == id)
            .map(|(_, text)| text.clone())
    }

    /// The caption at the pointer while a wire of the order is carried
    /// (DIAG-06c).
    #[cfg(test)]
    pub(crate) fn debug_caption(&self) -> Option<&str> {
        self.caption.as_deref()
    }

    /// Where a step carried from the Library would land (DIAG-06c).
    #[cfg(test)]
    pub(crate) fn debug_drop_mark(&self) -> Option<&DropMark> {
        self.drop_mark.as_ref()
    }

    /// Every output socket of a card's order, on the board: *Next*, *Yes*,
    /// *No* (DIAG-06c).
    #[cfg(test)]
    pub(crate) fn debug_out_sockets(&self, id: &str) -> Vec<(String, Pos2)> {
        self.cards
            .iter()
            .find(|card| card.id == id)
            .map(|card| {
                card.order_outputs()
                    .into_iter()
                    .filter_map(|port| card.output_anchor(port).map(|at| (port.to_owned(), at)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The wires the last frame drew.
    #[cfg(test)]
    pub(crate) fn debug_wires(&self) -> &[DrawnWire] {
        &self.wires
    }

    /// The sockets of a card's order, on the board: where the line from the
    /// step before comes in and where the line to the next goes out
    /// (LAY-45).
    #[cfg(test)]
    pub(crate) fn debug_order_sockets(&self, id: &str) -> (Option<Pos2>, Option<Pos2>) {
        self.cards
            .iter()
            .find(|card| card.id == id)
            .map_or((None, None), |card| {
                (
                    card.input_anchor(ORDER_PORT, &[]),
                    card.output_anchor(ORDER_PORT),
                )
            })
    }

    /// The wires selected on the board.
    #[cfg(test)]
    pub(crate) fn debug_selected_wires(&self) -> &[String] {
        &self.selected_wires
    }

    /// The chips the last frame drew at the readers (LAY-23).
    #[cfg(test)]
    pub(crate) fn debug_tags(&self) -> &[DrawnTag] {
        &self.tags
    }

    /// The reader counts the last frame drew at the sources' ports.
    #[cfg(test)]
    pub(crate) fn debug_counts(&self) -> &[DrawnCount] {
        &self.counts
    }

    /// The `+ var` marks the last frame drew (LAY-25).
    #[cfg(test)]
    pub(crate) fn debug_var_marks(&self) -> &[DrawnMark] {
        &self.var_marks
    }

    /// The choice at a drop, on screen, while it is open (LAY-25).
    #[cfg(test)]
    pub(crate) fn debug_offer(&self) -> Option<Rect> {
        self.offer.as_ref().and(self.offer_rect)
    }

    /// The statement the last frame showed for a pointed chip or input.
    #[cfg(test)]
    pub(crate) fn debug_statement(&self) -> Option<String> {
        self.statement.clone()
    }

    /// The tray of hidden inputs of the card `id`, on the board.
    #[cfg(test)]
    pub(crate) fn debug_input_tray(&self, id: &str) -> Option<Rect> {
        self.cards
            .iter()
            .find(|card| card.id == id)
            .and_then(Card::input_tray)
    }

    /// What the last frame gave the router: the rectangles of the cards
    /// drawn, then of the chips, and the wires as ports on them.
    #[cfg(test)]
    pub(crate) fn debug_geometry(
        &self,
    ) -> &(Vec<apiforge_layout::Rect>, Vec<apiforge_layout::Wire>) {
        &self.geometry
    }

    #[cfg(test)]
    pub(crate) fn debug_collapsed(&self) -> bool {
        self.collapsed
    }

    /// Draws every check as a card, leaving the camera as it is.
    #[cfg(test)]
    pub(crate) fn debug_draw_checks(&mut self) {
        self.collapsed = false;
    }

    /// Which data wires the board draws (LAY-23).
    #[cfg(test)]
    pub(crate) fn wire_mode(&self) -> WireMode {
        self.wire_mode
    }

    /// Draws every data wire, or chips at the readers of a value many steps
    /// read. Nothing moves: the cards keep their places and the camera stays,
    /// although the chips change what Fit shows (LAY-23, criterion 8). The
    /// camera is not taken as moved by the person either, so a later
    /// selection or resize places it as it would have in the other mode.
    pub fn set_wire_mode(&mut self, mode: WireMode) {
        self.wire_mode = mode;
        self.shown_port = None;
    }

    /// Fits the camera to the board in the next frame: Tidy up ends with the
    /// whole tidied board in view, wherever the camera was (LAY-06).
    pub fn fit_after_tidy_up(&mut self) {
        self.fit_pending = true;
    }

    /// Folds check cards into their subjects, or shows them again. Nothing
    /// moves: the cards still drawn keep their places and the camera stays
    /// (LAY-11, criterion 3).
    pub fn set_collapsed(&mut self, collapsed: bool) {
        if collapsed != self.collapsed && !self.fit_pending {
            self.camera_moved_by_user = true;
        }
        self.collapsed = collapsed;
    }

    /// The steps, by handle, whose own choice differs from the board's
    /// (LAY-30): shown while the board folds checks, folded while it draws
    /// them.
    pub fn set_here(&mut self, steps: impl IntoIterator<Item = String>) {
        self.here = steps.into_iter().collect();
    }

    /// Whether the step `id` of `graph` has its own choice.
    fn chosen(&self, graph: &Graph, id: &str) -> bool {
        graph
            .nodes
            .iter()
            .find(|node| node.id == id)
            .is_some_and(|node| self.here.contains(&node.handle))
    }

    /// The switch of `step`'s own checks (LAY-30), or `None` when the board
    /// folds none of them: a gate, or a check another step reads, keeps its
    /// card whatever the switch. `later` is as in [`collapsible`].
    pub fn checks_here(
        &self,
        graph: &Graph,
        later: Option<&Graph>,
        step: &str,
    ) -> Option<ChecksHere> {
        collapsible(graph, later)
            .iter()
            .any(|candidate| candidate.subjects.iter().any(|subject| subject == step))
            .then(|| ChecksHere {
                board_folds: self.collapsed,
                chosen: self.chosen(graph, step),
            })
    }

    /// The subject a check of `graph` is folded into on this board, or
    /// `None` while its card is drawn. `later` is as in [`collapsible`].
    pub fn folded_into(&self, graph: &Graph, later: Option<&Graph>, id: &str) -> Option<String> {
        collapsible(graph, later)
            .into_iter()
            .find(|candidate| candidate.check == id)
            .filter(|candidate| {
                let chosen = candidate
                    .subjects
                    .iter()
                    .any(|subject| self.chosen(graph, subject));
                // Folded by the board unless a subject shows it; drawn by the
                // board unless a subject folds it.
                self.collapsed != chosen
            })
            .map(|candidate| candidate.anchor)
    }

    /// Takes the folded checks out of `cards`, and hangs those of a step
    /// that shows its own under it (LAY-30). Returns each folded check with
    /// the subject it folds into and its name, and each hung check with the
    /// step it hangs under and its name.
    fn fold(
        &self,
        graph: &Graph,
        later: Option<&Graph>,
        cards: &mut Vec<Card>,
        chips: &mut [Chip],
    ) -> Folding {
        let mut folding = Folding::default();
        let present: HashSet<String> = cards.iter().map(|card| card.id.clone()).collect();
        let name_of = |cards: &[Card], id: &str| {
            cards
                .iter()
                .find(|card| card.id == id)
                .map(|card| card.name.clone())
                .unwrap_or_default()
        };
        let mut hung: Vec<(String, String)> = Vec::new();
        for candidate in collapsible(graph, later) {
            if !present.contains(&candidate.check) {
                continue;
            }
            let chooses =
                |subject: &&String| present.contains(*subject) && self.chosen(graph, subject);
            if !self.collapsed {
                // A board that draws its checks folds a step's that it chose
                // to fold, into the subject it hangs under.
                if candidate.subjects.iter().any(|subject| chooses(&subject))
                    && present.contains(&candidate.anchor)
                {
                    let name = name_of(cards, &candidate.check);
                    folding
                        .folded
                        .insert(candidate.check, (candidate.anchor, name));
                }
                continue;
            }
            let host = std::iter::once(&candidate.anchor)
                .chain(&candidate.subjects)
                .find(chooses);
            match host {
                Some(host) => hung.push((host.clone(), candidate.check)),
                None if present.contains(&candidate.anchor) => {
                    let name = name_of(cards, &candidate.check);
                    folding
                        .folded
                        .insert(candidate.check, (candidate.anchor, name));
                }
                None => {}
            }
        }
        cards.retain(|card| !folding.folded.contains_key(&card.id));
        for (host, check) in &hung {
            let name = name_of(cards, check);
            folding.hung.insert(check.clone(), (host.clone(), name));
        }
        let mut hosts: Vec<(f32, f32, String)> = Vec::new();
        for (host, _) in &hung {
            if hosts.iter().any(|(_, _, known)| known == host) {
                continue;
            }
            if let Some(card) = cards.iter().find(|card| &card.id == host) {
                hosts.push((card.rect().bottom(), card.rect().left(), host.clone()));
            }
        }
        hosts.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let mut pending: HashSet<&str> = hung.iter().map(|(_, check)| check.as_str()).collect();
        for (_, _, host) in &hosts {
            let Some(rect) = cards.iter().find(|card| &card.id == host).map(Card::rect) else {
                continue;
            };
            let mine: Vec<&str> = hung
                .iter()
                .filter(|(owner, _)| owner == host)
                .map(|(_, check)| check.as_str())
                .collect();
            let height: f32 = cards
                .iter()
                .filter(|card| mine.contains(&card.id.as_str()))
                .map(|card| card.height() + collapse::HANG_GAP)
                .sum();
            // Where the checks fit in the free room under their step, with
            // the stack gap below them, nothing moves (LAY-30, criterion 8);
            // else the rows below make room (LAY-11). What is beside or
            // above stays, and the camera with it.
            let room = Rect::from_min_max(
                rect.left_bottom(),
                egui::pos2(rect.right(), rect.bottom() + height + collapse::HANG_GAP),
            );
            let fits = !cards
                .iter()
                .filter(|card| &card.id != host && !pending.contains(card.id.as_str()))
                .map(Card::rect)
                .chain(chips.iter().map(Chip::rect))
                .any(|other| other.intersects(room));
            if !fits {
                for card in cards.iter_mut() {
                    if &card.id != host
                        && !pending.contains(card.id.as_str())
                        && card.rect().top() >= rect.bottom()
                    {
                        *card = card.translated(vec2(0.0, height));
                    }
                }
                for chip in chips.iter_mut() {
                    if chip.rect().top() >= rect.bottom() {
                        *chip = chip.translated(vec2(0.0, height));
                    }
                }
            }
            let mut top = rect.bottom() + collapse::HANG_GAP;
            for check in mine {
                if let Some(card) = cards.iter_mut().find(|card| card.id == check) {
                    card.origin = egui::pos2(rect.left(), top);
                    top += card.height() + collapse::HANG_GAP;
                }
                pending.remove(check);
            }
        }
        folding
    }

    /// Switches what a request card's box holds. The camera stays where it
    /// is: the board is repacked under it (LAY-16, criterion 4).
    pub fn set_density(&mut self, density: CardDensity) {
        if density != self.density && !self.fit_pending {
            self.camera_moved_by_user = true;
        }
        self.density = density;
    }

    /// Bring an explicitly located block into view without editing its position.
    pub fn reveal(&mut self, node_id: String) {
        self.reveal_pending = Some(node_id);
    }

    /// Shows the blocks just pasted or duplicated (UX-34): a fitted board
    /// fits again, around the copies and their originals; a board the user
    /// has moved brings the copies into view only when one of them is out of
    /// it.
    pub fn show_pasted(&mut self, node_ids: Vec<String>) {
        self.pasted_pending = node_ids;
    }

    /// The camera as `(pan x, pan y, zoom)` once the user has moved it, so a
    /// document's view can be kept between sessions. A fitted camera is not
    /// worth keeping: it is fitted again.
    pub fn camera(&self) -> Option<(f32, f32, f32)> {
        self.camera_moved_by_user.then_some((
            self.camera.pan.x,
            self.camera.pan.y,
            self.camera.zoom,
        ))
    }

    /// Restores a kept camera for a context, instead of fitting the board.
    pub fn restore_camera(&mut self, context: &str, (x, y, zoom): (f32, f32, f32)) {
        self.context = context.to_owned();
        self.camera = Camera {
            zoom: zoom.clamp(MIN_ZOOM, MAX_ZOOM),
            pan: vec2(x, y),
        };
        self.fit_pending = false;
        self.camera_moved_by_user = true;
    }

    /// Shows an error banner on the board, e.g. a rejected connection.
    pub fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
    }

    /// Board position of the visible centre, for blocks added from the library.
    pub fn visible_center(&self) -> Position {
        let local_center = self
            .last_focus
            .map_or((self.last_size / 2.0).to_pos2(), |focus| {
                (focus.center() - self.last_viewport.min).to_pos2()
            });
        let center = self.camera.to_world(local_center);
        Position {
            x: f64::from(center.x - layout::WIDTH / 2.0),
            y: f64::from(center.y - 80.0),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, input: &Input<'_>) -> Vec<Command> {
        let mut commands = Vec::new();
        if self.context != input.context {
            self.context = input.context.to_owned();
            self.fit_pending = true;
            self.camera_moved_by_user = false;
            self.last_focus = None;
            self.kept_in_view = None;
            self.drag = None;
            self.menu = None;
            self.offer = None;
            self.port_menu = None;
            self.inline = None;
            self.selected_wires.clear();
            self.shown_port = None;
            self.error = None;
            self.edges_reset();
        }
        let viewport = Self::viewport(ui);
        let measures = self.measure_overlays(ui, input);
        let overlays = layout::board_overlays(viewport, &measures);
        #[cfg(test)]
        {
            self.chrome = (overlays.minimap, overlays.zoom_controls);
        }
        // The toolbar comes before the board in the keyboard order: it ends the
        // header row, which reads before the board below it (KTLO-12).
        self.toolbar(ui, viewport, &overlays, input, &mut commands);
        // The INPUTS strip reads after the header row and before the board.
        if let (Some(rect), Some(inputs)) = (overlays.strip, input.inputs) {
            let pills: Vec<strip::Pill> = inputs
                .iter()
                .map(|port| strip::Pill::of(port, &input.project.contracts))
                .collect();
            let state = strip::State {
                selected: input.selected_input,
                adding: input.adding_input,
                can_add: !input.read_only,
            };
            match strip::show(ui, rect, &pills, state) {
                Some(strip::Outcome::Select(name)) => {
                    self.menu = None;
                    self.selected_wires.clear();
                    commands.push(Command::SelectInput(name));
                }
                Some(strip::Outcome::Add) => {
                    self.menu = None;
                    self.selected_wires.clear();
                    commands.push(Command::AddInput);
                }
                None => {}
            }
        }
        let background = ui.allocate_rect(viewport, Sense::click_and_drag());
        background.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Diagram board")
        });
        let painter = ui.painter().with_clip_rect(viewport);
        painter.rect_filled(viewport, 0.0, theme::BG);

        let mut cards: Vec<Card> = input
            .graph
            .nodes
            .iter()
            .map(|node| Card::build(node, input.reference_graph, input.project, self.density))
            .collect();
        // On a board that moves statements across braces an `if` has its own
        // *Next*, and a check, value, variable or export none (DIAG-06c).
        if input.moves {
            for card in &mut cards {
                if card.kind == NodeKind::If {
                    card.chain_out = true;
                } else if card.kind.is_data() {
                    card.chain_out = false;
                    card.buttons = false;
                }
            }
        }
        #[cfg(test)]
        {
            self.sources = cards
                .iter()
                .flat_map(|card| {
                    card.inputs.iter().filter_map(|row| match &row.value {
                        RowValue::Source(text) => Some((card.name.clone(), text.clone())),
                        _ => None,
                    })
                })
                .collect();
        }
        let mut chips: Vec<Chip> = input
            .graph
            .proxies
            .iter()
            .map(|proxy| Chip::build(proxy, input.graph, input.project, input.parameters))
            .collect();
        // Folded checks are not drawn, are no obstacle and take no part in
        // the camera; their subjects' footers sum them up (LAY-11).
        let Folding { folded, hung } = self.fold(input.graph, input.later, &mut cards, &mut chips);
        // The steps whose line ends with the switch of their own checks
        // (LAY-30): those the board can fold a check into.
        let switchable: HashSet<String> = collapsible(input.graph, input.later)
            .into_iter()
            .flat_map(|candidate| candidate.subjects)
            .collect();
        // Every step's place in the engine's order, the number of its row
        // in Run results (LAY-07, Q2 = A); a pure function of the graph.
        let numbers: HashMap<&str, usize> = sequence(input.graph)
            .into_iter()
            .enumerate()
            .map(|(index, id)| (id, index + 1))
            .collect();
        // A folded check has no badge; its subjects' lines carry its number
        // (LAY-27).
        let folded_ids: HashSet<&str> = folded.keys().map(String::as_str).collect();
        let mut aggregates =
            collapse::aggregates(input.graph, input.later, input.steps, &folded_ids, &numbers);
        // A step whose one check is folded writes it (LAY-30, Q1 = A).
        for (subject, check) in collapse::singles(input.graph, input.later, &folded_ids) {
            let check = input
                .reference_graph
                .nodes
                .iter()
                .chain(&input.graph.nodes)
                .find(|node| node.id == check);
            if let (Some(aggregate), Some(check)) = (aggregates.get_mut(&subject), check) {
                aggregate.single =
                    collapse::written(input.reference_graph, input.project, check, &subject);
            }
        }
        // The steps a failure kept from running, on the card that holds it:
        // a folded check's on its subject (LAY-19).
        let resolved;
        let ends = match (input.steps, input.ends) {
            (Some(_), Some(ends)) => Some(ends),
            (Some(steps), None) => {
                resolved = apiforge_core::run::CauseEnds::of(steps);
                Some(&resolved)
            }
            (None, _) => None,
        };
        let costs = ends
            .map(|ends| {
                crate::not_run::costs(ends, |id| {
                    folded.get(id).map(|(subject, _)| subject.as_str())
                })
            })
            .unwrap_or_default();
        #[cfg(test)]
        {
            self.hidden = folded.keys().cloned().collect();
            self.hidden.sort();
            self.marked = aggregates
                .iter()
                .filter(|(_, aggregate)| aggregate.failed())
                .map(|(id, _)| id.clone())
                .collect();
            self.marked.sort();
            self.aggregates = aggregates
                .iter()
                .map(|(id, aggregate)| (id.clone(), aggregate.text()))
                .collect();
        }

        // An `@after` of a folded check, or of one a step shows under it, is
        // drawn from that step (LAY-11, LAY-30).
        let redrawn: HashMap<String, (String, String)> = folded
            .iter()
            .chain(&hung)
            .map(|(check, place)| (check.clone(), place.clone()))
            .collect();
        // Which data wires are chips is decided on the cards where they rest,
        // before a drag moves them: a card being dragged turns no wire into a
        // chip or back until it is dropped (LAY-23).
        let rest_wires = match self.wire_mode {
            WireMode::All => Vec::new(),
            WireMode::Chips => folded_wires(input.graph, &cards, &chips, &redrawn).0,
        };
        let chipped = match self.wire_mode {
            WireMode::All => HashSet::new(),
            WireMode::Chips => readers::chipped(input.graph, &cards, &rest_wires),
        };
        let rest_plan = (!chipped.is_empty())
            .then(|| readers::plan(input.graph, input.project, &cards, &rest_wires, &chipped));
        // Fit shows the chips and the counts whole, the first column's too.
        // The chain of steps is a line between the cards, drawn in both
        // modes (LAY-45): no arrow at a badge and no mark at a row's end.
        let mut extents: Vec<Rect> = Vec::new();
        if let Some((tags, counts)) = &rest_plan {
            extents.extend(readers::bounds(tags, counts));
        }

        let pasted = std::mem::take(&mut self.pasted_pending);
        if let Some(group) = cards
            .iter()
            .filter(|card| pasted.contains(&card.id))
            .map(Card::rect)
            .reduce(Rect::union)
        {
            if !self.camera_moved_by_user {
                // A fitted board stays fitted, now around the copies too.
                self.fit_pending = true;
            } else if !overlays
                .focus
                .contains_rect(self.screen_camera(viewport).rect(group))
            {
                // Centre the copies, zooming out only as far as they need.
                self.camera.zoom = self
                    .camera
                    .zoom
                    .min(overlays.focus.width() / group.width())
                    .min(overlays.focus.height() / group.height())
                    .clamp(MIN_ZOOM, MAX_ZOOM);
                self.camera.pan = (overlays.focus.center() - viewport.min)
                    - group.center().to_vec2() * self.camera.zoom;
            }
        }
        let window = ui.ctx().content_rect().size();
        let rearranged = (window - self.last_window).length() > 1.0
            || input.arrangement != self.last_arrangement;
        let board_changed = self.place_camera(
            viewport,
            overlays.focus,
            rearranged,
            shapes(&cards, &chips, &extents),
        );
        self.keep_selection_in_view(
            viewport,
            overlays.focus,
            input.selection,
            &cards,
            board_changed,
        );
        self.last_size = viewport.size();
        self.last_focus = Some(overlays.focus);
        self.last_viewport = viewport;
        self.last_window = window;
        if input.arrangement != self.last_arrangement {
            input.arrangement.clone_into(&mut self.last_arrangement);
        }
        if let Some(id) = self.reveal_pending.take()
            && let Some(card) = cards.iter().find(|card| card.id == id)
        {
            self.camera.zoom = self
                .camera
                .zoom
                .max(0.85)
                .min(overlays.focus.width() / card.rect().width())
                .min(overlays.focus.height() / card.rect().height())
                .clamp(MIN_ZOOM, MAX_ZOOM);
            let local_focus_center = overlays.focus.center() - viewport.min;
            self.camera.pan =
                local_focus_center - card.rect().center().to_vec2() * self.camera.zoom;
            // Navigation is an explicit camera action: later toolbar measurement
            // or drawer resizing must preserve the chosen card, not refit all cards.
            self.camera_moved_by_user = true;
        }
        self.handle_camera(ui, &background, viewport);
        let origin_offset = viewport.min.to_vec2();
        // Cards are laid out in world space; the camera maps them into the viewport.
        let camera = Camera {
            zoom: self.camera.zoom,
            pan: self.camera.pan + origin_offset,
        };
        #[cfg(test)]
        {
            self.on_screen = cards
                .iter()
                .map(|card| (card.id.clone(), camera.rect(card.rect())))
                .collect();
            self.cards = cards.clone();
            self.lines.clear();
            self.drawn.clear();
        }
        if let Some(Drag::Nodes { ids, offset }) = &self.drag {
            for card in cards.iter_mut().filter(|card| ids.contains(&card.id)) {
                *card = card.translated(*offset);
            }
            for chip in chips.iter_mut().filter(|chip| ids.contains(&chip.proxy.id)) {
                *chip = chip.translated(*offset);
            }
        }

        let board_painter = painter.with_clip_rect(overlays.board);
        paint::grid(&board_painter, overlays.board, camera);

        // Wires go under the cards. They are routed around the cards and chips, so
        // what is drawn is also what hit testing uses.
        let selection: HashSet<&str> = input.selection.iter().map(String::as_str).collect();
        let (board_wires, wire_labels) = folded_wires(input.graph, &cards, &chips, &redrawn);
        // A kept wire the board no longer has, after an edit, an Undo or a
        // fold, is no longer kept: it does not come back kept (LAY-14).
        let kept = self.selected_wires.len();
        self.selected_wires
            .retain(|id| board_wires.iter().any(|wire| &wire.id == id));
        if self.selected_wires.len() != kept {
            ui.ctx().request_repaint();
        }
        self.edges_follow(input.graph, input.selection);
        // One selected step rings what it is related to and dims the rest
        // (LAY-10); its wires are lit, every other wire is dimmed.
        let focus = Focus::of(input.graph, input.selection, &board_wires, &cards);
        let traced_board = trace::Board {
            graph: input.graph,
            project: input.project,
            cards: &cards,
            chips: &chips,
            wires: &board_wires,
        };
        // The related cards out of view, while a step is selected or a
        // trace is kept (LAY-15).
        let free = edge::Free {
            board: overlays.board,
            minimap: overlays.minimap,
        };
        let away = edge::away(
            &traced_board,
            input.selection,
            &self.selected_wires,
            camera,
            free,
        );
        let mut lit: HashSet<String> = focus.as_ref().map_or_else(HashSet::new, |focus| {
            board_wires
                .iter()
                .filter(|wire| focus.lit(wire, &cards, &chips))
                .map(|wire| wire.id.clone())
                .collect()
        });
        // The chips of the wires decided above, at the cards as they are drawn
        // now; a wire with no chip stays a wire (LAY-23).
        let (tags, counts) = match rest_plan {
            Some(_) if self.drag.is_some() => {
                readers::plan(input.graph, input.project, &cards, &board_wires, &chipped)
            }
            Some(plan) => plan,
            None => (Vec::new(), Vec::new()),
        };
        let tagged: HashSet<&str> = tags
            .iter()
            .flat_map(|tag| tag.wires.iter().map(String::as_str))
            .collect();
        let hover = ui
            .input(|state| state.pointer.hover_pos())
            .filter(|pointer| overlays.board.contains(*pointer));
        // A tray of hidden inputs or outputs, shown beside a selected card,
        // takes the gap: the chips and the count there give way to it, and
        // their wires are drawn anyway.
        let shown_tags: Vec<readers::Tag> = tags
            .iter()
            .filter(|tag| {
                !(selection.contains(tag.reader.as_str()) && cards[tag.card].input_tray().is_some())
            })
            .cloned()
            .collect();
        let shown_counts: Vec<&readers::Count> = counts
            .iter()
            .filter(|count| {
                !(selection.contains(count.source.as_str())
                    && cards[count.card].output_tray().is_some())
            })
            .collect();
        // The chips and the counts keep clear of the cards, their badges and
        // the lines drawn at rest (LAY-37). Those lines are routed here, before
        // they are drawn below with the same request, which the router keeps;
        // while a card is dragged they are not waited for.
        let dragging = matches!(self.drag, Some(Drag::Nodes { .. }));
        let rest_lines: Vec<Vec<Pos2>> =
            if !dragging && !(shown_tags.is_empty() && shown_counts.is_empty()) {
                let at_rest: Vec<routing::BoardWire> = board_wires
                    .iter()
                    .filter(|wire| !tagged.contains(wire.id.as_str()))
                    .cloned()
                    .collect();
                self.router
                    .routes(&routing::Request {
                        cards: &cards,
                        chips: &chips,
                        wires: &at_rest,
                        moving: &HashSet::new(),
                        style: self.wire_style,
                        bridges: self.bridges,
                        among: &[],
                    })
                    .iter()
                    .filter(|wire| wire.routed)
                    .map(|wire| wire.points.iter().map(|at| camera.to_screen(*at)).collect())
                    .collect()
            } else {
                Vec::new()
            };
        let keep: Vec<Rect> = cards
            .iter()
            .flat_map(|card| {
                let rect = camera.rect(card.rect());
                [rect, paint::badge_corner(rect, camera.zoom)]
            })
            .collect();
        let clear = readers::Clear {
            boxes: &keep,
            lines: &rest_lines,
        };
        let laid_tags = readers::lay_tags(&painter, &shown_tags, camera, &clear);
        let laid_counts = readers::lay_counts(&painter, &shown_counts, camera, &clear, &laid_tags);
        // A step's badge keeps clear of the chips and counts (LAY-07).
        let chip_boxes: Vec<Rect> = laid_tags
            .iter()
            .chain(&laid_counts)
            .map(|laid| laid.rect)
            .collect();
        // Pointing at a chip, or at the input it stands at, shows its
        // statement and draws its wire.
        let pointed_tag = hover.and_then(|pointer| {
            laid_tags
                .iter()
                .position(|laid| laid.rect.contains(pointer))
        });
        // Pointing at a count lists the fields of its port read (LAY-18).
        let pointed_count = hover.and_then(|pointer| {
            laid_counts
                .iter()
                .position(|laid| laid.rect.contains(pointer))
        });
        // An input row names every value feeding it, a drawn line its own;
        // a click keeps a line or a chip (LAY-14).
        let pointed_input =
            hover.and_then(|pointer| trace::pointed_row(&cards, &board_wires, camera, pointer));
        let pointed_wire = hover
            .filter(|pointer| {
                pointed_tag.is_none()
                    && pointed_input.is_none()
                    && self.drag.is_none()
                    && ui.ctx().layer_id_at(*pointer) == Some(ui.layer_id())
            })
            .and_then(|pointer| {
                let over = cards
                    .iter()
                    .map(|card| camera.rect(card.rect()))
                    .chain(chips.iter().map(|chip| camera.rect(chip.rect())))
                    .chain(chip_boxes.iter().copied());
                Some((self.trace.pointed(camera, pointer, over)?, pointer))
            });
        let traced = trace::Trace::of(
            &traced_board,
            pointed_wire.as_ref().map(|(wire, _)| wire.as_str()),
            pointed_input
                .as_ref()
                .map(|(reader, name, _)| (reader.as_str(), name.as_str())),
            &self.selected_wires,
        );
        lit.extend(traced.lit().cloned());
        let dimming = focus.is_some() || traced.dims();
        let list = |index: usize| {
            let text =
                readers::field_list(input.graph, input.project, input.steps, shown_counts[index]);
            (text, laid_counts[index].rect)
        };
        let statement: Option<(String, Rect)> = match (pointed_tag, pointed_count, &pointed_input) {
            (Some(index), _, _) => {
                Some((shown_tags[index].statement.clone(), laid_tags[index].rect))
            }
            (None, Some(index), _) => Some(list(index)),
            (None, None, Some((_, _, row))) => traced.pointed_statement().map(|text| (text, *row)),
            (None, None, None) => pointed_wire
                .as_ref()
                .and_then(|(_, at)| Some((traced.pointed_statement()?, trace::at_pointer(*at)))),
        };
        let revealed: HashSet<String> = tags
            .iter()
            .filter(|tag| {
                selection.contains(tag.reader.as_str())
                    || selection.contains(tag.source.as_str())
                    || self.shown_port.as_ref() == Some(&(tag.source.clone(), tag.port.clone()))
                    || tag
                        .wires
                        .iter()
                        .any(|wire| self.selected_wires.contains(wire))
                    || pointed_tag.is_some_and(|index| shown_tags[index] == **tag)
                    || pointed_wire
                        .as_ref()
                        .is_some_and(|(wire, _)| tag.wires.contains(wire))
                    || pointed_input.as_ref().is_some_and(|(reader, name, _)| {
                        &tag.reader == reader && &tag.input == name
                    })
            })
            .flat_map(|tag| tag.wires.iter().cloned())
            .collect();
        let (revealed_wires, board_wires): (Vec<routing::BoardWire>, Vec<routing::BoardWire>) =
            board_wires
                .into_iter()
                .filter(|wire| !tagged.contains(wire.id.as_str()) || revealed.contains(&wire.id))
                .partition(|wire| tagged.contains(wire.id.as_str()));
        let (mut related_blocks, related_wires) = highlights(input.graph, &chips, &selection);
        // The input selected in the strip has no shape on the board: the
        // steps that read it are ringed (UX-31).
        let ringed = input
            .selected_input
            .map_or_else(HashSet::new, |name| input_readers(input.graph, name));
        #[cfg(test)]
        {
            self.ringed = ringed.iter().cloned().collect();
            self.ringed.sort();
        }
        related_blocks.extend(ringed);
        let moving: HashSet<String> = match &self.drag {
            Some(Drag::Nodes { ids, .. }) => ids.iter().cloned().collect(),
            _ => HashSet::new(),
        };
        let selected_wires: HashSet<String> = self.selected_wires.iter().cloned().collect();
        // A wire of the order carried from a *Next* that has a successor
        // fades the line it would replace (DIAG-06c).
        let fading: Option<(String, String)> = match &self.drag {
            Some(Drag::Wire {
                start: WireStart::Output { node, port },
                ..
            }) if input.moves && port == ORDER_PORT => board_wires
                .iter()
                .find(|wire| {
                    wire.control
                        && wire.branch.is_none()
                        && matches!(&wire.start, routing::Start::Output { card, .. } if cards[*card].id == *node)
                })
                .map(|wire| (wire.id.clone(), cards[wire.target].id.clone())),
            _ => None,
        };
        self.successor = fading.as_ref().map(|(_, next)| next.clone());
        let faded = |id: &str| fading.as_ref().is_some_and(|(line, _)| line == id);
        let mut drawn: Vec<(String, Vec<Pos2>)> = Vec::with_capacity(board_wires.len());
        // The lines at rest, which the wires brought back keep clear of.
        let mut at_rest: Vec<Vec<Pos2>> = Vec::new();
        #[cfg(test)]
        let mut bridged: HashMap<String, Vec<Pos2>> = HashMap::new();
        {
            let request = routing::Request {
                cards: &cards,
                chips: &chips,
                wires: &board_wires,
                moving: &moving,
                style: self.wire_style,
                bridges: self.bridges,
                among: &[],
            };
            #[cfg(test)]
            {
                self.geometry = routing::geometry(&request);
            }
            let routed = self.router.routes(&request);
            for wire in routed {
                if wire.routed {
                    at_rest.push(wire.points.clone());
                }
                paint::routed_wire(
                    &board_painter,
                    wire,
                    camera,
                    selected_wires.contains(&wire.id)
                        || related_wires.contains(&wire.id)
                        || lit.contains(&wire.id),
                    (dimming && !lit.contains(&wire.id)) || faded(&wire.id),
                );
                if let Some(text) = wire_labels.get(&wire.id) {
                    paint::wire_label(&board_painter, wire, camera, text);
                }
                drawn.push((wire.id.clone(), wire.points.clone()));
                #[cfg(test)]
                bridged.insert(
                    wire.id.clone(),
                    wire.bridges.iter().map(|(at, _)| *at).collect(),
                );
            }
        }
        // The lines of the chains as drawn, where a step carried from the
        // Library lands (DIAG-06c): after a line's source, by its port.
        self.chain_lines = board_wires
            .iter()
            .filter(|wire| wire.control)
            .filter_map(|wire| {
                let routing::Start::Output { card, .. } = &wire.start else {
                    return None;
                };
                let port = match wire.branch {
                    Some(apiforge_core::model::Branch::True) => apiforge_core::graph::TRUE_PORT,
                    Some(apiforge_core::model::Branch::False) => apiforge_core::graph::FALSE_PORT,
                    None => ORDER_PORT,
                };
                let points = drawn
                    .iter()
                    .find(|(id, _)| *id == wire.id)
                    .map(|(_, points)| points.clone())?;
                Some((cards[*card].id.clone(), port.to_owned(), points))
            })
            .collect();
        // The wires of chips brought back, over the others and apart from
        // them: their routes leave the routes drawn as they were and keep
        // the gap from them (LAY-03).
        let mut drawn_revealed: Vec<(String, Vec<Pos2>)> = Vec::new();
        #[cfg(test)]
        let mut label_boxes: HashMap<String, Rect> = HashMap::new();
        if !revealed_wires.is_empty() {
            let request = routing::Request {
                cards: &cards,
                chips: &chips,
                wires: &revealed_wires,
                moving: &moving,
                style: self.wire_style,
                bridges: self.bridges,
                among: &at_rest,
            };
            let routed = self.revealed_router.routes(&request).to_vec();
            for wire in &routed {
                paint::routed_wire(
                    &board_painter,
                    wire,
                    camera,
                    true,
                    dimming && !lit.contains(&wire.id),
                );
                drawn_revealed.push((wire.id.clone(), wire.points.clone()));
                #[cfg(test)]
                bridged.insert(
                    wire.id.clone(),
                    wire.bridges.iter().map(|(at, _)| *at).collect(),
                );
            }
            // A rail brought back from a folded check says the check, where
            // no other line drawn passes through the words (LAY-24).
            for wire in &routed {
                let Some(text) = wire_labels.get(&wire.id) else {
                    continue;
                };
                let others: Vec<Vec<Pos2>> = drawn
                    .iter()
                    .chain(&drawn_revealed)
                    .filter(|(id, _)| *id != wire.id)
                    .map(|(_, points)| points.iter().map(|at| camera.to_screen(*at)).collect())
                    .collect();
                let _label = paint::wire_label_clear(&board_painter, wire, camera, text, &others);
                #[cfg(test)]
                if let Some(label) = _label {
                    label_boxes.insert(
                        wire.id.clone(),
                        Rect::from_min_max(camera.to_world(label.min), camera.to_world(label.max)),
                    );
                }
            }
        }
        // Where a wire of the selected step meets a related card, its port
        // is ringed in the card's colour; painted over the cards below.
        let mut port_rings: Vec<(String, Pos2, Color32)> = Vec::new();
        if let Some(focus) = &focus {
            for (id, points) in drawn.iter().chain(&drawn_revealed) {
                let Some(wire) = board_wires
                    .iter()
                    .chain(&revealed_wires)
                    .find(|wire| &wire.id == id)
                else {
                    continue;
                };
                if !focus.lit(wire, &cards, &chips) {
                    continue;
                }
                let target = &cards[wire.target].id;
                let (other, at) = if *target == focus.selected {
                    match &wire.start {
                        routing::Start::Output { card, .. } => (&cards[*card].id, points.first()),
                        routing::Start::Chip(_) => continue,
                    }
                } else {
                    (target, points.last())
                };
                if let (Some(color), Some(at)) = (focus.rings.get(other), at) {
                    port_rings.push((other.clone(), camera.to_screen(*at), *color));
                }
            }
        }
        port_rings.extend(traced.rings(drawn.iter().chain(&drawn_revealed), camera));
        self.trace
            .remember(&traced, drawn.iter().chain(&drawn_revealed));
        for (tag, laid) in shown_tags.iter().zip(&laid_tags) {
            let lit = tag.wires.iter().all(|wire| revealed.contains(wire));
            readers::paint(&board_painter, laid, lit);
        }
        for (index, (count, laid)) in shown_counts.iter().zip(&laid_counts).enumerate() {
            let lit = count.wires.iter().any(|wire| revealed.contains(wire))
                || pointed_count == Some(index);
            readers::paint(&board_painter, laid, lit);
        }
        #[cfg(test)]
        {
            self.tags = shown_tags
                .iter()
                .zip(&laid_tags)
                .map(|(tag, laid)| DrawnTag {
                    tag: tag.clone(),
                    shown: laid.shown.clone(),
                    rect: laid.rect,
                    world: Rect::from_min_max(
                        camera.to_world(laid.rect.min),
                        camera.to_world(laid.rect.max),
                    ),
                    font: laid.font,
                })
                .collect();
            self.counts = shown_counts
                .iter()
                .zip(&laid_counts)
                .map(|(count, laid)| DrawnCount {
                    count: (*count).clone(),
                    world: Rect::from_min_max(
                        camera.to_world(laid.rect.min),
                        camera.to_world(laid.rect.max),
                    ),
                })
                .collect();
            self.statement = statement.as_ref().map(|(text, _)| text.clone());
        }
        #[cfg(test)]
        {
            self.wires = board_wires
                .iter()
                .chain(&revealed_wires)
                .map(|wire| DrawnWire {
                    id: wire.id.clone(),
                    source: match &wire.start {
                        routing::Start::Output { card, .. } => Some(cards[*card].id.clone()),
                        routing::Start::Chip(_) => None,
                    },
                    target: cards[wire.target].id.clone(),
                    label: wire_labels.get(&wire.id).cloned(),
                    dashed: wire.control,
                    chain: wire.control && wire.branch.is_none(),
                    points: drawn
                        .iter()
                        .chain(&drawn_revealed)
                        .find(|(id, _)| *id == wire.id)
                        .map(|(_, points)| points.clone())
                        .unwrap_or_default(),
                    bridges: bridged.get(&wire.id).map_or(0, Vec::len),
                    bridged_at: bridged.get(&wire.id).cloned().unwrap_or_default(),
                    revealed: revealed.contains(&wire.id),
                    label_box: label_boxes.get(&wire.id).copied(),
                    lit: lit.contains(&wire.id),
                    dimmed: dimming && !lit.contains(&wire.id),
                    faded: faded(&wire.id),
                })
                .collect();
        }

        let pointer = ui.input(|state| state.pointer.interact_pos());
        let modifiers = ui.input(|state| state.modifiers);
        let mut clicked_card = false;
        let carried = self.carried_value(ui.ctx());

        let mut board_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(overlays.board)
                .layout(*ui.layout()),
        );
        board_ui.set_clip_rect(overlays.board);

        // The switch of a step's own checks, on screen: whether it shows
        // them, and whether it is pointed at (LAY-30).
        let mut switches: Vec<(Rect, bool, bool)> = Vec::new();
        for card in &cards {
            let rect = camera.rect(card.rect());
            if !rect.intersects(overlays.board) {
                continue;
            }
            let clipped_card_rect = rect.intersect(overlays.board);
            let id = board_ui.id().with(("card", &card.id));
            let mut response = board_ui.interact(clipped_card_rect, id, Sense::click_and_drag());
            let title_rect = camera.rect(Rect::from_min_max(
                card.origin + vec2(56.0, 8.0),
                card.origin + vec2(layout::WIDTH - 48.0, 60.0),
            ));
            // A satellite's one line may be cut short: its tooltip has it whole.
            let line = card.compact.as_ref().and_then(|compact| {
                let row = compact
                    .rows
                    .iter()
                    .find(|row| row.role == layout::CompactRole::Line)?;
                let area = camera.rect(Rect::from_min_size(
                    card.origin + vec2(0.0, row.top),
                    vec2(layout::WIDTH, row.height),
                ));
                Some((area, compact.hover.clone()?))
            });
            if layout::CardDetail::at_zoom(camera.zoom) != layout::CardDetail::Full
                || pointer.is_some_and(|pos| title_rect.contains(pos))
                || pointer
                    .is_some_and(|pos| line.as_ref().is_some_and(|(area, _)| area.contains(pos)))
            {
                response = response.on_hover_text(match &line {
                    Some((_, line)) => format!("{}\n{}\n{line}", card.name, card.caption),
                    None => format!("{}\n{}", card.name, card.caption),
                });
            }
            let a11y_label = format!("{} ({})", card.name, card.caption);
            response.widget_info(move || {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &a11y_label)
            });
            if response.clicked() {
                clicked_card = true;
                self.menu = None;
                self.selected_wires.clear();
                commands.push(Command::Select(toggle_selection(
                    input.selection,
                    &card.id,
                    modifiers,
                )));
            }
            // A compact card has no field: a double-click on an input's row
            // opens the Inspector on that input (LAY-16, Q16 = A), on a
            // satellite's line its first input; elsewhere on a component call
            // it opens the component.
            let input_row = (card.compact.is_some())
                .then(|| {
                    let pointer = pointer?;
                    (0..card.inputs.len())
                        .find(|index| camera.rect(card.input_row(*index)).contains(pointer))
                        .or_else(|| {
                            let (area, _) = line.as_ref()?;
                            (area.contains(pointer) && !card.inputs.is_empty()).then_some(0)
                        })
                })
                .flatten();
            if response.double_clicked() {
                if let Some(index) = input_row {
                    commands.push(Command::InspectInput {
                        node: card.id.clone(),
                        input: card.inputs[index].name.clone(),
                        path: None,
                    });
                } else if card.kind == NodeKind::Component {
                    commands.push(Command::EditComponent(card.id.clone()));
                }
            }
            if response.secondary_clicked() && !input.read_only {
                let source = menu::default_source(input.graph, input.project, &card.id);
                let mut menu = ActionMenu::new(
                    pointer.unwrap_or(rect.center()),
                    camera.to_world(pointer.unwrap_or(rect.center())),
                    source,
                );
                if input.moves && menu::in_a_branch(input.graph, &card.id) {
                    menu.out_of_branch = Some(card.id.clone());
                }
                self.menu = Some(menu);
            }
            if response.drag_started_by(PointerButton::Primary) && !input.read_only {
                let ids = if selection.contains(card.id.as_str()) {
                    input.selection.to_vec()
                } else {
                    // As a click on it does, a new selection ends a kept
                    // wire (LAY-14).
                    self.selected_wires.clear();
                    commands.push(Command::Select(vec![card.id.clone()]));
                    vec![card.id.clone()]
                };
                // A step takes the cards hung under it along (LAY-31).
                let placed: Vec<Card> = input
                    .graph
                    .nodes
                    .iter()
                    .map(|node| {
                        Card::build(node, input.reference_graph, input.project, self.density)
                    })
                    .collect();
                let ids = with_hung(ids, &placed);
                self.drag = Some(Drag::Nodes {
                    ids,
                    offset: Vec2::ZERO,
                });
            }
            if response.dragged_by(PointerButton::Primary)
                && let Some(Drag::Nodes { offset, .. }) = &mut self.drag
            {
                *offset += response.drag_delta() / camera.zoom;
            }
            if response.drag_stopped()
                && let Some(Drag::Nodes { ids, offset }) = self.drag.take()
                && offset.length() > 0.0
            {
                commands.push(Command::Move(moved_positions(input.graph, &ids, offset)));
            }

            let step = input
                .steps
                .and_then(|steps| steps.iter().find(|step| step.node_id == card.id));
            let run = paint::CardRun {
                step,
                number: numbers.get(card.id.as_str()).copied(),
                running: input.progress.on && input.progress.running == Some(card.id.as_str()),
                pending: input.progress.on,
                previous: input
                    .progress
                    .previous
                    .and_then(|steps| steps.iter().find(|step| step.node_id == card.id)),
                clear_of: &chip_boxes,
                // A Value or an IF card keeps its line (LAY-19, 3 A).
                ends: ends.filter(|_| !matches!(card.kind, NodeKind::Value | NodeKind::If)),
                cost: costs
                    .get(card.id.as_str())
                    .copied()
                    .filter(|_| !matches!(card.kind, NodeKind::Value | NodeKind::If)),
            };
            let reveal = Reveal::of(
                card,
                camera,
                selection.contains(card.id.as_str()),
                carried.as_ref(),
                pointer,
            );
            // The checks line sums up the step's checks; a click on it opens
            // their list in the Inspector (LAY-11). It ends with the switch
            // of the step's own checks (LAY-30).
            let aggregate = aggregates
                .get(&card.id)
                .filter(|aggregate| aggregate.checks + aggregate.gates > 0);
            let switch = card
                .checks_switch(camera.zoom)
                .filter(|_| aggregate.is_some() && switchable.contains(&card.id))
                .map(|rect| camera.rect(rect));
            if let (Some(aggregate), Some(area)) = (aggregate, card.aggregate_area(camera.zoom)) {
                let mut area = camera.rect(area);
                if let Some(switch) = switch {
                    area.max.x = switch.left() - 2.0 * camera.zoom;
                }
                if area.intersects(overlays.board) {
                    let name = format!("Checks of {}", card.name);
                    let whole = aggregate.text();
                    let response = board_ui
                        .interact(
                            area.intersect(overlays.board),
                            board_ui.id().with(("aggregate", &card.id)),
                            Sense::click(),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        // The line whole, however much of it the row shows.
                        .on_hover_ui(|ui| {
                            ui.label(&whole);
                            ui.label(
                                egui::RichText::new("Click to list the checks in the Inspector")
                                    .color(theme::MUTED),
                            );
                        });
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name)
                    });
                    if response.clicked() {
                        clicked_card = true;
                        self.menu = None;
                        self.selected_wires.clear();
                        commands.push(Command::ShowChecks(card.id.clone()));
                    }
                }
            }
            if let Some(switch) = switch.filter(|switch| switch.intersects(overlays.board)) {
                let folds = self.collapsed != self.chosen(input.graph, &card.id);
                let name = format!(
                    "{} checks of {}",
                    if folds { "Expand" } else { "Collapse" },
                    card.name
                );
                let response = board_ui
                    .interact(
                        switch.expand(2.0 * camera.zoom).intersect(overlays.board),
                        board_ui.id().with(("checks-switch", &card.id)),
                        Sense::click(),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(if folds {
                        "Show this step's checks as cards under it"
                    } else {
                        "Fold this step's checks into this line"
                    });
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name)
                });
                if response.clicked() {
                    clicked_card = true;
                    self.menu = None;
                    commands.push(Command::ToggleChecksHere(card.id.clone()));
                }
                switches.push((switch, folds, response.hovered()));
            }
            // A failed or running card is never dimmed (LAY-10).
            let failed = step
                .is_some_and(|step| matches!(step.status, StepStatus::Failed | StepStatus::Error))
                || aggregate.is_some_and(collapse::Aggregate::failed);
            let look = paint::Look {
                ring: focus
                    .as_ref()
                    .and_then(|focus| focus.rings.get(&card.id).copied())
                    .or_else(|| related_blocks.contains(&card.id).then_some(paint::RELATED)),
                dimmed: (focus.as_ref().is_some_and(|focus| {
                    focus.selected != card.id && !focus.rings.contains_key(&card.id)
                }) || traced.dims_card(&card.id))
                    && !failed
                    && !run.running,
                switch,
            };
            let drawn_card = if card.compact.is_some() {
                let drawn = paint::compact_card(
                    &board_painter,
                    card,
                    camera,
                    selection.contains(card.id.as_str()),
                    look,
                    &run,
                    aggregate,
                );
                paint::trays(&board_painter, card, camera, reveal.inputs, reveal.outputs);
                if reveal.inputs {
                    for index in card.hidden_inputs() {
                        let Some((row, _)) = card.tray_input(index) else {
                            continue;
                        };
                        let name = format!("Input {} of {}", card.inputs[index].name, card.name);
                        board_ui
                            .interact(
                                camera.rect(row),
                                board_ui.id().with(("tray", &card.id, index)),
                                Sense::hover(),
                            )
                            .widget_info(|| {
                                egui::WidgetInfo::labeled(egui::WidgetType::Other, true, &name)
                            });
                    }
                }
                drawn
            } else {
                paint::card(
                    &board_painter,
                    card,
                    camera,
                    selection.contains(card.id.as_str()),
                    look,
                    &run,
                    aggregate,
                )
            };
            for (switch, folds, hovered) in switches.drain(..) {
                paint::checks_switch(&board_painter, switch, folds, hovered, camera.zoom);
            }
            // What a card drew is kept for tests only.
            #[cfg(not(test))]
            let _ = drawn_card;
            #[cfg(test)]
            {
                if card.compact.is_some() {
                    self.lines.push((card.id.clone(), drawn_card.lines.clone()));
                }
                self.drawn.push((card.id.clone(), drawn_card));
            }
            // A scope, so that a read-only card's disabled fields leave the
            // next card's interactions alone; its id is the card's, so the
            // controls keep their ids whichever cards are off screen.
            board_ui.scope_builder(
                egui::UiBuilder::new().id_salt(("card-controls", &card.id)),
                |ui| self.card_controls(ui, card, camera, input, reveal, &mut commands),
            );
        }

        for (_, at, color) in &port_rings {
            paint::port_ring(&board_painter, *at, *color, camera.zoom);
        }
        #[cfg(test)]
        {
            self.ringed_ports = port_rings
                .iter()
                .map(|(id, at, _)| (id.clone(), *at))
                .collect();
        }

        for chip in &chips {
            let rect = camera.rect(chip.rect());
            if !rect.intersects(overlays.board) {
                continue;
            }
            let id = board_ui.id().with(("chip", chip.id()));
            let response = board_ui
                .interact(rect.intersect(overlays.board), id, Sense::click_and_drag())
                .on_hover_text(format!(
                    "{} · {}{}",
                    chip.label,
                    chip.caption,
                    if chip.known {
                        ""
                    } else {
                        " — nothing of this name is on the board any more"
                    }
                ));
            let a11y_label = format!("{} ({} proxy)", chip.label, chip.caption);
            response.widget_info(move || {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &a11y_label)
            });
            if response.clicked() {
                clicked_card = true;
                self.menu = None;
                self.selected_wires.clear();
                commands.push(Command::Select(toggle_selection(
                    input.selection,
                    chip.id(),
                    modifiers,
                )));
            }
            if response.double_clicked()
                && let Some(node) = chip.source_node()
            {
                self.reveal(node.to_owned());
                commands.push(Command::Select(vec![node.to_owned()]));
            }
            if response.drag_started_by(PointerButton::Primary) && !input.read_only {
                let ids = if selection.contains(chip.id()) {
                    input.selection.to_vec()
                } else {
                    // As a click on it does, a new selection ends a kept
                    // wire (LAY-14).
                    self.selected_wires.clear();
                    commands.push(Command::Select(vec![chip.id().to_owned()]));
                    vec![chip.id().to_owned()]
                };
                self.drag = Some(Drag::Nodes {
                    ids,
                    offset: Vec2::ZERO,
                });
            }
            if response.dragged_by(PointerButton::Primary)
                && let Some(Drag::Nodes { offset, .. }) = &mut self.drag
            {
                *offset += response.drag_delta() / camera.zoom;
            }
            if response.drag_stopped()
                && let Some(Drag::Nodes { ids, offset }) = self.drag.take()
                && offset.length() > 0.0
            {
                commands.push(Command::Move(moved_positions(input.graph, &ids, offset)));
            }
            paint::chip(&board_painter, chip, camera, selection.contains(chip.id()));
            self.chip_controls(&mut board_ui, chip, camera, !input.read_only, &mut commands);
        }

        // The chips and counts, found by their names: pointing at a chip
        // draws its wire and a click keeps it (LAY-14), a click on a count
        // draws every wire of its port.
        for (tag, laid) in shown_tags.iter().zip(&laid_tags) {
            if !laid.rect.intersects(overlays.board) {
                continue;
            }
            let name = tag.statement.clone();
            let response = board_ui
                .interact(
                    laid.rect.intersect(overlays.board),
                    board_ui
                        .id()
                        .with(("reader-chip", &tag.reader, &tag.statement)),
                    Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            response.widget_info(move || {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name)
            });
            if response.clicked() {
                clicked_card = true;
                self.menu = None;
                self.trace
                    .keep_chip(&mut self.selected_wires, &tag.wires, modifiers.command);
                commands.push(Command::Select(vec![]));
            }
        }
        for (count, laid) in shown_counts.iter().zip(&laid_counts) {
            if !laid.rect.intersects(overlays.board) {
                continue;
            }
            let name = count.label.clone();
            let response = board_ui
                .interact(
                    laid.rect.intersect(overlays.board),
                    board_ui
                        .id()
                        .with(("reader-count", &count.source, &count.port)),
                    Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            response.widget_info(move || {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name)
            });
            if response.clicked() {
                clicked_card = true;
                self.menu = None;
                let port = (count.source.clone(), count.port.clone());
                self.shown_port = (self.shown_port.as_ref() != Some(&port)).then_some(port);
            }
        }
        // A port with values worth a variable shows `+ var` under its count;
        // its menu lists them (LAY-25).
        #[cfg(test)]
        let mut drawn_marks: Vec<DrawnMark> = Vec::new();
        let mut placed: Vec<Rect> = Vec::new();
        for (count, laid) in shown_counts.iter().zip(&laid_counts) {
            if !input
                .shared
                .iter()
                .any(|value| value.value.node_id == count.source && value.value.port == count.port)
            {
                continue;
            }
            // Clear of the chips, the counts, the other marks and the rows of
            // the card's other outputs.
            let card = &cards[count.card];
            let mut avoid: Vec<Rect> = chip_boxes.iter().chain(&placed).copied().collect();
            for row in card.outputs.iter().filter(|row| row.name != count.port) {
                if let Some(anchor) = card.output_anchor(&row.name) {
                    let anchor = camera.to_screen(anchor);
                    let half = laid.rect.height() / 2.0 + 1.0;
                    avoid.push(Rect::from_min_max(
                        egui::pos2(anchor.x, anchor.y - half),
                        egui::pos2(anchor.x + 400.0 * camera.zoom, anchor.y + half),
                    ));
                }
            }
            let Some(mark) = readers::lay_mark(&painter, laid, camera, &avoid) else {
                continue;
            };
            placed.push(mark.rect);
            if !mark.rect.intersects(overlays.board) {
                continue;
            }
            let open = self
                .port_menu
                .as_ref()
                .is_some_and(|menu| menu.source == count.source && menu.port == count.port);
            let step = input
                .graph
                .nodes
                .iter()
                .find(|node| node.id == count.source)
                .map_or(card.name.as_str(), |node| node.handle.as_str());
            let name = variables::mark_name(step);
            let response = board_ui
                .interact(
                    mark.rect.intersect(overlays.board),
                    board_ui.id().with(("var-mark", &count.source, &count.port)),
                    Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Save a value many steps read in a variable");
            readers::paint_mark(&board_painter, &mark, open || response.hovered());
            response.widget_info(move || {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name)
            });
            if response.clicked() {
                clicked_card = true;
                self.menu = None;
                self.offer = None;
                self.port_menu = (!open).then(|| variables::PortMenu {
                    source: count.source.clone(),
                    port: count.port.clone(),
                    at: mark.rect.left_bottom(),
                    opening: true,
                });
            }
            #[cfg(test)]
            drawn_marks.push(DrawnMark {
                source: count.source.clone(),
                port: count.port.clone(),
                rect: mark.rect,
            });
        }
        #[cfg(test)]
        {
            self.var_marks = drawn_marks;
        }
        // The list of the count clicked stays with its wires (LAY-18).
        let listed = shown_counts
            .iter()
            .position(|count| {
                self.shown_port.as_ref() == Some(&(count.source.clone(), count.port.clone()))
            })
            .map(list);
        // *Back* and the edge chips, over the cards and under the
        // statements, which keep clear of them (LAY-15).
        let avoid: Vec<Rect> = cards
            .iter()
            .filter(|card| selection.contains(card.id.as_str()))
            .map(|card| camera.rect(card.rect()))
            .chain([overlays.zoom_controls])
            .collect();
        let taken = self.show_edges(
            &mut board_ui,
            &painter,
            &away,
            &edge::Frame {
                cards: &cards,
                selection: input.selection,
                viewport,
                focus: overlays.focus,
                free,
                avoid: &avoid,
            },
            &mut commands,
        );
        // *Change source* on a kept statement opens the Inspector at the
        // field (LAY-42).
        if let Some(field) = self.trace.paint(
            &board_ui,
            &painter,
            overlays.board,
            camera,
            &traced,
            statement.as_ref(),
            listed.as_ref(),
            &shown_tags,
            &laid_tags,
            &taken,
            !input.read_only,
        ) {
            clicked_card = true;
            commands.push(Command::InspectInput {
                node: field.reader,
                input: field.input,
                path: Some(field.path),
            });
        }

        // The end of a branch names the step after its `if` (DIAG-06c).
        if input.moves {
            let joins = Joins::of(input.graph);
            for (end, next) in &joins.ends {
                let (Some(card), Some(next)) = (
                    cards.iter().find(|card| &card.id == end),
                    cards.iter().find(|card| &card.id == next),
                ) else {
                    continue;
                };
                let Some(socket) = card.output_anchor(ORDER_PORT) else {
                    continue;
                };
                let text = format!("↳ {}", next.name);
                let at = camera.to_screen(socket) + vec2(8.0 * camera.zoom.max(0.5), 0.0);
                let galley = board_painter.layout_no_wrap(
                    text.clone(),
                    egui::FontId::proportional((11.0 * camera.zoom).clamp(8.0, 13.0)),
                    theme::MUTED,
                );
                let rect = egui::Align2::LEFT_CENTER.anchor_size(at, galley.size());
                board_painter.galley(rect.min, galley, theme::MUTED);
                board_ui
                    .interact(
                        rect,
                        board_ui.id().with(("branch-end", end)),
                        Sense::hover(),
                    )
                    .widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &text)
                    });
            }
        }

        self.handle_wire_drag(&board_ui, &cards, camera, input, &mut commands);
        self.handle_box_selection(
            &board_ui,
            &background,
            shapes(&cards, &chips, &extents),
            camera,
            input,
            &mut commands,
        );

        if background.clicked() && !clicked_card {
            self.menu = None;
            self.inline = None;
            self.shown_port = None;
            // A wire of a chip brought back is a wire like any other here.
            drawn.extend(drawn_revealed.iter().cloned());
            let hit = pointer.and_then(|pointer| hit_wire(&drawn, camera, pointer));
            match hit {
                Some(wire) => {
                    if let Some(at) = pointer {
                        let at = trace::Anchor::Board(camera.to_world(at));
                        self.trace.anchor(vec![wire.clone()], at, modifiers.command);
                    }
                    if modifiers.command {
                        toggle(&mut self.selected_wires, &wire);
                    } else {
                        self.selected_wires = vec![wire];
                    }
                    commands.push(Command::Select(vec![]));
                }
                None => {
                    self.selected_wires.clear();
                    if !input.selection.is_empty() {
                        commands.push(Command::Select(vec![]));
                    }
                    if input.selected_input.is_some() {
                        commands.push(Command::SelectInput(None));
                    }
                }
            }
        }
        if background.secondary_clicked()
            && !input.read_only
            && let Some(pointer) = pointer
        {
            self.menu = Some(ActionMenu::new(pointer, camera.to_world(pointer), None));
        }

        if !input.read_only {
            self.handle_drop(
                &board_ui,
                overlays.board,
                &cards,
                camera,
                input,
                &mut commands,
            );
            self.handle_keys(ui, input, &mut commands);
        } else {
            self.handle_escape(ui, input, &mut commands);
        }

        let summary = paint::steps_summary(input.graph);
        if let Some(at) = paint::overlay_chrome(
            &painter,
            &overlays,
            &summary,
            input.header_leading.is_some(),
        ) {
            ui.interact(at, ui.id().with("execution-mode-hint"), Sense::hover())
                .on_hover_text(paint::mode_line_text(input.graph))
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &summary));
        }
        self.zoom_controls(ui, viewport, &overlays, shapes(&cards, &chips, &extents));
        if let Some((mm_inner, bounds, scale)) = paint::minimap(
            &painter,
            overlays.minimap,
            overlays.board,
            shapes(&cards, &chips, &extents),
            camera,
            &selection,
        ) && let Some(mm_rect) = overlays.minimap
        {
            let mm_response = ui
                .interact(
                    mm_rect,
                    ui.id().with("board-minimap"),
                    Sense::click_and_drag(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Click or drag to pan the board");
            mm_response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Board overview")
            });
            if (mm_response.clicked() || mm_response.dragged())
                && let Some(pos) = mm_response.interact_pointer_pos()
                && scale > 0.0
            {
                let world_target = bounds.min + (pos - mm_inner.min) / scale;
                let local_focus_center = overlays.focus.center() - viewport.min;
                self.camera.pan = local_focus_center - world_target.to_vec2() * self.camera.zoom;
                self.camera_moved_by_user = true;
            }
        }
        if input.graph.nodes.is_empty() && input.graph.proxies.is_empty() {
            self.empty_state(ui, overlays.board, input, &mut commands);
        }
        self.error_banner(ui, &overlays);
        // The right-click menu ends with *Save shared values as variables*
        // where the board proposes variables (LAY-25).
        let shared = input.scenario.map(|_| menu::Shared {
            values: input.shared.len(),
            editable: !input.read_only,
        });
        if let Some(menu) = &mut self.menu {
            match menu.show(
                ui.ctx(),
                input.graph,
                input.project,
                input.library,
                input.component,
                shared,
            ) {
                menu::Outcome::Open => {}
                menu::Outcome::Close => self.menu = None,
                menu::Outcome::Add(command) => {
                    commands.push(command);
                    self.menu = None;
                }
                menu::Outcome::SaveShared => {
                    commands.push(Command::SaveSharedValues);
                    self.menu = None;
                }
                menu::Outcome::Library(action) => {
                    // As a click in the library panel: at the visible centre.
                    commands.extend(library_command(action, self.visible_center()));
                    self.menu = None;
                }
            }
        }
        if let Some(offer) = &mut self.offer {
            let at = camera.to_screen(offer.anchor);
            let (outcome, _rect) = offer.show(ui.ctx(), at, overlays.board);
            #[cfg(test)]
            {
                self.offer_rect = Some(_rect);
            }
            match outcome {
                variables::Outcome::Open => {}
                variables::Outcome::Close => self.offer = None,
                variables::Outcome::Run(command) => {
                    commands.push(command);
                    self.offer = None;
                }
            }
        }
        if let Some(menu) = &mut self.port_menu {
            let values: Vec<&SharedValue> = input
                .shared
                .iter()
                .filter(|value| value.value.node_id == menu.source && value.value.port == menu.port)
                .collect();
            if values.is_empty() {
                self.port_menu = None;
            } else {
                match menu.show(ui.ctx(), &values, !input.read_only) {
                    variables::Outcome::Open => {}
                    variables::Outcome::Close => self.port_menu = None,
                    variables::Outcome::Run(command) => {
                        commands.push(command);
                        self.port_menu = None;
                    }
                }
            }
        }
        commands
    }

    /// Where the toolbar's size is kept as last drawn in one shape.
    fn toolbar_key(&self, ui: &egui::Ui, input: &Input<'_>, settings: SettingsShape) -> egui::Id {
        ui.id().with((
            "toolbar-size",
            input.selection.len(),
            !self.selected_wires.is_empty(),
            input.can_paste,
            settings,
            Self::checks_labelled(ui),
        ))
    }

    /// Whether *Checks* on the bar carries its word beside the eye (LAY-30):
    /// on a board as wide as [`CHECKS_LABEL_FROM`]; on a narrower one it is
    /// the eye alone, its name in its tooltip, so that the header row keeps
    /// what it showed before.
    fn checks_labelled(ui: &egui::Ui) -> bool {
        Self::viewport(ui).width() >= CHECKS_LABEL_FROM
    }

    fn toolbar_measured(
        &self,
        ui: &egui::Ui,
        input: &Input<'_>,
        settings: SettingsShape,
    ) -> Option<Vec2> {
        let key = self.toolbar_key(ui, input, settings);
        ui.ctx().data(|data| data.get_temp(key))
    }

    /// The toolbar's width before it has been drawn in a shape, with the
    /// settings button as an icon when it has one.
    fn toolbar_estimate(&self, ui: &egui::Ui, input: &Input<'_>) -> f32 {
        let has_selection = !input.selection.is_empty() || !self.selected_wires.is_empty();
        let checks = match (self.density_switch, Self::checks_labelled(ui)) {
            (false, _) => 0.0,
            (true, true) => 84.0,
            (true, false) => 30.0,
        };
        96.0 + checks
            + if input.can_paste { 56.0 } else { 0.0 }
            + if has_selection { 108.0 } else { 0.0 }
            + if input.settings.is_some() { 44.0 } else { 0.0 }
    }

    /// How the toolbar shows *Component settings* (UX-33): with its label
    /// while the header row has room for the caller's whole header beside
    /// the labelled toolbar, which the steps summary gives way to; else as
    /// an icon with a tooltip.
    fn settings_shape(&self, ui: &egui::Ui, input: &Input<'_>) -> SettingsShape {
        let Some(settings) = input.settings else {
            return SettingsShape::None;
        };
        let label = ui
            .painter()
            .layout_no_wrap(
                SETTINGS.to_owned(),
                FontId::proportional(theme::FONT_LABEL),
                theme::TEXT,
            )
            .size()
            .x;
        // A compact button's label and its gap from the icon, as
        // `widgets::ActionButton` lays them out, beside the square icon.
        let extra = 20.0 + 14.0 + 6.0 + label - 26.0;
        let labelled = self
            .toolbar_measured(ui, input, SettingsShape::Labelled)
            .map(|size| size.x)
            .or_else(|| {
                self.toolbar_measured(ui, input, SettingsShape::Icon)
                    .map(|size| size.x + extra)
            })
            .unwrap_or_else(|| self.toolbar_estimate(ui, input) + extra);
        let room = Self::viewport(ui).width()
            - layout::OVERLAY_PADDING_LEFT
            - layout::OVERLAY_PADDING_RIGHT
            - labelled
            - 2.0 * layout::OVERLAY_GAP;
        if room >= settings.header_width {
            SettingsShape::Labelled
        } else {
            SettingsShape::Icon
        }
    }

    fn measure_overlays(&self, ui: &egui::Ui, input: &Input<'_>) -> OverlayMeasures {
        let toolbar_size = self
            .toolbar_measured(ui, input, self.settings_shape(ui, input))
            .unwrap_or(vec2(self.toolbar_estimate(ui, input), 30.0));
        let banner_height = toolbar_size.y.max(26.0);

        let hint_galley = ui.fonts_mut(|fonts| {
            fonts.layout_no_wrap(
                paint::SHORTCUT_HINT_TEXT.to_owned(),
                FontId::proportional(theme::FONT_META),
                theme::MUTED,
            )
        });
        let error_size = self.error.as_ref().map(|_| vec2(320.0, 32.0));
        OverlayMeasures {
            strip_height: if input.inputs.is_some() {
                strip::HEIGHT
            } else {
                0.0
            },
            banner_height,
            toolbar_size,
            zoom_size: vec2(180.0, 36.0),
            hint_width: hint_galley.size().x,
            shortcuts_badge_width: 78.0,
            error_size,
            leading_size: input.header_leading.unwrap_or(Vec2::ZERO),
        }
    }

    /// The rect the board fills: what is left of `ui`.
    fn viewport(ui: &egui::Ui) -> Rect {
        let rect = ui.available_rect_before_wrap();
        Rect::from_min_size(rect.min, rect.size().max(vec2(1.0, 1.0)))
    }

    /// How wide a header at the left of the board's header row may be, beside
    /// the toolbar as last drawn for `input` (KTLO-12).
    pub fn header_room(&self, ui: &egui::Ui, input: &Input<'_>) -> HeaderRoom {
        // The header, a gap, the summary or nothing, a gap, the toolbar.
        let toolbar = self.measure_overlays(ui, input).toolbar_size.x;
        let row = Self::viewport(ui).width()
            - layout::OVERLAY_PADDING_LEFT
            - layout::OVERLAY_PADDING_RIGHT;
        // With too little room beside the toolbar for any header, the toolbar
        // moves under the row (`layout::board_overlays`), and the header and
        // the summary have the whole row.
        let stacked = row - toolbar - layout::OVERLAY_GAP < layout::MIN_BANNER_SIDE_BY_SIDE;
        let alone = if stacked {
            row
        } else {
            row - toolbar - 2.0 * layout::OVERLAY_GAP
        };
        let summary = ui
            .painter()
            .layout_no_wrap(
                paint::steps_summary(input.graph),
                paint::summary_font(),
                theme::MUTED,
            )
            .size()
            .x;
        HeaderRoom {
            alone,
            with_summary: alone - summary - if stacked { layout::OVERLAY_GAP } else { 0.0 },
        }
    }

    /// Where the caller's header goes on the board that `ui` is about to show
    /// for `input`: the left of the board's header row.
    pub fn leading_rect(&self, ui: &egui::Ui, input: &Input<'_>) -> Rect {
        let measures = self.measure_overlays(ui, input);
        layout::board_overlays(Self::viewport(ui), &measures).leading
    }

    /// Places the camera for this frame's board, and says whether the board
    /// changed size or place since the last frame.
    ///
    /// A new board is fitted. A resized or rearranged window fits a camera
    /// the person has not moved again, and keeps the middle of the board in
    /// the middle of a moved one. A panel opening, closing or resized beside
    /// the board (the Inspector, the library, Run results, Split's code)
    /// leaves the board where it is on screen and its zoom as it is: the
    /// panel covers the edge it opens from (UX-18). The board's own header
    /// settling, with nothing else changed, fits a fitted camera again and
    /// keeps the middle of a moved one.
    fn place_camera(
        &mut self,
        viewport: Rect,
        focus: Rect,
        rearranged: bool,
        shapes: Shapes<'_>,
    ) -> bool {
        if self.fit_pending || (self.last_size == Vec2::ZERO && !self.camera_moved_by_user) {
            self.fit(viewport, focus, shapes);
            return false;
        }
        let Some(previous) = self.last_focus else {
            return false;
        };
        let moved = (viewport.min - self.last_viewport.min).length() > 0.5;
        let resized = (viewport.size() - self.last_viewport.size()).length() > 0.5;
        let refocused = (focus.size() - previous.size()).length() > 1.0;
        if !(moved || resized || refocused) {
            return false;
        }
        let kept_middle =
            (focus.center() - viewport.min) - (previous.center() - self.last_viewport.min);
        let pan = self.camera.pan;
        if rearranged {
            if self.camera_moved_by_user {
                self.camera.pan += kept_middle;
            } else {
                self.fit(viewport, focus, shapes);
            }
        } else if moved || resized {
            self.camera.pan += self.last_viewport.min - viewport.min;
            self.fitted_for = None;
        } else if self.camera_moved_by_user {
            self.camera.pan += kept_middle;
        } else if self.fitted_for == Some(viewport.size()) {
            self.fit(viewport, focus, shapes);
        }
        let fitted = !self.camera_moved_by_user && self.fitted_for == Some(viewport.size());
        let moved_with = (viewport.min - self.last_viewport.min) + (self.camera.pan - pan);
        self.previous_focus = (!fitted).then(|| previous.translate(moved_with));
        true
    }

    /// Pans the least that shows the one selected card whole in the focus,
    /// once when it is selected or added, so the Inspector it opens never
    /// covers it (UX-18). When the board changes size it pans only for what
    /// the change newly hides: a card the person or a holder's save left
    /// partly outside stays so, and the storage band coming and going leaves
    /// the camera where it was (MCP-08a, #324). A card larger than the focus
    /// shows its top left corner. Nothing pans while a drag is under way:
    /// the card stays under the pointer.
    fn keep_selection_in_view(
        &mut self,
        viewport: Rect,
        focus: Rect,
        selection: &[String],
        cards: &[Card],
        board_changed: bool,
    ) {
        let selected = match selection {
            [one] => Some(one),
            _ => None,
        };
        if self.drag.is_some() {
            self.kept_in_view = selected.cloned();
            return;
        }
        let Some(id) = selected else {
            self.kept_in_view = None;
            return;
        };
        // A card being added is not on the board until its edit applies:
        // it is kept in view in the first frame that draws it.
        let Some(card) = cards.iter().find(|card| &card.id == id) else {
            return;
        };
        let on_screen = self.screen_camera(viewport).rect(card.rect());
        let outside = vec2(
            least_shift(on_screen.x_range(), focus.x_range()),
            least_shift(on_screen.y_range(), focus.y_range()),
        );
        let shift = match (self.kept_in_view.as_ref() == Some(id), self.previous_focus) {
            (false, _) => outside,
            (true, _) if !board_changed => Vec2::ZERO,
            (true, Some(previous)) => vec2(
                newly_hidden(
                    outside.x,
                    least_shift(on_screen.x_range(), previous.x_range()),
                ),
                newly_hidden(
                    outside.y,
                    least_shift(on_screen.y_range(), previous.y_range()),
                ),
            ),
            (true, None) => outside,
        };
        self.kept_in_view = Some(id.clone());
        if shift != Vec2::ZERO {
            self.camera.pan += shift;
            self.fitted_for = None;
        }
    }

    fn fit(&mut self, viewport: Rect, focus: Rect, shapes: Shapes<'_>) {
        self.fit_pending = false;
        self.camera_moved_by_user = false;
        self.fitted_for = Some(viewport.size());
        let local_focus_center = focus.center() - viewport.min;
        let Some(bounds) = shapes.bounds() else {
            self.camera = Camera {
                zoom: 1.0,
                pan: vec2(60.0, 60.0),
            };
            return;
        };
        let zoom = fit_zoom(focus, bounds);
        self.camera = Camera {
            zoom,
            pan: local_focus_center - bounds.center().to_vec2() * zoom,
        };
    }

    fn zoom_at(&mut self, viewport: Rect, screen: Pos2, factor: f32) {
        let before = self.camera.zoom;
        self.camera.zoom = (before * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let local = screen - viewport.min;
        self.camera.pan = local - (local - self.camera.pan) * (self.camera.zoom / before);
        self.camera_moved_by_user = true;
    }

    fn handle_camera(&mut self, ui: &egui::Ui, background: &egui::Response, viewport: Rect) {
        if background.contains_pointer() {
            let (zoom, scroll, pointer) = ui.input(|state| {
                (
                    state.zoom_delta(),
                    state.smooth_scroll_delta,
                    state.pointer.hover_pos(),
                )
            });
            if zoom != 1.0
                && let Some(pointer) = pointer
            {
                self.zoom_at(viewport, pointer, zoom);
            } else if scroll != Vec2::ZERO {
                self.camera.pan += scroll;
                self.camera_moved_by_user = true;
            }
        }
        if background.dragged_by(PointerButton::Middle)
            || background.dragged_by(PointerButton::Secondary)
        {
            self.camera.pan += background.drag_delta();
            self.camera_moved_by_user = true;
        }
    }

    /// A value being carried to an input: a data wire from a step's output, or
    /// a proxy's wire or a variable from the library, which come from no step.
    fn carried_value(&self, context: &egui::Context) -> Option<Carried> {
        match &self.drag {
            Some(Drag::Wire {
                start: WireStart::Output { node, port },
                ..
            }) if !port.starts_with("__") => Some(Carried(Some(node.clone()))),
            Some(Drag::Wire {
                start: WireStart::Proxy { .. },
                ..
            }) => Some(Carried(None)),
            _ => egui::DragAndDrop::payload::<Payload>(context)
                .filter(|payload| matches!(payload.as_ref(), Payload::Variable { .. }))
                .map(|_| Carried(None)),
        }
    }

    fn card_controls(
        &mut self,
        ui: &mut egui::Ui,
        card: &Card,
        camera: Camera,
        input: &Input<'_>,
        reveal: Reveal,
        commands: &mut Vec<Command>,
    ) {
        let zoom = camera.zoom;
        // Output sockets start wires; a compact card's hidden outputs start
        // them from the tray while it is selected.
        let mut outputs: Vec<(String, Pos2)> = card
            .outputs
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                card.output_anchor(&row.name)
                    .or_else(|| {
                        reveal
                            .outputs
                            .then(|| card.tray_output(index).map(|(_, socket)| socket))
                            .flatten()
                    })
                    .map(|anchor| (row.name.clone(), anchor))
            })
            .collect();
        outputs.extend(card.order_outputs().into_iter().filter_map(|port| {
            card.output_anchor(port)
                .map(|anchor| (port.to_string(), anchor))
        }));
        let editable = !input.read_only;
        for (port, anchor) in outputs {
            let center = camera.to_screen(anchor);
            let rect =
                Rect::from_center_size(center, Vec2::splat((SOCKET_HIT * zoom).max(3.0) * 2.0));
            let response = ui
                .interact(
                    rect,
                    ui.id().with(("socket", &card.id, &port)),
                    if editable {
                        Sense::drag()
                    } else {
                        Sense::hover()
                    },
                )
                .on_hover_cursor(if editable {
                    egui::CursorIcon::Crosshair
                } else {
                    egui::CursorIcon::Default
                });
            let name = format!("Output {port} of {}", card.name);
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Other, editable, &name)
            });
            if editable && response.drag_started() {
                self.drag = Some(Drag::Wire {
                    start: WireStart::Output {
                        node: card.id.clone(),
                        port: port.clone(),
                    },
                    from: center,
                });
            }
        }

        // A component call keeps *Open* in its header: a compact one wherever
        // its rows are legible, a full one at Full.
        let open = if card.compact.is_some() {
            layout::CardDetail::at_zoom(zoom) != layout::CardDetail::Overview
        } else {
            layout::CardDetail::at_zoom(zoom) == layout::CardDetail::Full
        };
        if card.kind == NodeKind::Component && open {
            let rect = camera.rect(card.open_button());
            let response = ui
                .interact(rect, ui.id().with(("open", &card.id)), Sense::click())
                .on_hover_text("Open the component's steps");
            let name = format!("Open the steps of {}", card.name);
            response
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
            if response.clicked() {
                commands.push(Command::EditComponent(card.id.clone()));
            }
        }

        // A compact card has no footer and no field: *Add step* is on the
        // right-click menu and the toolbar, values are edited in the Inspector.
        if card.compact.is_some() || layout::CardDetail::at_zoom(zoom) != layout::CardDetail::Full {
            return;
        }

        // Footer buttons.
        let action = camera.rect(card.action_button());
        let response = ui
            .interact(action, ui.id().with(("action", &card.id)), Sense::click())
            .on_hover_text("Add a connected step");
        let name = format!("Add a step connected to {}", card.name);
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, editable, &name));
        if editable && response.clicked() {
            let source = menu::default_source(input.graph, input.project, &card.id);
            self.menu = Some(ActionMenu::new(
                action.left_bottom(),
                camera.to_world(action.left_bottom()),
                source,
            ));
        }
        for (index, (port, label)) in card.controls.iter().enumerate() {
            if !card.buttons {
                break;
            }
            let rect = camera.rect(card.control_button(index));
            let next = format!(
                "Add the next step ({})",
                label.trim_end_matches(" →").to_lowercase()
            );
            let response = ui
                .interact(
                    rect,
                    ui.id().with(("control", &card.id, *port)),
                    Sense::click(),
                )
                .on_hover_text(&next);
            let name = format!("{next} after {}", card.name);
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, editable, &name)
            });
            if editable && response.clicked() {
                self.menu = Some(ActionMenu::new(
                    rect.center_bottom(),
                    camera.to_world(rect.center_bottom()),
                    Some((card.id.clone(), (*port).to_owned())),
                ));
            }
        }
        // Inline literal fields.
        if zoom < INLINE_EDIT_ZOOM {
            return;
        }
        if !editable {
            ui.disable();
        }
        for (index, row) in card.inputs.iter().enumerate() {
            let field = camera.rect(card.input_field(index));
            match &row.value {
                RowValue::Inline { text, value_type } => {
                    let editing = self.inline.as_ref().filter(|edit| {
                        edit.node == card.id && edit.input == row.name && edit.path == row.path
                    });
                    let mut buffer = editing.map_or_else(|| text.clone(), |edit| edit.text.clone());
                    let invalid = editing.and_then(|edit| edit.error.clone());
                    let response = ui.put(
                        field.shrink2(vec2(6.0 * zoom, 2.0 * zoom)),
                        egui::TextEdit::singleline(&mut buffer)
                            .id(ui.id().with(("inline", &card.id, &row.name, &row.path)))
                            .font(FontId::proportional(13.0 * zoom))
                            .margin(egui::Margin::ZERO)
                            .frame(egui::Frame::NONE)
                            .hint_text(if row.required { "Required" } else { "Optional" })
                            .text_color(if invalid.is_some() {
                                theme::DANGER
                            } else {
                                theme::TEXT
                            }),
                    );
                    let response =
                        crate::widgets::named(response, format!("{} of {}", row.label, card.name));
                    let response = match &invalid {
                        Some(message) => response.on_hover_text(message),
                        None => response.on_hover_text(&row.label),
                    };
                    if response.changed() || response.has_focus() {
                        self.inline = Some(InlineEdit {
                            node: card.id.clone(),
                            input: row.name.clone(),
                            path: row.path.clone(),
                            text: buffer.clone(),
                            error: invalid.clone(),
                        });
                    }
                    if response.lost_focus() {
                        if &buffer != text {
                            match parse_inline(&buffer, *value_type) {
                                Ok(value) => {
                                    commands.push(Command::SetLiteral {
                                        node: card.id.clone(),
                                        input: row.name.clone(),
                                        path: row.path.clone(),
                                        value,
                                    });
                                    self.inline = None;
                                }
                                Err(message) => {
                                    self.inline = Some(InlineEdit {
                                        node: card.id.clone(),
                                        input: row.name.clone(),
                                        path: row.path.clone(),
                                        text: buffer,
                                        error: Some(message),
                                    });
                                }
                            }
                        } else {
                            self.inline = None;
                        }
                    }
                }
                RowValue::TypeChoice(current) => {
                    let id = ui.id().with(("type", &card.id, &row.name, &row.path));
                    let mut selected = current.clone();
                    ui.scope_builder(egui::UiBuilder::new().max_rect(field), |ui| {
                        ui.spacing_mut().button_padding = vec2(6.0 * zoom, 2.0 * zoom);
                        ui.spacing_mut().interact_size.y = field.height() - 4.0 * zoom;
                        egui::ComboBox::from_id_salt(id)
                            .width(field.width() - 8.0 * zoom)
                            .selected_text(
                                egui::RichText::new(if selected.is_empty() {
                                    "Choose a type"
                                } else {
                                    &selected
                                })
                                .size(13.0 * zoom),
                            )
                            .show_ui(ui, |ui| {
                                for name in
                                    ["string", "number", "boolean", "object", "array", "null"]
                                {
                                    ui.selectable_value(&mut selected, name.to_owned(), name);
                                }
                            });
                    });
                    if &selected != current {
                        commands.push(Command::SetLiteral {
                            node: card.id.clone(),
                            input: row.name.clone(),
                            path: row.path.clone(),
                            value: serde_json::Value::String(selected),
                        });
                    }
                }
                _ => {}
            }
        }
    }

    /// The socket a proxy's wires leave from, and the button to its variable.
    fn chip_controls(
        &mut self,
        ui: &mut egui::Ui,
        chip: &Chip,
        camera: Camera,
        editable: bool,
        commands: &mut Vec<Command>,
    ) {
        if chip.source_node().is_some()
            && layout::CardDetail::at_zoom(camera.zoom) == layout::CardDetail::Full
        {
            let rect = camera.rect(chip.follow_button());
            let response = ui
                .interact(rect, ui.id().with(("follow", chip.id())), Sense::click())
                .on_hover_text("Go to the variable this stands for");
            let name = format!("Go to the variable {}", chip.label);
            response
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
            if response.clicked()
                && let Some(node) = chip.source_node()
            {
                self.reveal(node.to_owned());
                commands.push(Command::Select(vec![node.to_owned()]));
            }
        }
        let center = camera.to_screen(chip.output_anchor());
        let rect = Rect::from_center_size(
            center,
            Vec2::splat((SOCKET_HIT * camera.zoom).max(3.0) * 2.0),
        );
        let response = ui
            .interact(
                rect,
                ui.id().with(("chip-socket", chip.id())),
                if editable {
                    Sense::drag()
                } else {
                    Sense::hover()
                },
            )
            .on_hover_cursor(if editable {
                egui::CursorIcon::Crosshair
            } else {
                egui::CursorIcon::Default
            });
        let name = format!("Output of {}", chip.label);
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, editable, &name));
        if editable && response.drag_started() {
            self.drag = Some(Drag::Wire {
                start: WireStart::Proxy {
                    binding: proxy_binding(&chip.proxy.source),
                },
                from: center,
            });
        }
    }

    fn handle_wire_drag(
        &mut self,
        ui: &egui::Ui,
        cards: &[Card],
        camera: Camera,
        input: &Input<'_>,
        commands: &mut Vec<Command>,
    ) {
        self.caption = None;
        let Some(Drag::Wire { start, from }) = &self.drag else {
            return;
        };
        let (source, port) = match start {
            WireStart::Output { node, port } => (Some(node.clone()), port.clone()),
            // A proxy carries a value, not an output: it fits any argument.
            WireStart::Proxy { .. } => (None, String::new()),
        };
        let (pointer, released) =
            ui.input(|state| (state.pointer.interact_pos(), state.pointer.any_released()));
        let Some(pointer) = pointer else {
            return;
        };
        let control = port.starts_with("__");
        let carried = (!control).then(|| Carried(source.clone()));
        // Each slot says how it takes the wire: by the snap near its socket,
        // only under the pointer, or anywhere on its tray row.
        let exact = (SOCKET_HIT * camera.zoom).max(3.0);
        let target = cards
            .iter()
            .filter(|card| Some(&card.id) != source.as_ref())
            .flat_map(|card| {
                let mut slots: Vec<DropSlot> = Vec::new();
                let snap = SOCKET_HIT * 2.0;
                if control {
                    if let Some(anchor) = card.input_anchor(ORDER_PORT, &[]) {
                        slots.push(DropSlot::new(ORDER_PORT, &[], anchor, snap));
                    }
                } else {
                    for (index, row) in card.inputs.iter().enumerate() {
                        if let Some(anchor) = card.shown_row_anchor(index) {
                            // The URL's socket sits on the subtitle of a
                            // compact card, where a drop meant for the card
                            // would rewrite its address by the snap.
                            let reach = if card.compact.is_some() && row.name == URL_INPUT {
                                exact
                            } else {
                                snap
                            };
                            slots.push(DropSlot::new(&row.name, &row.path, anchor, reach));
                        }
                    }
                    let reveal = Reveal::of(
                        card,
                        camera,
                        input.selection.contains(&card.id),
                        carried.as_ref(),
                        Some(pointer),
                    );
                    if reveal.inputs {
                        for index in card.hidden_inputs() {
                            if let Some((row, anchor)) = card.tray_input(index) {
                                let input = &card.inputs[index];
                                let mut slot = DropSlot::new(&input.name, &input.path, anchor, 0.0);
                                slot.area = Some(camera.rect(row));
                                slots.push(slot);
                            }
                        }
                    }
                }
                slots.into_iter().filter_map(move |slot| {
                    let anchor = camera.to_screen(slot.anchor);
                    let distance = match slot.area {
                        Some(area) if area.contains(pointer) => 0.0,
                        _ => anchor.distance(pointer),
                    };
                    (distance <= slot.reach.max(f32::EPSILON))
                        .then(|| (distance, (card.id.clone(), slot.name, slot.path, anchor)))
                })
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, target)| target);

        // What a wire from a *Next* with a successor will do (DIAG-06c).
        if input.moves
            && port == ORDER_PORT
            && let (Some(after), Some(before)) = (&source, &self.successor)
        {
            let over = target.as_ref().map(|(card, ..)| card.as_str());
            let text = carry_caption(input.graph, after, before, over);
            let painter = ui.painter();
            let galley =
                painter.layout_no_wrap(text.clone(), egui::FontId::proportional(12.0), theme::TEXT);
            // Below and right of the pointer, inside the board: above it or
            // left of it where the board ends (DIAG-06c).
            let within = ui.max_rect();
            let size = galley.size() + vec2(16.0, 10.0);
            let mut at = pointer + vec2(16.0, 16.0);
            if at.x + size.x > within.right() {
                at.x = (pointer.x - 16.0 - size.x).max(within.left());
            }
            if at.y + size.y > within.bottom() {
                at.y = (pointer.y - 16.0 - size.y).max(within.top());
            }
            let rect = Rect::from_min_size(at, size);
            painter.rect(
                rect,
                6.0,
                theme::WHITE,
                Stroke::new(1.0, theme::BORDER),
                StrokeKind::Inside,
            );
            painter.galley(rect.min + vec2(8.0, 5.0), galley, theme::TEXT);
            ui.interact(rect, ui.id().with("carry-caption"), Sense::hover())
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &text));
            self.caption = Some(text);
        }
        let end = target.as_ref().map_or(pointer, |(.., anchor)| *anchor);
        let painter = ui.painter();
        paint::wire(
            painter,
            *from,
            end,
            control,
            branch_of(&port),
            true,
            camera.zoom,
        );

        if released {
            let binding = match start {
                WireStart::Proxy { binding } => Some(binding.clone()),
                WireStart::Output { .. } => None,
            };
            self.drag = None;
            match (target, binding) {
                // A proxy binds the value it stands for, as dragging the variable
                // out of the panel does.
                (Some((target, input_name, path, _)), Some(binding)) => {
                    commands.push(Command::Bind {
                        node: target,
                        input: input_name,
                        path,
                        binding,
                    });
                }
                (Some((target, input_name, path, anchor)), None) => {
                    let source = source.unwrap_or_default();
                    let value = edit::ValueRef::new(&source, &port, "");
                    let direct = Command::Connect {
                        source,
                        source_port: port,
                        target: target.clone(),
                        target_input: input_name,
                        target_path: path,
                    };
                    self.connect_or_offer(
                        input,
                        cards,
                        value,
                        &target,
                        camera.to_world(anchor),
                        direct,
                        commands,
                    );
                }
                (None, binding) => {
                    if binding.is_some()
                        || cards
                            .iter()
                            .any(|card| camera.rect(card.rect()).contains(pointer))
                    {
                        self.error = Some(if control {
                            "Drop the wire on the top input of the next step.".into()
                        } else {
                            "Drop the wire on an input port.".into()
                        });
                    } else {
                        self.menu = Some(ActionMenu::new(
                            pointer,
                            camera.to_world(pointer),
                            source.map(|source| (source, port)),
                        ));
                        let _ = input;
                    }
                }
            }
        }
    }

    fn handle_box_selection(
        &mut self,
        ui: &egui::Ui,
        background: &egui::Response,
        shapes: Shapes<'_>,
        camera: Camera,
        input: &Input<'_>,
        commands: &mut Vec<Command>,
    ) {
        if background.drag_started_by(PointerButton::Primary)
            && let Some(from) = background.interact_pointer_pos()
        {
            self.drag = Some(Drag::Select {
                from,
                additive: ui.input(|state| state.modifiers.command),
            });
        }
        let Some(Drag::Select { from, additive }) = &self.drag else {
            return;
        };
        let Some(pointer) = ui.input(|state| state.pointer.interact_pos()) else {
            return;
        };
        let area = Rect::from_two_pos(*from, pointer);
        ui.painter().rect(
            area,
            2.0,
            theme::ACCENT.gamma_multiply(0.08),
            Stroke::new(1.0, theme::ACCENT),
            StrokeKind::Inside,
        );
        if background.drag_stopped() {
            let mut selected: Vec<String> = if *additive {
                input.selection.to_vec()
            } else {
                vec![]
            };
            for card in shapes.cards {
                if camera.rect(card.rect()).intersects(area) && !selected.contains(&card.id) {
                    selected.push(card.id.clone());
                }
            }
            for chip in shapes.chips {
                let id = chip.id().to_owned();
                if camera.rect(chip.rect()).intersects(area) && !selected.contains(&id) {
                    selected.push(id);
                }
            }
            self.drag = None;
            self.selected_wires.clear();
            commands.push(Command::Select(selected));
        }
    }

    /// Where a step carried from the Library to `pointer` would land, and
    /// its mark on the board (DIAG-06c): on the nearest line of a chain
    /// within 60 px on screen, else behind the last step of the main flow.
    fn mark_drop(
        &self,
        ui: &egui::Ui,
        input: &Input<'_>,
        cards: &[Card],
        camera: Camera,
        pointer: Pos2,
    ) -> Option<DropMark> {
        const REACH: f32 = 60.0;
        let nearest = self
            .chain_lines
            .iter()
            .filter_map(|(source, port, points)| {
                points
                    .windows(2)
                    .map(|pair| {
                        let (a, b) = (camera.to_screen(pair[0]), camera.to_screen(pair[1]));
                        let along = b - a;
                        let t = if along.length_sq() <= f32::EPSILON {
                            0.0
                        } else {
                            ((pointer - a).dot(along) / along.length_sq()).clamp(0.0, 1.0)
                        };
                        let at = a + along * t;
                        (at.distance(pointer), at)
                    })
                    .min_by(|x, y| x.0.total_cmp(&y.0))
                    .map(|(distance, at)| (distance, at, source, port))
            })
            .filter(|(distance, ..)| *distance <= REACH)
            .min_by(|x, y| x.0.total_cmp(&y.0));
        let painter = ui.painter();
        if let Some((_, at, source, port)) = nearest {
            painter.circle(at, 6.0, theme::ACCENT, Stroke::new(2.0, theme::WHITE));
            return Some(DropMark::Line {
                source: source.clone(),
                port: port.clone(),
            });
        }
        let braces = apiforge_core::chain::braces(input.graph);
        let last = sequence(input.graph)
            .into_iter()
            .rev()
            .filter(|id| !braces.contains_key(id))
            .filter_map(|id| input.graph.nodes.iter().find(|node| node.id == id))
            .find(|node| !node.kind.is_data())?;
        if let Some(at) = cards
            .iter()
            .find(|card| card.id == last.id)
            .and_then(|card| card.output_anchor(ORDER_PORT))
        {
            painter.circle(
                camera.to_screen(at),
                6.0,
                theme::ACCENT,
                Stroke::new(2.0, theme::WHITE),
            );
        }
        Some(DropMark::End {
            last: last.id.clone(),
        })
    }

    fn handle_drop(
        &mut self,
        ui: &egui::Ui,
        viewport: Rect,
        cards: &[Card],
        camera: Camera,
        input: &Input<'_>,
        commands: &mut Vec<Command>,
    ) {
        self.drop_mark = None;
        let (pointer, released) =
            ui.input(|state| (state.pointer.interact_pos(), state.pointer.any_released()));
        let Some(pointer) = pointer.filter(|pointer| viewport.contains(*pointer)) else {
            return;
        };
        let Some(payload) = egui::DragAndDrop::payload::<Payload>(ui.ctx()) else {
            return;
        };
        // A step carried from the Library lands on the nearest line of a
        // chain, or behind the last step of the main flow (DIAG-06c).
        if let Payload::Node { block, .. } = payload.as_ref()
            && input.moves
            && apiforge_core::chain::is_step(block.kind())
            && block.kind() != NodeKind::Start
        {
            self.drop_mark = self.mark_drop(ui, input, cards, camera, pointer);
        }
        // Highlight the input a variable would bind to.
        // A compact card's hidden inputs take the variable on their tray rows.
        let row_under = |cards: &[Card]| {
            cards.iter().find_map(|card| {
                let reveal = Reveal::of(
                    card,
                    camera,
                    input.selection.contains(&card.id),
                    Some(&Carried(None)),
                    Some(pointer),
                );
                (0..card.inputs.len()).find_map(|index| {
                    let row = card.input_row(index);
                    let row = if row != Rect::NOTHING {
                        row
                    } else if reveal.inputs {
                        card.tray_input(index)?.0
                    } else {
                        return None;
                    };
                    let rect = camera.rect(row);
                    rect.contains(pointer).then(|| {
                        (
                            card.id.clone(),
                            card.inputs[index].name.clone(),
                            card.inputs[index].path.clone(),
                            rect,
                        )
                    })
                })
            })
        };
        if let Payload::Variable { .. } = payload.as_ref()
            && let Some((.., rect)) = row_under(cards)
        {
            ui.painter().rect_stroke(
                rect.shrink(2.0),
                4.0,
                Stroke::new(1.5, theme::ACCENT),
                StrokeKind::Inside,
            );
        }
        if !released {
            return;
        }
        let Some(payload) = egui::DragAndDrop::take_payload::<Payload>(ui.ctx()) else {
            return;
        };
        let world = camera.to_world(pointer);
        let position = Position {
            x: f64::from(world.x).round(),
            y: f64::from(world.y).round(),
        };
        let on_line = match self.drop_mark.take() {
            Some(DropMark::Line { source, port }) => Some((source, port)),
            _ => None,
        };
        match payload.as_ref() {
            Payload::Node { block, component } if on_line.is_some() => {
                commands.push(Command::AddOnLine {
                    block: *block,
                    component: component.clone(),
                    position,
                    after: on_line.expect("a line"),
                });
            }
            Payload::Node { block, component } => commands.push(Command::AddNode {
                block: *block,
                component: component.clone(),
                position,
                source: None,
            }),
            Payload::Operator(operator) => commands.push(Command::AddOperator {
                operator: *operator,
                position,
            }),
            Payload::Variable { binding, .. } => match row_under(cards) {
                Some((node, target_input, path, row)) => {
                    let direct = Command::Bind {
                        node: node.clone(),
                        input: target_input,
                        path,
                        binding: binding.clone(),
                    };
                    match edit::ValueRef::of(binding) {
                        Some(value) => self.connect_or_offer(
                            input,
                            cards,
                            value,
                            &node,
                            camera.to_world(row.left_center()),
                            direct,
                            commands,
                        ),
                        None => commands.push(direct),
                    }
                }
                // Dropped on a card but beside its arguments, it was aimed at one
                // of them.
                None if cards
                    .iter()
                    .any(|card| camera.rect(card.rect()).contains(pointer)) =>
                {
                    self.error = Some("Drop a variable on an argument of a step.".into());
                }
                // Dropped beside the steps, the value is placed there instead, so
                // that its consumers can take it from nearby.
                None => match placeable(binding, input.graph) {
                    Some(source) => commands.push(Command::AddProxy { source, position }),
                    None => {
                        self.error =
                            Some("Only a whole variable, parameter or environment value can be placed on the board. Drop this on an argument of a step.".into());
                    }
                },
            },
        }
    }

    /// A far value dropped on an input asks first (LAY-25): the choice of
    /// reading it through a variable or wiring it directly opens at the
    /// input. A value of a step that is not a request or a component call,
    /// a reader beside its source and a board that proposes no variable
    /// connect at once, with `direct`.
    #[allow(clippy::too_many_arguments)]
    fn connect_or_offer(
        &mut self,
        input: &Input<'_>,
        cards: &[Card],
        value: edit::ValueRef,
        target: &str,
        anchor: Pos2,
        direct: Command,
        commands: &mut Vec<Command>,
    ) {
        let source = input
            .reference_graph
            .nodes
            .iter()
            .chain(&input.graph.nodes)
            .find(|node| node.id == value.node_id);
        let card = |id: &str| cards.iter().find(|card| card.id == id);
        // A wire of the order is no value to save first (DIAG-06a): from
        // *Next*, *Yes* or *No* it moves the step it is dropped on.
        let control = value.port.starts_with("__");
        let far = !control
            && match (source, card(&value.node_id), card(target)) {
                (Some(source), Some(from), Some(to)) => {
                    matches!(source.kind, NodeKind::Request | NodeKind::Component)
                        && readers::far(cards, from, to)
                }
                _ => false,
            };
        match input.scenario {
            Some((main, cleanup)) if far && !input.read_only => {
                self.menu = None;
                self.port_menu = None;
                self.offer = Some(variables::Offer {
                    anchor,
                    proposal: edit::propose(main, cleanup, input.project, &value),
                    direct,
                    focus: true,
                });
            }
            _ => commands.push(direct),
        }
    }

    /// Esc closes what is open first; on a quiet board it clears the
    /// selection, and the wires it brought back return to chips (LAY-23).
    /// A read-only board takes it too: it selects and reveals as well.
    fn handle_escape(&mut self, ui: &egui::Ui, input: &Input<'_>, commands: &mut Vec<Command>) {
        if ui.ctx().egui_wants_keyboard_input()
            || ui.ctx().memory(|memory| memory.top_modal_layer().is_some())
            || !ui.input(|state| state.key_pressed(Key::Escape))
        {
            return;
        }
        if self.offer.take().is_some() || self.port_menu.take().is_some() {
            return;
        }
        if self.menu.is_none() && self.drag.is_none() {
            self.shown_port = None;
            self.selected_wires.clear();
            if !input.selection.is_empty() {
                commands.push(Command::Select(Vec::new()));
            }
        }
        self.menu = None;
        self.drag = None;
    }

    fn handle_keys(&mut self, ui: &egui::Ui, input: &Input<'_>, commands: &mut Vec<Command>) {
        // Keys belong to text fields while one has focus.
        if ui.ctx().egui_wants_keyboard_input()
            || ui.ctx().memory(|memory| memory.top_modal_layer().is_some())
        {
            return;
        }
        self.handle_escape(ui, input, commands);
        if ui.input_mut(|state| {
            state.consume_shortcut(&egui::KeyboardShortcut::new(
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::L,
            ))
        }) {
            commands.push(Command::TidyUp(input.selection.to_vec()));
        }
        let delete =
            ui.input(|state| state.key_pressed(Key::Delete) || state.key_pressed(Key::Backspace));
        if delete && (!input.selection.is_empty() || !self.selected_wires.is_empty()) {
            commands.push(Command::Delete {
                ids: input.selection.to_vec(),
                wires: std::mem::take(&mut self.selected_wires),
            });
        }
    }

    fn toolbar(
        &mut self,
        ui: &mut egui::Ui,
        viewport: Rect,
        overlays: &BoardOverlays,
        input: &Input<'_>,
        commands: &mut Vec<Command>,
    ) {
        let has_selection = !input.selection.is_empty();
        let shape = self.settings_shape(ui, input);
        let checks_labelled = Self::checks_labelled(ui);
        let measured = egui::Area::new(ui.id().with("board-toolbar"))
            .enabled(ui.is_enabled())
            .constrain_to(viewport)
            .fixed_pos(overlays.toolbar.right_top())
            .pivot(Align2::RIGHT_TOP)
            .fade_in(false)
            .show(ui.ctx(), |ui| {
                ui.spacing_mut().interact_size.y = 26.0;
                ui.spacing_mut().button_padding.y = 4.0;
                egui::Frame::new()
                    .fill(theme::WHITE)
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .corner_radius(8)
                    .inner_margin(egui::Margin::symmetric(6, 3))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // On a narrow board *Checks* is an eye alone and
                            // the tools stand closer, so that the header row
                            // keeps what it showed before it (LAY-30).
                            if self.density_switch && !checks_labelled {
                                ui.spacing_mut().item_spacing.x = 4.0;
                            }
                            let editable = !input.read_only;
                            if let Some(settings) = input.settings {
                                let labelled = shape == SettingsShape::Labelled;
                                // An icon's own tooltip is its label: there
                                // the longer text, under the button's name.
                                let button = if labelled {
                                    widgets::action(Icon::Settings, SETTINGS).compact().quiet()
                                } else {
                                    widgets::icon_button(Icon::Settings, SETTINGS_HOVER)
                                        .named(SETTINGS)
                                        .compact()
                                };
                                let mut response = ui
                                    .add_enabled(
                                        settings.available,
                                        button.toggle(settings.open && settings.available),
                                    )
                                    .on_disabled_hover_text(SETTINGS_UNAVAILABLE);
                                if labelled {
                                    response = response.on_hover_text(SETTINGS_HOVER);
                                }
                                if response.clicked() {
                                    self.menu = None;
                                    commands.push(Command::ComponentSettings);
                                }
                                ui.separator();
                            }
                            if ui
                                .add_enabled(
                                    editable,
                                    widgets::action(Icon::Plus, "Add step").compact().quiet(),
                                )
                                .on_hover_text("Add a step to the board")
                                .clicked()
                            {
                                let at = ui.min_rect().left_bottom() + vec2(-6.0, 12.0);
                                let camera = self.screen_camera(viewport);
                                self.menu = Some(ActionMenu::bar(at, camera.to_world(at)));
                            }
                            if input.can_paste
                                && editable
                                && ui
                                    .add(
                                        widgets::icon_button(
                                            Icon::Clipboard,
                                            "Paste steps (Ctrl+V)",
                                        )
                                        .compact(),
                                    )
                                    .clicked()
                            {
                                commands.push(Command::Paste);
                            }
                            if ui
                                .add_enabled(
                                    editable,
                                    widgets::icon_button(Icon::Layout, "Tidy up").compact(),
                                )
                                .on_hover_text(if has_selection {
                                    "Lay out the selected steps (Ctrl+Shift+L)"
                                } else {
                                    "Lay out the whole board (Ctrl+Shift+L)"
                                })
                                .clicked()
                            {
                                commands.push(Command::TidyUp(input.selection.to_vec()));
                            }
                            // Every check as a card, or folded into its
                            // step's line: the eye open while they are drawn
                            // (LAY-30, out of the view menu).
                            if self.density_switch {
                                let shown = !self.collapsed;
                                let icon = if shown { Icon::Eye } else { Icon::EyeOff };
                                let hover = if shown {
                                    "Checks are drawn as cards under their steps. Click to fold them into each step's line"
                                } else {
                                    "Checks are folded into each step's line. Click to draw them as cards"
                                };
                                // An icon's own tooltip is its label: there
                                // the longer text, under the button's name.
                                let labelled = checks_labelled;
                                let button = if labelled {
                                    widgets::action(icon, CHECKS_SWITCH).compact().quiet()
                                } else {
                                    widgets::icon_button(icon, hover)
                                        .named(CHECKS_SWITCH)
                                        .compact()
                                };
                                let mut response = ui.add(button.toggle(shown));
                                if labelled {
                                    response = response.on_hover_text(hover);
                                }
                                if response.clicked() {
                                    commands.push(Command::SetChecksCollapsed(shown));
                                }
                            }
                            widgets::action_menu(
                                ui,
                                // How the board draws, beside each other: wires, and
                                // what a card holds (LAY-16, decided 2026-11-09).
                                widgets::icon_button(Icon::Wire, "Board view").compact(),
                                |ui| {
                                    // *Curves* returns when it draws curves (UX-28).
                                    let style = WireStyle::Orthogonal;
                                    if ui
                                        .add(
                                            widgets::action(
                                                if self.wire_style == style {
                                                    Icon::Check
                                                } else {
                                                    Icon::Wire
                                                },
                                                "Right angles",
                                            )
                                            .quiet(),
                                        )
                                        .clicked()
                                    {
                                        commands.push(Command::SetWireStyle(style));
                                        ui.close();
                                    }
                                    ui.separator();
                                    if ui
                                        .add(
                                            widgets::action(
                                                if self.bridges {
                                                    Icon::Check
                                                } else {
                                                    Icon::Wire
                                                },
                                                "Bridges at crossings",
                                            )
                                            .quiet(),
                                        )
                                        .clicked()
                                    {
                                        commands.push(Command::SetBridges(!self.bridges));
                                        ui.close();
                                    }
                                    if self.density_switch {
                                        ui.separator();
                                        let compact = self.density == CardDensity::Compact;
                                        if ui
                                            .add_enabled(
                                                editable,
                                                widgets::action(
                                                    if compact {
                                                        Icon::Check
                                                    } else {
                                                        Icon::Density
                                                    },
                                                    "Compact cards",
                                                )
                                                .quiet(),
                                            )
                                            .on_hover_text(
                                                "A request card shows only what it uses; Tidy up repacks the board",
                                            )
                                            .clicked()
                                        {
                                            commands.push(Command::SetCompactCards(!compact));
                                            ui.close();
                                        }
                                        // Which data wires are drawn (LAY-23).
                                        ui.separator();
                                        for (mode, label, hover) in [
                                            (
                                                WireMode::All,
                                                "All wires",
                                                "Draw every data wire",
                                            ),
                                            (
                                                WireMode::Chips,
                                                "Chips for shared values",
                                                "A value three or more steps read is named at each far reader instead of a long wire; point at it or select a step to draw the wire",
                                            ),
                                        ] {
                                            if ui
                                                .add(
                                                    widgets::action(
                                                        if self.wire_mode == mode {
                                                            Icon::Check
                                                        } else {
                                                            Icon::Wire
                                                        },
                                                        label,
                                                    )
                                                    .quiet(),
                                                )
                                                .on_hover_text(hover)
                                                .clicked()
                                            {
                                                commands.push(Command::SetWires(mode));
                                                ui.close();
                                            }
                                        }
                                    }
                                },
                            );
                            // Keep infrequent selection actions in one menu rather
                            // than covering the board with disabled commands.
                            if has_selection || !self.selected_wires.is_empty() {
                                widgets::action_menu(
                                    ui,
                                    widgets::icon_button(Icon::Layers, "Selection actions")
                                        .compact(),
                                    |ui| {
                                        let items = [
                                            (Icon::Copy, "Copy", "Ctrl+C", Command::Copy),
                                            (Icon::Copy, "Duplicate", "Ctrl+D", Command::Duplicate),
                                            (
                                                Icon::Save,
                                                "Save as component",
                                                "",
                                                Command::SaveSelectionAsComponent,
                                            ),
                                        ];
                                        // A variable chip is a view of a value, not
                                        // a step, so a chip alone has nothing to
                                        // copy (UX-34).
                                        let has_block = input.selection.iter().any(|id| {
                                            input.graph.nodes.iter().any(|node| &node.id == id)
                                        });
                                        for (icon, label, shortcut, command) in items {
                                            let copies = matches!(
                                                command,
                                                Command::Copy | Command::Duplicate
                                            );
                                            let allowed = has_selection
                                                && (editable || matches!(command, Command::Copy))
                                                && (has_block || !copies);
                                            let mut response = ui
                                                .add_enabled(
                                                    allowed,
                                                    widgets::action(icon, label).quiet(),
                                                )
                                                .on_hover_text(shortcut);
                                            if copies && has_selection && !has_block {
                                                response = response
                                                    .on_disabled_hover_text(CHIP_NOT_COPIED);
                                            }
                                            if response.clicked() {
                                                commands.push(command);
                                                ui.close();
                                            }
                                        }
                                        ui.separator();
                                        if ui
                                            .add_enabled(
                                                editable,
                                                widgets::action(Icon::Trash, "Delete selection")
                                                    .danger(),
                                            )
                                            .clicked()
                                        {
                                            commands.push(Command::Delete {
                                                ids: input.selection.to_vec(),
                                                wires: std::mem::take(&mut self.selected_wires),
                                            });
                                            ui.close();
                                        }
                                    },
                                );
                            }
                        });
                    });
            });
        let key = self.toolbar_key(ui, input, shape);
        let size = measured.response.rect.size();
        if ui.ctx().data(|data| data.get_temp::<Vec2>(key)) != Some(size) {
            ui.ctx().data_mut(|data| data.insert_temp(key, size));
            ui.ctx().request_repaint();
        }
    }

    fn screen_camera(&self, viewport: Rect) -> Camera {
        Camera {
            zoom: self.camera.zoom,
            pan: self.camera.pan + viewport.min.to_vec2(),
        }
    }

    fn zoom_controls(
        &mut self,
        ui: &mut egui::Ui,
        viewport: Rect,
        overlays: &BoardOverlays,
        shapes: Shapes<'_>,
    ) {
        egui::Area::new(ui.id().with("board-zoom"))
            .fixed_pos(overlays.zoom_controls.left_center())
            .pivot(Align2::LEFT_CENTER)
            .fade_in(false)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(theme::WHITE)
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(4, 2))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            ui.spacing_mut().button_padding = vec2(6.0, 3.0);
                            ui.spacing_mut().interact_size = vec2(24.0, 26.0);
                            let center = overlays.focus.center();
                            if ui
                                .add(widgets::icon_button(Icon::Minus, "Zoom out").compact())
                                .clicked()
                            {
                                self.zoom_at(viewport, center, 1.0 / 1.2);
                            }
                            ui.add_sized(
                                [44.0, 26.0],
                                egui::Label::new(
                                    egui::RichText::new(format!(
                                        "{}%",
                                        (self.camera.zoom * 100.0).round() as u32
                                    ))
                                    .size(11.0)
                                    .color(theme::TEXT),
                                ),
                            );
                            if ui
                                .add(widgets::icon_button(Icon::Plus, "Zoom in").compact())
                                .clicked()
                            {
                                self.zoom_at(viewport, center, 1.2);
                            }
                            ui.separator();
                            if ui
                                .add(widgets::icon_button(Icon::Fit, "Fit the view").compact())
                                .clicked()
                            {
                                self.fit(viewport, overlays.focus, shapes);
                            }
                        });
                    });
            });
        if let HintPlacement::Tooltip(badge_rect) = overlays.hint {
            egui::Area::new(ui.id().with("board-shortcuts-hint"))
                .fixed_pos(badge_rect.right_center())
                .pivot(Align2::RIGHT_CENTER)
                .fade_in(false)
                .show(ui.ctx(), |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new("Shortcuts")
                                .size(11.0)
                                .color(theme::MUTED),
                        )
                        .sense(Sense::hover()),
                    )
                    .on_hover_text(paint::SHORTCUT_HINT_TEXT);
                });
        }
    }

    fn empty_state(
        &mut self,
        ui: &mut egui::Ui,
        viewport: Rect,
        input: &Input<'_>,
        commands: &mut Vec<Command>,
    ) {
        let library_in_view = input.library_in_view;
        let rect = Rect::from_center_size(viewport.center() - vec2(0.0, 40.0), vec2(360.0, 150.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("From the first request to a complete test")
                        .size(17.0)
                        .family(theme::bold_family()),
                );
                let hint = if library_in_view {
                    "Drag a block from the library or add the first step."
                } else {
                    "Add the first step."
                };
                ui.label(egui::RichText::new(hint).color(theme::MUTED));
                ui.add_space(8.0);
                if ui
                    .add_enabled(
                        !input.read_only,
                        widgets::action(Icon::Plus, "Add an HTTP request"),
                    )
                    .clicked()
                {
                    commands.push(Command::AddNode {
                        block: Block::Request,
                        component: None,
                        position: Position { x: 100.0, y: 100.0 },
                        source: None,
                    });
                }
            });
        });
    }

    fn error_banner(&mut self, ui: &mut egui::Ui, overlays: &BoardOverlays) {
        let Some(message) = self.error.clone() else {
            return;
        };
        let pos = overlays
            .error_banner
            .map_or(overlays.board.center_top() + vec2(0.0, 24.0), |r| {
                r.center_top()
            });
        egui::Area::new(ui.id().with("board-error"))
            .fixed_pos(pos)
            .pivot(Align2::CENTER_TOP)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgb(253, 236, 232))
                    .stroke(Stroke::new(1.0, theme::DANGER))
                    .corner_radius(8)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        // A long message, such as a refused delete naming its
                        // readers (DIAG-06e), wraps within the board.
                        let width = (overlays.board.width() - 80.0).clamp(160.0, 520.0);
                        ui.horizontal_top(|ui| {
                            ui.allocate_ui_with_layout(
                                vec2(width, 0.0),
                                egui::Layout::top_down(egui::Align::LEFT),
                                |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&message).color(theme::DANGER),
                                        )
                                        .wrap(),
                                    );
                                },
                            );
                            if ui
                                .add(widgets::icon_button(Icon::Close, "Dismiss").compact())
                                .clicked()
                            {
                                self.error = None;
                            }
                        });
                    });
            });
    }
}

/// The zoom that shows `bounds` whole in `focus`.
fn fit_zoom(focus: Rect, bounds: Rect) -> f32 {
    (focus.width().max(60.0) / bounds.width().max(1.0))
        .min(focus.height().max(60.0) / bounds.height().max(1.0))
        .clamp(MIN_ZOOM, 1.0)
}

impl Shapes<'_> {
    /// Everything the board draws, as one rectangle.
    fn bounds(self) -> Option<Rect> {
        self.cards
            .iter()
            .map(Card::rect)
            .chain(self.chips.iter().map(Chip::rect))
            .chain(self.extents.iter().copied())
            .reduce(Rect::union)
    }
}

fn shapes<'a>(cards: &'a [Card], chips: &'a [Chip], extents: &'a [Rect]) -> Shapes<'a> {
    Shapes {
        cards,
        chips,
        extents,
    }
}

/// What dragging `ids` moves (LAY-31, the owner's decision 2 = A): a spine
/// step takes the cards hung under it along, its folded checks among them,
/// so its group stays one; a card hung under a step and dragged alone
/// leaves the stack until the next Tidy up. `cards` are every block's card
/// where the document places it.
fn with_hung(mut ids: Vec<String>, cards: &[Card]) -> Vec<String> {
    let groups = shape::groups(cards);
    let steps: Vec<usize> = cards
        .iter()
        .enumerate()
        .filter(|(_, card)| shape::on_spine(card.kind) && ids.contains(&card.id))
        .map(|(index, _)| index)
        .collect();
    for step in steps {
        for (index, card) in cards.iter().enumerate() {
            if shape::hung_under(cards, &groups, step, index) && !ids.contains(&card.id) {
                ids.push(card.id.clone());
            }
        }
    }
    ids
}

/// The positions a drag leaves behind, for the blocks and proxies it moved.
fn moved_positions(graph: &Graph, ids: &[String], offset: Vec2) -> Vec<(String, Position)> {
    let moved = |position: &Position| Position {
        x: (position.x + f64::from(offset.x)).round(),
        y: (position.y + f64::from(offset.y)).round(),
    };
    graph
        .nodes
        .iter()
        .filter(|node| ids.contains(&node.id))
        .map(|node| (node.id.clone(), moved(&node.position)))
        .chain(
            graph
                .proxies
                .iter()
                .filter(|proxy| ids.contains(&proxy.id))
                .map(|proxy| (proxy.id.clone(), moved(&proxy.position))),
        )
        .collect()
}

/// What a dragged value can be placed as, or `None` for something no proxy stands
/// for: a field of a response, or a value that is not a variable.
fn placeable(binding: &Binding, graph: &Graph) -> Option<ProxySource> {
    let source = apiforge_core::edit::proxy_source(binding)?;
    match &source {
        ProxySource::Variable { node_id } => graph
            .nodes
            .iter()
            .any(|node| &node.id == node_id && node.is_variable())
            .then_some(source),
        ProxySource::Parameter { .. } | ProxySource::Environment { .. } => Some(source),
    }
}

/// Whether a binding reads what a proxy stands for, a field of it included.
fn reads_source(source: &ProxySource, leaf: &Binding) -> bool {
    match (source, leaf) {
        (ProxySource::Variable { node_id }, Binding::Reference { node_id: read, .. }) => {
            read == node_id
        }
        (ProxySource::Parameter { name }, Binding::Parameter { name: read, .. })
        | (ProxySource::Environment { name }, Binding::Environment { name: read }) => read == name,
        _ => false,
    }
}

/// The nearest proxy `wanted` accepts, with its distance to `end`.
fn nearest_chip(chips: &[Chip], wanted: impl Fn(&Chip) -> bool, end: Pos2) -> Option<(usize, f32)> {
    chips
        .iter()
        .enumerate()
        .filter(|(_, chip)| wanted(chip))
        .map(|(index, chip)| (index, chip.output_anchor().distance(end)))
        .min_by(|left, right| left.1.total_cmp(&right.1))
}

/// Where every wire of the board starts and ends.
///
/// A consumer takes its value from the nearest place that value is shown: a proxy
/// of it, or the block itself when the block is nearer. Each reference draws
/// exactly one wire, however many proxies stand for it. A parameter or an
/// environment value has no block, so it is drawn only where it has been placed —
/// without a proxy such a binding is text on the card, as before.
#[cfg(test)]
fn board_wires(graph: &Graph, cards: &[Card], chips: &[Chip]) -> Vec<routing::BoardWire> {
    folded_wires(graph, cards, chips, &HashMap::new()).0
}

/// The board's wires with the checks in `folded` taken out: a wire to or
/// from a folded check is not drawn, except an `@after` on one, which is
/// drawn from the check's subject and labelled with the check's name, so
/// the order it sets stays on the board (LAY-11). Returns the labels by
/// wire id beside the wires.
fn folded_wires(
    graph: &Graph,
    cards: &[Card],
    chips: &[Chip],
    folded: &HashMap<String, (String, String)>,
) -> (Vec<routing::BoardWire>, HashMap<String, String>) {
    let card_of = |id: &str| cards.iter().position(|card| card.id == id);
    let mut result = Vec::new();
    let mut labels = HashMap::new();
    // Where an `if` has its own *Next*, the join lines from the ends of its
    // branches are not drawn: one line leaves its *Next* (DIAG-06c).
    let joins = if cards
        .iter()
        .any(|card| card.kind == NodeKind::If && card.chain_out)
    {
        Joins::of(graph)
    } else {
        Joins::default()
    };
    let mut lined: HashSet<&str> = HashSet::new();
    for wire in wires(graph) {
        if joins.skip.contains(&wire.id) {
            // The line out of the `if`'s *Next* is routed where its first
            // join line was, in the board's order of lines.
            if let Some((x, next)) = joins.after.iter().find(|(_, next)| *next == wire.target)
                && lined.insert(x.as_str())
                && let (Some(source), Some(target)) = (card_of(x), card_of(next))
            {
                result.push(routing::BoardWire {
                    id: Joins::line_id(x),
                    start: routing::Start::Output {
                        card: source,
                        port: ORDER_PORT.to_owned(),
                    },
                    target,
                    input: ORDER_PORT.to_owned(),
                    operand_path: Vec::new(),
                    control: true,
                    branch: None,
                });
            }
            continue;
        }
        let redrawn = match folded.get(&wire.source) {
            Some((subject, name)) if wire.kind == WireKind::Order => Some((subject, name)),
            _ => None,
        };
        // Nor is the order or the branch into it drawn: a check a step shows
        // under it while the board folds the others reads as part of that
        // step, as when it is folded, so that no line is rerouted (LAY-30).
        if wire.kind == WireKind::Order && folded.contains_key(&wire.target) {
            continue;
        }
        let source_id = redrawn.map_or(wire.source.as_str(), |(subject, _)| subject.as_str());
        let (Some(source), Some(target)) = (card_of(source_id), card_of(&wire.target)) else {
            continue;
        };
        if source == target {
            continue;
        }
        if let Some((_, name)) = redrawn {
            labels.insert(wire.id.clone(), format!("after {name}"));
        }
        let (output, input) = wire_ports(&wire);
        let from_block = routing::Start::Output {
            card: source,
            port: output.clone(),
        };
        let start = match (
            wire.kind,
            cards[target].input_anchor(&input, &wire.operand_path),
        ) {
            (WireKind::Data, Some(end)) => {
                let direct = cards[source]
                    .output_anchor(&output)
                    .map_or(f32::INFINITY, |anchor| anchor.distance(end));
                match nearest_chip(
                    chips,
                    |chip| chip.source_node() == Some(wire.source.as_str()),
                    end,
                ) {
                    Some((index, distance)) if distance < direct => routing::Start::Chip(index),
                    _ => from_block,
                }
            }
            _ => from_block,
        };
        result.push(routing::BoardWire {
            id: wire.id,
            start,
            target,
            input,
            operand_path: wire.operand_path,
            control: wire.kind == WireKind::Order,
            branch: wire.branch,
        });
    }
    for node in &graph.nodes {
        let Some(target) = card_of(&node.id) else {
            continue;
        };
        for (input, binding) in &node.inputs {
            binding.for_each_leaf(&mut |leaf, path| {
                // References are wires of the document already; a parameter or an
                // environment value becomes one only where it is placed.
                if matches!(leaf, Binding::Reference { .. }) {
                    return;
                }
                let Some(end) = cards[target].input_anchor(input, path) else {
                    return;
                };
                let Some((index, _)) =
                    nearest_chip(chips, |chip| reads_source(&chip.proxy.source, leaf), end)
                else {
                    return;
                };
                result.push(routing::BoardWire {
                    id: data_edge_id(&node.id, input, path),
                    start: routing::Start::Chip(index),
                    target,
                    input: input.clone(),
                    operand_path: path.to_vec(),
                    control: false,
                    branch: None,
                });
            });
        }
    }
    (result, labels)
}

/// The chain after the `if`s of a board on which an `if` has its own *Next*
/// (DIAG-06c): which ordering edges join the ends of its branches into the
/// step after it, which are drawn as one line from the `if` instead, and
/// which cards end a branch and name that step.
#[derive(Debug, Default)]
pub(crate) struct Joins {
    /// The ordering edges from the ends of an `if`'s branches into the
    /// step after it.
    pub skip: HashSet<String>,
    /// Each `if` with a step after it, and that step.
    pub after: Vec<(String, String)>,
    /// Each card that ends a branch, and the step after its `if`.
    pub ends: Vec<(String, String)>,
}

impl Joins {
    /// The identifier of the line from an `if`'s *Next*, which no edge of
    /// the document has: deleting it is refused as any ordering line is.
    pub fn line_id(x: &str) -> String {
        format!("{AFTER_IF}{x}")
    }

    pub fn of(graph: &Graph) -> Self {
        let branches = apiforge_core::branches::Branches::new(graph);
        let kinds: HashMap<&str, NodeKind> = graph
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node.kind))
            .collect();
        let mut joins = Self::default();
        for node in &graph.nodes {
            let sources: Vec<(&str, Option<apiforge_core::model::Branch>)> = graph
                .edges
                .iter()
                .filter(|edge| edge.target == node.id)
                .map(|edge| (edge.source.as_str(), edge.branch))
                .collect();
            for (x, branch) in branches.shorten(sources.clone()) {
                if branch.is_some()
                    || kinds.get(x) != Some(&NodeKind::If)
                    || sources.contains(&(x, None))
                {
                    continue;
                }
                let Some(ends) = branches.ends(x) else {
                    continue;
                };
                for edge in &graph.edges {
                    if edge.target == node.id && ends.contains(&(edge.source.as_str(), edge.branch))
                    {
                        joins.skip.insert(edge.id.clone());
                    }
                }
                joins.after.push((x.to_owned(), node.id.clone()));
                for &(end, branch) in ends {
                    if branch.is_none() {
                        joins.ends.push((end.to_owned(), node.id.clone()));
                    }
                }
            }
        }
        joins
    }
}

/// What a wire carried from the *Next* of `after`, whose successor is
/// `before`, will do (DIAG-06c): a new step between them over the board, or
/// the step `over` moved after `after`, with what is inside it and the
/// braces it goes into.
fn carry_caption(graph: &Graph, after: &str, before: &str, over: Option<&str>) -> String {
    let handle = |id: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.id == id)
            .map_or_else(|| id.to_owned(), |node| node.handle.clone())
    };
    let Some(over) = over else {
        return format!(
            "New step after `{}`, before `{}`",
            handle(after),
            handle(before)
        );
    };
    let mut text = format!("Move `{}` after `{}`", handle(over), handle(after));
    let braces = apiforge_core::chain::braces(graph);
    let inside = |id: &str| {
        let mut current = id;
        while let Some(&(parent, _)) = braces.get(current) {
            if parent == over {
                return true;
            }
            current = parent;
        }
        false
    };
    let within: Vec<&apiforge_core::format::Node> =
        graph.nodes.iter().filter(|node| inside(&node.id)).collect();
    let steps = within.iter().filter(|node| !node.kind.is_data()).count();
    match (steps, within.len()) {
        (0, 0) => {}
        (1, _) => text.push_str(" and the step inside it"),
        (0, 1) => text.push_str(" and the statement inside it"),
        (0, count) => text.push_str(&format!(" and the {count} statements inside it")),
        (count, _) => text.push_str(&format!(" and the {count} steps inside it")),
    }
    let to = braces.get(after).copied();
    let from = braces.get(over).copied();
    if to != from {
        match (to, from) {
            (Some((x, branch)), _) => text.push_str(&format!(
                " into {} of `{}`",
                if branch == apiforge_core::model::Branch::True {
                    "Yes"
                } else {
                    "No"
                },
                handle(x)
            )),
            (None, Some((y, _))) => text.push_str(&format!(" out of `{}`", handle(y))),
            (None, None) => {}
        }
    }
    text
}

/// The prefix of the line from an `if`'s own *Next* (DIAG-06c).
pub(crate) const AFTER_IF: &str = "after-if:";

/// The one selected step and what it is related to (LAY-10, Q9 = A): the
/// cards its halo rings, each in its colour. Every other card and wire is
/// dimmed while it is selected.
struct Focus {
    selected: String,
    rings: HashMap<String, Color32>,
}

impl Focus {
    /// The focus of `selection` when it is one step of `graph`: its direct
    /// producers, readers and ordering neighbours in the related hue, its
    /// checks and gates, or a selected check's subjects and values, in the
    /// check colour. An `@after` on a folded check is drawn from its
    /// subject: the step it reaches from there is ringed too.
    fn of(
        graph: &Graph,
        selection: &[String],
        wires: &[routing::BoardWire],
        cards: &[Card],
    ) -> Option<Self> {
        let [selected] = selection else {
            return None;
        };
        let node = graph.nodes.iter().find(|node| &node.id == selected)?;
        let found = relations(graph, node);
        let mut rings: HashMap<String, Color32> = HashMap::new();
        let check = paint::check_ring();
        for id in found
            .checks
            .iter()
            .map(|of| of.check)
            .chain(found.gates.iter().copied())
            .chain(found.subjects.iter().map(|subject| subject.step))
            .chain(found.values.iter().copied())
        {
            rings.insert(id.to_owned(), check);
        }
        for id in found
            .producers
            .iter()
            .chain(&found.readers)
            .map(|feed| feed.step)
            .chain(found.after.iter().copied())
            .chain(found.followers.iter().copied())
        {
            rings.entry(id.to_owned()).or_insert(paint::RELATED);
        }
        for wire in wires.iter().filter(|wire| wire.control) {
            let routing::Start::Output { card, .. } = &wire.start else {
                continue;
            };
            let (source, target) = (&cards[*card].id, &cards[wire.target].id);
            if source == selected {
                rings.entry(target.clone()).or_insert(paint::RELATED);
            } else if target == selected {
                rings.entry(source.clone()).or_insert(paint::RELATED);
            }
        }
        rings.remove(selected);
        Some(Self {
            selected: selected.clone(),
            rings,
        })
    }

    /// Whether `wire` is the selected step's: one of its ends is the step,
    /// or a chip of it.
    fn lit(&self, wire: &routing::BoardWire, cards: &[Card], chips: &[Chip]) -> bool {
        let source = match &wire.start {
            routing::Start::Output { card, .. } => Some(cards[*card].id.as_str()),
            routing::Start::Chip(index) => chips[*index].source_node(),
        };
        cards[wire.target].id == self.selected || source == Some(self.selected.as_str())
    }
}

/// What the selected proxies stand for and everything that reads it: the blocks to
/// ring and the wires to pick out.
fn highlights(
    graph: &Graph,
    chips: &[Chip],
    selection: &HashSet<&str>,
) -> (HashSet<String>, HashSet<String>) {
    let mut blocks = HashSet::new();
    let mut wire_ids = HashSet::new();
    for chip in chips.iter().filter(|chip| selection.contains(chip.id())) {
        if let Some(node) = chip.source_node() {
            blocks.insert(node.to_owned());
        }
        let (readers, wires) = readers(graph, &chip.proxy.source);
        blocks.extend(readers);
        wire_ids.extend(wires);
    }
    (blocks, wire_ids)
}

/// The blocks that read the parameter or component input `name`.
pub fn input_readers(graph: &Graph, name: &str) -> HashSet<String> {
    readers(
        graph,
        &ProxySource::Parameter {
            name: name.to_owned(),
        },
    )
    .0
}

/// The blocks that read what `source` stands for, a field of it included,
/// and the data wires a proxy of it would draw to them.
fn readers(graph: &Graph, source: &ProxySource) -> (HashSet<String>, HashSet<String>) {
    let mut blocks = HashSet::new();
    let mut wire_ids = HashSet::new();
    for node in &graph.nodes {
        for (input, binding) in &node.inputs {
            binding.for_each_leaf(&mut |leaf, path| {
                if reads_source(source, leaf) {
                    blocks.insert(node.id.clone());
                    wire_ids.insert(data_edge_id(&node.id, input, path));
                }
            });
        }
    }
    (blocks, wire_ids)
}

fn toggle(list: &mut Vec<String>, id: &str) {
    if let Some(index) = list.iter().position(|item| item == id) {
        list.remove(index);
    } else {
        list.push(id.to_owned());
    }
}

fn toggle_selection(selection: &[String], id: &str, modifiers: Modifiers) -> Vec<String> {
    if modifiers.command || modifiers.shift {
        let mut next = selection.to_vec();
        toggle(&mut next, id);
        next
    } else {
        vec![id.to_owned()]
    }
}

fn branch_of(port: &str) -> Option<Branch> {
    match port {
        apiforge_core::graph::TRUE_PORT => Some(Branch::True),
        apiforge_core::graph::FALSE_PORT => Some(Branch::False),
        _ => None,
    }
}

/// The output and input names a wire connects on its cards.
fn wire_ports(wire: &apiforge_core::graph::Wire) -> (String, String) {
    match wire.kind {
        WireKind::Order => (
            match wire.branch {
                Some(Branch::True) => apiforge_core::graph::TRUE_PORT,
                Some(Branch::False) => apiforge_core::graph::FALSE_PORT,
                None => ORDER_PORT,
            }
            .to_owned(),
            ORDER_PORT.to_owned(),
        ),
        WireKind::Data => (
            wire.source_port.clone().unwrap_or_default(),
            wire.target_input.clone().unwrap_or_default(),
        ),
    }
}

/// The wire under the pointer, within a few pixels of the route that was drawn.
fn hit_wire(drawn: &[(String, Vec<Pos2>)], camera: Camera, pointer: Pos2) -> Option<String> {
    drawn
        .iter()
        .filter_map(|(id, points)| {
            let distance = points
                .windows(2)
                .map(|pair| {
                    routing::distance_to_segment(
                        pointer,
                        camera.to_screen(pair[0]),
                        camera.to_screen(pair[1]),
                    )
                })
                .fold(f32::INFINITY, f32::min);
            (distance <= 6.0).then(|| (distance, id.clone()))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_core::{
        format::{Node, Proxy},
        graph::create_node,
    };

    fn board(variable_x: f64, consumer_x: f64, binding: Binding) -> (Graph, Project) {
        let mut variable = create_node(
            NodeKind::Value,
            Position {
                x: variable_x,
                y: 0.0,
            },
            None,
        );
        variable.id = "token".into();
        variable.handle = "token".into();
        variable.config.variable = Some(true);
        let mut consumer = create_node(
            NodeKind::Assert,
            Position {
                x: consumer_x,
                y: 0.0,
            },
            None,
        );
        consumer.id = "check".into();
        consumer.handle = "check".into();
        consumer.inputs.insert("actual".into(), binding);
        let project = apiforge_core::demos::shop_project();
        (
            Graph {
                nodes: vec![variable, consumer],
                edges: vec![],
                proxies: Vec::new(),
            },
            project,
        )
    }

    fn placed(graph: &Graph, source: ProxySource, x: f64) -> Graph {
        let mut next = graph.clone();
        next.proxies.push(Proxy {
            id: format!("proxy-{}", next.proxies.len()),
            source,
            position: Position { x, y: 40.0 },
        });
        next
    }

    fn drawn(graph: &Graph, project: &Project) -> Vec<routing::BoardWire> {
        let cards: Vec<Card> = graph
            .nodes
            .iter()
            .map(|node: &Node| Card::build(node, graph, project, CardDensity::Full))
            .collect();
        let chips: Vec<Chip> = graph
            .proxies
            .iter()
            .map(|proxy| Chip::build(proxy, graph, project, &["quantity".to_owned()]))
            .collect();
        board_wires(graph, &cards, &chips)
    }

    #[test]
    fn a_consumer_takes_its_value_from_the_nearest_place_it_is_shown() {
        let (graph, project) = board(0.0, 3000.0, Binding::reference("token", "value", ""));
        // Without a proxy the wire crosses the board from the block itself.
        let wires = drawn(&graph, &project);
        assert_eq!(wires.len(), 1);
        assert!(matches!(
            wires[0].start,
            routing::Start::Output { card: 0, .. }
        ));

        // A proxy beside the consumer is nearer, so the wire starts there instead —
        // and it is still one wire, not two.
        let near = placed(
            &graph,
            ProxySource::Variable {
                node_id: "token".into(),
            },
            2700.0,
        );
        let wires = drawn(&near, &project);
        assert_eq!(wires.len(), 1);
        assert_eq!(wires[0].start, routing::Start::Chip(0));
        assert_eq!(wires[0].target, 1);

        // A proxy that sits by the block changes nothing about where the wire
        // starts, so placing one never makes the board longer.
        let far = placed(
            &graph,
            ProxySource::Variable {
                node_id: "token".into(),
            },
            -200.0,
        );
        let wires = drawn(&far, &project);
        assert_eq!(wires.len(), 1);
        assert!(matches!(
            wires[0].start,
            routing::Start::Output { card: 0, .. }
        ));

        // Of several places, the nearest wins.
        let both = placed(
            &far,
            ProxySource::Variable {
                node_id: "token".into(),
            },
            2700.0,
        );
        let wires = drawn(&both, &project);
        assert_eq!(wires.len(), 1);
        assert_eq!(wires[0].start, routing::Start::Chip(1));
    }

    #[test]
    fn a_parameter_is_drawn_only_where_it_has_been_placed() {
        let (graph, project) = board(0.0, 600.0, Binding::parameter_field("quantity", "unit"));
        assert!(drawn(&graph, &project).is_empty(), "no proxy, no wire");
        let with_proxy = placed(
            &graph,
            ProxySource::Parameter {
                name: "quantity".into(),
            },
            400.0,
        );
        let wires = drawn(&with_proxy, &project);
        assert_eq!(wires.len(), 1);
        assert_eq!(wires[0].start, routing::Start::Chip(0));
        assert_eq!(wires[0].input, "actual");
        assert!(!wires[0].control);

        // The proxy stands for the whole parameter, so a field of it is a use too.
        let selection = HashSet::from(["proxy-0"]);
        let chips = vec![Chip::build(
            &with_proxy.proxies[0],
            &with_proxy,
            &project,
            &["quantity".to_owned()],
        )];
        let (blocks, highlighted) = highlights(&with_proxy, &chips, &selection);
        assert_eq!(blocks, HashSet::from(["check".to_owned()]));
        assert_eq!(highlighted, HashSet::from([wires[0].id.clone()]));
    }

    #[test]
    fn selecting_a_proxy_points_at_its_variable_and_everything_reading_it() {
        let (graph, project) = board(0.0, 600.0, Binding::reference("token", "value", "id"));
        let graph = placed(
            &graph,
            ProxySource::Variable {
                node_id: "token".into(),
            },
            400.0,
        );
        let chips = vec![Chip::build(&graph.proxies[0], &graph, &project, &[])];
        assert!(chips[0].known);
        assert_eq!(chips[0].source_node(), Some("token"));
        let (blocks, wires) = highlights(&graph, &chips, &HashSet::from(["proxy-0"]));
        assert_eq!(
            blocks,
            HashSet::from(["token".to_owned(), "check".to_owned()])
        );
        assert_eq!(wires.len(), 1);
        // Nothing is highlighted while the proxy is not selected.
        assert_eq!(
            highlights(&graph, &chips, &HashSet::new()),
            (HashSet::new(), HashSet::new())
        );
    }

    #[test]
    fn a_response_field_is_not_something_the_board_can_place() {
        let (graph, _) = board(0.0, 600.0, Binding::literal(serde_json::json!(1)));
        // What the Variables panel offers can be placed.
        assert_eq!(
            placeable(&Binding::reference("token", "value", ""), &graph),
            Some(ProxySource::Variable {
                node_id: "token".into()
            })
        );
        assert_eq!(
            placeable(&Binding::environment("baseUrl"), &graph),
            Some(ProxySource::Environment {
                name: "baseUrl".into()
            })
        );
        // A field of a response, a plain Value and a literal cannot: they are not
        // listed with the variables, so there would be nothing to show.
        assert_eq!(
            placeable(&Binding::reference("token", "value", "id"), &graph),
            None
        );
        assert_eq!(
            placeable(&Binding::reference("check", "value", ""), &graph),
            None
        );
        assert_eq!(
            placeable(&Binding::literal(serde_json::json!("x")), &graph),
            None
        );
    }

    #[test]
    fn a_proxy_of_something_the_board_no_longer_has_says_so() {
        let (graph, project) = board(0.0, 600.0, Binding::parameter("gone"));
        let graph = placed(
            &graph,
            ProxySource::Parameter {
                name: "gone".into(),
            },
            400.0,
        );
        let chip = Chip::build(&graph.proxies[0], &graph, &project, &[]);
        assert!(!chip.known);
        assert_eq!(chip.label, "gone");
        assert_eq!(chip.source_node(), None);
    }

    #[test]
    fn adding_a_block_uses_the_focus_center_with_viewport_offsets() {
        let canvas = Canvas {
            camera: Camera {
                zoom: 0.7,
                pan: vec2(12.0, -35.0),
            },
            last_viewport: Rect::from_min_size(egui::pos2(235.0, 220.0), vec2(800.0, 500.0)),
            last_focus: Some(Rect::from_min_size(
                egui::pos2(259.0, 290.0),
                vec2(600.0, 340.0),
            )),
            ..Default::default()
        };
        let position = canvas.visible_center();
        let world_center = egui::pos2(
            position.x as f32 + layout::WIDTH / 2.0,
            position.y as f32 + 80.0,
        );
        let actual = canvas
            .screen_camera(canvas.last_viewport)
            .to_screen(world_center);
        assert!(actual.distance(canvas.last_focus.unwrap().center()) < 0.01);
    }
    /// #324: a change of the board pans a kept card only for what it newly
    /// hides; a focus that grows, or leaves the card as it was, pans
    /// nothing, even for a card the person left partly outside.
    #[test]
    fn a_change_of_the_board_pans_only_what_it_newly_hides() {
        assert_eq!(newly_hidden(-30.0, 0.0), -30.0, "newly hidden: all of it");
        assert_eq!(
            newly_hidden(-50.0, -30.0),
            -20.0,
            "hidden further: the rest"
        );
        assert_eq!(newly_hidden(-30.0, -30.0), 0.0, "as it was");
        assert_eq!(newly_hidden(0.0, -30.0), 0.0, "uncovered: nothing");
        assert_eq!(newly_hidden(-10.0, -30.0), 0.0, "less hidden: nothing");
        assert_eq!(newly_hidden(20.0, -30.0), 20.0, "now on the other side");
    }

    #[test]
    fn navigator_reveal_survives_toolbar_and_drawer_layout_settling() {
        let context = egui::Context::default();
        crate::theme::apply(&context);
        let project = apiforge_core::demos::shop_project();
        let graph = &project.scenarios[0].graph;
        let target = &graph.nodes[0];
        let mut canvas = Canvas::default();
        let mut selection = Vec::new();
        for frame in 0..8 {
            if frame == 3 {
                selection = vec![target.id.clone()];
                canvas.reveal(target.id.clone());
            }
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        Pos2::ZERO,
                        vec2(if frame < 3 { 1200.0 } else { 850.0 }, 600.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        canvas.show(
                            ui,
                            &Input {
                                graph,
                                reference_graph: graph,
                                later: None,
                                project: &project,
                                parameters: &[],
                                selection: &selection,
                                steps: None,
                                ends: None,
                                progress: Progress::default(),
                                can_paste: false,
                                context: "test",
                                library_in_view: true,
                                read_only: false,
                                component: None,
                                header_leading: None,
                                library: None,
                                inputs: None,
                                selected_input: None,
                                adding_input: None,
                                settings: None,
                                arrangement: "",
                                scenario: None,
                                shared: &[],
                                moves: false,
                            },
                        );
                    });
                },
            );
            output.textures_delta.clear();
        }
        let card = Card::build(target, graph, &project, canvas.density);
        let actual = canvas.screen_camera(canvas.last_viewport).rect(card.rect());
        let focus = canvas.last_focus.unwrap();
        assert!(
            actual.center().distance(focus.center()) < 1.0,
            "{actual:?} outside center {focus:?}"
        );
        assert!(focus.expand(0.1).contains_rect(actual));
    }

    #[test]
    fn a_header_as_wide_as_its_room_shares_the_row_with_the_summary_and_toolbar() {
        let context = egui::Context::default();
        crate::theme::apply(&context);
        let project = apiforge_core::demos::shop_project();
        let graph = &project.scenarios[0].graph;
        for width in [960.0, 1600.0] {
            let mut canvas = Canvas::default();
            for frame in 0..3 {
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width, 600.0))),
                        ..Default::default()
                    },
                    |ui| {
                        egui::CentralPanel::default().show(ui, |ui| {
                            let mut input = Input {
                                graph,
                                reference_graph: graph,
                                later: None,
                                project: &project,
                                parameters: &[],
                                selection: &[],
                                steps: None,
                                ends: None,
                                progress: Progress::default(),
                                can_paste: false,
                                context: "test",
                                library_in_view: true,
                                read_only: false,
                                component: None,
                                header_leading: None,
                                library: None,
                                inputs: None,
                                selected_input: None,
                                adding_input: None,
                                settings: None,
                                arrangement: "",
                                scenario: None,
                                shared: &[],
                                moves: false,
                            };
                            let room = canvas.header_room(ui, &input);
                            input.header_leading =
                                Some(vec2(room.with_summary, theme::CONTROL_HEIGHT));
                            let leading = canvas.leading_rect(ui, &input);
                            // The toolbar's size is measured by the frames before.
                            if frame == 2 {
                                let overlays = layout::board_overlays(
                                    Canvas::viewport(ui),
                                    &canvas.measure_overlays(ui, &input),
                                );
                                let summary = ui
                                    .painter()
                                    .layout_no_wrap(
                                        paint::steps_summary(graph),
                                        paint::summary_font(),
                                        theme::MUTED,
                                    )
                                    .size()
                                    .x;
                                assert_eq!(leading, overlays.leading);
                                assert!((leading.width() - room.with_summary).abs() < 0.01);
                                assert!((room.alone - room.with_summary - summary).abs() < 0.01);
                                assert!(
                                    (leading.center().y - overlays.toolbar.center().y).abs() <= 3.0,
                                    "one row at {width} px"
                                );
                                assert!(overlays.banner.width() >= summary - 0.01);
                                assert!(overlays.banner.right() < overlays.toolbar.left());
                            }
                            canvas.show(ui, &input);
                        });
                    },
                );
                output.textures_delta.clear();
            }
        }
    }
    /// UX-33: on a board too narrow for any header beside the toolbar, the
    /// toolbar moves under the row and the header has the whole row, so a
    /// wider toolbar does not cut the file name short for nothing.
    #[test]
    fn a_header_has_the_whole_row_when_the_toolbar_moves_below() {
        let context = egui::Context::default();
        crate::theme::apply(&context);
        let project = apiforge_core::demos::shop_project();
        let component = &project.components[0];
        let graph = &component.graph;
        let mut canvas = Canvas::default();
        for frame in 0..3 {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(340.0, 600.0))),
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        let mut input = Input {
                            graph,
                            reference_graph: graph,
                            later: None,
                            project: &project,
                            parameters: &[],
                            selection: &[],
                            steps: None,
                            ends: None,
                            progress: Progress::default(),
                            can_paste: false,
                            context: "test",
                            library_in_view: true,
                            read_only: false,
                            component: None,
                            header_leading: None,
                            library: None,
                            inputs: Some(&component.inputs),
                            selected_input: None,
                            adding_input: None,
                            settings: Some(SettingsButton {
                                open: true,
                                available: true,
                                header_width: 200.0,
                            }),
                            arrangement: "",
                            scenario: None,
                            shared: &[],
                            moves: false,
                        };
                        let room = canvas.header_room(ui, &input);
                        input.header_leading = Some(vec2(200.0, theme::CONTROL_HEIGHT));
                        if frame == 2 {
                            let row = Canvas::viewport(ui).width()
                                - layout::OVERLAY_PADDING_LEFT
                                - layout::OVERLAY_PADDING_RIGHT;
                            assert!((room.alone - row).abs() < 0.01, "{room:?} of {row}");
                            assert_eq!(canvas.settings_shape(ui, &input), SettingsShape::Icon);
                            let overlays = layout::board_overlays(
                                Canvas::viewport(ui),
                                &canvas.measure_overlays(ui, &input),
                            );
                            assert!((overlays.leading.width() - 200.0).abs() < 0.01);
                            assert!(overlays.toolbar.top() >= overlays.leading.bottom());
                        }
                        canvas.show(ui, &input);
                    });
                },
            );
            output.textures_delta.clear();
        }
    }
}
