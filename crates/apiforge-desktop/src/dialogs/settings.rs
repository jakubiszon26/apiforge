//! Project and scenario settings: names, description, error policy, scenario
//! parameters and the project's data contracts.
//!
//! Unlike the reference editor, which applied every keystroke, the dialog edits a
//! working copy and applies it as a whole, so an unparsable JSON field can never
//! leave half a change behind.

use super::{Chrome, Outcome, autofocus, json_text, parse_json};
use crate::icons::Icon;
use crate::widgets;
use apiforge_core::{
    format::{Contract, Project},
    js,
    model::ErrorPolicy,
};
use eframe::egui::{self, Ui};
use indexmap::IndexMap;
use serde_json::Value;

/// The settings to apply to the active project and its current scenario.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsChange {
    pub project_name: String,
    pub scenario_id: String,
    pub scenario_name: String,
    pub description: String,
    pub error_policy: ErrorPolicy,
    pub parameters: IndexMap<String, Value>,
    pub contracts: Vec<Contract>,
}

impl SettingsChange {
    /// Remove only assignments to contracts explicitly deleted in this edit.
    pub fn apply(self, project: &mut Project) {
        let removed: std::collections::HashSet<_> = project
            .contracts
            .iter()
            .filter(|old| !self.contracts.iter().any(|new| new.id == old.id))
            .map(|old| old.id.clone())
            .collect();
        let clear = |id: &mut Option<String>| {
            if id.as_ref().is_some_and(|id| removed.contains(id)) {
                *id = None;
            }
        };
        let clear_graph = |graph: &mut apiforge_core::format::Graph| {
            for node in &mut graph.nodes {
                clear(&mut node.config.contract_id);
            }
        };
        for scenario in &mut project.scenarios {
            clear_graph(&mut scenario.graph);
            clear_graph(&mut scenario.cleanup);
        }
        for component in &mut project.components {
            clear_graph(&mut component.graph);
            for port in &mut component.inputs {
                clear(&mut port.contract_id);
            }
            if let Some(action) = &mut component.action {
                clear(&mut action.contract_id);
            }
        }
        project.name = self.project_name;
        project.contracts = self.contracts;
        if let Some(scenario) = project
            .scenarios
            .iter_mut()
            .find(|s| s.id == self.scenario_id)
        {
            scenario.name = self.scenario_name;
            scenario.description = self.description;
            scenario.error_policy = self.error_policy;
            scenario.parameters = self.parameters;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Scenario,
    Contracts,
}

#[derive(Clone, Debug, PartialEq)]
struct ContractRow {
    id: String,
    name: String,
    sample: String,
}

/// Problems that keep the settings from being saved.
#[derive(Debug, Default, PartialEq)]
struct Problems {
    project_name: Option<&'static str>,
    scenario_name: Option<&'static str>,
    parameters: Option<String>,
    /// Per contract: the name problem and the sample problem.
    contracts: Vec<(Option<&'static str>, Option<String>)>,
}

impl Problems {
    fn is_empty(&self) -> bool {
        self.project_name.is_none()
            && self.scenario_name.is_none()
            && self.parameters.is_none()
            && self
                .contracts
                .iter()
                .all(|(name, sample)| name.is_none() && sample.is_none())
    }

    fn contracts_invalid(&self) -> bool {
        self.contracts
            .iter()
            .any(|(name, sample)| name.is_some() || sample.is_some())
    }

    fn scenario_invalid(&self) -> bool {
        self.project_name.is_some() || self.scenario_name.is_some() || self.parameters.is_some()
    }
}

fn required_name(name: &str) -> Option<&'static str> {
    let trimmed = js::trim(name);
    if trimmed.is_empty() {
        Some("Enter a name.")
    } else if super::RESERVED_NAMES.contains(&trimmed) {
        Some("This name is reserved. Choose another one.")
    } else if js::utf16_len(trimmed) > super::NAME_LIMIT {
        Some("Use at most 200 characters.")
    } else {
        None
    }
}

/// Scenario parameters: a JSON object whose keys the runner can use.
fn parse_parameters(text: &str) -> Result<IndexMap<String, Value>, String> {
    match parse_json(text)? {
        Value::Object(fields) => {
            if let Some(key) = fields
                .keys()
                .find(|key| super::RESERVED_NAMES.contains(&key.as_str()))
            {
                return Err(format!(
                    "“{key}” is reserved. Choose another parameter name."
                ));
            }
            Ok(fields.into_iter().collect())
        }
        _ => Err("Parameters must be a JSON object, e.g. {\"userId\": 1}.".into()),
    }
}

/// Edits the active project's name and contracts and the current scenario.
pub struct SettingsDialog {
    tab: Tab,
    project_name: String,
    scenario_id: String,
    scenario_name: String,
    description: String,
    error_policy: ErrorPolicy,
    parameters: String,
    contracts: Vec<ContractRow>,
    focused: bool,
    project_only: bool,
    error: Option<String>,
}

impl SettingsDialog {
    /// Project-level fields only, including empty/component-only authoring.
    pub fn project(project: &Project) -> Self {
        Self {
            tab: Tab::Scenario,
            project_name: project.name.clone(),
            scenario_id: String::new(),
            scenario_name: String::new(),
            description: String::new(),
            error_policy: ErrorPolicy::Stop,
            parameters: "{}".into(),
            contracts: project
                .contracts
                .iter()
                .map(|contract| ContractRow {
                    id: contract.id.clone(),
                    name: contract.name.clone(),
                    sample: json_text(&contract.sample),
                })
                .collect(),
            focused: false,
            project_only: true,
            error: None,
        }
    }

    pub fn new(project: &Project, scenario_id: &str) -> Self {
        let scenario = project
            .scenarios
            .iter()
            .find(|scenario| scenario.id == scenario_id)
            .unwrap_or(&project.scenarios[0]);
        let mut dialog = Self::project(project);
        dialog.project_only = false;
        dialog.scenario_id = scenario.id.clone();
        dialog.scenario_name = scenario.name.clone();
        dialog.description = scenario.description.clone();
        dialog.error_policy = scenario.error_policy;
        dialog.parameters = json_text(&Value::Object(
            scenario.parameters.clone().into_iter().collect(),
        ));
        dialog
    }

    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }

    fn problems(&self) -> Problems {
        Problems {
            project_name: required_name(&self.project_name),
            scenario_name: if self.project_only {
                None
            } else {
                required_name(&self.scenario_name)
            },
            parameters: parse_parameters(&self.parameters).err(),
            contracts: self
                .contracts
                .iter()
                .map(|row| {
                    let duplicate = self
                        .contracts
                        .iter()
                        .filter(|other| {
                            js::trim(&other.name).to_lowercase()
                                == js::trim(&row.name).to_lowercase()
                        })
                        .count()
                        > 1;
                    (
                        required_name(&row.name)
                            .or(duplicate.then_some("This name is already used.")),
                        parse_json(&row.sample).err(),
                    )
                })
                .collect(),
        }
    }

    fn result(&self) -> Option<SettingsChange> {
        if !self.problems().is_empty() {
            return None;
        }
        Some(SettingsChange {
            project_name: js::trim(&self.project_name).to_owned(),
            scenario_id: self.scenario_id.clone(),
            scenario_name: js::trim(&self.scenario_name).to_owned(),
            description: self.description.clone(),
            error_policy: self.error_policy,
            parameters: parse_parameters(&self.parameters).ok()?,
            contracts: self
                .contracts
                .iter()
                .map(|row| {
                    Some(Contract {
                        id: row.id.clone(),
                        name: js::trim(&row.name).to_owned(),
                        sample: parse_json(&row.sample).ok()?,
                    })
                })
                .collect::<Option<_>>()?,
        })
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome<SettingsChange> {
        let chrome = Chrome {
            id: "settings-dialog",
            eyebrow: "PROJECT SETTINGS",
            title: if self.project_only {
                "Project settings"
            } else {
                "Project and scenario"
            },
            width: 600.0,
            apply_label: if self.project_only {
                "Apply settings"
            } else {
                "Save settings"
            },
            note: self
                .project_only
                .then_some("Apply changes, then use Save all to save the project settings."),
        };
        let action = chrome.show(
            ctx,
            self,
            |ui, dialog| dialog.body(ui),
            |dialog| dialog.problems().is_empty(),
        );
        match action {
            super::Action::Cancel => Outcome::Cancelled,
            super::Action::Apply => self.result().map_or(Outcome::Open, Outcome::Applied),
            super::Action::None => Outcome::Open,
        }
    }

    fn body(&mut self, ui: &mut Ui) {
        let problems = self.problems();
        if let Some(error) = &self.error {
            widgets::field_error(ui, error);
        }
        ui.horizontal(|ui| {
            let scenario = if self.project_only {
                if problems.scenario_invalid() {
                    "Project  ⚠"
                } else {
                    "Project"
                }
            } else if problems.scenario_invalid() {
                "Project and scenario  ⚠"
            } else {
                "Project and scenario"
            };
            let contracts = if problems.contracts_invalid() {
                "Data contracts  ⚠"
            } else {
                "Data contracts"
            };
            widgets::tab_value(ui, &mut self.tab, Tab::Scenario, scenario);
            widgets::tab_value(ui, &mut self.tab, Tab::Contracts, contracts);
        });
        ui.add_space(4.0);
        match self.tab {
            Tab::Scenario => self.scenario_tab(ui, &problems),
            Tab::Contracts => self.contracts_tab(ui, &problems),
        }
    }

    fn scenario_tab(&mut self, ui: &mut Ui, problems: &Problems) {
        let field = super::labelled_field(
            ui,
            "Project name",
            &mut self.project_name,
            "e.g. Platform API",
            problems.project_name,
        );
        autofocus(&field, &mut self.focused);
        if self.project_only {
            return;
        }
        super::labelled_field(
            ui,
            "Scenario name",
            &mut self.scenario_name,
            "e.g. User registration",
            problems.scenario_name,
        );
        widgets::field_label(ui, "Description");
        widgets::named(
            ui.add(
                crate::widgets::text_area(&mut self.description)
                    .hint_text("What does this scenario check?")
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            ),
            "Description",
        );
        widgets::field_label(ui, "After an error");
        let policy = egui::ComboBox::from_id_salt("settings-error-policy")
            .width(260.0)
            .selected_text(policy_label(self.error_policy))
            .show_ui(ui, |ui| {
                for policy in [ErrorPolicy::Continue, ErrorPolicy::Stop] {
                    ui.selectable_value(&mut self.error_policy, policy, policy_label(policy));
                }
            });
        widgets::named(policy.response, "After an error");
        widgets::field_label(ui, "Scenario parameters (JSON object)");
        widgets::named(
            super::json_field(ui, &mut self.parameters, 5, problems.parameters.is_some()),
            "Scenario parameters (JSON object)",
        );
        if let Some(error) = &problems.parameters {
            widgets::field_error(ui, error.clone());
        }
        widgets::hint(
            ui,
            "Blocks read parameters by name when an argument's source is Parameter.",
        );
    }

    fn contracts_tab(&mut self, ui: &mut Ui, problems: &Problems) {
        widgets::hint(
            ui,
            "Contracts name the project's data and decide which custom actions appear next \
             to a component output.",
        );
        if self.contracts.is_empty() {
            ui.add_space(4.0);
            widgets::hint(ui, "No contracts yet.");
        }
        widgets::hint(
            ui,
            "Deleting a contract clears its assignments from steps and components.",
        );
        let mut remove = None;
        for (index, row) in self.contracts.iter_mut().enumerate() {
            let (name_error, sample_error) =
                problems.contracts.get(index).cloned().unwrap_or_default();
            ui.push_id(("contract", row.id.as_str()), |ui| {
                ui.add_space(6.0);
                ui.separator();
                ui.horizontal(|ui| {
                    widgets::hint(ui, format!("ID: {}", row.id));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                widgets::icon_button(Icon::Trash, "Delete contract")
                                    .compact()
                                    .danger(),
                            )
                            .clicked()
                        {
                            remove = Some(index);
                        }
                    });
                });
                super::labelled_field(ui, "Contract name", &mut row.name, "e.g. Cart", name_error);
                widgets::field_label(ui, "Sample structure");
                widgets::named(
                    super::json_field(ui, &mut row.sample, 4, sample_error.is_some()),
                    "Sample structure",
                );
                if let Some(error) = sample_error {
                    widgets::field_error(ui, error);
                }
            });
        }
        if let Some(index) = remove {
            self.contracts.remove(index);
        }
        ui.add_space(8.0);
        if ui
            .add(widgets::action(Icon::Plus, "Add contract").quiet())
            .clicked()
        {
            self.contracts.push(ContractRow {
                id: apiforge_core::ids::new_id("contract"),
                name: "New contract".into(),
                sample: "{}".into(),
            });
        }
    }
}

fn policy_label(policy: ErrorPolicy) -> &'static str {
    match policy {
        ErrorPolicy::Continue => "Continue with the possible steps",
        ErrorPolicy::Stop => "Stop the test",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::test_support::{press, run_frames};
    use apiforge_core::demos;
    use serde_json::json;

    #[test]
    fn parameters_must_be_an_object_without_reserved_keys() {
        assert_eq!(
            parse_parameters("{\"a\": 1, \"b\": null}").unwrap(),
            IndexMap::from([("a".to_owned(), json!(1)), ("b".to_owned(), Value::Null)])
        );
        assert!(parse_parameters("[1]").is_err());
        assert!(parse_parameters("{\"__proto__\": 1}").is_err());
        assert!(parse_parameters("{").unwrap_err().contains("line"));
    }

    #[test]
    fn the_result_keeps_order_and_trims_names() {
        let project = demos::shop_project();
        let mut dialog = SettingsDialog::new(&project, &project.scenarios[0].id);
        dialog.project_name = "  Shop  ".into();
        dialog.parameters = "{\"z\": 1, \"a\": 2}".into();
        dialog.error_policy = ErrorPolicy::Stop;
        dialog.contracts.push(ContractRow {
            id: "contract-new".into(),
            name: " New cart ".into(),
            sample: "{\"total\": 0}".into(),
        });
        let change = dialog.result().expect("valid");
        assert_eq!(change.project_name, "Shop");
        assert_eq!(change.scenario_id, project.scenarios[0].id);
        assert_eq!(change.parameters.keys().collect::<Vec<_>>(), ["z", "a"]);
        assert_eq!(change.error_policy, ErrorPolicy::Stop);
        let added = change.contracts.last().unwrap();
        assert_eq!(
            (added.name.as_str(), &added.sample),
            ("New cart", &json!({"total": 0}))
        );
        assert_eq!(change.contracts.len(), project.contracts.len() + 1);
    }

    #[test]
    fn invalid_fields_block_saving() {
        let project = demos::shop_project();
        let mut dialog = SettingsDialog::new(&project, &project.scenarios[0].id);
        dialog.scenario_name = "   ".into();
        assert!(dialog.result().is_none());
        dialog.scenario_name = "ok".into();
        dialog.contracts.push(ContractRow {
            id: "c".into(),
            name: "x".into(),
            sample: "not json".into(),
        });
        let problems = dialog.problems();
        assert!(problems.contracts_invalid() && !problems.scenario_invalid());
        assert!(dialog.result().is_none());
    }

    #[test]
    fn reserved_and_overlong_contract_names_are_rejected_and_contracts_can_be_removed() {
        let project = demos::shop_project();
        let mut dialog = SettingsDialog::new(&project, &project.scenarios[0].id);
        let initial_len = dialog.contracts.len();
        assert!(initial_len >= 1);
        dialog.contracts.push(ContractRow {
            id: "contract-reserved".into(),
            name: "  __proto__  ".into(),
            sample: "{}".into(),
        });
        let problems = dialog.problems();
        assert!(problems.contracts_invalid());
        assert_eq!(
            problems.contracts.last().unwrap().0,
            Some("This name is reserved. Choose another one.")
        );
        assert!(dialog.result().is_none());

        dialog.contracts.last_mut().unwrap().name = "x".repeat(201);
        let problems = dialog.problems();
        assert_eq!(
            problems.contracts.last().unwrap().0,
            Some("Use at most 200 characters.")
        );
        assert!(dialog.result().is_none());

        // Removing the invalid contract restores validity; removing an existing contract shrinks the list.
        dialog.contracts.pop();
        dialog.contracts.remove(0);
        let change = dialog
            .result()
            .expect("valid after removing invalid contract");
        assert_eq!(change.contracts.len(), initial_len - 1);
    }

    #[test]
    fn an_unchanged_dialog_applies_the_current_settings() {
        for project in demos::demo_projects() {
            let mut dialog = SettingsDialog::new(&project, &project.scenarios[0].id);
            run_frames(2, |ctx| assert_eq!(dialog.show(ctx), Outcome::Open));
            let scenario = &project.scenarios[0];
            let change = dialog.result().expect("demo settings are valid");
            assert_eq!(change.parameters, scenario.parameters);
            assert_eq!(change.contracts, project.contracts);
        }
        let project = demos::tickets_project();
        let ctx = egui::Context::default();
        let mut dialog = SettingsDialog::new(&project, &project.scenarios[0].id);
        let mut outcome = Outcome::Open;
        press(&ctx, egui::Key::Escape, egui::Modifiers::NONE, |ctx| {
            outcome = dialog.show(ctx);
        });
        assert_eq!(outcome, Outcome::Cancelled);
    }
    #[test]
    fn contract_names_reject_case_and_whitespace_duplicates() {
        let project = demos::shop_project();
        let mut dialog = SettingsDialog::new(&project, &project.scenarios[0].id);
        let mut duplicate = dialog.contracts[0].clone();
        duplicate.id = "duplicate-contract".into();
        duplicate.name = format!("  {}  ", duplicate.name.to_uppercase());
        dialog.contracts.push(duplicate);
        assert!(dialog.problems().contracts_invalid());
        assert!(dialog.result().is_none());
    }

    #[test]
    fn removing_a_contract_clears_assignments_in_all_graphs_and_component_ports() {
        let mut project = demos::shop_project();
        let removed = project.contracts[0].id.clone();
        for scenario in &mut project.scenarios {
            for graph in [&mut scenario.graph, &mut scenario.cleanup] {
                for node in &mut graph.nodes {
                    node.config.contract_id = Some(removed.clone());
                }
            }
        }
        for component in &mut project.components {
            for node in &mut component.graph.nodes {
                node.config.contract_id = Some(removed.clone());
            }
            for port in &mut component.inputs {
                port.contract_id = Some(removed.clone());
            }
            if let Some(action) = &mut component.action {
                action.contract_id = Some(removed.clone());
            }
        }
        let mut dialog = SettingsDialog::new(&project, &project.scenarios[0].id);
        dialog.contracts.remove(0);
        dialog.result().unwrap().apply(&mut project);
        fn check(value: &Value, removed: &str) {
            match value {
                Value::Object(fields) => {
                    assert_ne!(
                        fields.get("contractId").and_then(Value::as_str),
                        Some(removed)
                    );
                    for value in fields.values() {
                        check(value, removed);
                    }
                }
                Value::Array(items) => {
                    for item in items {
                        check(item, removed);
                    }
                }
                _ => {}
            }
        }
        check(&serde_json::to_value(&project).unwrap(), &removed);
        apiforge_core::validate_project(&project).unwrap();
    }
}

#[cfg(test)]
mod folder_project_tests {
    use super::*;

    #[test]
    fn project_settings_work_without_a_scenario_or_a_placeholder() {
        let mut project = apiforge_core::demos::empty_project();
        project.scenarios.clear();
        let mut dialog = SettingsDialog::project(&project);
        dialog.project_name = "Renamed empty project".into();
        let result = dialog.result().unwrap();
        assert_eq!(result.project_name, "Renamed empty project");
        assert!(result.scenario_id.is_empty());
        assert!(project.scenarios.is_empty());
        dialog.project_name.clear();
        assert!(dialog.result().is_none());
    }
}
