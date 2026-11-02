//! COMP-00 mock-up switches (throwaway branch `mock/comp-00`). The design
//! review views in `folder::comp00_review` turn them on for their own thread;
//! with every switch off the window behaves as on `main`.
#![allow(dead_code)]

use apiforge_core::format::{Component, OptionalJson, Port};
use std::cell::RefCell;

/// Q1: where a component's inputs are shown on its board.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Inputs {
    /// As on `main`: nowhere, unless a proxy was dragged out of the Library.
    #[default]
    Off,
    /// A: an *Inputs* card at the board's left edge, one row per input and
    /// a *+ Add input* row, wired to the steps that read each input.
    Card,
    /// B: one chip per input beside the component's entry card, wired.
    Chips,
    /// C: a strip of input pills under the board's header row; no wires,
    /// the steps reading the selected input are ringed.
    Band,
}

/// Q2: how *+ Add input* takes the new input's details.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Adding {
    #[default]
    Off,
    /// A: the Inspector opens a *New input* form, Name focused; the board
    /// shows a placeholder row.
    InspectorForm,
    /// B: an inline row on the board: name and type typed in place.
    InlineRow,
}

/// What the Inspector shows instead of a block.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub enum Panel {
    #[default]
    Off,
    /// Q2: the *New input* form.
    NewInput,
    /// Q3: the selected input, with an optional draft name that fails
    /// validation.
    Input {
        name: String,
        draft_name: Option<String>,
    },
    /// Q4: the component's settings, with nothing selected.
    Component,
}

/// Q6: the header's *Publish*.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Publish {
    /// As on `main`: *Publish*, enabled whenever publishable.
    #[default]
    Off,
    /// A: the working file equals revision `n`: disabled, reads *Published rn*.
    Latest(u32),
    /// A, after an edit: enabled, reads *Publish r(n+1)*.
    Changed(u32),
    /// B: disabled *Publish* with the revision as a caption beside it.
    LatestCaption(u32),
}

#[derive(Clone, Default, Debug)]
pub struct Flags {
    pub inputs: Inputs,
    pub adding: Adding,
    pub panel: Panel,
    pub publish: Publish,
    /// The input shown selected on the board.
    pub selected_input: Option<String>,
    /// The name being added (Q2), shown in the form and the placeholder row.
    pub new_name: String,
    /// Q5: the *Save as component* dialog over the scenario.
    pub save_dialog: bool,
    /// Q5: what the dialog does after writing the file.
    pub publish_choice: bool,
    /// The board shows the inputs at a working zoom, as revealing a block
    /// does, instead of fitting everything.
    pub reveal_inputs: bool,
}

/// An input of the component on the board, as the window shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardInput {
    pub name: String,
    pub label: Option<String>,
    /// `number`, or `object · Product` with a contract.
    pub type_label: String,
    /// The default as written in code, `1` or `"EUR"`.
    pub default: Option<String>,
    pub required: bool,
    pub exposed: bool,
    pub action: bool,
}

impl BoardInput {
    pub fn from_port(port: &Port, action: Option<&str>) -> Self {
        let mut type_label = port.value_type.as_str().to_owned();
        if let Some(contract) = &port.contract_id {
            type_label = format!("{type_label} · {contract}");
        }
        let default = match &port.default_value {
            OptionalJson::Present(value) => Some(value.to_string()),
            OptionalJson::Missing => None,
        };
        Self {
            name: port.name.clone(),
            label: port.label.clone(),
            type_label,
            // An input with a default is optional unless declared required
            // (KTLO-32).
            required: port.required.unwrap_or(default.is_none()),
            default,
            exposed: port.exposed.unwrap_or(true),
            action: action == Some(port.name.as_str()),
        }
    }

    /// `number = 1`, `object · Product`.
    pub fn detail(&self) -> String {
        match &self.default {
            Some(default) => format!("{} = {default}", self.type_label),
            None => self.type_label.clone(),
        }
    }
}

/// The component on the board this frame: its inputs, name and description.
#[derive(Clone, Debug, Default)]
pub struct Board {
    pub inputs: Vec<BoardInput>,
    pub name: String,
    pub handle: String,
    pub description: String,
    pub action: Option<String>,
    pub outputs: Vec<(String, String)>,
}

impl Board {
    pub fn of(component: &Component) -> Self {
        let action = component.action.as_ref().map(|action| action.input.clone());
        Self {
            inputs: component
                .inputs
                .iter()
                .map(|port| BoardInput::from_port(port, action.as_deref()))
                .collect(),
            name: component.name.clone(),
            handle: component.handle.clone(),
            description: component.description.clone(),
            action,
            outputs: component
                .graph
                .nodes
                .iter()
                .filter(|node| node.kind == apiforge_core::model::NodeKind::Export)
                .map(|node| {
                    let mut type_label = node
                        .config
                        .output_type
                        .map_or("any".to_owned(), |value_type| {
                            value_type.as_str().to_owned()
                        });
                    if let Some(contract) = &node.config.contract_id {
                        type_label = format!("{type_label} · {contract}");
                    }
                    (node.handle.clone(), type_label)
                })
                .collect(),
        }
    }
}

thread_local! {
    static FLAGS: RefCell<Flags> = RefCell::new(Flags::default());
    static BOARD: RefCell<Option<Board>> = const { RefCell::new(None) };
}

pub fn get() -> Flags {
    FLAGS.with(|flags| flags.borrow().clone())
}

pub fn set(flags: Flags) {
    FLAGS.with(|cell| *cell.borrow_mut() = flags);
}

pub fn update(change: impl FnOnce(&mut Flags)) {
    FLAGS.with(|cell| change(&mut cell.borrow_mut()));
}

/// The component the board draws this frame, or `None` for a scenario.
pub fn set_board(board: Option<Board>) {
    BOARD.with(|cell| *cell.borrow_mut() = board);
}

pub fn board() -> Option<Board> {
    BOARD.with(|cell| cell.borrow().clone())
}

/// The id of the synthetic proxy standing for an input on the board.
pub fn chip_id(name: &str) -> String {
    format!("comp00-input:{name}")
}

pub fn input_of_chip(id: &str) -> Option<&str> {
    id.strip_prefix("comp00-input:")
}

/// The id of the placeholder row of an input being added.
pub const NEW_ROW: &str = "comp00-input:\u{0}new";

thread_local! {
    static CHIP_WIDTHS: RefCell<std::collections::HashMap<String, f32>> =
        RefCell::new(std::collections::HashMap::new());
}

pub fn set_chip_width(id: &str, width: f32) {
    CHIP_WIDTHS.with(|cell| {
        cell.borrow_mut().insert(id.to_owned(), width);
    });
}

/// The width of one of the mock-up's input chips.
pub fn chip_width(id: &str) -> Option<f32> {
    input_of_chip(id)?;
    CHIP_WIDTHS.with(|cell| cell.borrow().get(id).copied())
}

thread_local! {
    static EXTRA: std::cell::Cell<Option<eframe::egui::Rect>> = const { std::cell::Cell::new(None) };
}

/// What the mock-up adds to the board, for fitting the view.
pub fn set_extra_bounds(rect: Option<eframe::egui::Rect>) {
    EXTRA.with(|cell| cell.set(rect));
}

pub fn extra_bounds() -> Option<eframe::egui::Rect> {
    EXTRA.with(std::cell::Cell::get)
}

thread_local! {
    static DIALOG: RefCell<Option<crate::dialogs::ComponentDialog>> = const { RefCell::new(None) };
}

/// Q5: the *Save as component* dialog shown over the window.
pub fn open_dialog(dialog: crate::dialogs::ComponentDialog) {
    DIALOG.with(|cell| *cell.borrow_mut() = Some(dialog));
}

pub fn close_dialog() {
    DIALOG.with(|cell| *cell.borrow_mut() = None);
}

pub fn show_dialog(context: &eframe::egui::Context, project: &apiforge_core::format::Project) {
    DIALOG.with(|cell| {
        if let Some(dialog) = cell.borrow_mut().as_mut() {
            let _ = dialog.show(context, project);
        }
    });
}
