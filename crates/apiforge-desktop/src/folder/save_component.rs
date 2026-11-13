//! *Selection actions → Save as component* in a folder project (UX-30,
//! COMP-00 decision 5): a dialog proposes the component made from the
//! selected steps, writes it under `components/` and, by default, publishes
//! revision 1 and calls it in place of the steps as one undoable edit.
use super::*;
use apiforge_core::{
    edit::{ComponentDraft, Extraction, InputRow, OutputRow},
    format::{Binding, Component, Contract},
    handle::handle_base,
    js,
    model::{NodeKind, ValueType},
};
use apiforge_storage::project::ProjectPath;

/// The folder a new component's file goes into.
const FOLDER: &str = "components/";
const PUBLISH: &str = "Publish revision 1 and call it here, in place of the selected steps";
const ONLY_CREATE: &str = "Only create the file; the steps stay in the scenario";
/// The footer sentence of each choice; the first records what undo does
/// (criterion 6).
const PUBLISH_NOTE: &str = "Publishes revision 1; the scenario calls it instead of these steps. \
                            Undo brings the steps back; revision 1 stays published.";
const CREATE_NOTE: &str = "Creates the file; the scenario keeps these steps.";
const ACTION_LABEL: &str = "Offered after a block that outputs (action)";
const NO_ACTION: &str = "No action";

/// The dialog's working copy; nothing is written until it is applied.
pub(super) struct SaveComponent {
    document_id: String,
    cleanup: bool,
    graph: Graph,
    reference: Graph,
    later: Vec<Graph>,
    selected: Vec<String>,
    draft: ComponentDraft,
    file: String,
    /// The file name was typed, so it no longer follows the name.
    file_typed: bool,
    publish: bool,
    /// Save even though a later step reads an output left unticked, leaving
    /// that step's input unconnected (UX-37). Unticked whenever the dialog
    /// opens.
    leave_unconnected: bool,
    focused: bool,
    /// Why the last attempt to save failed.
    error: Option<String>,
    /// What keeps the dialog from being applied, as of this frame.
    problems: Vec<String>,
    contracts: Vec<Contract>,
    /// Lowercased names, and the names in code, of the project's components.
    names: HashSet<String>,
    handles: HashSet<String>,
    /// Lowercased registered paths.
    paths: HashSet<String>,
    root: PathBuf,
    /// The last path checked on disk and whether something is there.
    on_disk: Option<(String, bool)>,
}

/// What an applied dialog asks the window to do.
pub(super) struct Request {
    component: Component,
    path: String,
    publish: bool,
}

/// What the dialog says about outputs a later step reads but the draft
/// drops (UX-30, UX-37).
struct Unkept {
    /// The refusal.
    message: String,
    /// The later steps that would be left unconnected.
    readers: Vec<String>,
    /// For each request that uses a dropped value in a template, where:
    /// such a reader cannot be left unconnected.
    reasons: Vec<String>,
}

/// Why a read output must stay, or nothing.
fn unkept_message(unkept: &[String]) -> Option<String> {
    match unkept {
        [] => None,
        [one] => Some(format!(
            "A step after the selection reads {one}; keep it as an output."
        )),
        many => Some(format!(
            "Steps after the selection read {}; keep them as outputs.",
            many.join(", ")
        )),
    }
}

/// Whether something is at `path` under `root`, or at a path that differs
/// from it only in letter case, which registration refuses as well.
fn exists_in_any_case(root: &std::path::Path, path: &str) -> bool {
    let full = root.join(path);
    if full.exists() {
        return true;
    }
    let (Some(parent), Some(name)) = (full.parent(), full.file_name()) else {
        return false;
    };
    let name = name.to_string_lossy().to_lowercase();
    std::fs::read_dir(parent).is_ok_and(|entries| {
        entries
            .flatten()
            .any(|entry| entry.file_name().to_string_lossy().to_lowercase() == name)
    })
}

/// Where an input's value comes from, after its block and slot.
fn origin(row: &InputRow) -> String {
    match &row.original {
        Some(Binding::Reference { .. }) => "a wire from outside the selection".into(),
        Some(Binding::Parameter { .. }) => format!(
            "param.{} from outside the selection",
            match &row.original {
                Some(Binding::Parameter { name, .. }) => name.as_str(),
                _ => "",
            }
        ),
        Some(Binding::Literal { value }) => format!(
            "the value {} becomes its default",
            js::json_stringify(value)
        ),
        Some(_) => "a value set on the step".into(),
        None => "not set on the step".into(),
    }
}

impl SaveComponent {
    /// The dialog for `selected` blocks of `model`'s board, Start left out.
    fn new(
        document_id: &str,
        model: &Document,
        cleanup: bool,
        selection: &[String],
        project: &Project,
        session: &ProjectSession,
    ) -> Option<Self> {
        let graph = graph_of(model, cleanup).clone();
        let selected: Vec<String> = graph
            .nodes
            .iter()
            .filter(|node| selection.contains(&node.id) && node.kind != NodeKind::Start)
            .map(|node| node.id.clone())
            .collect();
        if selected.is_empty() {
            return None;
        }
        let reference = graph_of(model, false).clone();
        let later = match model {
            Document::Scenario(scenario) if !cleanup => vec![scenario.cleanup.clone()],
            _ => Vec::new(),
        };
        let name = match selected.as_slice() {
            [one] => graph
                .nodes
                .iter()
                .find(|node| &node.id == one)
                .map_or("New component", |node| node.display_name())
                .to_owned(),
            _ => "New component".to_owned(),
        };
        let draft = {
            let ids: HashSet<&str> = selected.iter().map(String::as_str).collect();
            let later: Vec<&Graph> = later.iter().collect();
            edit::propose_extraction(
                &Extraction {
                    graph: &graph,
                    reference: &reference,
                    later: &later,
                    selected: &ids,
                },
                project,
                name,
            )
        };
        let mut names: HashSet<String> = HashSet::new();
        let mut handles: HashSet<String> = HashSet::new();
        for component in &project.components {
            names.insert(component.name.to_lowercase());
            handles.insert(component.handle.clone());
        }
        for revision in &session.manifest().revisions {
            handles.insert(revision.handle.clone());
        }
        for document in session.documents() {
            if let Some(Document::Component(component)) = document.model() {
                names.insert(component.name.to_lowercase());
                handles.insert(component.handle.clone());
            }
        }
        let paths = session
            .documents()
            .iter()
            .map(|document| document.entry().path.as_str().to_lowercase())
            .collect();
        let file = format!("{}.afs", file_stem(&draft.name));
        Some(Self {
            document_id: document_id.to_owned(),
            cleanup,
            graph,
            reference,
            later,
            selected,
            draft,
            file,
            file_typed: false,
            publish: true,
            leave_unconnected: false,
            focused: false,
            error: None,
            problems: Vec::new(),
            contracts: project.contracts.clone(),
            names,
            handles,
            paths,
            root: session.store().root().to_owned(),
            on_disk: None,
        })
    }

    fn path(&self) -> String {
        format!("{FOLDER}{}", self.file.trim())
    }

    /// Why the name cannot be used, if it cannot.
    fn name_problem(&self) -> Option<String> {
        let name = js::trim(&self.draft.name);
        if name.is_empty() {
            return Some("Enter a component name.".into());
        }
        if self.names.contains(&name.to_lowercase()) {
            return Some(format!("A component named {name} already exists."));
        }
        let handle = handle_base(name);
        self.handles.contains(&handle).then(|| {
            format!("Another component is already named {handle} in code. Choose another name.")
        })
    }

    /// Why the file cannot be written, if it cannot.
    fn file_problem(&mut self) -> Option<String> {
        let file = self.file.trim();
        if file.len() <= ".afs".len() || !file.ends_with(".afs") {
            return Some("The file name ends in .afs.".into());
        }
        let path = self.path();
        if ProjectPath::working_source(&path).is_err() {
            return Some("Use a name of letters, digits, - and _ that ends in .afs.".into());
        }
        let exists = match &self.on_disk {
            Some((checked, exists)) if *checked == path => *exists,
            _ => {
                let exists = exists_in_any_case(&self.root, &path);
                self.on_disk = Some((path.clone(), exists));
                exists
            }
        };
        (exists || self.paths.contains(&path.to_lowercase()))
            .then(|| format!("{path} already exists."))
    }

    fn with_extraction<R>(&self, read: impl FnOnce(&Extraction<'_>) -> R) -> R {
        let selected: HashSet<&str> = self.selected.iter().map(String::as_str).collect();
        let later: Vec<&Graph> = self.later.iter().collect();
        read(&Extraction {
            graph: &self.graph,
            reference: &self.reference,
            later: &later,
            selected: &selected,
        })
    }

    /// Why one call cannot stand in for the steps, when they are to be
    /// replaced, and whether the person may go ahead anyway.
    ///
    /// Only the default choice replaces the steps, so only it is refused
    /// when one call cannot stand in for them; a file alone changes nothing
    /// the steps after them read. A branch leaving the selection is always
    /// refused; an output a later step reads may be dropped on request,
    /// leaving those steps unconnected (UX-37), unless a request uses it in
    /// its URL, headers, query or body, which the refusal then names
    /// (option B).
    fn unkept(&self) -> Option<Unkept> {
        if !self.publish {
            return None;
        }
        if self.with_extraction(edit::branch_leaves) {
            return Some(Unkept {
                message: edit::EditError::BranchLeavesSelection.to_string(),
                readers: Vec::new(),
                reasons: Vec::new(),
            });
        }
        let message = unkept_message(
            &self.with_extraction(|extraction| edit::unkept_outputs(extraction, &self.draft)),
        )?;
        let (readers, uses) = self.with_extraction(|extraction| {
            (
                edit::unkept_readers(extraction, &self.draft),
                edit::template_readers(extraction, &self.draft),
            )
        });
        let reasons = uses
            .into_iter()
            .map(|(reader, place)| format!("\"{reader}\" uses this in its {place}."))
            .collect();
        Some(Unkept {
            message,
            readers,
            reasons,
        })
    }

    /// The draft's own problems; a missing name is the name field's.
    fn draft_problems(&self) -> Vec<String> {
        self.with_extraction(|extraction| {
            edit::validate_draft(extraction.graph, extraction.selected, &self.draft)
        })
        .iter()
        .filter(|error| !matches!(error, edit::DraftError::MissingName))
        .map(ToString::to_string)
        .collect()
    }

    fn request(&self) -> Result<Request, String> {
        self.with_extraction(|extraction| edit::extraction_component(extraction, &self.draft))
            .map(|component| Request {
                component,
                path: self.path(),
                publish: self.publish,
            })
            .map_err(|error| error.to_string())
    }

    fn show(&mut self, context: &egui::Context) -> dialogs::Outcome<Request> {
        let eyebrow = match self.selected.len() {
            1 => format!("{} · 1 STEP", FOLDER.to_uppercase()),
            count => format!("{} · {count} STEPS", FOLDER.to_uppercase()),
        };
        let (apply, note) = if self.publish {
            ("Save and publish", PUBLISH_NOTE)
        } else {
            ("Create file", CREATE_NOTE)
        };
        let chrome = dialogs::Chrome {
            id: "folder-save-component",
            eyebrow: &eyebrow,
            title: "Save as component",
            width: 700.0,
            apply_label: apply,
            note: Some(note),
        };
        match chrome.show(
            context,
            self,
            |ui, dialog| dialog.body(ui),
            |dialog| dialog.problems.is_empty(),
        ) {
            dialogs::Action::Cancel => dialogs::Outcome::Cancelled,
            dialogs::Action::Apply => match self.request() {
                Ok(request) => dialogs::Outcome::Applied(request),
                Err(error) => {
                    self.error = Some(error);
                    dialogs::Outcome::Open
                }
            },
            dialogs::Action::None => dialogs::Outcome::Open,
        }
    }

    fn body(&mut self, ui: &mut egui::Ui) {
        let mut problems = Vec::new();
        ui.horizontal_top(|ui| {
            let width = ui.available_width();
            let name_width = (width * 0.48).max(200.0);
            ui.vertical(|ui| {
                ui.set_width(name_width);
                widgets::field_label(ui, "Name");
                let problem = self.name_problem();
                let field = widgets::validated(ui, problem.is_some(), |ui| {
                    widgets::named(
                        ui.add(
                            widgets::text_edit(&mut self.draft.name)
                                .hint_text("e.g. Check cart price")
                                .desired_width(name_width),
                        ),
                        "Component name",
                    )
                });
                if !self.focused {
                    field.request_focus();
                    self.focused = true;
                }
                if field.changed() && !self.file_typed {
                    self.file = format!("{}.afs", file_stem(&self.draft.name));
                }
                if let Some(problem) = self.name_problem() {
                    widgets::field_error(ui, &problem);
                    problems.push(problem);
                }
            });
            ui.add_space(theme::SPACE_MD);
            ui.vertical(|ui| {
                widgets::field_label(ui, "File");
                let problem = self.file_problem();
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(FOLDER)
                            .monospace()
                            .size(theme::FONT_LABEL)
                            .color(theme::MUTED),
                    );
                    let field = widgets::validated(ui, problem.is_some(), |ui| {
                        widgets::named(
                            ui.add(
                                widgets::text_edit(&mut self.file)
                                    .desired_width(ui.available_width()),
                            ),
                            "Component file",
                        )
                    });
                    if field.changed() {
                        self.file_typed = true;
                    }
                });
                if let Some(problem) = self.file_problem() {
                    widgets::field_error(ui, &problem);
                    problems.push(problem);
                }
            });
        });
        widgets::field_label(ui, "Description");
        widgets::named(
            ui.add(
                widgets::text_edit(&mut self.draft.description)
                    .hint_text("What does this component do?")
                    .desired_width(f32::INFINITY),
            ),
            "Component description",
        );

        widgets::field_label(ui, "After saving");
        ui.radio_value(&mut self.publish, true, PUBLISH);
        ui.radio_value(&mut self.publish, false, ONLY_CREATE);

        section(
            ui,
            "Inputs",
            "from the wires and values entering the selection",
        );
        if self.draft.inputs.is_empty() {
            widgets::hint(ui, "The steps read nothing from outside the selection.");
        }
        let contracts = self.contracts.clone();
        for (index, row) in self.draft.inputs.iter_mut().enumerate() {
            ui.push_id(("input", row.key.clone()), |ui| {
                input_row(ui, index + 1, row, &contracts);
            });
        }

        section(
            ui,
            "Outputs",
            "read after the selection, here or in cleanup",
        );
        let names: HashMap<String, String> = self
            .graph
            .nodes
            .iter()
            .map(|node| (node.id.clone(), node.display_name().to_owned()))
            .collect();
        for (index, row) in self.draft.outputs.iter_mut().enumerate() {
            let source = names
                .get(&row.node_id)
                .map_or("Saved binding", String::as_str);
            let caption = format!("{source} → {}", row.port);
            ui.push_id(("output", row.key.clone()), |ui| {
                output_row(ui, index + 1, row, &caption, &contracts);
            });
        }
        match self.unkept() {
            Some(unkept) if !unkept.readers.is_empty() && unkept.reasons.is_empty() => {
                // Chosen, the refusal stays as a muted note of what saving
                // leaves to be fixed by hand.
                if self.leave_unconnected {
                    widgets::hint(ui, &unkept.message);
                } else {
                    widgets::field_error(ui, &unkept.message);
                    problems.push(unkept.message);
                }
                let label = match unkept.readers.as_slice() {
                    [one] => format!("Save anyway and leave \"{one}\" unconnected"),
                    many => format!(
                        "Save anyway and leave {} later steps unconnected",
                        many.len()
                    ),
                };
                widgets::named(ui.checkbox(&mut self.leave_unconnected, &label), label);
            }
            Some(unkept) => {
                // A reader that cannot lose the value: the refusal alone,
                // with where it is used.
                self.leave_unconnected = false;
                widgets::field_error(ui, &unkept.message);
                for reason in &unkept.reasons {
                    widgets::field_error(ui, reason);
                }
                problems.push(unkept.message);
            }
            None => self.leave_unconnected = false,
        }

        self.action_picker(ui);

        let draft = self.draft_problems();
        if !draft.is_empty() {
            ui.add_space(theme::SPACE_SM);
            widgets::error_box(ui, &draft);
        }
        problems.extend(draft);
        if let Some(error) = &self.error {
            ui.add_space(theme::SPACE_SM);
            widgets::error_box(ui, std::slice::from_ref(error));
        }
        self.problems = problems;
    }

    /// The input a value dropped after a block feeds, and the kind of value
    /// that offers the component, as UX-33's settings choose them.
    fn action_picker(&mut self, ui: &mut egui::Ui) {
        let enabled: Vec<(String, String, ValueType, Option<String>)> = self
            .draft
            .inputs
            .iter()
            .filter(|row| row.enabled)
            .map(|row| {
                (
                    row.key.clone(),
                    js::trim(&row.name).to_owned(),
                    row.value_type,
                    row.contract_id.clone(),
                )
            })
            .collect();
        ui.add_space(theme::SPACE_MD);
        super::inputs::unnamed_field_label(ui, ACTION_LABEL);
        let selected = self
            .draft
            .action
            .as_ref()
            .and_then(|key| enabled.iter().find(|(candidate, ..)| candidate == key))
            .map_or(NO_ACTION, |(_, name, ..)| name.as_str());
        let mut chosen = self.draft.action.clone();
        let combo = egui::ComboBox::from_id_salt("save-component-action")
            .width(320.0)
            .selected_text(widgets::ellipsis(selected, 40))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut chosen, None, NO_ACTION);
                for (key, name, ..) in &enabled {
                    ui.selectable_value(&mut chosen, Some(key.clone()), name);
                }
            });
        widgets::named(combo.response, ACTION_LABEL);
        if chosen != self.draft.action {
            let row = chosen
                .as_ref()
                .and_then(|key| enabled.iter().find(|(candidate, ..)| candidate == key));
            self.draft.action_type =
                row.map_or(ValueType::Any, |(_, _, value_type, _)| *value_type);
            self.draft.action_contract = row.and_then(|(.., contract)| contract.clone());
            self.draft.action = chosen;
        }
        if self.draft.action.is_some() {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    widgets::field_label(ui, "Show for data of type");
                    type_combo(ui, "Action type", &mut self.draft.action_type, 140.0);
                });
                ui.vertical(|ui| {
                    widgets::field_label(ui, "Or of the project contract");
                    contract_combo(
                        ui,
                        "Action contract",
                        &mut self.draft.action_contract,
                        &self.contracts,
                        "Match by type",
                        180.0,
                    );
                });
            });
        }
    }
}

/// A section's bold title with its muted explanation on the same line.
fn section(ui: &mut egui::Ui, title: &str, explanation: &str) {
    ui.add_space(theme::SPACE_LG);
    ui.horizontal(|ui| {
        widgets::section_inline(ui, title);
        widgets::hint(ui, explanation);
    });
    ui.add_space(4.0);
}

/// A bordered row, tinted while it is used.
fn row_frame(ui: &mut egui::Ui, used: bool, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(4.0);
    egui::Frame::new()
        .fill(if used { theme::FIELD } else { theme::WHITE })
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add_contents(ui);
        });
}

fn input_row(ui: &mut egui::Ui, number: usize, row: &mut InputRow, contracts: &[Contract]) {
    let caption = format!("{} · {}", row.label, origin(row));
    row_frame(ui, row.enabled, |ui| {
        ui.horizontal_wrapped(|ui| {
            let used = ui.add_enabled(
                !row.mandatory,
                egui::Checkbox::without_text(&mut row.enabled),
            );
            let used = widgets::named(used, format!("Use input {number}"));
            if row.mandatory {
                used.on_disabled_hover_text(
                    "Read from outside the selection, so it must be an input",
                );
            }
            if !row.enabled {
                ui.label(&row.label);
                return;
            }
            widgets::named(
                ui.add(
                    widgets::text_edit(&mut row.name)
                        .hint_text("input name")
                        .desired_width(150.0),
                ),
                format!("Input {number} name"),
            );
            type_combo(
                ui,
                &format!("Input {number} type"),
                &mut row.value_type,
                96.0,
            );
            contract_combo(
                ui,
                &format!("Input {number} contract"),
                &mut row.contract_id,
                contracts,
                "No contract",
                120.0,
            );
            widgets::named(
                ui.checkbox(&mut row.required, "Required"),
                format!("Input {number} required"),
            );
        });
        if row.enabled {
            widgets::hint(ui, caption);
        }
    });
}

fn output_row(
    ui: &mut egui::Ui,
    number: usize,
    row: &mut OutputRow,
    caption: &str,
    contracts: &[Contract],
) {
    row_frame(ui, row.enabled, |ui| {
        ui.horizontal_wrapped(|ui| {
            widgets::named(
                ui.add(egui::Checkbox::without_text(&mut row.enabled)),
                format!("Use output {number}"),
            );
            if !row.enabled {
                ui.label(caption);
                return;
            }
            widgets::named(
                ui.add(
                    widgets::text_edit(&mut row.name)
                        .hint_text("output name")
                        .desired_width(150.0),
                ),
                format!("Output {number} name"),
            );
            type_combo(
                ui,
                &format!("Output {number} type"),
                &mut row.value_type,
                96.0,
            );
            contract_combo(
                ui,
                &format!("Output {number} contract"),
                &mut row.contract_id,
                contracts,
                "No contract",
                120.0,
            );
        });
        if row.enabled {
            widgets::hint(ui, caption);
        }
    });
}

/// A value type list named `name`.
fn type_combo(ui: &mut egui::Ui, name: &str, value: &mut ValueType, width: f32) {
    let combo = egui::ComboBox::from_id_salt(name)
        .width(width)
        .selected_text(dialogs::type_label(*value))
        .show_ui(ui, |ui| {
            for option in dialogs::VALUE_TYPES {
                ui.selectable_value(value, option, dialogs::type_label(option));
            }
        });
    widgets::named(combo.response, name);
}

/// A project contract list named `name`; `None` reads `empty`.
fn contract_combo(
    ui: &mut egui::Ui,
    name: &str,
    value: &mut Option<String>,
    contracts: &[Contract],
    empty: &str,
    width: f32,
) {
    let selected = value
        .as_deref()
        .map(|id| {
            contracts
                .iter()
                .find(|contract| contract.id == id)
                .map_or_else(|| format!("Missing contract ({id})"), |c| c.name.clone())
        })
        .unwrap_or_else(|| empty.to_owned());
    let combo = egui::ComboBox::from_id_salt(name)
        .width(width)
        .selected_text(widgets::ellipsis(&selected, 28))
        .show_ui(ui, |ui| {
            ui.selectable_value(value, None, empty);
            for contract in contracts {
                ui.selectable_value(value, Some(contract.id.clone()), &contract.name);
            }
        });
    widgets::named(combo.response, name);
}

impl FolderApp {
    /// Opens the dialog for the selection on `document_id`'s board.
    pub(super) fn open_save_component(
        &mut self,
        document_id: &str,
        model: &Document,
        cleanup: bool,
        selection: &[String],
    ) {
        let project = self.context_project();
        let Some(session) = self.session.as_ref() else {
            return;
        };
        match SaveComponent::new(document_id, model, cleanup, selection, &project, session) {
            Some(dialog) => self.prompt = Some(Prompt::SaveComponent(Box::new(dialog))),
            None => self.notify("Select the steps to save as a component.", false),
        }
    }

    pub(super) fn save_component_prompt(
        &mut self,
        context: &egui::Context,
        mut dialog: Box<SaveComponent>,
    ) -> Option<Prompt> {
        match dialog.show(context) {
            dialogs::Outcome::Open => Some(Prompt::SaveComponent(dialog)),
            dialogs::Outcome::Cancelled => None,
            dialogs::Outcome::Applied(request) => {
                self.save_component(dialog, request);
                self.prompt.take()
            }
        }
    }

    /// Writes the component and, when asked, publishes it and calls it in
    /// place of the steps. A refusal reopens the dialog with its reason.
    fn save_component(&mut self, mut dialog: Box<SaveComponent>, request: Request) {
        if request.publish
            && let Err(reason) = self.check_call(&dialog, &request.component)
        {
            dialog.error = Some(reason);
            self.prompt = Some(Prompt::SaveComponent(dialog));
            return;
        }
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let text = match apiforge_script::print_folder_document(
            session.context(),
            &Document::Component(request.component),
        ) {
            Ok(text) => text,
            Err(_) => {
                dialog.error = Some("The component cannot be written as code.".into());
                self.prompt = Some(Prompt::SaveComponent(dialog));
                return;
            }
        };
        let (path, publish) = (request.path, request.publish);
        self.working_project(
            "Saving component…",
            move |session| {
                let id =
                    match session.create_document_sealed(DocumentKind::Component, &path, &text)? {
                        Ok(id) => id,
                        Err(check) => {
                            let reason = check
                                .problems
                                .first()
                                .map(|problem| problem.message().to_owned())
                                .unwrap_or_else(|| "The component has errors.".to_owned());
                            return Ok(Err(reason));
                        }
                    };
                let published = if publish {
                    match session.publish(&id, DIAGRAM) {
                        Ok(published) => Some(published),
                        Err(error) => {
                            // Nothing half done stays behind: the new file
                            // goes to the project's trash, so trying again
                            // finds its path free.
                            let _ = session
                                .preview_delete(&[path.as_str()])
                                .and_then(|preview| session.delete(&preview, DIAGRAM));
                            return Ok(Err(format!(
                                "The component could not be published: {error}"
                            )));
                        }
                    }
                } else {
                    None
                };
                Ok(Ok((path, published)))
            },
            move |app, result| match result {
                Ok(Ok((path, published))) => {
                    app.watcher.rescan();
                    match published {
                        Some(published) => app.call_saved_component(&dialog, &published),
                        None => app.notify(
                            format!("Created {path}; the steps stay in the scenario."),
                            false,
                        ),
                    }
                    app.touched();
                }
                Ok(Err(reason)) => {
                    dialog.error = Some(reason);
                    app.prompt = Some(Prompt::SaveComponent(dialog));
                }
                Err(error) => {
                    dialog.error = Some(error.to_string());
                    app.prompt = Some(Prompt::SaveComponent(dialog));
                }
            },
        );
    }

    /// Replaces the dialog's steps by a call to the revision just published.
    fn call_saved_component(
        &mut self,
        dialog: &SaveComponent,
        published: &apiforge_project::Published,
    ) {
        // The publication changed the context, so the scenario's model waits
        // for its analysis; the edit needs it now.
        if let Some(Err(error)) = self
            .session
            .as_mut()
            .map(|session| session.settle(&dialog.document_id))
        {
            self.notify(
                format!("The component was published, but the steps stay: {error}"),
                true,
            );
            return;
        }
        let project = self.context_project();
        let Some(component) = project.components.iter().find(|component| {
            component.id == published.component_id && component.revision == published.revision
        }) else {
            self.notify(
                "The component was published, but the scenario could not call it yet.",
                true,
            );
            return;
        };
        let Some(model) = self
            .session
            .as_ref()
            .and_then(|session| session.document(&dialog.document_id))
            .and_then(|document| document.model().cloned())
        else {
            return;
        };
        // The board the dialog was opened on, as it is now: nothing could
        // change it while the dialog and the save held the window.
        match replaced_model(dialog, &model, component) {
            Ok((next, call)) => {
                // A refusal of the edit replaces this notice with its reason.
                self.notify(
                    format!(
                        "Published {} revision {}; the scenario calls it in place of {}.",
                        component.name,
                        published.revision,
                        match dialog.selected.len() {
                            1 => "the step".to_owned(),
                            count => format!("{count} steps"),
                        }
                    ),
                    false,
                );
                self.component_action(
                    &dialog.document_id,
                    publication::Action::Model {
                        next: Box::new(next),
                        selection: Some(vec![call]),
                        check: None,
                    },
                );
            }
            Err(error) => self.notify(
                format!("The component was published, but the steps stay: {error}"),
                true,
            ),
        }
    }

    /// Whether the scenario that would result, calling `component` in place
    /// of the steps, is valid: asked before anything is written or published,
    /// so that a refused edit leaves no component file and no revision behind
    /// (#322). With *Save anyway* ticked, the readers of a dropped output are
    /// left unconnected (UX-37, criterion 5).
    fn check_call(&self, dialog: &SaveComponent, component: &Component) -> Result<(), String> {
        const UNCONNECTED: &str = "Saving would leave the scenario invalid, so nothing was saved. \
                                   Keep the output instead.";
        const INVALID: &str = "Calling the component here would leave the scenario invalid, \
                               so nothing was saved.";
        let refused = if dialog.leave_unconnected {
            UNCONNECTED
        } else {
            INVALID
        };
        let session = self.session.as_ref().ok_or(refused)?;
        let model = session
            .document(&dialog.document_id)
            .and_then(|document| document.model().cloned())
            .ok_or(refused)?;
        let (next, _) = replaced_model(dialog, &model, component).map_err(|error| {
            if dialog.leave_unconnected {
                UNCONNECTED.to_owned()
            } else {
                format!("{error} The call cannot replace the steps, so nothing was saved.")
            }
        })?;
        let mut project = (**session.context()).clone();
        project.components.push(component.clone());
        apiforge_script::validate_document_model(
            &next,
            &apiforge_script::LowerContext::folder(&project),
        )
        .map_err(|_| refused.to_owned())
    }
}

/// `model` with the dialog's steps replaced by one call to `component`, and
/// the call's identifier; with *Save anyway* ticked, the readers of a dropped
/// output are left unconnected.
fn replaced_model(
    dialog: &SaveComponent,
    model: &Document,
    component: &Component,
) -> Result<(Document, String), edit::EditError> {
    // The board the dialog was opened on, as it is now: nothing could
    // change it while the dialog and the save held the window.
    let later: Vec<&Graph> = match model {
        Document::Scenario(scenario) if !dialog.cleanup => vec![&scenario.cleanup],
        _ => Vec::new(),
    };
    let selected: HashSet<&str> = dialog.selected.iter().map(String::as_str).collect();
    let replace = if dialog.leave_unconnected {
        edit::replace_with_call_unconnecting
    } else {
        edit::replace_with_call
    };
    let replaced = replace(
        &Extraction {
            graph: graph_of(model, dialog.cleanup),
            reference: graph_of(model, false),
            later: &later,
            selected: &selected,
        },
        &dialog.draft,
        component,
    )?;
    let mut next = with_graph(model, dialog.cleanup, replaced.graph);
    if let (Document::Scenario(scenario), Some(cleanup)) =
        (&mut next, replaced.later.into_iter().next())
    {
        scenario.cleanup = cleanup;
    }
    Ok((next, replaced.call))
}
