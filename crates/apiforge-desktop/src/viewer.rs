//! Read-only native document inspection while editing commands are being migrated.
//! UI geometry is a disposable adapter; it never replaces the v1 graph document.
use crate::document_io::read_document;
use crate::graph_view::{Canvas, project_graph};
use crate::{inspector, theme};
use apiforge_core::{ImportLimits, ValidatedDocument, format::Project};
use eframe::egui::{self, Color32, RichText};

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
};

type Loaded = Result<Option<ValidatedDocument>, String>;
pub struct Desktop {
    document: Option<ValidatedDocument>,
    project: usize,
    scenario: usize,
    cleanup: bool,
    graph: Canvas,
    search: String,
    report_open: bool,
    report_height: f32,
    report_handle: Option<egui::Rect>,
    api_mode: bool,
    receiver: Option<Receiver<Loaded>>,
    smoke: Option<Arc<AtomicBool>>,
    drawn_frames: u8,
    capture: Option<crate::capture::Capture>,
    loading: bool,
    error: Option<String>,
    context: egui::Context,
}
impl Desktop {
    pub fn new(
        context: egui::Context,
        path: Option<PathBuf>,
        smoke: Option<Arc<AtomicBool>>,
        screenshot: Option<PathBuf>,
    ) -> Self {
        theme::apply(&context);
        let mut app = Self {
            document: None,
            project: 0,
            scenario: 0,
            cleanup: false,
            graph: Canvas::default(),
            search: String::new(),
            report_open: true,
            report_height: 270.0,
            report_handle: None,
            api_mode: false,
            receiver: None,
            smoke,
            drawn_frames: 0,
            capture: screenshot.map(crate::capture::Capture::new),
            loading: false,
            error: None,
            context,
        };
        if let Some(path) = path {
            app.load(Some(path));
        } else {
            match apiforge_core::import_document(
                include_bytes!("../../../fixtures/compatibility/workspace-v1.json"),
                &ImportLimits::default(),
            ) {
                Ok(document) => {
                    app.document = Some(document);
                    app.refresh_graph();
                }
                Err(error) => app.error = Some(error.to_string()),
            }
        }
        app
    }
    fn load(&mut self, path: Option<PathBuf>) {
        if self.loading {
            return;
        }
        self.loading = true;
        self.error = None;
        let (sender, receiver) = mpsc::sync_channel(1);
        self.receiver = Some(receiver);
        let context = self.context.clone();
        // One in-flight import, one bounded result. No file I/O or dialogs in a paint pass.
        struct RepaintOnDrop(egui::Context);
        impl Drop for RepaintOnDrop {
            fn drop(&mut self) {
                self.0.request_repaint();
            }
        }
        let worker = std::thread::Builder::new()
            .name("project-import".into())
            .spawn(move || {
                let _repaint = RepaintOnDrop(context);
                let path = path.or_else(|| {
                    rfd::FileDialog::new()
                        .add_filter("Projekty JSON", &["json"])
                        .pick_file()
                });
                let result = path
                    .map(|path| read_document(&path, &ImportLimits::default()))
                    .transpose();
                let _ = sender.send(result);
            });
        if worker.is_err() {
            self.loading = false;
            self.receiver = None;
            self.error = Some("Nie można uruchomić odczytu pliku.".into());
        }
    }
    fn project(&self) -> Option<&Project> {
        self.document
            .as_ref()?
            .workspace()
            .projects
            .get(self.project)
    }
    fn refresh_graph(&mut self) {
        self.graph = self
            .project()
            .and_then(|project| {
                project.scenarios.get(self.scenario).map(|scenario| {
                    project_graph(
                        if self.cleanup {
                            &scenario.cleanup
                        } else {
                            &scenario.graph
                        },
                        project,
                    )
                })
            })
            .unwrap_or_default();
        let first = self.graph.nodes().next().map(|node| node.id.clone());
        self.graph.selected = first;
    }
}
impl eframe::App for Desktop {
    fn logic(&mut self, _context: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_load();
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}
impl Desktop {
    fn poll_load(&mut self) {
        let result = self
            .receiver
            .as_ref()
            .and_then(|receiver| match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => {
                    Some(Err("Odczyt pliku został przerwany.".into()))
                }
            });
        if let Some(result) = result {
            self.receiver = None;
            self.loading = false;
            match result {
                Ok(Some(document)) => {
                    self.project = document
                        .workspace()
                        .projects
                        .iter()
                        .position(|p| p.id == document.workspace().active_id)
                        .unwrap_or(0);
                    self.document = Some(document);
                    self.scenario = 0;
                    self.cleanup = false;
                    self.refresh_graph();
                }
                Ok(None) => {}
                Err(message) => self.error = Some(message),
            }
        }
    }
    fn show(&mut self, ui: &mut egui::Ui) {
        let old = (self.project, self.scenario, self.cleanup);
        egui::Panel::left("workspace-sidebar")
            .exact_size(235.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(theme::WHITE).inner_margin(15))
            .show(ui, |ui| self.sidebar(ui));
        if old != (self.project, self.scenario, self.cleanup) {
            self.refresh_graph();
        }
        egui::Panel::top("breadcrumbs")
            .exact_size(52.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::WHITE)
                    .inner_margin(egui::Margin::symmetric(25, 10)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    let name = self
                        .project()
                        .map(|project| project.name.as_str())
                        .unwrap_or("Obszar roboczy");
                    ui.label(
                        RichText::new(format!("▱  {name}    ›    Scenariusze"))
                            .size(11.0)
                            .color(theme::MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(
                                !self.loading,
                                egui::Button::new("Importuj JSON").frame(false),
                            )
                            .clicked()
                        {
                            self.load(None);
                        }
                        ui.label(RichText::new("Tryb odczytu").size(10.0).color(theme::MUTED));
                    });
                });
            });
        egui::Panel::top("scenario-heading").exact_size(118.0).frame(egui::Frame::new().fill(theme::WHITE).inner_margin(egui::Margin::symmetric(25,18))).show(ui,|ui|{
            ui.horizontal(|ui|{
                let width=(ui.available_width()-290.0).max(180.0);
                ui.allocate_ui_with_layout(egui::vec2(width,78.0),egui::Layout::top_down(egui::Align::Min),|ui|{
                    if let Some(project)=self.project(){let scenario=&project.scenarios[self.scenario];
                        ui.label(RichText::new(format!("TEST REGRESJI API     {} kroków · {} po teście",scenario.graph.nodes.len(),scenario.cleanup.nodes.len())).size(9.0).color(theme::MUTED));
                        ui.add_space(3.0);ui.label(RichText::new(&scenario.name).size(23.0).family(theme::bold_family()));
                        ui.add(egui::Label::new(RichText::new(&scenario.description).size(10.0).color(theme::MUTED)).truncate());
                    }
                });
                ui.horizontal(|ui|{
                    ui.selectable_value(&mut self.api_mode,false,"Symulacja");ui.selectable_value(&mut self.api_mode,true,"API");
                    ui.add_enabled(false,egui::Button::new(RichText::new("▷  Uruchom test").color(theme::WHITE)).fill(theme::ACCENT).min_size(egui::vec2(132.0,36.0)))
                        .on_disabled_hover_text("Silnik wykonania jest jeszcze w trakcie migracji. Nie uruchomiono żadnego requestu.");
                });
            });
            if self.loading{ui.spinner();}
            if let Some(error)=&self.error{ui.colored_label(Color32::from_rgb(180,60,45),error);}
        });
        egui::Panel::top("phase-tabs")
            .exact_size(42.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::WHITE)
                    .inner_margin(egui::Margin::symmetric(25, 6)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let old = self.cleanup;
                    ui.selectable_value(&mut self.cleanup, false, "Scenariusz");
                    let count = self
                        .project()
                        .map(|project| project.scenarios[self.scenario].cleanup.nodes.len())
                        .unwrap_or(0);
                    ui.selectable_value(&mut self.cleanup, true, format!("Po teście   {count}"));
                    if old != self.cleanup {
                        self.refresh_graph();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new("Podgląd zaimportowanego projektu")
                                .size(10.0)
                                .color(theme::MUTED),
                        );
                    });
                });
            });
        self.report(ui);
        if self.graph.selected.is_some() {
            egui::Panel::right("node-configuration")
                .default_size(310.0)
                .size_range(280.0..=380.0)
                .frame(egui::Frame::new().fill(theme::WHITE).inner_margin(18))
                .show(ui, |ui| {
                    let nodes: Vec<_> = self
                        .project()
                        .map(|project| {
                            let scenario = &project.scenarios[self.scenario];
                            (if self.cleanup {
                                &scenario.cleanup
                            } else {
                                &scenario.graph
                            })
                            .nodes
                            .iter()
                            .map(|node| (node.id.clone(), node.name.clone()))
                            .collect()
                        })
                        .unwrap_or_default();
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("selected-step")
                            .width((ui.available_width() - 36.0).max(150.0))
                            .selected_text("Wybrany klocek")
                            .show_ui(ui, |ui| {
                                for (id, name) in nodes {
                                    ui.selectable_value(&mut self.graph.selected, Some(id), name);
                                }
                            });
                        if ui
                            .button("×")
                            .on_hover_text("Zamknij konfigurację")
                            .clicked()
                        {
                            self.graph.selected = None;
                            self.graph.request_fit();
                        }
                    });
                    if let Some(project) = self.project() {
                        let scenario = &project.scenarios[self.scenario];
                        let graph = if self.cleanup {
                            &scenario.cleanup
                        } else {
                            &scenario.graph
                        };
                        if let Some(node) = graph
                            .nodes
                            .iter()
                            .find(|node| Some(&node.id) == self.graph.selected.as_ref())
                        {
                            inspector::show(ui, node, graph, project);
                        }
                    }
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.graph.show(ui));
        if let Some(capture) = &mut self.capture {
            if let Some(result) = capture.update(ui.ctx(), self.document.is_some() && !self.loading)
            {
                if let Err(message) = &result {
                    self.error = Some(message.clone());
                }
                if let Some(completed) = &self.smoke {
                    completed.store(result.is_ok(), Ordering::Release);
                }
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        } else if let Some(completed) = &self.smoke {
            if self.document.is_some() {
                self.drawn_frames += 1;
            }
            if self.drawn_frames >= 3 || self.error.is_some() {
                completed.store(self.error.is_none(), Ordering::Release);
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ui.ctx().request_repaint();
            }
        }
    }
    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(32.0, 32.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 7.0, theme::ACCENT);
            theme::kind_icon(
                ui.painter(),
                rect,
                apiforge_core::model::NodeKind::If,
                theme::WHITE,
            );
            ui.label(
                RichText::new("apiforge")
                    .size(23.0)
                    .family(theme::bold_family()),
            );
        });
        ui.add_space(19.0);
        egui::Frame::new()
            .fill(Color32::from_rgb(253, 252, 249))
            .stroke(egui::Stroke::new(1.0, theme::BORDER))
            .corner_radius(8)
            .inner_margin(10)
            .show(ui, |ui| {
                ui.label(
                    RichText::new("OBSZAR ROBOCZY")
                        .size(9.0)
                        .color(theme::MUTED),
                );
                let names: Vec<_> = self
                    .document
                    .as_ref()
                    .map(|doc| {
                        doc.workspace()
                            .projects
                            .iter()
                            .map(|project| project.name.clone())
                            .collect()
                    })
                    .unwrap_or_default();
                let selected = names
                    .get(self.project)
                    .cloned()
                    .unwrap_or_else(|| "Otwórz projekt".into());
                egui::ComboBox::from_id_salt("project-choice")
                    .selected_text(selected)
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for (index, name) in names.iter().enumerate() {
                            if ui.selectable_label(self.project == index, name).clicked() {
                                self.project = index;
                                self.scenario = 0;
                            }
                        }
                    });
            });
        ui.add_space(17.0);
        eyebrow(ui, "SCENARIUSZE");
        ui.add_space(3.0);
        let scenarios: Vec<_> = self
            .project()
            .map(|project| {
                project
                    .scenarios
                    .iter()
                    .map(|scenario| scenario.name.clone())
                    .collect()
            })
            .unwrap_or_default();
        for (index, name) in scenarios.iter().enumerate() {
            if navigation(ui, name, self.scenario == index) {
                self.scenario = index;
            }
        }
        ui.add_space(18.0);
        eyebrow(ui, "BIBLIOTEKA KLOCKÓW");
        ui.add_space(4.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text("⌕  Szukaj klocka…")
                .desired_width(f32::INFINITY),
        );
        ui.add_space(8.0);
        let height = (ui.available_height() - 110.0).max(40.0);
        egui::ScrollArea::vertical()
            .id_salt("block-library")
            .max_height(height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                eyebrow(ui, "OPERACJE PODSTAWOWE");
                for (kind, description) in [
                    (
                        apiforge_core::model::NodeKind::Trigger,
                        "Uruchom po kliknięciu Uruchom",
                    ),
                    (
                        apiforge_core::model::NodeKind::If,
                        "Wybierz gałąź: tak albo nie",
                    ),
                    (
                        apiforge_core::model::NodeKind::Request,
                        "Dowolny endpoint REST API",
                    ),
                    (
                        apiforge_core::model::NodeKind::Select,
                        "Wybierz pole z odpowiedzi",
                    ),
                    (
                        apiforge_core::model::NodeKind::Build,
                        "Przygotuj obiekt lub listę",
                    ),
                    (
                        apiforge_core::model::NodeKind::Formula,
                        "Matematyka, logika, tekst i dane",
                    ),
                    (
                        apiforge_core::model::NodeKind::Assert,
                        "Sprawdź otrzymany wynik",
                    ),
                    (
                        apiforge_core::model::NodeKind::Condition,
                        "Sprawdź warunki scenariusza",
                    ),
                ] {
                    if self.search.is_empty()
                        || kind
                            .label()
                            .to_lowercase()
                            .contains(&self.search.to_lowercase())
                    {
                        library_item(ui, kind, kind.label(), description);
                    }
                }
                ui.add_space(8.0);
                eyebrow(ui, "ZMIENNE");
                ui.label(
                    RichText::new("Odwołania do argumentów i środowiska")
                        .size(10.0)
                        .color(theme::MUTED),
                );
                if let Some(project) = self.project() {
                    for name in std::iter::once("baseUrl")
                        .chain(project.environment.variables.keys().map(String::as_str))
                        .chain(project.environment.secret_names.iter().map(String::as_str))
                    {
                        egui::Frame::new()
                            .fill(Color32::from_rgb(255, 244, 222))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(235, 195, 137)))
                            .corner_radius(15)
                            .inner_margin(8)
                            .show(ui, |ui| {
                                ui.set_min_width(160.0);
                                ui.label(
                                    RichText::new(name)
                                        .size(11.0)
                                        .color(Color32::from_rgb(153, 104, 45)),
                                );
                            });
                    }
                    ui.add_space(12.0);
                    eyebrow(ui, "KOMPONENTY PROJEKTU");
                    for component in &project.components {
                        if self.search.is_empty()
                            || component
                                .name
                                .to_lowercase()
                                .contains(&self.search.to_lowercase())
                        {
                            library_item(
                                ui,
                                apiforge_core::model::NodeKind::Component,
                                &component.name,
                                &format!("Komponent · rewizja {}", component.revision),
                            );
                        }
                    }
                }
            });
        ui.add_space(12.0);
        egui::Frame::new()
            .stroke(egui::Stroke::new(1.0, theme::BORDER))
            .corner_radius(7)
            .inner_margin(10)
            .show(ui, |ui| {
                ui.set_min_width(180.0);
                eyebrow(ui, "ŚRODOWISKO");
                let address = self
                    .project()
                    .map(|project| {
                        project
                            .environment
                            .base_url
                            .split("://")
                            .last()
                            .unwrap_or("Lokalne")
                            .split('/')
                            .next()
                            .unwrap_or("Lokalne")
                    })
                    .unwrap_or("Nie wybrano");
                ui.label(
                    RichText::new(format!("●  {address}"))
                        .size(10.0)
                        .color(theme::MUTED),
                );
            });
        ui.add_space(10.0);
        ui.label(
            RichText::new("●  Importowany dokument · odczyt")
                .size(9.0)
                .color(theme::MUTED),
        );
    }
    fn report(&mut self, ui: &mut egui::Ui) {
        self.report_handle = None;
        let max_height = (ui.ctx().content_rect().height() - 440.0).max(160.0);
        self.report_height = self.report_height.clamp(160.0, max_height);
        let height = if self.report_open {
            self.report_height
        } else {
            34.0
        };
        egui::Panel::bottom("run-report")
            .exact_size(height)
            .resizable(false)
            .frame(egui::Frame::new().fill(theme::WHITE))
            .show(ui, |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgb(253, 252, 249))
                    .inner_margin(egui::Margin::symmetric(18, 6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    egui::Button::new(if self.report_open {
                                        "⌄  Ukryj przebieg"
                                    } else {
                                        "›  Pokaż przebieg"
                                    })
                                    .frame(false),
                                )
                                .clicked()
                            {
                                self.report_open = !self.report_open;
                                self.graph.request_fit();
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        RichText::new("Wyniki i dane każdego kroku")
                                            .size(10.0)
                                            .color(theme::MUTED),
                                    );
                                },
                            );
                        });
                    });
                if !self.report_open {
                    return;
                }
                let (handle, response) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 12.0),
                    egui::Sense::click_and_drag(),
                );
                self.report_handle = Some(handle);
                ui.painter().rect_filled(
                    egui::Rect::from_center_size(handle.center(), egui::vec2(44.0, 3.0)),
                    2.0,
                    theme::BORDER,
                );
                let response = response
                    .on_hover_cursor(egui::CursorIcon::ResizeVertical)
                    .on_hover_text(
                        "Zmień wysokość przebiegu. Strzałki góra/dół po kliknięciu separatora.",
                    );
                if response.clicked() {
                    response.request_focus();
                }
                if response.dragged() {
                    self.report_height = (self.report_height
                        - ui.input(|input| input.pointer.delta().y))
                    .clamp(160.0, max_height);
                }
                if response.has_focus() {
                    ui.input(|input| {
                        if input.key_pressed(egui::Key::ArrowUp) {
                            self.report_height = (self.report_height + 32.0).min(max_height);
                        }
                        if input.key_pressed(egui::Key::ArrowDown) {
                            self.report_height = (self.report_height - 32.0).max(160.0);
                        }
                        if input.key_pressed(egui::Key::Home) {
                            self.report_height = 160.0;
                        }
                        if input.key_pressed(egui::Key::End) {
                            self.report_height = max_height;
                        }
                    });
                }
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(24, 9))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("▷  Przebieg testu").strong().size(13.0));
                            ui.label(
                                RichText::new("BRAK PRZEBIEGU")
                                    .size(9.0)
                                    .color(theme::MUTED),
                            );
                        });
                    });
                ui.separator();
                ui.allocate_ui_with_layout(
                    ui.available_size(),
                    egui::Layout::centered_and_justified(egui::Direction::TopDown),
                    |ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new("Wyniki pojawią się po uruchomieniu scenariusza")
                                    .size(13.0)
                                    .color(theme::MUTED),
                            );
                            ui.label(
                                RichText::new(
                                    "Uruchamianie testów jest jeszcze w trakcie migracji.",
                                )
                                .size(10.0)
                                .color(theme::MUTED),
                            );
                        });
                    },
                );
            });
    }
}
fn eyebrow(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(9.0).color(theme::MUTED));
}
fn navigation(ui: &mut egui::Ui, label: &str, selected: bool) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 36.0), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, label)
    });
    if selected {
        ui.painter()
            .rect_filled(rect, 7.0, Color32::from_rgb(252, 239, 230));
    }
    ui.painter().with_clip_rect(rect.shrink(8.0)).text(
        rect.left_center() + egui::vec2(10.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(11.0),
        if selected { theme::ACCENT } else { theme::TEXT },
    );
    response.on_hover_text(label).clicked()
}
fn library_item(
    ui: &mut egui::Ui,
    kind: apiforge_core::model::NodeKind,
    name: &str,
    description: &str,
) {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 47.0), egui::Sense::hover());
    let painter = ui.painter().with_clip_rect(rect);
    let color = theme::kind_color(kind);
    let icon = egui::Rect::from_min_size(rect.min + egui::vec2(1.0, 9.0), egui::vec2(29.0, 29.0));
    painter.rect_filled(icon, 7.0, color.gamma_multiply(0.12));
    theme::kind_icon(&painter, icon, kind, color);
    painter.text(
        rect.min + egui::vec2(40.0, 18.0),
        egui::Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(11.0),
        theme::TEXT,
    );
    painter.text(
        rect.min + egui::vec2(40.0, 34.0),
        egui::Align2::LEFT_CENTER,
        description,
        egui::FontId::proportional(9.0),
        theme::MUTED,
    );
    response.on_hover_text("Biblioteka z importowanego projektu. Dodawanie i edycja zostaną przeniesione razem z komendami edytora.");
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_core::import_document;
    fn fixture() -> ValidatedDocument {
        import_document(
            include_bytes!("../../../fixtures/compatibility/workspace-v1.json"),
            &ImportLimits::default(),
        )
        .expect("fixture")
    }
    #[test]
    fn failed_or_disconnected_import_preserves_the_open_document() {
        let mut app = Desktop::new(egui::Context::default(), None, None, None);
        app.document = Some(fixture());
        app.refresh_graph();
        let before = app
            .document
            .as_ref()
            .expect("document")
            .to_json()
            .expect("serialize");
        for disconnected in [false, true] {
            let (sender, receiver) = mpsc::sync_channel(1);
            app.receiver = Some(receiver);
            app.loading = true;
            if !disconnected {
                sender.send(Err("invalid fixture".into())).expect("send");
            }
            drop(sender);
            app.poll_load();
            assert!(!app.loading);
            assert!(app.error.is_some());
            assert_eq!(
                before,
                app.document
                    .as_ref()
                    .expect("preserved")
                    .to_json()
                    .expect("serialize")
            );
        }
    }
    #[test]
    fn every_project_and_phase_can_be_rendered_without_changing_the_document() {
        let context = egui::Context::default();
        let mut app = Desktop::new(context.clone(), None, None, None);
        app.document = Some(fixture());
        let before = app
            .document
            .as_ref()
            .expect("document")
            .to_json()
            .expect("serialize");
        for project in 0..4 {
            for cleanup in [false, true] {
                app.project = project;
                app.cleanup = cleanup;
                app.refresh_graph();
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1440.0, 900.0),
                    )),
                    ..Default::default()
                };
                let mut output = context.run_ui(input, |ui| app.show(ui));
                assert!(!output.shapes.is_empty());
                // Headless layout tests intentionally do not upload textures to a GPU.
                output.textures_delta.clear();
            }
        }
        assert_eq!(
            before,
            app.document
                .as_ref()
                .expect("document")
                .to_json()
                .expect("serialize")
        );
    }
    #[test]
    fn report_resizes_with_keyboard_and_keeps_its_height_when_collapsed() {
        let context = egui::Context::default();
        let mut app = Desktop::new(context.clone(), None, None, None);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1000.0));
        let draw = |app: &mut Desktop, events| {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                },
                |ui| app.show(ui),
            );
            output.textures_delta.clear();
        };
        draw(&mut app, vec![]);
        let handle = app.report_handle.expect("resize handle").center();
        for pressed in [true, false] {
            draw(
                &mut app,
                vec![
                    egui::Event::PointerMoved(handle),
                    egui::Event::PointerButton {
                        pos: handle,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        let before = app.report_height;
        draw(
            &mut app,
            vec![egui::Event::Key {
                key: egui::Key::ArrowUp,
                physical_key: Some(egui::Key::ArrowUp),
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert_eq!(app.report_height, before + 32.0);
        let resized = app.report_height;
        app.report_open = false;
        draw(&mut app, vec![]);
        assert!(app.report_handle.is_none());
        app.report_open = true;
        draw(&mut app, vec![]);
        assert_eq!(app.report_height, resized);
    }
}
