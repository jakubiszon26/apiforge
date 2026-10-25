// Include from folder/tests.rs in an isolated checkout (see run.sh).
// These are acceptance probes: failures identify unmet UI requirements.
mod ui_quality {
    use super::*;
    use egui::epaint::Shape;

    fn render(
        app: &mut FolderApp,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(960.0, 540.0),
                )),
                events,
                ..Default::default()
            },
            |ui| app.ui(ui),
        );
        out.textures_delta.clear();
        out
    }

    fn texts(shape: &Shape, clip: egui::Rect, out: &mut Vec<(String, egui::Rect, egui::Rect)>) {
        match shape {
            Shape::Vec(shapes) => {
                for s in shapes {
                    texts(s, clip, out);
                }
            }
            Shape::Text(t) => out.push((
                t.galley.job.text.clone(),
                t.galley.rect.translate(t.pos.to_vec2()),
                clip,
            )),
            _ => {}
        }
    }

    fn labels(out: &egui::FullOutput) -> Vec<(String, egui::Rect, egui::Rect)> {
        let mut labels = Vec::new();
        for s in &out.shapes {
            texts(&s.shape, s.clip_rect, &mut labels);
        }
        labels
    }

    fn pointer(app: &mut FolderApp, ctx: &egui::Context, p: egui::Pos2, pressed: bool) {
        render(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }

    fn click(app: &mut FolderApp, ctx: &egui::Context, p: egui::Pos2) {
        render(app, ctx, vec![egui::Event::PointerMoved(p)]);
        pointer(app, ctx, p, true);
        pointer(app, ctx, p, false);
        let deadline = Instant::now() + Duration::from_secs(15);
        while app.storage.is_busy() && Instant::now() < deadline {
            render(app, ctx, vec![]);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(!app.storage.is_busy(), "file opening did not complete");
    }

    fn fixture() -> (tempfile::TempDir, egui::Context, FolderApp, String) {
        let dir = tempfile::tempdir().unwrap();
        let mut s = ProjectSession::create(dir.path(), "UI QA").unwrap();
        s.create_document(
            DocumentKind::Scenario,
            "alpha.afs",
            "scenario \"Alpha\" {}\n",
        )
        .unwrap();
        let beta = s
            .create_document(DocumentKind::Scenario, "beta.afs", "scenario \"Beta\" {}\n")
            .unwrap();
        std::fs::write(
            dir.path().join(".gitignore"),
            "# UI_QUALITY_SENTINEL\n/.apiforge/local/\n",
        )
        .unwrap();
        drop(s);
        let (ctx, mut app) = open(dir.path());
        watch(&mut app, &ctx);
        render(&mut app, &ctx, vec![]);
        (dir, ctx, app, beta)
    }

    fn row_position(app: &mut FolderApp, ctx: &egui::Context, name: &str) -> egui::Pos2 {
        labels(&render(app, ctx, vec![]))
            .into_iter()
            .find(|(text, rect, clip)| {
                text == name && rect.center().x < 230.0 && clip.contains(rect.center())
            })
            .unwrap_or_else(|| panic!("No visible explorer row {name}"))
            .1
            .center()
    }

    #[test]
    fn ui_quality_one_click_opens_a_registered_file_without_refresh() {
        let (_dir, ctx, mut app, beta) = fixture();
        let p = row_position(&mut app, &ctx, "beta.afs");
        click(&mut app, &ctx, p);
        assert_eq!(
            app.workbench.active(),
            Some(beta.as_str()),
            "One click must open the selected file; selection alone is insufficient"
        );
    }

    #[test]
    fn ui_quality_control_double_click_opens_a_registered_file() {
        let (_dir, ctx, mut app, beta) = fixture();
        let p = row_position(&mut app, &ctx, "beta.afs");
        click(&mut app, &ctx, p);
        click(&mut app, &ctx, p);
        assert_eq!(app.workbench.active(), Some(beta.as_str()));
    }

    #[test]
    fn ui_quality_control_enter_opens_the_selected_file() {
        let (_dir, ctx, mut app, beta) = fixture();
        let p = row_position(&mut app, &ctx, "beta.afs");
        click(&mut app, &ctx, p);
        render(&mut app, &ctx, vec![key(Key::Enter, Modifiers::NONE)]);
        assert_eq!(app.workbench.active(), Some(beta.as_str()));
    }

    #[test]
    fn ui_quality_gitignore_opens_as_text_inside_the_application() {
        let (_dir, ctx, mut app, _) = fixture();
        let p = row_position(&mut app, &ctx, ".gitignore");
        click(&mut app, &ctx, p);
        let out = render(&mut app, &ctx, vec![]);
        assert!(
            labels(&out)
                .iter()
                .any(|(text, rect, clip)| text.contains("UI_QUALITY_SENTINEL")
                    && rect.intersects(*clip)),
            "Opening .gitignore must show its text inside the application"
        );
    }

    #[test]
    fn ui_quality_project_root_is_a_selectable_explorer_row() {
        let (_dir, ctx, mut app, _) = fixture();
        let out = render(&mut app, &ctx, vec![]);
        let root = labels(&out).into_iter().find(|(text, rect, clip)| {
            text == "UI QA"
                && rect.center().x < 230.0
                && rect.center().y > 90.0
                && clip.contains(rect.center())
        });
        assert!(
            root.is_some(),
            "The project root needs a visible selectable row in the explorer, apart from the window heading"
        );
        click(&mut app, &ctx, root.unwrap().1.center());
        assert_eq!(app.explorer.selected.as_deref(), Some(""));
    }

    fn shapes<'a>(shape: &'a Shape, out: &mut Vec<&'a Shape>) {
        if let Shape::Vec(children) = shape {
            for child in children {
                shapes(child, out);
            }
        } else {
            out.push(shape);
        }
    }

    #[test]
    fn ui_quality_close_icon_is_inside_its_tab_surface() {
        let (_dir, ctx, mut app, _) = fixture();
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(960.0, 540.0),
                )),
                ..Default::default()
            },
            |ui| app.tab_strip(ui),
        );
        out.textures_delta.clear();
        let label = labels(&out)
            .into_iter()
            .find(|(text, _, _)| text == "alpha.afs")
            .unwrap()
            .1;
        let mut all = Vec::new();
        for shape in &out.shapes {
            shapes(&shape.shape, &mut all);
        }
        let surface = all
            .iter()
            .filter_map(|s| {
                if let Shape::Rect(r) = s {
                    Some(r)
                } else {
                    None
                }
            })
            .filter(|r| r.fill != egui::Color32::TRANSPARENT && r.rect.contains_rect(label))
            .min_by(|a, b| a.rect.area().total_cmp(&b.rect.area()))
            .expect("Tab must have a visible selected surface")
            .rect;
        let cross: Vec<_> = all
            .iter()
            .filter_map(|s| {
                if let Shape::LineSegment { points, .. } = s {
                    Some(*points)
                } else {
                    None
                }
            })
            .collect();
        assert!(cross.len() >= 2, "The tab needs a visible close icon");
        assert!(
            cross.iter().flatten().all(|p| surface.contains(*p)),
            "Close icon lies outside its tab surface: surface={surface:?}, strokes={cross:?}"
        );
    }

    #[test]
    fn ui_quality_active_tab_stays_visible_with_many_open_files() {
        let (dir, ctx, mut app, _) = fixture();
        let mut last = String::new();
        for i in 0..16 {
            let id = app
                .session
                .as_mut()
                .unwrap()
                .create_document(
                    DocumentKind::Scenario,
                    &format!("task-{i:02}.afs"),
                    &format!("scenario \"Task {i}\" {{}}\n"),
                )
                .unwrap();
            app.workbench.open(&id);
            last = format!("task-{i:02}.afs");
        }
        let out = render(&mut app, &ctx, vec![]);
        let visible = labels(&out).iter().any(|(text, rect, clip)| {
            text == &last && rect.center().y < 100.0 && clip.contains(rect.center())
        });
        assert!(
            visible,
            "The active tab {last} is clipped/offscreen at 960x540"
        );
        drop(dir);
    }
    #[test]
    fn ui_quality_explorer_files_have_named_accessibility_nodes() {
        let (_dir, ctx, mut app, _) = fixture();
        ctx.enable_accesskit();
        let out = render(&mut app, &ctx, vec![]);
        let tree = out
            .platform_output
            .accesskit_update
            .expect("Accessibility tree must be enabled");
        assert!(
            tree.nodes
                .iter()
                .any(|(_, n)| n.label() == Some("Close alpha.afs")),
            "Control: standard close button must be labelled"
        );
        assert!(
            tree.nodes
                .iter()
                .any(|(_, n)| n.label() == Some("beta.afs")),
            "Explorer filenames are painted, but have no named accessibility node"
        );
    }

    #[test]
    fn ui_quality_tabs_disambiguate_equal_names_from_different_folders() {
        let (_dir, ctx, mut app, _) = fixture();
        let id = app
            .session
            .as_mut()
            .unwrap()
            .create_document(
                DocumentKind::Scenario,
                "nested/alpha.afs",
                "scenario \"Nested\" {}\n",
            )
            .unwrap();
        app.workbench.open(&id);
        let out = render(&mut app, &ctx, vec![]);
        assert!(
            labels(&out)
                .iter()
                .any(|(text, rect, clip)| text.contains("nested")
                    && rect.center().y < 100.0
                    && clip.contains(rect.center())),
            "Two alpha.afs tabs must visibly distinguish their folders without requiring a tooltip"
        );
    }
    #[test]
    fn ui_quality_active_file_is_revealed_in_a_long_explorer() {
        let (_dir, ctx, mut app, _) = fixture();
        let mut last_id = String::new();
        for i in 0..24 {
            last_id = app
                .session
                .as_mut()
                .unwrap()
                .create_document(
                    DocumentKind::Scenario,
                    &format!("task-{i:02}.afs"),
                    &format!("scenario \"Task {i}\" {{}}\n"),
                )
                .unwrap();
        }
        app.workbench.open(&last_id);
        watch(&mut app, &ctx);
        for _ in 0..3 {
            render(&mut app, &ctx, vec![]);
        }
        let out = render(&mut app, &ctx, vec![]);
        assert!(
            labels(&out)
                .iter()
                .any(|(text, rect, clip)| text == "task-23.afs"
                    && rect.center().x < 230.0
                    && rect.center().y > 90.0
                    && clip.contains(rect.center())),
            "Activating a file must scroll its explorer row into view even when virtualised"
        );
    }
}
