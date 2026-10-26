//! The diagram view: the active document's graph on the board, and the
//! inspector of its selected block.

use super::*;

impl FolderApp {
    pub(super) fn diagram_area(&mut self, ui: &mut egui::Ui, active: Option<&str>) {
        let Some(document_id) = active else {
            centred_hint(ui, "Open a scenario or component to see its diagram.");
            return;
        };
        if let Some(path) = self.text_of(document_id) {
            centred_hint(
                ui,
                &format!(
                    "{} is a text file, not a scenario or component, so it has no diagram.",
                    file_name(path)
                ),
            );
            return;
        }
        let Some(snapshot) = self.snapshot(document_id) else {
            return;
        };
        let Some(model) = snapshot.model.clone() else {
            centred_hint(
                ui,
                match (snapshot.unavailable, snapshot.analysis) {
                    (Some(Unavailable::Missing), _) => {
                        "This file is missing from the project folder."
                    }
                    (Some(Unavailable::NotUtf8), _) => {
                        "This file is not UTF-8 text, so it has no diagram."
                    }
                    (Some(Unavailable::TooLarge), _) => {
                        "This file exceeds the 1 MiB source limit, so it has no diagram."
                    }
                    (None, AnalysisState::Pending) => "Checking the code…",
                    _ => {
                        "The code has errors and there is no valid version to show. \
                         Fix the code to see its diagram."
                    }
                },
            );
            return;
        };
        let cleanup = self.cleanup(document_id);
        // The document's header shares the board's header row (KTLO-12): Split
        // leaves the file name and kind to the tab strip beside the code.
        let header = Header {
            name: (self.workbench.mode == Mode::Diagram)
                .then(|| (file_name(&snapshot.path), kind_label(snapshot.kind))),
            tabs: matches!(*model, Document::Scenario(_)),
        };
        if !snapshot.editable {
            let text = if snapshot.held {
                "Unsaved work from an earlier session needs a decision in the code first."
            } else if snapshot.analysis == AnalysisState::Pending {
                "Checking the latest code…"
            } else if snapshot.model_state == ModelState::LastValid {
                "The code has errors. This is the last valid version; it is read-only until \
                 the code is fixed."
            } else {
                "This diagram is read-only."
            };
            egui::Frame::new()
                .fill(theme::ACCENT_SOFT)
                .inner_margin(egui::Margin::symmetric(12, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal_wrapped(|ui| {
                        ui.label(text);
                        if self.workbench.mode == Mode::Diagram && ui.button("Show code").clicked()
                        {
                            self.workbench.mode = Mode::Code;
                            self.touched();
                        }
                    });
                });
        }
        let project = self.context_project();
        let run = self.fresh_run(document_id);
        let phase = if cleanup {
            apiforge_core::run::Phase::Cleanup
        } else {
            apiforge_core::run::Phase::Main
        };
        let steps = run.as_ref().map(|run| {
            run.steps
                .iter()
                .filter(|step| step.phase == phase)
                .cloned()
                .collect::<Vec<_>>()
        });
        let graph = graph_of(&model, cleanup);
        let reference = graph_of(&model, false);
        let parameters = parameters_of(&model);
        let selection = self
            .selections
            .get(document_id)
            .cloned()
            .unwrap_or_default();
        let context = context_key(document_id, cleanup);
        let can_paste = self
            .clipboard
            .as_ref()
            .is_some_and(|clipboard| clipboard.context == context);
        let library_in_view = self.library_in_view(ui.ctx());
        // Without the panel, the Add step popup carries its sections (KTLO-05d),
        // also while it is folded to its rail (KTLO-17).
        let library = (!library_in_view)
            .then(|| library_context(&project, document_id, &model, cleanup, snapshot.editable));
        if !self.canvases.contains_key(document_id) {
            let canvas = self.new_canvas();
            self.canvases.insert(document_id.to_owned(), canvas);
        }
        let canvas = self.canvases.get_mut(document_id).expect("inserted above");
        let mut chosen = cleanup;
        let commands = ui
            .scope(|ui| {
                if !snapshot.editable {
                    // Keep the last valid graph legible as a reference.
                    ui.visuals_mut().disabled_alpha = 1.0;
                    ui.disable();
                }
                let mut input = canvas::Input {
                    graph,
                    reference_graph: reference,
                    project: &project,
                    parameters: &parameters,
                    selection: &selection,
                    steps: steps.as_deref(),
                    can_paste,
                    context: &context,
                    library_in_view,
                    header_leading: None,
                    library: library.as_ref(),
                };
                let fit = header.fit(ui, canvas.header_room(ui, &input));
                input.header_leading = Some(egui::vec2(fit.width, theme::CONTROL_HEIGHT));
                let row = canvas.leading_rect(ui, &input);
                // Drawn before the board, so the keyboard reaches the header
                // first and then the board's toolbar on the same row.
                header.show(ui.ctx(), ui.id(), row, &fit, &mut chosen);
                canvas.show(ui, &input)
            })
            .inner;
        if chosen != cleanup {
            if let Some(tab) = self.workbench.tab_mut(document_id) {
                tab.cleanup = chosen;
            }
            self.selections.remove(document_id);
            self.touched();
        }
        for command in commands {
            self.board_command(document_id, &model, cleanup, snapshot.editable, command);
        }
    }

    pub(super) fn board_command(
        &mut self,
        document_id: &str,
        model: &Document,
        cleanup: bool,
        editable: bool,
        command: Command,
    ) {
        if !editable
            && !matches!(
                command,
                Command::Select(_)
                    | Command::Copy
                    | Command::SetWireStyle(_)
                    | Command::SetBridges(_)
            )
        {
            return;
        }
        let project = self.context_project();
        let graph = graph_of(model, cleanup);
        let reference = graph_of(model, false);
        let parameters = parameters_of(model);
        let board = Board {
            graph,
            reference,
            project: &project,
            parameters: &parameters,
        };
        let selection = self
            .selections
            .get(document_id)
            .cloned()
            .unwrap_or_default();
        let result: Option<Result<Edit, String>> = match command {
            Command::Select(ids) => {
                self.selections.insert(document_id.to_owned(), ids);
                None
            }
            Command::Move(positions) => Some(Ok(board.moved(positions).into())),
            Command::TidyUp(selection) => board.tidy_up(&selection).map(|graph| Ok(graph.into())),
            Command::SetWireStyle(style) => {
                self.preferences.wire_style = style;
                let preferences = self.preferences.clone();
                self.storage_io("Saving preferences…", move || preferences.save(), |_, _| {});
                for canvas in self.canvases.values_mut() {
                    canvas.wire_style = style;
                }
                None
            }
            Command::SetBridges(bridges) => {
                self.preferences.bridges = bridges;
                let preferences = self.preferences.clone();
                self.storage_io("Saving preferences…", move || preferences.save(), |_, _| {});
                for canvas in self.canvases.values_mut() {
                    canvas.bridges = bridges;
                }
                None
            }
            Command::AddNode {
                block,
                component,
                position,
                source,
            } => Some(board.add_node(block, component, position, source)),
            Command::AddOperator { operator, position } => {
                Some(Ok(board.push_node(create_operator_node(operator, position))))
            }
            Command::SetLiteral {
                node,
                input,
                path,
                value,
            } => board
                .set_input(
                    &node,
                    &input,
                    &path,
                    apiforge_core::format::Binding::literal(value),
                )
                .transpose()
                .map(|result| result.map(Into::into)),
            Command::Bind {
                node,
                input,
                path,
                binding,
            } => board
                .set_input(&node, &input, &path, binding)
                .transpose()
                .map(|result| result.map(Into::into)),
            Command::Delete { ids, wires } => {
                let next = board.delete(&ids, &wires);
                let kept = selection
                    .into_iter()
                    .filter(|id| !ids.contains(id))
                    .collect();
                Some(Ok(Edit {
                    graph: next,
                    selection: Some(kept),
                }))
            }
            Command::AddProxy { source, position } => Some(board.add_proxy(source, position)),
            Command::Connect {
                source,
                source_port,
                target,
                target_input,
                target_path,
            } => Some(
                board
                    .connect(&source, &source_port, &target, &target_input, &target_path)
                    .map(Into::into),
            ),
            Command::Copy => {
                let selected = selection.iter().map(String::as_str).collect();
                let fragment = edit::copy_selection(graph, &selected);
                if !fragment.nodes.is_empty() {
                    self.clipboard = Some(Clipboard {
                        context: context_key(document_id, cleanup),
                        fragment,
                        pastes: 0,
                    });
                }
                None
            }
            Command::Paste => self
                .clipboard
                .as_mut()
                .filter(|clipboard| clipboard.context == context_key(document_id, cleanup))
                .map(|clipboard| {
                    clipboard.pastes += 1;
                    let count = f64::from(clipboard.pastes);
                    board.paste(
                        &clipboard.fragment,
                        &Position {
                            x: edit::PASTE_OFFSET.x * count,
                            y: edit::PASTE_OFFSET.y * count,
                        },
                    )
                }),
            Command::Duplicate => (!selection.is_empty()).then(|| {
                let selected = selection.iter().map(String::as_str).collect();
                board.paste(&edit::copy_selection(graph, &selected), &edit::PASTE_OFFSET)
            }),
            Command::EditComponent(node_id) => {
                self.open_component(graph, &node_id);
                None
            }
            Command::SaveSelectionAsComponent => {
                self.notify(
                    "Saving a selection as a component is not available in folder projects yet.",
                    false,
                );
                None
            }
        };
        if let Some(result) = result {
            self.apply_edit(document_id, model, cleanup, result);
        }
    }

    /// A board edit becomes a model edit of the whole document: the session
    /// reprints the source for a semantic change and keeps layout separate.
    pub(super) fn apply_edit(
        &mut self,
        document_id: &str,
        model: &Document,
        cleanup: bool,
        result: Result<Edit, String>,
    ) {
        match result {
            Ok(edit) => self.component_action(
                document_id,
                publication::Action::Model {
                    next: Box::new(with_graph(model, cleanup, edit.graph)),
                    selection: edit.selection,
                },
            ),
            Err(message) => {
                if let Some(canvas) = self.canvases.get_mut(document_id) {
                    canvas.set_error(message);
                }
            }
        }
    }

    /// Opens the working document of the component a block calls.
    pub(super) fn open_component(&mut self, graph: &Graph, node_id: &str) {
        let project = self.context_project();
        let Some(component_id) = graph
            .nodes
            .iter()
            .find(|node| node.id == node_id)
            .and_then(|node| apiforge_core::graph::component_definition(&project, node))
            .map(|definition| definition.id.clone())
        else {
            return;
        };
        let document = self.session.as_ref().and_then(|session| {
            session
                .documents()
                .iter()
                .find(|document| {
                    document.entry().kind == DocumentKind::Component
                        && document.entry().entity_id == component_id
                })
                .map(|document| document.id().to_owned())
        });
        match document {
            Some(document_id) => {
                self.activate_tab(&document_id);
                self.touched();
            }
            None => self.notify(
                "This component has no working document in the project.",
                false,
            ),
        }
    }

    pub(super) fn inspector_panel(&mut self, ui: &mut egui::Ui, active: Option<&str>) {
        let Some(document_id) = active else {
            return;
        };
        let Some(snapshot) = self.snapshot(document_id) else {
            return;
        };
        let Some(model) = snapshot.model.clone() else {
            return;
        };
        let cleanup = self.cleanup(document_id);
        let graph = graph_of(&model, cleanup);
        let Some(node) = self
            .selections
            .get(document_id)
            .filter(|selection| selection.len() == 1)
            .and_then(|selection| graph.nodes.iter().find(|node| node.id == selection[0]))
            .cloned()
        else {
            return;
        };
        let project = self.context_project();
        let project_id = self
            .session
            .as_ref()
            .map(|session| session.manifest().id.clone())
            .unwrap_or_default();
        let reference = graph_of(&model, false);
        let parameters = parameters_of(&model);
        let run = self.fresh_run(document_id);
        let step = run
            .as_ref()
            .and_then(|run| run.steps.iter().find(|step| step.node_id == node.id));
        // Adopt fields the session changed before the inspector reads them.
        self.sync_field_drafts(document_id, &project_id);
        let mut close = false;
        let actions = egui::Panel::right("folder-inspector")
            .default_size(360.0)
            .size_range(300.0..=520.0)
            .frame(egui::Frame::new().fill(theme::WHITE).inner_margin(16))
            .show(ui, |ui| {
                inspector::drafts::set_document_owner(ui.ctx(), &project_id, document_id);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    close = ui
                        .add(widgets::icon_button(Icon::Close, "Close Inspector").compact())
                        .clicked();
                });
                ui.add_enabled_ui(snapshot.editable, |ui| {
                    self.inspector.show(
                        ui,
                        &inspector::Context {
                            node: &node,
                            graph: reference,
                            board: graph,
                            project: &project,
                            parameters: &parameters,
                            run: run.as_ref(),
                            step,
                        },
                    )
                })
                .inner
            })
            .inner;
        widgets::name_resize_handle(
            ui.ctx(),
            egui::Id::new("folder-inspector"),
            "Resize inspector",
        );
        if close {
            self.selections.remove(document_id);
        }
        for action in actions {
            match action {
                inspector::Action::Reveal(handle) => self.reveal_in_code(document_id, &handle),
                _ if !snapshot.editable => {}
                inspector::Action::UpdateNode(updated) => {
                    let mut next = graph.clone();
                    if let Some(slot) = next.nodes.iter_mut().find(|node| node.id == updated.id) {
                        *slot = *updated;
                        self.apply_edit(document_id, &model, cleanup, Ok(next.into()));
                    }
                }
                inspector::Action::UpdateGraph(next) => {
                    self.apply_edit(document_id, &model, cleanup, Ok(next.into()))
                }
                inspector::Action::EditComponent => self.open_component(graph, &node.id),
                inspector::Action::AssertField { port, path, value } => {
                    if let Some(run) = &run {
                        self.folder_report_action(
                            document_id,
                            run,
                            false,
                            crate::report::Action::AssertField {
                                node_id: node.id.clone(),
                                port,
                                path,
                                value,
                            },
                        );
                    }
                }
                inspector::Action::AssignField { port, path } => {
                    if let Some(run) = &run {
                        self.folder_report_action(
                            document_id,
                            run,
                            false,
                            crate::report::Action::AssignField {
                                node_id: node.id.clone(),
                                port,
                                path,
                            },
                        );
                    }
                }
                inspector::Action::Error(message) => {
                    if let Some(canvas) = self.canvases.get_mut(document_id) {
                        canvas.set_error(message);
                    }
                }
            }
        }
        self.sync_field_drafts(document_id, &project_id);
    }

    /// Unfinished inspector text joins the document's local recovery.
    pub(super) fn sync_field_drafts(&mut self, document_id: &str, project_id: &str) {
        if self.persistence.closing || self.storage.blocks_edits() {
            return;
        }
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let Some(document) = session.document(document_id) else {
            return;
        };
        let owned = document.field_drafts().to_vec();
        let agreed = self.agreed_fields.get(document_id);
        if agreed.map_or(&[][..], Vec::as_slice) != owned.as_slice() {
            // The session changed the fields itself; the inspector follows, and
            // its fields drop the text they still hold.
            inspector::forget_field_text(&self.ui_context);
            let entries = owned
                .iter()
                .map(|field| CodeDraft {
                    project_id: project_id.to_owned(),
                    target: DraftTarget::Field {
                        owner: Box::new(inspector::drafts::document_target(document_id)),
                        widget: field.widget,
                    },
                    base_hash: field.base_hash.clone(),
                    text: field.text.clone(),
                    cursor: field.selection.caret,
                })
                .collect();
            inspector::drafts::replace_document_entries(
                &self.ui_context,
                project_id,
                document_id,
                entries,
            );
            self.agreed_fields.insert(document_id.to_owned(), owned);
            return;
        }
        let owner = inspector::drafts::document_target(document_id);
        let fields: Vec<FieldRecovery> = inspector::drafts::entries(&self.ui_context)
            .into_iter()
            .filter(|draft| draft.project_id == project_id)
            .filter_map(|draft| match draft.target {
                DraftTarget::Field { owner: of, widget } if *of == owner => Some(FieldRecovery {
                    widget,
                    base_hash: draft.base_hash,
                    selection: TextSelection {
                        anchor: draft.cursor,
                        caret: draft.cursor,
                    },
                    text: draft.text,
                }),
                _ => None,
            })
            .collect();
        if fields != owned
            && session
                .set_field_drafts(document_id, fields.clone())
                .is_ok()
        {
            self.agreed_fields.insert(document_id.to_owned(), fields);
            self.touched();
        }
    }

    /// “Open in code”: the owning document's statement, in a view with code.
    pub(super) fn reveal_in_code(&mut self, document_id: &str, handle: &str) {
        if self.workbench.mode == Mode::Diagram {
            self.workbench.mode = Mode::Code;
            self.touched();
        }
        let Some(text) = self
            .session
            .as_ref()
            .and_then(|session| session.document(document_id))
            .map(|document| document.text().to_owned())
        else {
            return;
        };
        if let Some(range) = apiforge_script::complete::statement_range(&text, handle) {
            self.code.reveal(range.start);
            self.revealed = Some(handle.to_owned());
            self.ui_context
                .memory_mut(|memory| memory.request_focus(editor_id(document_id)));
        }
    }
}

/// The narrowest the file name is shortened to before the steps summary goes.
const NAME_MIN_WIDTH: f32 = 80.0;

/// The document's part of the board's header row (KTLO-12): in Diagram the
/// file name and kind, then Main / Cleanup for a scenario; in Split only
/// Main / Cleanup, since the tab strip beside the code names the file.
struct Header<'a> {
    name: Option<(&'a str, &'static str)>,
    tabs: bool,
}

/// What of the header fits, and its width.
struct HeaderFit {
    name_width: f32,
    kind: bool,
    width: f32,
}

impl Header<'_> {
    fn text_width(ui: &egui::Ui, text: &str, font: egui::FontId) -> f32 {
        ui.painter()
            .layout_no_wrap(text.to_owned(), font, theme::TEXT)
            .size()
            .x
    }

    /// Gives up, as the row narrows, first the kind, then the name's full
    /// width (its full text stays on hover), then the steps summary, which the
    /// board leaves out once the header takes its room.
    fn fit(&self, ui: &egui::Ui, room: canvas::HeaderRoom) -> HeaderFit {
        let gap = ui.spacing().item_spacing.x;
        let tabs = if self.tabs {
            widgets::tab_value_width(ui, "Main") + gap + widgets::tab_value_width(ui, "Cleanup")
        } else {
            0.0
        };
        let Some((name, kind)) = self.name else {
            return HeaderFit {
                name_width: 0.0,
                kind: false,
                width: tabs,
            };
        };
        let full = Self::text_width(
            ui,
            name,
            egui::FontId::new(theme::FONT_BODY, theme::bold_family()),
        );
        let kind_width =
            gap + Self::text_width(ui, kind, egui::FontId::proportional(theme::FONT_LABEL));
        let fixed = if self.tabs {
            gap + theme::SPACE_LG + tabs
        } else {
            0.0
        };
        // A point to spare keeps rounding from pushing the summary out.
        let (name_width, kind) = if full + kind_width + fixed <= room.with_summary {
            (full, true)
        } else if full + fixed <= room.with_summary {
            (full, false)
        } else if room.with_summary - fixed - 1.0 >= NAME_MIN_WIDTH {
            (room.with_summary - fixed - 1.0, false)
        } else {
            (
                (room.alone - fixed - 1.0).clamp(NAME_MIN_WIDTH.min(full), full),
                false,
            )
        };
        HeaderFit {
            name_width,
            kind,
            width: name_width + if kind { kind_width } else { 0.0 } + fixed,
        }
    }

    /// Draws the header into `row`, the left of the board's header row.
    fn show(
        &self,
        ctx: &egui::Context,
        id: egui::Id,
        row: egui::Rect,
        fit: &HeaderFit,
        cleanup: &mut bool,
    ) {
        egui::Area::new(id.with("board-document-header"))
            .fixed_pos(row.left_center())
            .pivot(egui::Align2::LEFT_CENTER)
            .fade_in(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some((name, kind)) = self.name {
                        ui.scope(|ui| {
                            ui.set_max_width(fit.name_width);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(name)
                                        .size(theme::FONT_BODY)
                                        .family(theme::bold_family()),
                                )
                                .truncate(),
                            );
                        });
                        if fit.kind {
                            widgets::hint(ui, kind);
                        }
                        if self.tabs {
                            ui.add_space(theme::SPACE_LG);
                        }
                    }
                    if self.tabs {
                        widgets::tab_value(ui, cleanup, false, "Main");
                        widgets::tab_value(ui, cleanup, true, "Cleanup");
                    }
                });
            });
    }
}
