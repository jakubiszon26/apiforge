//! The explorer: the project directory as it is on disk, with registered
//! documents, their status and file operations. Rows come from the watcher's
//! latest scan plus registered documents whose files are missing; only the
//! visible rows are drawn.

use super::*;
use apiforge_storage::project::{TreeEntry, TreeEntryKind, TreeScan, tree_order};
use std::collections::{BTreeSet, HashSet};

const ROW_HEIGHT: f32 = 24.0;
const INDENT: f32 = 14.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowKind {
    /// The project itself, at the empty path: the explicit destination for new
    /// files, folders, imports and drops that belong at the top level.
    Root {
        name: String,
    },
    Directory,
    Document {
        id: String,
        kind: DocumentKind,
    },
    /// An `.afs` file that is not registered in the project.
    Source,
    File,
    Link,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub path: String,
    pub depth: usize,
    pub kind: RowKind,
    /// The name can be the subject of a file operation.
    pub portable: bool,
    /// A registered document whose file is not on disk.
    pub missing: bool,
}

impl Row {
    pub fn name(&self) -> &str {
        match &self.kind {
            RowKind::Root { name } => name,
            _ => file_name(&self.path),
        }
    }
    /// A real directory: the subject of renames, moves and deletion.
    pub fn is_directory(&self) -> bool {
        self.kind == RowKind::Directory
    }
    /// Anything that contains rows: a directory or the project root.
    pub fn is_folder(&self) -> bool {
        self.is_directory() || self.is_root()
    }
    pub fn is_root(&self) -> bool {
        matches!(self.kind, RowKind::Root { .. })
    }
}

/// A row being dragged onto a folder.
#[derive(Clone, Debug)]
struct Dragged(String);

/// Explorer view state; expanded folders live in the workbench.
#[derive(Default)]
pub struct ExplorerState {
    pub selected: Option<String>,
    /// Scroll the selected row into view on the next frame.
    reveal: bool,
    /// The active document last revealed, so a newly activated one is.
    followed: Option<String>,
    /// The project row collapsed, hiding the tree. Not part of the saved view
    /// state: a project opens showing itself.
    root_collapsed: bool,
    /// Height of the row viewport last frame. A resize can push the active
    /// row out of view without the user scrolling, so it is revealed again.
    viewport: f32,
    /// How many rows there were last frame. The folder scan arrives after the
    /// first frames and moves every row down, which has the same effect.
    rows: usize,
}

/// A registered document as the explorer lists it.
pub struct Registered {
    pub id: String,
    pub path: String,
    pub kind: DocumentKind,
    /// The session found no file for it. A document that a bounded scan left
    /// out is not missing.
    pub missing: bool,
}

/// Every row of the tree, in explorer order, before folding. The first row is
/// the project itself.
pub fn tree(scan: Option<&TreeScan>, documents: &[Registered], name: &str) -> Vec<Row> {
    let mut entries: Vec<TreeEntry> = scan.map(|scan| scan.entries.clone()).unwrap_or_default();
    let mut known: HashSet<String> = entries.iter().map(|entry| entry.path.clone()).collect();
    let missing: HashSet<&str> = documents
        .iter()
        .filter(|document| document.missing)
        .map(|document| document.path.as_str())
        .collect();
    for Registered { path, .. } in documents {
        if known.contains(path) {
            continue;
        }
        let mut parent = String::new();
        for part in path.split('/').collect::<Vec<_>>().split_last().unwrap().1 {
            if !parent.is_empty() {
                parent.push('/');
            }
            parent.push_str(part);
            if known.insert(parent.clone()) {
                entries.push(TreeEntry {
                    path: parent.clone(),
                    kind: TreeEntryKind::Directory,
                    portable: true,
                });
            }
        }
        known.insert(path.clone());
        entries.push(TreeEntry {
            path: path.clone(),
            kind: TreeEntryKind::File {
                bytes: 0,
                sha256: None,
            },
            portable: true,
        });
    }
    entries.sort_by(tree_order);
    std::iter::once(Row {
        path: String::new(),
        depth: 0,
        kind: RowKind::Root {
            name: name.to_owned(),
        },
        portable: false,
        missing: false,
    })
    .chain(entries.into_iter().map(|entry| {
        let document = documents
            .iter()
            .find(|document| document.path == entry.path);
        let kind = match (&entry.kind, document) {
            (_, Some(document)) => RowKind::Document {
                id: document.id.clone(),
                kind: document.kind,
            },
            (TreeEntryKind::Directory, None) => RowKind::Directory,
            (TreeEntryKind::Link, None) => RowKind::Link,
            (TreeEntryKind::File { .. }, None) if entry.path.ends_with(".afs") => RowKind::Source,
            (TreeEntryKind::File { .. }, None) => RowKind::File,
        };
        Row {
            depth: entry.path.matches('/').count() + 1,
            missing: missing.contains(entry.path.as_str()),
            path: entry.path,
            kind,
            portable: entry.portable,
        }
    }))
    .collect()
}

/// The rows whose every ancestor folder is expanded. A collapsed project row
/// hides the whole tree.
pub fn visible(rows: Vec<Row>, expanded: &BTreeSet<String>, root_collapsed: bool) -> Vec<Row> {
    rows.into_iter()
        .filter(|row| {
            if row.is_root() {
                return true;
            }
            if root_collapsed {
                return false;
            }
            let mut ancestor = row.path.as_str();
            while let Some((parent, _)) = ancestor.rsplit_once('/') {
                if !expanded.contains(parent) {
                    return false;
                }
                ancestor = parent;
            }
            true
        })
        .collect()
}

pub fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

/// The folder new items go into for a selected row.
pub fn folder_of(row: Option<&Row>) -> String {
    match row {
        Some(row) if row.is_folder() => row.path.clone(),
        Some(row) => parent(&row.path).to_owned(),
        None => String::new(),
    }
}

/// Where a moved item ends up inside `folder`.
pub fn moved_path(path: &str, folder: &str) -> String {
    let name = file_name(path);
    if folder.is_empty() {
        name.to_owned()
    } else {
        format!("{folder}/{name}")
    }
}

/// What the user asked for in the explorer this frame.
pub(super) enum Action {
    Select(String),
    Toggle(String),
    Open(String),
    /// Open a project file as text, by path.
    OpenText(String),
    Reveal(String),
    Rename(String),
    MoveTo(String),
    Drop {
        path: String,
        folder: String,
    },
    Delete(String),
    NewDocument {
        kind: DocumentKind,
        folder: String,
    },
    NewFolder(String),
    Import(String),
    Register(String),
}

impl FolderApp {
    pub(super) fn explorer_rows(&self) -> Vec<Row> {
        let Some(session) = self.session.as_ref() else {
            return Vec::new();
        };
        let documents: Vec<_> = session
            .documents()
            .iter()
            .map(|document| Registered {
                id: document.id().to_owned(),
                path: document.entry().path.as_str().to_owned(),
                kind: document.entry().kind,
                missing: document.unavailable() == Some(Unavailable::Missing),
            })
            .collect();
        visible(
            tree(
                self.watcher.scan.as_ref(),
                &documents,
                &session.manifest().name,
            ),
            &self.workbench.expanded,
            self.explorer.root_collapsed,
        )
    }

    /// Selects and reveals the active document when another one becomes active.
    fn follow_active(&mut self) {
        let active = self.workbench.active().map(str::to_owned);
        if active == self.explorer.followed {
            return;
        }
        self.explorer.followed = active.clone();
        let Some(path) = active.and_then(|tab| match self.text_of(&tab) {
            Some(path) => Some(path.to_owned()),
            None => {
                let document = self.session.as_ref()?.document(&tab)?;
                Some(document.entry().path.as_str().to_owned())
            }
        }) else {
            return;
        };
        self.explorer.root_collapsed = false;
        let mut folder = parent(&path);
        while !folder.is_empty() {
            self.workbench.expanded.insert(folder.to_owned());
            folder = parent(folder);
        }
        self.explorer.selected = Some(path);
        self.explorer.reveal = true;
    }

    /// The path of the file the active tab shows, document or text.
    fn active_path(&self) -> Option<String> {
        let tab = self.workbench.active()?;
        if let Some(path) = self.text_of(tab) {
            return Some(path.to_owned());
        }
        let document = self.session.as_ref()?.document(tab)?;
        Some(document.entry().path.as_str().to_owned())
    }

    /// What activating a row does: a registered document opens as a document,
    /// any other file as text, and a folder folds.
    fn activate(row: &Row) -> Action {
        match &row.kind {
            RowKind::Root { .. } | RowKind::Directory => Action::Toggle(row.path.clone()),
            RowKind::Document { id, .. } => Action::Open(id.clone()),
            RowKind::Link => Action::Reveal(row.path.clone()),
            RowKind::Source | RowKind::File => Action::OpenText(row.path.clone()),
        }
    }

    pub(super) fn explorer_panel(&mut self, ui: &mut egui::Ui) {
        self.follow_active();
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let writable = !session.is_read_only();
        let rows = self.explorer_rows();
        let selected_row = self
            .explorer
            .selected
            .as_ref()
            .and_then(|selected| rows.iter().find(|row| &row.path == selected))
            .cloned();
        let mut actions = Vec::new();
        ui.horizontal(|ui| {
            widgets::caption(ui, "EXPLORER");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(widgets::icon_button(Icon::Refresh, "Refresh").compact())
                    .clicked()
                {
                    self.watcher.rescan();
                }
                if ui
                    .add(widgets::icon_button(Icon::Trash, "Trash").compact())
                    .clicked()
                {
                    self.open_trash();
                }
                ui.add_enabled_ui(writable, |ui| {
                    let folder = folder_of(selected_row.as_ref());
                    widgets::action_menu(
                        ui,
                        widgets::icon_button(Icon::Plus, "New").compact(),
                        |ui| new_items_menu(ui, &folder, &mut actions),
                    );
                });
            });
        });
        ui.add_space(theme::SPACE_SM);
        if let Some(problem) = &self.watcher.problem {
            ui.colored_label(theme::WARNING, problem);
        }
        if self
            .watcher
            .scan
            .as_ref()
            .is_some_and(|scan| scan.truncated)
        {
            widgets::hint(ui, "The folder is too large to list completely.");
        }
        let id = egui::Id::new("folder-explorer");
        // The panel behind the rows: it holds keyboard focus, takes drops onto
        // the project root and offers the root's context menu. Rows drawn later
        // are on top of it.
        let background = ui.interact(ui.available_rect_before_wrap(), id, egui::Sense::click());
        ui.ctx().accesskit_node_builder(id, |node| {
            node.set_role(egui::accesskit::Role::Tree);
            node.set_label("Project files");
        });
        let focused = ui.memory(|memory| memory.has_focus(id));
        if focused {
            ui.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    id,
                    egui::EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        tab: false,
                        escape: false,
                    },
                )
            });
            self.explorer_keys(ui.ctx(), &rows, &mut actions);
        }
        let selected = self.explorer.selected.clone();
        let status = self.row_status();
        let root_expanded = !self.explorer.root_collapsed;
        let focus = focused;
        // The rows get what is left of the panel; measuring it here, before
        // they are drawn, lets the offset be corrected in this same frame.
        let viewport = ui.available_height();
        let resized = (viewport - self.explorer.viewport).abs() > 0.5;
        let grew = rows.len() != self.explorer.rows;
        self.explorer.viewport = viewport;
        self.explorer.rows = rows.len();
        // Neither a resize nor a changed tree may leave the active file's row
        // off screen.
        let reveal = std::mem::take(&mut self.explorer.reveal)
            || ((resized || grew) && selected == self.active_path());
        let revealed = reveal
            .then(|| {
                selected
                    .as_deref()
                    .and_then(|path| rows.iter().position(|row| row.path == path))
            })
            .flatten();
        // A file activated before the folder was scanned has no row yet. Keep
        // the request until its row exists, instead of losing it silently.
        self.explorer.reveal = reveal && revealed.is_none();
        // The scroll takes effect in the next frame, so the row is checked
        // against the range that was actually drawn.
        let mut drawn = 0..0;
        // A row is exactly `ROW_HEIGHT` tall with nothing between rows. The
        // list derives its own row pitch from this spacing, so setting it
        // afterwards would leave its index arithmetic out of step with the
        // rows it draws, and the last rows unreachable.
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::ScrollArea::vertical()
            .id_salt("folder-explorer-rows")
            // Revealing a row must land in the next frame, not drift towards
            // the row over several.
            .animated(false)
            .auto_shrink([false, true])
            .show_rows(ui, ROW_HEIGHT, rows.len(), |ui, range| {
                drawn = range.clone();
                // A row outside the drawn range cannot ask to be scrolled to, so
                // its place is worked out from its index and the range's own top.
                if let Some(index) = revealed {
                    let top = ui.cursor().top() + (index as f32 - range.start as f32) * ROW_HEIGHT;

                    // Without an animation the row is in view in the next
                    // frame rather than several frames later.
                    ui.scroll_to_rect_animation(
                        egui::Rect::from_min_size(
                            egui::pos2(ui.cursor().left(), top),
                            egui::vec2(ui.available_width(), ROW_HEIGHT),
                        ),
                        None,
                        egui::style::ScrollAnimation::none(),
                    );
                }
                for row in &rows[range] {
                    let response = explorer_row(
                        ui,
                        row,
                        &RowFace {
                            selected: selected.as_deref() == Some(row.path.as_str()),
                            expanded: if row.is_root() {
                                root_expanded
                            } else {
                                self.workbench.expanded.contains(&row.path)
                            },
                            status: status.get(&row.path),
                            writable,
                            focused: focus,
                        },
                        &mut actions,
                    );
                    if response.clicked() || response.secondary_clicked() {
                        ui.memory_mut(|memory| memory.request_focus(id));
                    }
                }
            });
        // Asked for, but still outside what the list drew: ask again.
        if let Some(index) = revealed
            && !drawn.contains(&index)
        {
            self.explorer.reveal = true;
        }
        if let Some(dragged) = background.dnd_release_payload::<Dragged>()
            && writable
            && !parent(&dragged.0).is_empty()
        {
            actions.push(Action::Drop {
                path: dragged.0.clone(),
                folder: String::new(),
            });
        }
        if background.clicked() {
            background.request_focus();
        }
        background.context_menu(|ui| {
            widgets::name_menu(ui, "Project folder menu");
            ui.add_enabled_ui(writable, |ui| new_items_menu(ui, "", &mut actions));
        });
        for action in actions {
            self.explorer_action(action);
        }
    }

    /// Arrow keys move and fold, Enter opens, F2 renames, Delete deletes.
    fn explorer_keys(&mut self, context: &egui::Context, rows: &[Row], actions: &mut Vec<Action>) {
        let pressed = |key| context.input_mut(|input| input.consume_key(Modifiers::NONE, key));
        let index = self
            .explorer
            .selected
            .as_ref()
            .and_then(|selected| rows.iter().position(|row| &row.path == selected));
        let row = index.map(|index| &rows[index]);
        if pressed(Key::ArrowDown) {
            let next = index.map_or(0, |index| (index + 1).min(rows.len().saturating_sub(1)));
            if let Some(row) = rows.get(next) {
                actions.push(Action::Select(row.path.clone()));
            }
        }
        if pressed(Key::ArrowUp) {
            let previous = index.map_or(0, |index| index.saturating_sub(1));
            if let Some(row) = rows.get(previous) {
                actions.push(Action::Select(row.path.clone()));
            }
        }
        let Some(row) = row else { return };
        let expanded = if row.is_root() {
            !self.explorer.root_collapsed
        } else {
            self.workbench.expanded.contains(&row.path)
        };
        if pressed(Key::ArrowRight) && row.is_folder() && !expanded {
            actions.push(Action::Toggle(row.path.clone()));
        }
        if pressed(Key::ArrowLeft) {
            if row.is_folder() && expanded {
                actions.push(Action::Toggle(row.path.clone()));
            } else if !row.is_root() {
                // Every row's parent is a row, the project itself included.
                actions.push(Action::Select(parent(&row.path).to_owned()));
            }
        }
        if pressed(Key::Enter) {
            actions.push(Self::activate(row));
        }
        let writable = self
            .session
            .as_ref()
            .is_some_and(|session| !session.is_read_only());
        if writable && row.portable && !row.is_root() {
            if pressed(Key::F2) && !row.missing {
                actions.push(Action::Rename(row.path.clone()));
            }
            // A missing document can still be deleted: it leaves the registry
            // and its unsaved work goes to the trash.
            if pressed(Key::Delete) {
                actions.push(Action::Delete(row.path.clone()));
            }
        }
    }

    /// Status shown after a row's name, by path: documents report their own;
    /// folders report the most urgent status of what they contain.
    fn row_status(&self) -> HashMap<String, Status> {
        let mut status: HashMap<String, Status> = HashMap::new();
        let Some(session) = self.session.as_ref() else {
            return status;
        };
        for document in session.documents() {
            let state = if document.conflict().is_some() {
                Status::Conflict
            } else if document.analysis() == AnalysisState::Invalid
                && document.unavailable().is_none()
            {
                Status::Error
            } else if document.needs_recovery() {
                Status::Unsaved
            } else {
                continue;
            };
            let mut path = document.entry().path.as_str();
            loop {
                let entry = status.entry(path.to_owned()).or_insert(state);
                *entry = (*entry).max(state);
                match path.rsplit_once('/') {
                    Some((parent, _)) => path = parent,
                    None => break,
                }
            }
        }
        status
    }

    pub(super) fn explorer_action(&mut self, action: Action) {
        match action {
            Action::Select(path) => {
                self.explorer.selected = Some(path);
                self.explorer.reveal = true;
            }
            Action::Toggle(path) => {
                if path.is_empty() {
                    self.explorer.root_collapsed = !self.explorer.root_collapsed;
                } else if !self.workbench.expanded.remove(&path) {
                    self.workbench.expanded.insert(path.clone());
                }
                self.explorer.selected = Some(path);
                self.touched();
            }
            Action::Open(document_id) => {
                self.activate_tab(&document_id);
                self.touched();
            }
            Action::OpenText(path) => self.open_text(&path),
            Action::Reveal(path) => reveal(&self.root.join(&path)),
            Action::Rename(path) => {
                let name = file_name(&path).to_owned();
                self.prompt = Some(Prompt::Rename {
                    path,
                    name,
                    error: None,
                });
            }
            Action::MoveTo(path) => {
                self.prompt = Some(Prompt::MoveTo {
                    folder: parent(&path).to_owned(),
                    path,
                    error: None,
                });
            }
            Action::Drop { path, folder } => {
                let target = moved_path(&path, &folder);
                if target != path
                    && !folder.starts_with(&format!("{path}/"))
                    && folder != path
                    && let Err(message) = self.relocate(&path, &target)
                {
                    self.notify(
                        format!("Could not move {}: {message}", file_name(&path)),
                        true,
                    );
                }
            }
            Action::Delete(path) => self.request_delete(vec![path]),
            Action::NewDocument { kind, folder } => {
                self.prompt = Some(Prompt::NewDocument {
                    kind,
                    folder: Some(folder),
                    name: String::new(),
                    error: None,
                });
            }
            Action::NewFolder(parent) => {
                self.prompt = Some(Prompt::NewFolder {
                    parent,
                    name: String::new(),
                    error: None,
                });
            }
            Action::Import(folder) => self.pick_imports(folder),
            Action::Register(path) => {
                let target = path.clone();
                self.storage_project(
                    "Reading source header…",
                    apiforge_project::StorageMode::Read,
                    false,
                    move |session| {
                        let relative =
                            apiforge_storage::project::ProjectPath::working_source(target)?;
                        let snapshot = session.store().read(std::slice::from_ref(&relative))?;
                        Ok(snapshot.files[&relative]
                            .as_deref()
                            .and_then(|bytes| std::str::from_utf8(bytes).ok())
                            .and_then(apiforge_project::declaration)
                            .map(|declared| declared.kind))
                    },
                    move |app, result| match result {
                        Ok(Some(kind)) => {
                            if let Err(error) = app.register(&path, kind) {
                                app.notify(error, true);
                            }
                        }
                        Ok(None) => {
                            app.prompt = Some(Prompt::Register {
                                path,
                                kind: DocumentKind::Scenario,
                                error: None,
                            })
                        }
                        Err(error) => {
                            app.notify(format!("Cannot inspect this source: {error}"), true)
                        }
                    },
                );
            }
        }
    }

    /// Queues registration; success is acknowledged only after its transaction.
    pub(super) fn register(&mut self, path: &str, kind: DocumentKind) -> Result<(), String> {
        if self.session.is_none() {
            return Err("No project is open.".into());
        }
        let path = path.to_owned();
        let target = path.clone();
        self.working_project(
            "Registering source…",
            move |session| {
                session.register_existing(&[Registration { path: target, kind }], DIAGRAM)
            },
            move |app, result| match result {
                Ok(ids) => {
                    if let Some(id) = ids.first() {
                        app.activate_tab(id);
                    }
                    app.watcher.rescan();
                    app.touched();
                }
                Err(error) => app.restore_prompt(
                    Prompt::Register {
                        path,
                        kind,
                        error: None,
                    },
                    error.to_string(),
                ),
            },
        );
        Ok(())
    }

    pub(super) fn relocate(&mut self, from: &str, to: &str) -> Result<(), String> {
        self.relocate_with(from, to, None)
    }

    pub(super) fn relocate_with(
        &mut self,
        from: &str,
        to: &str,
        failed: Option<Prompt>,
    ) -> Result<(), String> {
        if self.session.is_none() {
            return Err("No project is open.".into());
        }
        let from = from.to_owned();
        let to = to.to_owned();
        let source = from.clone();
        let target = to.clone();
        self.working_project(
            "Moving file…",
            move |session| session.relocate(&source, &target, DIAGRAM),
            move |app, result| match result {
                Ok(_) => app.finish_relocation(&from, &to),
                Err(error) => {
                    if let Some(prompt) = failed {
                        app.restore_prompt(prompt, error.to_string());
                    } else {
                        app.notify(
                            format!("Could not move {}: {error}", file_name(&from)),
                            true,
                        );
                    }
                }
            },
        );
        Ok(())
    }

    fn finish_relocation(&mut self, from: &str, to: &str) {
        if self.explorer.selected.as_deref() == Some(from) {
            self.explorer.selected = Some(to.to_owned());
        }
        let prefix = format!("{from}/");
        let moved: Vec<_> = self
            .workbench
            .expanded
            .iter()
            .filter(|path| *path == from || path.starts_with(&prefix))
            .cloned()
            .collect();
        for path in moved {
            self.workbench.expanded.remove(&path);
            self.workbench
                .expanded
                .insert(format!("{to}{}", &path[from.len()..]));
        }
        let tabs: Vec<_> = self
            .workbench
            .tabs()
            .iter()
            .filter_map(|tab| {
                if self
                    .session
                    .as_ref()
                    .is_some_and(|session| session.document(&tab.document_id).is_some())
                {
                    return None;
                }
                let path = tab.document_id.strip_prefix(TEXT_TAB)?;
                (path == from || path.starts_with(&prefix)).then(|| {
                    (
                        tab.document_id.clone(),
                        text_tab(&format!("{to}{}", &path[from.len()..])),
                    )
                })
            })
            .collect();
        for (old, new) in tabs {
            if let Some(state) =
                egui::text_edit::TextEditState::load(&self.ui_context, editor_id(&old))
            {
                self.ui_context.data_mut(|data| {
                    data.remove::<egui::text_edit::TextEditState>(editor_id(&old))
                });
                state.store(&self.ui_context, editor_id(&new));
            }
            self.workbench.rename_tab(&old, &new);
            if self.shown.as_deref() == Some(old.as_str()) {
                self.shown = None;
            }
        }
        let parent = parent(to);
        if !parent.is_empty() {
            self.workbench.expanded.insert(parent.to_owned());
        }
        self.watcher.rescan();
        self.touched();
    }

    pub(super) fn request_delete(&mut self, paths: Vec<String>) {
        if self.session.is_none() {
            return;
        }
        let target = paths.clone();
        self.storage_project(
            "Inspecting items to delete…",
            apiforge_project::StorageMode::Read,
            false,
            move |session| {
                session.preview_delete(&target.iter().map(String::as_str).collect::<Vec<_>>())
            },
            move |app, result| match result {
                Ok(preview) => {
                    app.prompt = Some(Prompt::Delete {
                        preview: Box::new(preview),
                        paths,
                        error: None,
                    })
                }
                Err(error) => app.notify(format!("Cannot delete: {error}"), true),
            },
        );
    }

    pub(super) fn open_trash(&mut self) {
        if self.session.is_none() {
            return;
        }
        self.storage_project(
            "Reading trash…",
            apiforge_project::StorageMode::Read,
            false,
            |session| session.trash_operations(),
            |app, result| match result {
                Ok(operations) => {
                    app.prompt = Some(Prompt::Trash {
                        operations,
                        error: None,
                    })
                }
                Err(error) => app.notify(format!("Cannot read the trash: {error}"), true),
            },
        );
    }
}

/// Most urgent last, so that `max` picks what a folder shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Status {
    Unsaved,
    Error,
    Conflict,
}

fn new_items_menu(ui: &mut egui::Ui, folder: &str, actions: &mut Vec<Action>) {
    widgets::hint(
        ui,
        &if folder.is_empty() {
            "In the project folder".to_owned()
        } else {
            format!("In {folder}")
        },
    );
    if ui.button("New scenario").clicked() {
        actions.push(Action::NewDocument {
            kind: DocumentKind::Scenario,
            folder: folder.to_owned(),
        });
    }
    if ui.button("New component").clicked() {
        actions.push(Action::NewDocument {
            kind: DocumentKind::Component,
            folder: folder.to_owned(),
        });
    }
    if ui.button("New folder").clicked() {
        actions.push(Action::NewFolder(folder.to_owned()));
    }
    ui.separator();
    if ui.button("Import files…").clicked() {
        actions.push(Action::Import(folder.to_owned()));
    }
}

/// How a row is shown this frame: its state in the tree and what the session
/// allows, apart from the row itself.
pub(super) struct RowFace<'a> {
    selected: bool,
    expanded: bool,
    status: Option<&'a Status>,
    writable: bool,
    /// The list has the keyboard, so the selected row shows the focus.
    focused: bool,
}

fn explorer_row(
    ui: &mut egui::Ui,
    row: &Row,
    face: &RowFace<'_>,
    actions: &mut Vec<Action>,
) -> egui::Response {
    let RowFace {
        selected,
        expanded,
        status,
        writable,
        focused,
    } = *face;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), ROW_HEIGHT),
        egui::Sense::click_and_drag(),
    );
    // Named, with its role and state, so keyboard and assistive navigation can
    // follow the tree that is otherwise only painted.
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            if row.is_folder() {
                egui::WidgetType::CollapsingHeader
            } else {
                egui::WidgetType::SelectableLabel
            },
            true,
            selected,
            row.name(),
        )
    });
    if row.is_folder() {
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_expanded(expanded);
        });
    }
    let response = response.on_hover_text(match (&row.kind, row.missing) {
        (RowKind::Root { .. }, _) => "The project folder".to_owned(),
        (_, true) => format!("{} — missing from the folder", row.path),
        (RowKind::Source, _) => format!("{} — not registered in this project", row.path),
        (RowKind::Link, _) => format!("{} — a link; not followed", row.path),
        _ => row.path.clone(),
    });
    let painter = ui.painter_at(rect);
    let drop_target = row.is_folder()
        && response
            .dnd_hover_payload::<Dragged>()
            .is_some_and(|dragged| dragged.0 != row.path);
    if selected || drop_target {
        painter.rect_filled(rect, 4.0, theme::ACCENT_SOFT);
    } else if response.hovered() {
        painter.rect_filled(rect, 4.0, theme::FIELD);
    }
    // Keyboard focus is in the list as a whole, so the selected row shows it.
    if selected && focused {
        painter.rect_stroke(
            rect.shrink(1.0),
            4.0,
            egui::Stroke::new(1.0, theme::ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    let mut x = rect.left() + 4.0 + row.depth as f32 * INDENT;
    let icon_rect = |x: f32| {
        egui::Rect::from_center_size(egui::pos2(x + 7.0, rect.center().y), egui::vec2(14.0, 14.0))
    };
    let chevron = row.is_folder().then(|| icon_rect(x));
    if let Some(chevron) = chevron {
        if expanded {
            Icon::ChevronDown
        } else {
            Icon::ChevronRight
        }
        .paint(&painter, chevron, theme::MUTED);
    }
    x += 16.0;
    let (icon, colour) = match &row.kind {
        RowKind::Root { .. } => (Icon::Folder, theme::TEXT),
        RowKind::Directory => (Icon::Folder, theme::MUTED),
        RowKind::Document {
            kind: DocumentKind::Component,
            ..
        } => (Icon::Layers, theme::TEXT),
        RowKind::Document { .. } => (Icon::Code, theme::TEXT),
        _ => (Icon::File, theme::MUTED),
    };
    icon.paint(&painter, icon_rect(x), colour);
    x += 20.0;
    let text_colour = match (&row.kind, row.missing) {
        (_, true) => theme::MUTED,
        (RowKind::Document { .. } | RowKind::Directory | RowKind::Root { .. }, _) => theme::TEXT,
        _ => theme::MUTED,
    };
    let badge_width = if status.is_some() { 16.0 } else { 0.0 };
    let mut job = egui::text::LayoutJob::single_section(
        row.name().to_owned(),
        egui::TextFormat::simple(egui::FontId::proportional(theme::FONT_BODY), text_colour),
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(
        (rect.right() - x - badge_width - 6.0).max(0.0),
    );
    let galley = painter.layout_job(job);
    painter.galley(
        egui::pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        text_colour,
    );
    if let Some(status) = status {
        let centre = egui::pos2(rect.right() - 10.0, rect.center().y);
        match status {
            Status::Conflict => {
                painter.circle_stroke(centre, 4.0, egui::Stroke::new(1.6, theme::WARNING));
            }
            Status::Error => {
                painter.circle_filled(centre, 3.5, theme::DANGER);
            }
            Status::Unsaved => {
                painter.circle_filled(centre, 3.5, theme::MUTED);
            }
        }
    }
    // One click selects and activates: a file opens, a folder folds. A second
    // click on an open file activates its existing tab rather than a new one.
    // Folding the project hides every row, so the project row does that only
    // through its chevron, Enter or the arrow keys.
    if response.clicked() {
        actions.push(Action::Select(row.path.clone()));
        let on_chevron = chevron.zip(response.interact_pointer_pos());
        if !row.is_root() || on_chevron.is_some_and(|(chevron, at)| chevron.contains(at)) {
            actions.push(FolderApp::activate(row));
        }
    }
    if writable && row.portable && !row.missing && !row.is_root() && response.drag_started() {
        response.dnd_set_drag_payload(Dragged(row.path.clone()));
    }
    if row.is_folder()
        && writable
        && let Some(dragged) = response.dnd_release_payload::<Dragged>()
        && dragged.0 != row.path
        && !row.path.starts_with(&format!("{}/", dragged.0))
    {
        actions.push(Action::Drop {
            path: dragged.0.clone(),
            folder: row.path.clone(),
        });
    }
    response.context_menu(|ui| {
        widgets::name_menu(ui, &format!("{} menu", row.name()));
        actions.push(Action::Select(row.path.clone()));
        match &row.kind {
            RowKind::Document { id, .. } if ui.button("Open").clicked() => {
                actions.push(Action::Open(id.clone()));
            }
            RowKind::Source | RowKind::File if ui.button("Open as text").clicked() => {
                actions.push(Action::OpenText(row.path.clone()));
            }
            _ => {}
        }
        if row.kind == RowKind::Source
            && row.portable
            && ui
                .add_enabled(writable, egui::Button::new("Register in project"))
                .clicked()
        {
            actions.push(Action::Register(row.path.clone()));
        }
        if ui.button("Reveal in file manager").clicked() {
            actions.push(Action::Reveal(row.path.clone()));
        }
        ui.separator();
        ui.add_enabled_ui(writable, |ui| {
            if row.is_folder() {
                new_items_menu(ui, &row.path, actions);
            }
            // The project itself is not renamed, moved or deleted through
            // ordinary explorer file actions.
            if row.is_root() {
                return;
            }
            if row.is_directory() {
                ui.separator();
            }
            ui.add_enabled_ui(row.portable && !row.missing, |ui| {
                if ui.button("Rename…").on_hover_text("F2").clicked() {
                    actions.push(Action::Rename(row.path.clone()));
                }
                if ui.button("Move to…").clicked() {
                    actions.push(Action::MoveTo(row.path.clone()));
                }
            });
            if ui
                .add_enabled(row.portable, egui::Button::new("Delete…"))
                .on_hover_text("Delete")
                .clicked()
            {
                actions.push(Action::Delete(row.path.clone()));
            }
        });
    });
    response
}

/// Shows a file or folder in the system file manager.
pub fn reveal(path: &std::path::Path) {
    let path = path.to_owned();
    std::thread::spawn(move || {
        let path = path.as_path();
        #[cfg(target_os = "windows")]
        let command = std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn();
        #[cfg(target_os = "macos")]
        let command = std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn();
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let command = {
            let target = if path.is_dir() {
                path
            } else {
                path.parent().unwrap_or(path)
            };
            std::process::Command::new("xdg-open").arg(target).spawn()
        };
        if let Ok(mut child) = command {
            let _ = child.wait();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(paths: &[(&str, bool)]) -> TreeScan {
        TreeScan {
            generation: 1,
            entries: paths
                .iter()
                .map(|(path, directory)| TreeEntry {
                    path: (*path).into(),
                    kind: if *directory {
                        TreeEntryKind::Directory
                    } else {
                        TreeEntryKind::File {
                            bytes: 1,
                            sha256: None,
                        }
                    },
                    portable: true,
                })
                .collect(),
            truncated: false,
        }
    }

    #[test]
    fn rows_mark_documents_sources_and_missing_files_and_fold_by_folder() {
        let scan = scan(&[
            ("flows", true),
            ("flows/main.afs", false),
            ("flows/loose.afs", false),
            ("README.md", false),
        ]);
        let documents = vec![
            Registered {
                id: "main".into(),
                path: "flows/main.afs".into(),
                kind: DocumentKind::Scenario,
                missing: false,
            },
            Registered {
                id: "gone".into(),
                path: "lost/gone.afs".into(),
                kind: DocumentKind::Component,
                missing: true,
            },
            // Left out of a bounded scan, but present.
            Registered {
                id: "deep".into(),
                path: "deep/far.afs".into(),
                kind: DocumentKind::Scenario,
                missing: false,
            },
        ];
        let rows = tree(Some(&scan), &documents, "Shop");
        let summary: Vec<_> = rows
            .iter()
            .map(|row| (row.path.as_str(), row.depth, row.missing))
            .collect();
        assert_eq!(
            summary,
            [
                ("", 0, false),
                ("deep", 1, false),
                ("deep/far.afs", 2, false),
                ("flows", 1, false),
                ("flows/loose.afs", 2, false),
                ("flows/main.afs", 2, false),
                ("lost", 1, false),
                ("lost/gone.afs", 2, true),
                ("README.md", 1, false),
            ]
        );
        assert_eq!(rows[0].name(), "Shop", "the project is the first row");
        assert!(rows[0].is_root() && !rows[0].portable);
        assert_eq!(rows[4].kind, RowKind::Source);
        assert!(matches!(&rows[5].kind, RowKind::Document { id, .. } if id == "main"));
        assert_eq!(rows[8].kind, RowKind::File);

        let folded = visible(rows.clone(), &BTreeSet::new(), false);
        assert_eq!(
            folded
                .iter()
                .map(|row| row.path.as_str())
                .collect::<Vec<_>>(),
            ["", "deep", "flows", "lost", "README.md"]
        );
        let open = visible(rows.clone(), &BTreeSet::from(["flows".to_owned()]), false);
        assert_eq!(open.len(), 7);
        let collapsed = visible(rows, &BTreeSet::new(), true);
        assert_eq!(collapsed.len(), 1, "a collapsed project hides its tree");
    }

    #[test]
    fn new_items_go_into_the_selected_folder_or_beside_the_selected_file() {
        let rows = tree(
            Some(&scan(&[("a", true), ("a/b.afs", false), ("c.md", false)])),
            &[],
            "Shop",
        );
        assert_eq!(folder_of(Some(&rows[0])), "", "the project row");
        assert_eq!(folder_of(Some(&rows[1])), "a");
        assert_eq!(folder_of(Some(&rows[2])), "a");
        assert_eq!(folder_of(Some(&rows[3])), "");
        assert_eq!(folder_of(None), "");
        assert_eq!(moved_path("a/b.afs", ""), "b.afs");
        assert_eq!(moved_path("c.md", "a"), "a/c.md");
    }
}
