//! A document inspector, deliberately separate from future validated editing commands.
use crate::theme::{self, ACCENT, BG, BORDER, MUTED};
use apiforge_core::{
    format::{Binding, Graph, Node, Project},
    model::{Branch, HttpMethod, NodeKind, Operator, Predicate},
};
use eframe::egui::{self, RichText};

pub fn show(ui: &mut egui::Ui, node: &Node, graph: &Graph, project: &Project) {
    ui.add_space(10.0);
    ui.label(
        RichText::new(node.kind.label().to_uppercase())
            .size(10.0)
            .color(theme::kind_color(node.kind))
            .strong(),
    );
    ui.add_space(2.0);
    ui.label(
        RichText::new(&node.name)
            .size(20.0)
            .family(theme::bold_family()),
    );
    ui.add_space(8.0);
    ui.label(
        RichText::new("Konfiguracja")
            .size(12.0)
            .color(ACCENT)
            .strong(),
    );
    ui.separator();
    ui.label(
        RichText::new("Podgląd dokumentu · wartości można zaznaczyć i skopiować.")
            .small()
            .color(MUTED),
    );
    ui.add_space(12.0);
    egui::ScrollArea::vertical()
        .id_salt(("node-inspector", &node.id))
        .show(ui, |ui| {
            configuration(ui, node, project);
            if !node.inputs.is_empty() {
                section(ui, "Argumenty");
                for (name, binding) in &node.inputs {
                    ui.label(RichText::new(name).size(12.0).strong());
                    egui::Frame::new()
                        .fill(BG)
                        .stroke(egui::Stroke::new(1.0, BORDER))
                        .corner_radius(6)
                        .inner_margin(9)
                        .show(ui, |ui| {
                            ui.set_min_width((ui.available_width() - 2.0).max(0.0));
                            if sensitive(name, project) {
                                ui.label("••••••");
                            } else {
                                binding_view(ui, binding, graph, project, 0);
                            }
                        });
                    ui.add_space(8.0);
                }
            }
            let links: Vec<_> = graph
                .edges
                .iter()
                .filter(|edge| edge.source == node.id)
                .collect();
            if !links.is_empty() {
                section(ui, "Kolejne kroki");
                for edge in links {
                    let destination = graph
                        .nodes
                        .iter()
                        .find(|candidate| candidate.id == edge.target)
                        .map_or("Nieznany klocek", |candidate| candidate.name.as_str());
                    field(
                        ui,
                        match edge.branch {
                            Some(Branch::True) => "Tak",
                            Some(Branch::False) => "Nie",
                            None => "Dalej",
                        },
                        destination,
                        project,
                    );
                }
            }
            ui.add_space(12.0);
        });
}

fn configuration(ui: &mut egui::Ui, node: &Node, project: &Project) {
    field(ui, "Nazwa klocka", &node.name, project);
    match node.kind {
        NodeKind::Request => {
            field(
                ui,
                "Metoda HTTP",
                method(node.config.method.unwrap_or(HttpMethod::Get)),
                project,
            );
            field(
                ui,
                "URL lub ścieżka",
                node.config.url.as_deref().unwrap_or("—"),
                project,
            );
            if let Some(timeout) = node.config.timeout_ms {
                field(ui, "Limit czasu", &format!("{timeout} ms"), project);
            }
            if let Some(query) = &node.config.query {
                for (key, value) in query {
                    field(
                        ui,
                        &format!("Parametr · {key}"),
                        if sensitive(key, project) {
                            "••••••"
                        } else {
                            value
                        },
                        project,
                    );
                }
            }
            if let Some(headers) = &node.config.headers {
                for (key, value) in headers {
                    field(
                        ui,
                        &format!("Nagłówek · {key}"),
                        if sensitive(key, project) {
                            "••••••"
                        } else {
                            value
                        },
                        project,
                    );
                }
            }
        }
        NodeKind::Select => field(
            ui,
            "Ścieżka pola",
            node.config
                .path
                .as_deref()
                .filter(|p| !p.is_empty())
                .unwrap_or("Cała wartość"),
            project,
        ),
        NodeKind::Assert | NodeKind::Condition | NodeKind::If => {
            field(
                ui,
                "Porównanie",
                predicate(node.config.predicate.unwrap_or_default()),
                project,
            );
            if let Some(path) = &node.config.path {
                field(ui, "Ścieżka pola", path, project);
            }
            if let Some(tolerance) = node.config.tolerance {
                field(ui, "Tolerancja", &tolerance.to_string(), project);
            }
            if node.kind == NodeKind::If {
                hint(
                    ui,
                    "Wynik porównania wybiera gałąź Tak albo Nie. Fałsz nie oznacza błędu testu.",
                );
            }
            if let Some(predicates) = &node.config.predicates {
                for condition in predicates {
                    field(
                        ui,
                        "Dodatkowy warunek",
                        &format!(
                            "{} {} {}",
                            condition.input,
                            predicate(condition.predicate),
                            condition.expected
                        ),
                        project,
                    );
                    if let Some(path) = &condition.path {
                        field(ui, "Ścieżka warunku", path, project);
                    }
                }
            }
        }
        NodeKind::Component => {
            if let Some(component) = project.components.iter().find(|component| {
                Some(&component.id) == node.config.component_id.as_ref()
                    && Some(component.revision) == node.config.revision
            }) {
                field(ui, "Definicja", &component.name, project);
                field(ui, "Rewizja", &component.revision.to_string(), project);
                if !component.description.is_empty() {
                    hint(ui, &component.description);
                }
                for port in &component.inputs {
                    field(
                        ui,
                        "Parametr publiczny",
                        port.label.as_deref().unwrap_or(&port.name),
                        project,
                    );
                }
            }
        }
        NodeKind::Trigger => hint(
            ui,
            "Wyzwalacz ręczny. Po uruchomieniu scenariusza wykonanie przechodzi do pierwszego połączonego kroku.",
        ),
        NodeKind::Variable => hint(
            ui,
            "Wartość zmiennej jest obliczana z argumentu podczas wykonania scenariusza.",
        ),
        NodeKind::Formula => {
            if let Some(operation) = node.config.operation {
                field(ui, "Operator", operator_label(operation), project);
            }
        }
        NodeKind::Build => field(
            ui,
            "Struktura",
            if node.config.build_type == Some(apiforge_core::model::BuildType::Array) {
                "Lista"
            } else {
                "Obiekt"
            },
            project,
        ),
    }
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(6.0);
    ui.label(RichText::new(title).strong());
    ui.add_space(4.0);
}

fn hint(ui: &mut egui::Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).small().color(MUTED)).wrap());
    ui.add_space(8.0);
}

fn field(ui: &mut egui::Ui, label: &str, value: &str, project: &Project) {
    ui.label(RichText::new(label).size(11.0).color(MUTED));
    copyable(ui, value, project);
    ui.add_space(8.0);
}

fn copyable(ui: &mut egui::Ui, value: &str, project: &Project) {
    // Secret declarations apply to all textual surfaces, including embedded literals.
    let mut redacted = value.to_owned();
    for name in &project.environment.secret_names {
        if let Some(secret) = project
            .environment
            .variables
            .get(name)
            .filter(|value| !value.is_empty())
        {
            redacted = redacted.replace(secret, "••••••");
        }
    }
    let shortened: String = redacted.chars().take(4_000).collect();
    ui.add(egui::Label::new(shortened).selectable(true).wrap());
    if redacted.chars().count() > 4_000 {
        hint(ui, "Podgląd długiej wartości został skrócony.");
    }
}

fn binding_view(
    ui: &mut egui::Ui,
    binding: &Binding,
    graph: &Graph,
    project: &Project,
    depth: usize,
) {
    // Imported expressions are validated, but a UI recursion bound also protects rendering.
    if depth > 12 {
        hint(ui, "Zagnieżdżone wyrażenie");
        return;
    }
    match binding {
        Binding::Literal { value } => {
            let kind = if value.is_null() {
                "null"
            } else if value.is_boolean() {
                "boolean"
            } else if value.is_number() {
                "number"
            } else if value.is_string() {
                "string"
            } else if value.is_array() {
                "array"
            } else {
                "object"
            };
            hint(ui, &format!("Stała · {kind}"));
            copyable(ui, &value.to_string(), project);
        }
        Binding::Reference {
            node_id,
            port,
            path,
        } => {
            let source = graph
                .nodes
                .iter()
                .find(|node| &node.id == node_id)
                .map_or(node_id.as_str(), |node| node.name.as_str());
            hint(ui, "Wartość z klocka");
            copyable(ui, &format!("{source} → {port}"), project);
            if !path.is_empty() {
                field(ui, "Pole odpowiedzi", path, project);
            }
        }
        Binding::Parameter { name } => {
            hint(ui, "Parametr scenariusza");
            copyable(ui, name, project);
        }
        Binding::Environment { name } => {
            hint(ui, "Zmienna środowiska");
            copyable(ui, name, project);
        }
        Binding::Expression { operator, inputs } => {
            hint(ui, &format!("Wyrażenie · {}", operator_label(*operator)));
            for (name, input) in inputs {
                ui.label(RichText::new(name).small().strong());
                ui.indent(("binding", depth, name), |ui| {
                    binding_view(ui, input, graph, project, depth + 1)
                });
            }
        }
    }
}

fn sensitive(name: &str, project: &Project) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "authorization",
        "cookie",
        "token",
        "secret",
        "password",
        "api-key",
        "api_key",
    ]
    .iter()
    .any(|part| name.contains(part))
        || project
            .environment
            .secret_names
            .iter()
            .any(|secret| secret.eq_ignore_ascii_case(&name))
}

fn method(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Head => "HEAD",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
        HttpMethod::Options => "OPTIONS",
    }
}

fn predicate(predicate: Predicate) -> &'static str {
    match predicate {
        Predicate::Equals => "jest równe",
        Predicate::NotEquals => "nie jest równe",
        Predicate::Greater => "jest większe niż",
        Predicate::Less => "jest mniejsze niż",
        Predicate::Exists => "istnieje",
        Predicate::NotEmpty => "nie jest puste",
        Predicate::Contains => "zawiera",
        Predicate::Type => "ma typ",
        Predicate::Approximately => "jest w przybliżeniu równe",
    }
}

fn operator_label(operator: Operator) -> &'static str {
    match operator {
        Operator::Add => "Dodawanie (+)",
        Operator::Subtract => "Odejmowanie (−)",
        Operator::Multiply => "Mnożenie (×)",
        Operator::Divide => "Dzielenie (÷)",
        Operator::Modulo => "Reszta z dzielenia",
        Operator::Random => "Liczba losowa",
        Operator::Round => "Zaokrąglenie",
        Operator::Equals => "Równe (=)",
        Operator::NotEquals => "Różne (≠)",
        Operator::Greater => "Większe (>)",
        Operator::Less => "Mniejsze (<)",
        Operator::AtLeast => "Co najmniej (≥)",
        Operator::AtMost => "Co najwyżej (≤)",
        Operator::And => "Oraz",
        Operator::Or => "Lub",
        Operator::Not => "Nie",
        Operator::Concat => "Połącz tekst",
        Operator::Letter => "Litera tekstu",
        Operator::Length => "Długość",
        Operator::Contains => "Zawiera",
        Operator::Abs => "Wartość bezwzględna",
        Operator::Floor => "Zaokrąglij w dół",
        Operator::Ceiling => "Zaokrąglij w górę",
        Operator::Sqrt => "Pierwiastek",
        Operator::Sin => "Sinus",
        Operator::Cos => "Cosinus",
        Operator::Tan => "Tangens",
        Operator::Asin => "Arcus sinus",
        Operator::Acos => "Arcus cosinus",
        Operator::Atan => "Arcus tangens",
        Operator::Ln => "Logarytm naturalny",
        Operator::Log => "Logarytm dziesiętny",
        Operator::Exp => "Potęga e",
        Operator::Power10 => "Potęga 10",
        Operator::TypeOf => "Typ wartości",
        Operator::Exists => "Wartość istnieje",
        Operator::Field => "Pole obiektu",
    }
}
