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
        file_name(&self.path)
    }
    pub fn is_directory(&self) -> bool {
        self.kind == RowKind::Directory
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
}

/// Every row of the tree, in explorer order, before folding.
pub fn tree(scan: Option<&TreeScan>, documents: &[(String, String, DocumentKind)]) -> Vec<Row> {
    let mut entries: Vec<TreeEntry> = scan.map(|scan| scan.entries.clone()).unwrap_or_default();
    let mut known: HashSet<String> = entries.iter().map(|entry| entry.path.clone()).collect();
    let mut missing = HashSet::new();
    for (_, path, _) in documents {
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
        missing.insert(path.clone());
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
    entries
        .into_iter()
        .map(|entry| {
            let document = documents.iter().find(|(_, path, _)| *path == entry.path);
            let kind = match (&entry.kind, document) {
                (_, Some((id, _, kind))) => RowKind::Document {
                    id: id.clone(),
                    kind: *kind,
                },
                (TreeEntryKind::Directory, None) => RowKind::Directory,
                (TreeEntryKind::Link, None) => RowKind::Link,
                (TreeEntryKind::File { .. }, None) if entry.path.ends_with(".afs") => {
                    RowKind::Source
                }
                (TreeEntryKind::File { .. }, None) => RowKind::File,
            };
            Row {
                depth: entry.path.matches('/').count(),
                missing: missing.contains(&entry.path),
                path: entry.path,
                kind,
                portable: entry.portable,
            }
        })
        .collect()
}

/// The rows whose every ancestor folder is expanded.
pub fn visible(rows: Vec<Row>, expanded: &BTreeSet<String>) -> Vec<Row> {
    rows.into_iter()
        .filter(|row| {
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
        Some(row) if row.is_directory() => row.path.clone(),
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
    Reveal(String),
    Rename(String),
    MoveTo(String),
    Drop { path: String, folder: String },
    Delete(String),
    NewDocument { kind: DocumentKind, folder: String },
    NewFolder(String),
    Import(String),
}

impl FolderApp {
    pub(super) fn explorer_rows(&self) -> Vec<Row> {
        let Some(session) = self.session.as_ref() else {
            return Vec::new();
        };
        let documents: Vec<_> = session
            .documents()
            .iter()
            .map(|document| {
                (
                    document.id().to_owned(),
                    document.entry().path.as_str().to_owned(),
                    document.entry().kind,
                )
            })
            .collect();
        visible(
            tree(self.watcher.scan.as_ref(), &documents),
            &self.workbench.expanded,
        )
    }

    /// Selects and reveals the active document when another one becomes active.
    fn follow_active(&mut self) {
        let active = self.workbench.active().map(str::to_owned);
        if active == self.explorer.followed {
            return;
        }
        self.explorer.followed = active.clone();
        let Some(path) = active.and_then(|id| {
            let document = self.session.as_ref()?.document(&id)?;
            Some(document.entry().path.as_str().to_owned())
        }) else {
            return;
        };
        let mut folder = parent(&path);
        while !folder.is_empty() {
            self.workbench.expanded.insert(folder.to_owned());
            folder = parent(folder);
        }
        self.explorer.selected = Some(path);
        self.explorer.reveal = true;
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
        let reveal = std::mem::take(&mut self.explorer.reveal);
        let selected = self.explorer.selected.clone();
        let status = self.row_status();
        egui::ScrollArea::vertical()
            .id_salt("folder-explorer-rows")
            .auto_shrink([false, true])
            .show_rows(ui, ROW_HEIGHT, rows.len(), |ui, range| {
                for row in &rows[range] {
                    let response = explorer_row(
                        ui,
                        row,
                        selected.as_deref() == Some(row.path.as_str()),
                        self.workbench.expanded.contains(&row.path),
                        status.get(&row.path),
                        writable,
                        &mut actions,
                    );
                    if reveal && selected.as_deref() == Some(row.path.as_str()) {
                        response.scroll_to_me(None);
                    }
                    if response.clicked() || response.secondary_clicked() {
                        ui.memory_mut(|memory| memory.request_focus(id));
                    }
                }
            });
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
        let expanded = self.workbench.expanded.contains(&row.path);
        if pressed(Key::ArrowRight) && row.is_directory() && !expanded {
            actions.push(Action::Toggle(row.path.clone()));
        }
        if pressed(Key::ArrowLeft) {
            if row.is_directory() && expanded {
                actions.push(Action::Toggle(row.path.clone()));
            } else if !parent(&row.path).is_empty() {
                actions.push(Action::Select(parent(&row.path).to_owned()));
            }
        }
        if pressed(Key::Enter) {
            actions.push(match &row.kind {
                RowKind::Directory => Action::Toggle(row.path.clone()),
                RowKind::Document { id, .. } => Action::Open(id.clone()),
                _ => Action::Reveal(row.path.clone()),
            });
        }
        let writable = self
            .session
            .as_ref()
            .is_some_and(|session| !session.is_read_only());
        if writable && row.portable && !row.missing {
            if pressed(Key::F2) {
                actions.push(Action::Rename(row.path.clone()));
            }
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
                if !self.workbench.expanded.remove(&path) {
                    self.workbench.expanded.insert(path.clone());
                }
                self.explorer.selected = Some(path);
                self.touched();
            }
            Action::Open(document_id) => {
                self.workbench.open(&document_id);
                self.touched();
            }
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
        }
    }

    /// Renames or moves, keeping document identity, and reports the result.
    pub(super) fn relocate(&mut self, from: &str, to: &str) -> Result<(), String> {
        let Some(session) = self.session.as_mut() else {
            return Ok(());
        };
        match session.relocate(from, to, DIAGRAM) {
            Ok(_) => {
                if self
                    .explorer
                    .selected
                    .as_deref()
                    .is_some_and(|selected| selected == from)
                {
                    self.explorer.selected = Some(to.to_owned());
                }
                // Keep moved folders expanded under their new names.
                let prefix = format!("{from}/");
                let moved: Vec<String> = self
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
                let parent = parent(to);
                if !parent.is_empty() {
                    self.workbench.expanded.insert(parent.to_owned());
                }
                self.watcher.rescan();
                self.touched();
                Ok(())
            }
            Err(error) => Err(error.to_string()),
        }
    }

    pub(super) fn request_delete(&mut self, paths: Vec<String>) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
        match session.preview_delete(&refs) {
            Ok(preview) => {
                self.prompt = Some(Prompt::Delete {
                    preview: Box::new(preview),
                    paths,
                    error: None,
                })
            }
            Err(error) => self.notify(format!("Cannot delete: {error}"), true),
        }
    }

    pub(super) fn open_trash(&mut self) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        match session.trash_operations() {
            Ok(operations) => {
                self.prompt = Some(Prompt::Trash {
                    operations,
                    error: None,
                })
            }
            Err(error) => self.notify(format!("Cannot read the trash: {error}"), true),
        }
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

fn explorer_row(
    ui: &mut egui::Ui,
    row: &Row,
    selected: bool,
    expanded: bool,
    status: Option<&Status>,
    writable: bool,
    actions: &mut Vec<Action>,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), ROW_HEIGHT),
        egui::Sense::click_and_drag(),
    );
    let response = response.on_hover_text(match (&row.kind, row.missing) {
        (_, true) => format!("{} — missing from the folder", row.path),
        (RowKind::Source, _) => format!("{} — not registered in this project", row.path),
        (RowKind::Link, _) => format!("{} — a link; not followed", row.path),
        _ => row.path.clone(),
    });
    let painter = ui.painter_at(rect);
    let drop_target = row.is_directory()
        && response
            .dnd_hover_payload::<Dragged>()
            .is_some_and(|dragged| dragged.0 != row.path);
    if selected || drop_target {
        painter.rect_filled(rect, 4.0, theme::ACCENT_SOFT);
    } else if response.hovered() {
        painter.rect_filled(rect, 4.0, theme::FIELD);
    }
    let mut x = rect.left() + 4.0 + row.depth as f32 * INDENT;
    let icon_rect = |x: f32| {
        egui::Rect::from_center_size(egui::pos2(x + 7.0, rect.center().y), egui::vec2(14.0, 14.0))
    };
    if row.is_directory() {
        let chevron = if expanded {
            Icon::ChevronDown
        } else {
            Icon::ChevronRight
        };
        chevron.paint(&painter, icon_rect(x), theme::MUTED);
    }
    x += 16.0;
    let (icon, colour) = match &row.kind {
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
        (RowKind::Document { .. } | RowKind::Directory, _) => theme::TEXT,
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
    if response.clicked() {
        actions.push(Action::Select(row.path.clone()));
        if row.is_directory() {
            actions.push(Action::Toggle(row.path.clone()));
        }
    }
    if response.double_clicked() {
        match &row.kind {
            RowKind::Document { id, .. } => actions.push(Action::Open(id.clone())),
            RowKind::Directory => {}
            _ => actions.push(Action::Reveal(row.path.clone())),
        }
    }
    if writable && row.portable && !row.missing && response.drag_started() {
        response.dnd_set_drag_payload(Dragged(row.path.clone()));
    }
    if row.is_directory()
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
        actions.push(Action::Select(row.path.clone()));
        if let RowKind::Document { id, .. } = &row.kind
            && ui.button("Open").clicked()
        {
            actions.push(Action::Open(id.clone()));
        }
        if ui.button("Reveal in file manager").clicked() {
            actions.push(Action::Reveal(row.path.clone()));
        }
        ui.separator();
        ui.add_enabled_ui(writable, |ui| {
            if row.is_directory() {
                new_items_menu(ui, &row.path, actions);
                ui.separator();
            }
            ui.add_enabled_ui(row.portable && !row.missing, |ui| {
                if ui.button("Rename…").on_hover_text("F2").clicked() {
                    actions.push(Action::Rename(row.path.clone()));
                }
                if ui.button("Move to…").clicked() {
                    actions.push(Action::MoveTo(row.path.clone()));
                }
                if ui.button("Delete…").on_hover_text("Delete").clicked() {
                    actions.push(Action::Delete(row.path.clone()));
                }
            });
        });
    });
    response
}

/// Shows a file or folder in the system file manager.
pub fn reveal(path: &std::path::Path) {
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
    // Reap the child off the UI thread; a missing opener is not an error here.
    if let Ok(mut child) = command {
        std::thread::spawn(move || child.wait());
    }
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
            (
                "main".to_owned(),
                "flows/main.afs".to_owned(),
                DocumentKind::Scenario,
            ),
            (
                "gone".to_owned(),
                "lost/gone.afs".to_owned(),
                DocumentKind::Component,
            ),
        ];
        let rows = tree(Some(&scan), &documents);
        let summary: Vec<_> = rows
            .iter()
            .map(|row| (row.path.as_str(), row.depth, row.missing))
            .collect();
        assert_eq!(
            summary,
            [
                ("flows", 0, false),
                ("flows/loose.afs", 1, false),
                ("flows/main.afs", 1, false),
                ("lost", 0, false),
                ("lost/gone.afs", 1, true),
                ("README.md", 0, false),
            ]
        );
        assert_eq!(rows[1].kind, RowKind::Source);
        assert!(matches!(&rows[2].kind, RowKind::Document { id, .. } if id == "main"));
        assert_eq!(rows[5].kind, RowKind::File);

        let folded = visible(rows.clone(), &BTreeSet::new());
        assert_eq!(
            folded
                .iter()
                .map(|row| row.path.as_str())
                .collect::<Vec<_>>(),
            ["flows", "lost", "README.md"]
        );
        let open = visible(rows, &BTreeSet::from(["flows".to_owned()]));
        assert_eq!(open.len(), 5);
    }

    #[test]
    fn new_items_go_into_the_selected_folder_or_beside_the_selected_file() {
        let rows = tree(
            Some(&scan(&[("a", true), ("a/b.afs", false), ("c.md", false)])),
            &[],
        );
        assert_eq!(folder_of(Some(&rows[0])), "a");
        assert_eq!(folder_of(Some(&rows[1])), "a");
        assert_eq!(folder_of(Some(&rows[2])), "");
        assert_eq!(folder_of(None), "");
        assert_eq!(moved_path("a/b.afs", ""), "b.afs");
        assert_eq!(moved_path("c.md", "a"), "a/c.md");
    }
}
