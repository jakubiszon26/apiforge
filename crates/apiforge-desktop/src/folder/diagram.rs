//! The diagram view: the active document's graph on the board, and the
//! inspector of its selected block.

use super::*;
use apiforge_core::graph::{FALSE_PORT, ORDER_PORT, TRUE_PORT};

impl FolderApp {
    /// Where the sleeping dog is in its four-second loop, or its rest frame in
    /// a window that does not animate.
    fn dog_motion(&self, ui: &egui::Ui) -> dog::Motion {
        if self.animation {
            dog::Motion::at(ui.input(|input| input.time))
        } else {
            dog::Motion::REST
        }
    }

    pub(super) fn diagram_area(&mut self, ui: &mut egui::Ui, active: Option<&str>) {
        let Some(document_id) = active else {
            // The sleeping dog keeps the empty board company (KTLO-20); the
            // board's other hints announce a problem or a wait and stay plain.
            let motion = self.dog_motion(ui);
            let drawn = hint_with_dog(
                ui,
                "Open a scenario or component to see its diagram.",
                motion,
            );
            // The loop runs only while the drawing is on screen: the frame
            // that draws it is the one that asks for the next.
            if drawn && self.animation {
                dog::next_frame(ui.ctx());
            }
            return;
        };
        if let Some(path) = self.text_of(document_id) {
            let text = format!(
                "{} is a text file, not a scenario or component, so it has no diagram.",
                file_name(path)
            );
            // Diagram offers the way to its text, as the read-only board does
            // (UX-23, decision 15); Split shows the text beside it already.
            if self.workbench.mode != Mode::Diagram {
                centred_hint(ui, &text);
            } else if centred_hint_with_action(ui, &text, "Show code") {
                self.workbench.mode = Mode::Code;
                self.touched();
            }
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
        let document_title = match &*model {
            Document::Scenario(scenario) => scenario.name.clone(),
            Document::Component(component) => component.name.clone(),
        };
        let header = Header {
            name: (self.workbench.mode == Mode::Diagram)
                .then(|| (file_name(&snapshot.path), kind_label(snapshot.kind))),
            tabs: matches!(*model, Document::Scenario(_)),
            title: (crate::mock::title() != 0).then_some(document_title.as_str()),
        };
        // While observing, the bar at the bottom says why (MCP-08a, M1). A
        // save or a publication holds the board for a moment, and the
        // analysis a publication starts checks code that did not change:
        // neither puts a notice above the board (UX-25).
        let rechecking = snapshot.analysis == AnalysisState::Pending
            && snapshot.model_describes_text
            && !snapshot.held
            && snapshot.conflict.is_none()
            && snapshot.unavailable.is_none();
        if !snapshot.editable && !snapshot.paused && !snapshot.busy && !rechecking {
            let text = read_only_reason(&snapshot);
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
        // Resolved with the report, once per run (LAY-19).
        let ends = self.run_ends(document_id);
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
        // Where a run that is on has got to, and the run before it (LAY-07).
        let on = run
            .as_ref()
            .is_some_and(|run| run.status == apiforge_core::run::RunStatus::Running);
        let running = run
            .as_ref()
            .filter(|_| on)
            .and_then(|run| run.running.clone());
        let previous = self.previous_run(document_id).map(|run| {
            run.steps
                .iter()
                .filter(|step| step.phase == phase)
                .cloned()
                .collect::<Vec<_>>()
        });
        let graph = graph_of(&model, cleanup);
        // Built once per model, not every frame (UX-16).
        let combined = cleanup.then(|| self.cleanup_reference(&model));
        let reference = combined
            .as_deref()
            .unwrap_or_else(|| graph_of(&model, false));
        let parameters = parameters_of(&model);
        let shared = self.shared_values_of(&model);
        let selection = self
            .selections
            .get(document_id)
            .cloned()
            .unwrap_or_default();
        let context = context_key(document_id, cleanup);
        // A component's board shows its inputs in the INPUTS strip (UX-31).
        let inputs = match &*model {
            Document::Component(component) => Some(component.inputs.as_slice()),
            Document::Scenario(_) => None,
        };
        let input_selected =
            selected_input(&model, &selection, self.selected_inputs.get(document_id))
                .map(|port| port.name.clone());
        if input_selected.is_none() {
            // A block selected elsewhere, or an input the code no longer
            // declares, ends the input's selection.
            self.selected_inputs.remove(document_id);
        }
        // A block selected by any path, a run's step or an added block among
        // them, leaves the *New input* form (UX-32).
        if !selection.is_empty() {
            self.new_inputs.remove(document_id);
        }
        // A selected block or input, the *New input* form, or a scenario's
        // board leaves no room for the component's settings (UX-33).
        if !selection.is_empty()
            || input_selected.is_some()
            || inputs.is_none()
            || self.new_inputs.contains_key(document_id)
        {
            self.component_settings.remove(document_id);
        }
        let settings = inputs.map(|_| canvas::SettingsButton {
            open: self.component_settings.contains(document_id),
            available: self.library_room(ui.ctx()),
            header_width: header.full_width(ui),
        });
        // The input the *New input* form is adding (UX-32).
        let adding_input = inputs
            .and_then(|_| self.new_inputs.get(document_id))
            .map(|form| form.name.clone());
        let can_paste = self.clipboard.is_some();
        let library_in_view = self.library_in_view(ui.ctx());
        // Without the panel, the Add step popup carries its sections (KTLO-05d),
        // also while it is folded to its rail (KTLO-17).
        let library = (!library_in_view)
            .then(|| library_context(&project, document_id, &model, cleanup, snapshot.editable));
        if !self.canvases.contains_key(document_id) {
            let canvas = self.new_canvas();
            self.canvases.insert(document_id.to_owned(), canvas);
            self.show_checks_here(document_id);
        }
        let mode = self.workbench.mode;
        let canvas = self.canvases.get_mut(document_id).expect("inserted above");
        // An edit that is not the delete, made anywhere, ends its notice
        // (DIAG-06b).
        if !canvas.has_notice()
            || !self
                .notice_models
                .get(document_id)
                .is_some_and(|at_delete| Arc::ptr_eq(at_delete, &model))
        {
            canvas.clear_notice();
            self.notice_models.remove(document_id);
        }
        let mut chosen = cleanup;
        let commands = ui
            .scope(|ui| {
                if !snapshot.editable && !snapshot.paused {
                    // Keep the last valid graph legible as a reference.
                    ui.visuals_mut().disabled_alpha = 1.0;
                    ui.disable();
                }
                let mut input = canvas::Input {
                    graph,
                    reference_graph: reference,
                    later: later_of(&model, cleanup),
                    project: &project,
                    parameters: &parameters,
                    selection: &selection,
                    steps: steps.as_deref(),
                    ends: ends.as_deref(),
                    progress: canvas::Progress {
                        on,
                        running: running.as_deref(),
                        previous: previous.as_deref(),
                    },
                    can_paste,
                    context: &context,
                    library_in_view,
                    header_leading: None,
                    library: library.as_ref(),
                    read_only: !snapshot.editable,
                    component: component_id(&model),
                    inputs,
                    selected_input: input_selected.as_deref(),
                    adding_input: adding_input.as_deref(),
                    settings,
                    arrangement: match mode {
                        Mode::Split => "split",
                        Mode::Code | Mode::Diagram => "diagram",
                    },
                    scenario: Some(scenario_of(&model)),
                    shared: &shared,
                    moves: true,
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
        // What Back restores beside the board lasts as long as Back.
        if !self
            .canvases
            .get(document_id)
            .is_some_and(canvas::Canvas::has_back)
        {
            self.jump_views.remove(document_id);
        }
    }

    /// The values of `model` worth a variable, from the cache while the
    /// model is the same one (LAY-25).
    fn shared_values_of(
        &mut self,
        model: &Arc<Document>,
    ) -> Arc<Vec<apiforge_core::edit::SharedValue>> {
        if let Some((cached, shared)) = &self.shared_values
            && Arc::ptr_eq(cached, model)
        {
            return shared.clone();
        }
        let project = self.context_project();
        let (main, cleanup) = scenario_of(model);
        let shared = Arc::new(apiforge_core::edit::shared_values(main, cleanup, &project));
        self.shared_values = Some((model.clone(), shared.clone()));
        shared
    }

    /// Saves `values` in variables as one model edit (LAY-25), on `edited`,
    /// the board's graph with the connection that brought the proposal
    /// made; the variables added on this board are moved clear of its
    /// cards, and the variables are selected: every one, or with
    /// `only_new` the new ones.
    #[allow(clippy::too_many_arguments)]
    fn save_values(
        &mut self,
        document_id: &str,
        model: &Document,
        cleanup: bool,
        board: &Board<'_>,
        edited: Graph,
        values: &[apiforge_core::edit::ValueRef],
        only_new: bool,
    ) {
        let (main, other) = match model {
            Document::Scenario(scenario) if cleanup => (scenario.graph.clone(), Some(edited)),
            Document::Scenario(scenario) => (edited, Some(scenario.cleanup.clone())),
            Document::Component(_) => (edited, None),
        };
        match apiforge_core::edit::save_values(&main, other.as_ref(), board.project, values) {
            Ok(mut saved) => {
                let on_board = if cleanup {
                    saved.cleanup.as_mut()
                } else {
                    Some(&mut saved.main)
                };
                if let Some(graph) = on_board {
                    *graph = board.place_under_sources(std::mem::take(graph), &saved.created);
                }
                let selection = if only_new {
                    saved.created.clone()
                } else {
                    saved.variables.clone()
                };
                self.component_action(
                    document_id,
                    publication::Action::Model {
                        next: Box::new(with_graphs(model, saved.main, saved.cleanup)),
                        selection: Some(selection),
                        check: None,
                        moved: Vec::new(),
                        placed: false,
                    },
                );
            }
            Err(error) => {
                if let Some(canvas) = self.canvases.get_mut(document_id) {
                    canvas.set_error(error.to_string());
                }
            }
        }
    }

    /// The steps a Cleanup card may read for `model`, from the cache while
    /// the model is the same one.
    fn cleanup_reference(&mut self, model: &Arc<Document>) -> Arc<Graph> {
        if let Some((cached, graph)) = &self.cleanup_reference
            && Arc::ptr_eq(cached, model)
        {
            return graph.clone();
        }
        let graph = Arc::new(reference_of(model, true).into_owned());
        self.cleanup_reference = Some((model.clone(), graph.clone()));
        graph
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
                    | Command::SelectInput(_)
                    | Command::ComponentSettings
                    // Opening a called component is navigation, which stays
                    // free while observing (MCP-08a).
                    | Command::EditComponent(_)
                    | Command::Copy
                    | Command::SetWireStyle(_)
                    | Command::SetBridges(_)
                    | Command::InspectInput { .. }
                    // Folding checks and opening their list change no
                    // document (LAY-11).
                    | Command::SetChecksCollapsed(_)
                    | Command::ShowChecks(_)
                    | Command::ToggleChecksHere(_)
                    // Neither does the Wires setting (LAY-23).
                    | Command::SetWires(_)
                    // A jump and Back are navigation (LAY-15).
                    | Command::Jumped
                    | Command::Back(_)
            )
        {
            return;
        }
        let project = self.context_project();
        let graph = graph_of(model, cleanup);
        let reference = reference_of(model, cleanup);
        let parameters = parameters_of(model);
        let board = Board {
            graph,
            reference: &reference,
            project: &project,
            parameters: &parameters,
            sibling: sibling_of(model, cleanup),
            density: self.density(),
            satellites: true,
            lines: lines_of(&self.preferences),
        };
        let selection = self
            .selections
            .get(document_id)
            .cloned()
            .unwrap_or_default();
        let result: Option<Result<Edit, String>> = match command {
            Command::Select(ids) => {
                if !ids.is_empty() {
                    // Selecting a block leaves the *New input* form and the
                    // component's settings.
                    self.new_inputs.remove(document_id);
                    self.component_settings.remove(document_id);
                }
                self.selections.insert(document_id.to_owned(), ids);
                self.selected_inputs.remove(document_id);
                None
            }
            Command::AddInput => {
                if matches!(model, Document::Component(_)) {
                    self.open_new_input(document_id);
                }
                None
            }
            Command::ComponentSettings => {
                // The button opens the settings, or closes them when open.
                if matches!(model, Document::Component(_))
                    && !self.component_settings.remove(document_id)
                {
                    self.component_settings.insert(document_id.to_owned());
                    self.selections.remove(document_id);
                    self.selected_inputs.remove(document_id);
                    self.new_inputs.remove(document_id);
                }
                None
            }
            Command::SelectInput(name) => {
                match name {
                    Some(name) => self.open_input(document_id, name),
                    None => {
                        self.selected_inputs.remove(document_id);
                    }
                }
                None
            }
            Command::Move(positions) => Some(Ok(board.moved(positions).into())),
            Command::TidyUp(selection) => {
                if let Some(canvas) = self.canvases.get_mut(document_id) {
                    canvas.fit_after_tidy_up();
                }
                board.tidy_up(&selection).map(|graph| Ok(graph.into()))
            }
            Command::SetWireStyle(style) => {
                self.preferences.wire_style = style;
                let preferences = self.preferences.clone();
                self.storage_io(
                    "Saving preferences…",
                    move || preferences.save(crate::preferences::Preference::WireStyle),
                    |_, _| {},
                );
                for canvas in self.canvases.values_mut() {
                    canvas.wire_style = style;
                }
                None
            }
            Command::SetBridges(bridges) => {
                self.preferences.bridges = bridges;
                let preferences = self.preferences.clone();
                self.storage_io(
                    "Saving preferences…",
                    move || preferences.save(crate::preferences::Preference::Bridges),
                    |_, _| {},
                );
                for canvas in self.canvases.values_mut() {
                    canvas.bridges = bridges;
                }
                None
            }
            Command::SetCompactCards(compact) => {
                let before = self.density();
                let density = if compact {
                    canvas::CardDensity::Compact
                } else {
                    canvas::CardDensity::Full
                };
                self.switch_density(density);
                // The board repacks at the new sizes, as one edit (Q14 = A),
                // which Undo takes back together with the density.
                let current = self.shared_model(document_id);
                let repacked = Board { density, ..board }.tidy_up(&[]);
                if let Some(result) = repacked.map(|graph| Ok(graph.into())) {
                    self.apply_edit(document_id, model, cleanup, result);
                }
                self.density_switch = self
                    .shared_model(document_id)
                    .filter(|after| current.is_none_or(|current| !Arc::ptr_eq(&current, after)))
                    .map(|after| DensitySwitch {
                        document: document_id.to_owned(),
                        before,
                        after: density,
                        model: after,
                    });
                None
            }
            Command::SetChecksCollapsed(collapsed) => {
                self.collapse_checks(collapsed);
                None
            }
            Command::SetWires(mode) => {
                self.switch_wires(mode);
                None
            }
            Command::ShowChecks(node) => {
                self.show_checks(document_id, node);
                None
            }
            Command::ToggleChecksHere(node) => {
                if let Some(step) = graph.nodes.iter().find(|step| step.id == node) {
                    let chosen = self.checks_chosen(document_id, &step.handle);
                    self.choose_checks_here(document_id, &step.handle, !chosen);
                }
                None
            }
            // The first jump on the board: what Back restores beside it.
            Command::Jumped => {
                let view = self.inspector.view(&self.ui_context);
                self.jump_views.insert(document_id.to_owned(), view);
                None
            }
            // Back: the selection and the Inspector as they were (LAY-15).
            Command::Back(ids) => {
                let view = self.jump_views.remove(document_id);
                if !ids.is_empty() {
                    self.new_inputs.remove(document_id);
                    self.component_settings.remove(document_id);
                }
                if let (Some(view), [_]) = (view, ids.as_slice()) {
                    self.inspector.restore(&self.ui_context, view);
                }
                self.selections.insert(document_id.to_owned(), ids);
                self.selected_inputs.remove(document_id);
                None
            }
            Command::InspectInput { node, input, path } => {
                self.new_inputs.remove(document_id);
                self.component_settings.remove(document_id);
                self.selections.insert(document_id.to_owned(), vec![node]);
                self.selected_inputs.remove(document_id);
                match path {
                    Some(path) => self.inspector.focus_field(input, path),
                    None => self.inspector.focus_input(input),
                }
                None
            }
            Command::AddNode {
                block,
                component,
                position,
                source,
            } => Some(
                board
                    .add_node(block, component, position, source)
                    .map(|edit| board.made_room(edit)),
            ),
            // A step dropped on a line of a chain lands there (DIAG-06c).
            Command::AddOnLine {
                block,
                component,
                position,
                after: (source, port),
            } => Some(
                board
                    .add_on_line(block, component, position, &source, &port)
                    .map(|edit| board.made_room(edit)),
            ),
            Command::MoveOutOfBranch(id) => {
                let placed = board.move_out_of_branch(&id);
                self.apply_move(document_id, model, cleanup, placed);
                None
            }
            // One step earlier or later in its braces (DIAG-06b).
            Command::MoveStep { id, later } => {
                let placed = board
                    .move_by_one(&id, later)
                    .map(|(graph, moved)| (board.make_room(graph), moved));
                self.apply_move(document_id, model, cleanup, placed);
                None
            }
            // *Undo* on the notice after a delete (DIAG-06b): only while the
            // document is still at the delete, never an edit after it.
            Command::Undo => {
                let at_delete = self.notice_models.remove(document_id);
                if at_delete
                    .zip(self.shared_model(document_id))
                    .is_some_and(|(at_delete, now)| Arc::ptr_eq(&at_delete, &now))
                {
                    self.step_model_history(document_id, true);
                } else if let Some(canvas) = self.canvases.get_mut(document_id) {
                    canvas.clear_notice();
                }
                None
            }
            // After the selection, in its braces (DIAG-06a).
            Command::AddStep {
                block,
                component,
                position,
            } => Some(
                board
                    .add_step(block, component, position, &selection)
                    .map(|edit| board.made_room(edit)),
            ),
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
            // One preview drives the refusal, the question and the delete
            // (DIAG-06e).
            // An ordering line is the order of the written steps: it is not
            // deleted on its own (DIAG-06a).
            Command::Delete { wires, .. }
                if wires.iter().any(|wire| {
                    wire.starts_with(crate::canvas::AFTER_IF)
                        || graph.edges.iter().any(|edge| &edge.id == wire)
                }) =>
            {
                Some(Err(apiforge_core::edit::EditError::OrderLine.to_string()))
            }
            Command::Delete { ids, wires } => {
                let needs_start = matches!(model, Document::Scenario(_)) && !cleanup;
                let deletion = board.deletion(&ids, needs_start);
                let confirmed = self.confirmed_delete.take();
                if let Some(refusal) = deletion.refusal() {
                    Some(Err(refusal))
                } else if let Some(question) = deletion.question().filter(|question| {
                    // Confirmed only as asked: the same selection, question
                    // and statements; anything else asks again.
                    confirmed.as_ref()
                        != Some(&ConfirmedDelete {
                            ids: ids.clone(),
                            question: question.clone(),
                            gone: deletion.gone.clone(),
                        })
                }) {
                    self.prompt = Some(Prompt::DeleteStatements {
                        document_id: document_id.to_owned(),
                        cleanup,
                        ids,
                        wires,
                        question,
                        gone: deletion.gone.clone(),
                    });
                    None
                } else {
                    let kept = selection
                        .into_iter()
                        .filter(|id| !ids.contains(id) && !deletion.gone.contains(id))
                        .collect();
                    // What now follows what, said once the delete is
                    // applied (DIAG-06b).
                    let joined = joined_after_delete(graph, &ids, &deletion.gone);
                    let current = self.shared_model(document_id);
                    self.apply_edit(
                        document_id,
                        model,
                        cleanup,
                        Ok(Edit {
                            graph: board.delete_checked(&deletion, &ids, &wires),
                            selection: Some(kept),
                        }),
                    );
                    let applied = current
                        .as_ref()
                        .zip(self.shared_model(document_id).as_ref())
                        .is_some_and(|(current, now)| !Arc::ptr_eq(current, now));
                    if applied
                        && let Some(text) = joined
                        && let Some(now) = self.shared_model(document_id)
                        && let Some(canvas) = self.canvases.get_mut(document_id)
                    {
                        canvas.set_notice(text);
                        self.notice_models.insert(document_id.to_owned(), now);
                    }
                    None
                }
            }
            Command::AddProxy { source, position } => Some(board.add_proxy(source, position)),
            // A wire of the order from a step dropped on a statement of the
            // board moves it (DIAG-06a): into other braces too, and a check
            // only from *Yes* or *No* (DIAG-06d).
            Command::Connect {
                source,
                source_port,
                target,
                target_input,
                ..
            } if target_input == ORDER_PORT
                && [ORDER_PORT, TRUE_PORT, FALSE_PORT].contains(&source_port.as_str())
                && graph
                    .nodes
                    .iter()
                    .any(|node| node.id == source && apiforge_core::chain::is_step(node.kind))
                && graph.nodes.iter().any(|node| node.id == target) =>
            {
                let placed = board
                    .place(&source, &source_port, &target)
                    .map(|(graph, moved)| (board.make_room(graph), moved));
                self.apply_move(document_id, model, cleanup, placed);
                None
            }
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
            // The clipboard belongs to the window, not to a board: what is
            // copied pastes into any graph of any document (UX-34), an `if`
            // with its braces, after the selected step (DIAG-06f).
            Command::Copy => {
                if let Some(fragment) = board.copy_statements(&selection) {
                    self.clipboard = Some(fragment);
                }
                None
            }
            Command::Paste => {
                let component = matches!(model, Document::Component(_));
                let pasted = self
                    .clipboard
                    .as_ref()
                    .map(|fragment| board.paste_statements(fragment, &selection, component));
                self.apply_paste(document_id, model, cleanup, pasted);
                None
            }
            Command::Duplicate => {
                let component = matches!(model, Document::Component(_));
                let pasted = selection
                    .iter()
                    .any(|id| graph.nodes.iter().any(|node| &node.id == id))
                    .then(|| board.duplicate_statements(&selection, component));
                self.apply_paste(document_id, model, cleanup, pasted);
                None
            }
            Command::EditComponent(node_id) => {
                self.open_component(graph, &node_id);
                None
            }
            Command::SaveSelectionAsComponent => {
                self.open_save_component(document_id, model, cleanup, &selection);
                None
            }
            Command::SaveValues { values, first } => {
                let edited = match first.map(|first| *first) {
                    Some(Command::Connect {
                        source,
                        source_port,
                        target,
                        target_input,
                        target_path,
                    }) => {
                        board.connect(&source, &source_port, &target, &target_input, &target_path)
                    }
                    Some(Command::Bind {
                        node,
                        input,
                        path,
                        binding,
                    }) => board
                        .set_input(&node, &input, &path, binding)
                        .and_then(|graph| {
                            graph.ok_or_else(|| {
                                apiforge_core::edit::EditError::NodeNotFound.to_string()
                            })
                        }),
                    _ => Ok(graph.clone()),
                };
                match edited {
                    Ok(edited) => self.save_values(
                        document_id,
                        model,
                        cleanup,
                        &board,
                        edited,
                        &values,
                        false,
                    ),
                    Err(message) => self.apply_edit(document_id, model, cleanup, Err(message)),
                }
                None
            }
            Command::SaveSharedValues => {
                let (main, other) = scenario_of(model);
                let values: Vec<_> = apiforge_core::edit::shared_values(main, other, &project)
                    .into_iter()
                    .map(|value| value.value)
                    .collect();
                if !values.is_empty() {
                    self.save_values(
                        document_id,
                        model,
                        cleanup,
                        &board,
                        graph.clone(),
                        &values,
                        true,
                    );
                }
                None
            }
        };
        if let Some(result) = result {
            self.apply_edit(document_id, model, cleanup, result);
        }
    }

    /// The one question a delete asks: an `if` with statements in its braces
    /// (DIAG-06e). *Delete* runs the delete again on the document as it is
    /// then, so a refusal that has appeared meanwhile still refuses, and a
    /// delete that would now take or ask anything else asks again.
    pub(super) fn delete_statements_prompt(
        &mut self,
        context: &egui::Context,
        document_id: String,
        cleanup: bool,
        wires: Vec<String>,
        asked: ConfirmedDelete,
    ) -> Option<Prompt> {
        let (mut confirm, mut cancel) = (false, false);
        let response =
            dialogs::centered_modal(egui::Id::new("board-delete"), context).show(context, |ui| {
                ui.set_width(420.0);
                ui.heading(&asked.question);
                ui.add_space(theme::SPACE_MD);
                ui.horizontal(|ui| {
                    confirm = ui.add(widgets::primary_button("Delete")).clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if response.should_close() || cancel {
            return None;
        }
        if confirm {
            let snapshot = self.snapshot(&document_id)?;
            let model = snapshot.model?;
            let ids = asked.ids.clone();
            self.confirmed_delete = Some(asked);
            self.board_command(
                &document_id,
                &model,
                cleanup,
                snapshot.editable,
                Command::Delete { ids, wires },
            );
            self.confirmed_delete = None;
            return self.prompt.take();
        }
        let ConfirmedDelete {
            ids,
            question,
            gone,
        } = asked;
        Some(Prompt::DeleteStatements {
            document_id,
            cleanup,
            ids,
            wires,
            question,
            gone,
        })
    }

    /// Applies a paste or a duplicate as one model edit whose chain it made
    /// (DIAG-06f), so one undo removes it, and brings the copies into view;
    /// a refused one shows its sentence in the board's banner.
    fn apply_paste(
        &mut self,
        document_id: &str,
        model: &Document,
        cleanup: bool,
        pasted: Option<Result<Edit, String>>,
    ) {
        let Some(pasted) = pasted else {
            return;
        };
        let edit = match pasted {
            Ok(edit) => edit,
            Err(message) => {
                self.apply_edit(document_id, model, cleanup, Err(message));
                return;
            }
        };
        // Shown once the edit is applied, which may wait for the person to
        // accept losing unattached comments.
        self.pasted = Some(PastedOutcome {
            document_id: document_id.to_owned(),
            ids: edit.selection.clone().unwrap_or_default(),
        });
        self.component_action(
            document_id,
            publication::Action::Model {
                next: Box::new(with_graph(model, cleanup, edit.graph)),
                selection: edit.selection,
                check: None,
                moved: Vec::new(),
                placed: true,
            },
        );
    }

    /// After an applied model edit that selects `selection`: when it is the
    /// last paste, brings the copies into view, as copies from another graph
    /// keep their place.
    pub(super) fn pasted_applied(&mut self, document_id: &str, selection: &[String]) {
        let Some(pasted) = self
            .pasted
            .take_if(|pasted| pasted.document_id == document_id && pasted.ids == selection)
        else {
            return;
        };
        if let Some(canvas) = self.canvases.get_mut(document_id) {
            canvas.show_pasted(pasted.ids);
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
                    check: None,
                    moved: Vec::new(),
                    placed: false,
                },
            ),
            Err(message) => {
                if let Some(canvas) = self.canvases.get_mut(document_id) {
                    canvas.set_error(message);
                }
            }
        }
    }

    /// Applies a move on the board as one model edit whose moved blocks the
    /// text moves line by line (DIAG-06a), or shows its refusal in the
    /// board's banner.
    fn apply_move(
        &mut self,
        document_id: &str,
        model: &Document,
        cleanup: bool,
        placed: Result<(Graph, Vec<String>), String>,
    ) {
        match placed {
            Ok((graph, moved)) => self.component_action(
                document_id,
                publication::Action::Model {
                    next: Box::new(with_graph(model, cleanup, graph)),
                    selection: None,
                    check: None,
                    moved,
                    placed: false,
                },
            ),
            Err(message) => {
                if let Some(canvas) = self.canvases.get_mut(document_id) {
                    canvas.set_error(message);
                }
            }
        }
    }

    /// The board of `document_id` as a whole *Tidy up* would lay it out
    /// now, without applying it (DIAG-06c).
    #[cfg(test)]
    pub(crate) fn debug_tidied(&self, document_id: &str) -> Option<Graph> {
        let snapshot = self.snapshot(document_id)?;
        let model = snapshot.model?;
        let cleanup = self.cleanup(document_id);
        let project = self.context_project();
        let reference = reference_of(&model, cleanup);
        let parameters = parameters_of(&model);
        let board = Board {
            graph: graph_of(&model, cleanup),
            reference: &reference,
            project: &project,
            parameters: &parameters,
            sibling: sibling_of(&model, cleanup),
            density: self.density(),
            satellites: true,
            lines: lines_of(&self.preferences),
        };
        board.tidy_up(&[])
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
                self.navigated_to(&document_id);
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
        // While the board shows Main, the Cleanup checks that read a Main step
        // are listed under that step (LAY-20).
        let in_cleanup = match &*model {
            Document::Scenario(scenario) if !cleanup => Some(&scenario.cleanup),
            _ => None,
        };
        let Some(node) = self
            .selections
            .get(document_id)
            .filter(|selection| selection.len() == 1)
            .and_then(|selection| graph.nodes.iter().find(|node| node.id == selection[0]))
            .cloned()
        else {
            let Document::Component(component) = &*model else {
                return;
            };
            // A component's input being added or selected (UX-32).
            if self.new_inputs.contains_key(document_id) {
                self.new_input_panel(ui, document_id, component, snapshot.editable);
                return;
            }
            let selection = self
                .selections
                .get(document_id)
                .cloned()
                .unwrap_or_default();
            if let Some(port) =
                selected_input(&model, &selection, self.selected_inputs.get(document_id))
            {
                let port = port.clone();
                self.input_panel(ui, document_id, component, &port, snapshot.editable);
            } else if self.component_settings.contains(document_id) {
                self.component_settings_panel(ui, document_id, component, snapshot.editable);
            }
            return;
        };
        let project = self.context_project();
        let project_id = self
            .session
            .as_ref()
            .map(|session| session.manifest().id.clone())
            .unwrap_or_default();
        // A Cleanup block reads Main and the Cleanup steps, as its card does
        // (UX-16, #284).
        let cleanup_reference = cleanup.then(|| self.cleanup_reference(&model));
        let reference = cleanup_reference
            .as_deref()
            .unwrap_or_else(|| graph_of(&model, false));
        let parameters = parameters_of(&model);
        let run = self.fresh_run(document_id);
        let step = run
            .as_ref()
            .and_then(|run| run.steps.iter().find(|step| step.node_id == node.id));
        let expand_here = self
            .canvases
            .get(document_id)
            .and_then(|canvas| canvas.checks_here(graph, in_cleanup, &node.id));
        // Adopt fields the session changed before the inspector reads them.
        self.sync_field_drafts(document_id, &project_id);
        let checks = self.check_index(&model);
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
                            document: egui::Id::new(("document", &project_id, document_id)),
                            node: &node,
                            graph: reference,
                            board: graph,
                            cleanup: in_cleanup,
                            project: &project,
                            parameters: &parameters,
                            run: run.as_ref(),
                            step,
                            expand_here,
                            read_only: (!snapshot.editable).then(|| read_only_reason(&snapshot)),
                            checks: Some(&checks),
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
                // Selecting is navigation, which a read-only board allows too.
                inspector::Action::Select(step) => self.select_step(document_id, step),
                // A view kept on this computer, which changes no document
                // (LAY-30).
                inspector::Action::ExpandChecksHere(chosen) => {
                    self.choose_checks_here(document_id, &node.handle, chosen);
                }
                // A check chosen in a field's menu is selected, on the board
                // that holds it (DIAG-12).
                inspector::Action::SelectCheck(check) => {
                    let cleanup = cleanup_holds(&model, &check);
                    self.select_on_board(document_id, &model, check, cleanup);
                }
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
                inspector::Action::ReadDirectly => {
                    let (main, other) = scenario_of(&model);
                    match apiforge_core::edit::read_directly(main, other, &node.id) {
                        Ok(read) => self.component_action(
                            document_id,
                            publication::Action::Model {
                                next: Box::new(with_graphs(&model, read.main, read.cleanup)),
                                selection: Some(Vec::new()),
                                check: None,
                                moved: Vec::new(),
                                placed: false,
                            },
                        ),
                        Err(error) => {
                            if let Some(canvas) = self.canvases.get_mut(document_id) {
                                canvas.set_error(error.to_string());
                            }
                        }
                    }
                }
                // The run's data or, before any run, the sample (DIAG-09).
                inspector::Action::AssertField { port, path, check } => self.field_action(
                    document_id,
                    crate::report::Action::AssertField {
                        node_id: node.id.clone(),
                        port,
                        path,
                        check,
                    },
                ),
                inspector::Action::AssignField { port, path } => self.field_action(
                    document_id,
                    crate::report::Action::AssignField {
                        node_id: node.id.clone(),
                        port,
                        path,
                    },
                ),
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
/// Why the board takes no edit now: said above the board, which leaves out
/// a pause and a save (UX-25), and on the hover of an Inspector action it
/// disables (DIAG-09).
fn read_only_reason(snapshot: &Snapshot) -> &'static str {
    if snapshot.paused {
        "Editing is paused here while another process holds the project."
    } else if snapshot.busy {
        "The project's files are being written. Try again in a moment."
    } else if snapshot.held {
        "Unsaved work from an earlier session needs a decision in the code first."
    } else if snapshot.analysis == AnalysisState::Pending {
        "Checking the latest code…"
    } else if snapshot.model_state == ModelState::LastValid {
        "The code has errors. This is the last valid version; it is read-only until \
         the code is fixed."
    } else {
        "This diagram is read-only."
    }
}

const NAME_MIN_WIDTH: f32 = 80.0;

/// The document's part of the board's header row (KTLO-12): in Diagram the
/// file name and kind, then Main / Cleanup for a scenario; in Split only
/// Main / Cleanup, since the tab strip beside the code names the file.
struct Header<'a> {
    name: Option<(&'a str, &'static str)>,
    tabs: bool,
    /// WS-05 mock: the document's title, shown before the file name.
    title: Option<&'a str>,
}

/// What of the header fits, and its width.
struct HeaderFit {
    /// WS-05 mock: the room of the file name after the title; 0 leaves it out.
    file_width: f32,
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

    /// The width the header takes when nothing of it is given up.
    fn full_width(&self, ui: &egui::Ui) -> f32 {
        self.fit(
            ui,
            canvas::HeaderRoom {
                alone: f32::INFINITY,
                with_summary: f32::INFINITY,
            },
        )
        .width
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
                file_width: 0.0,
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
        if let Some(title) = self.title {
            let title_full = Self::text_width(
                ui,
                title,
                egui::FontId::new(theme::FONT_BODY, theme::bold_family()),
            );
            let file_full =
                gap + Self::text_width(ui, name, egui::FontId::proportional(theme::FONT_LABEL));
            let alone = room.alone - fixed - 1.0;
            let room = room.with_summary - fixed - 1.0;
            let (name_width, file_width, kind) = if title_full + file_full + kind_width <= room {
                (title_full, file_full, true)
            } else if title_full + file_full <= room {
                (title_full, file_full, false)
            } else if title_full <= room {
                (title_full, 0.0, false)
            } else if room >= NAME_MIN_WIDTH {
                (room, 0.0, false)
            } else {
                (
                    alone.clamp(NAME_MIN_WIDTH.min(title_full), title_full),
                    0.0,
                    false,
                )
            };
            return HeaderFit {
                file_width,
                name_width,
                kind,
                width: name_width + file_width + if kind { kind_width } else { 0.0 } + fixed,
            };
        }
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
            file_width: 0.0,
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
                        let file = name;
                        let name = self.title.unwrap_or(name);
                        if self.title.is_some() && crate::mock::EDIT.load(std::sync::atomic::Ordering::Relaxed) {
                            let mut text = name.to_owned();
                            let field = ui.add(
                                egui::TextEdit::singleline(&mut text)
                                    .desired_width(fit.name_width + 12.0)
                                    .font(egui::FontId::new(theme::FONT_BODY, theme::bold_family())),
                            );
                            field.request_focus();
                        } else {
                        ui.scope(|ui| {
                            ui.set_max_width(fit.name_width);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(name)
                                        .size(theme::FONT_BODY)
                                        .family(theme::bold_family()),
                                )
                                .truncate(),
                            )
                            .on_hover_text(format!("{name}\n{file}"));
                        });
                        }
                        if self.title.is_some() && fit.file_width > 0.0 {
                            ui.scope(|ui| {
                                ui.set_max_width(fit.file_width - ui.spacing().item_spacing.x);
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(file)
                                            .size(theme::FONT_LABEL)
                                            .color(theme::MUTED),
                                    )
                                    .truncate(),
                                );
                            });
                        }
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

/// The board's notice after a delete (DIAG-06b): when the statements `ids`
/// hold one step and it stood between two steps of its braces, which now
/// follow one another, the sentence that says so. `None` for the first or
/// the last step of its braces, for a check, value, variable or export, and
/// for several steps at once. Start is no step here, as for a move by one.
fn joined_after_delete(graph: &Graph, ids: &[String], gone: &[String]) -> Option<String> {
    use apiforge_core::chain::braces;
    let is_step = |kind: apiforge_core::model::NodeKind| {
        apiforge_core::chain::is_step(kind) && kind != apiforge_core::model::NodeKind::Start
    };
    let mut deleted = graph
        .nodes
        .iter()
        .filter(|node| ids.contains(&node.id) && is_step(node.kind));
    let step = deleted.next()?;
    if deleted.next().is_some() {
        return None;
    }
    let written = braces(graph);
    let own = written.get(step.id.as_str()).copied();
    let siblings: Vec<&apiforge_core::format::Node> = graph
        .nodes
        .iter()
        .filter(|node| {
            is_step(node.kind)
                && written.get(node.id.as_str()).copied() == own
                && (node.id == step.id || !gone.contains(&node.id))
        })
        .collect();
    let index = siblings.iter().position(|node| node.id == step.id)?;
    let before = siblings.get(index.checked_sub(1)?)?;
    let after = siblings.get(index + 1)?;
    Some(format!(
        "`{}` deleted. `{}` now follows `{}`.",
        step.handle, after.handle, before.handle
    ))
}
