//! Syntax to document semantics. Temporary node identities are handles; only
//! reconciliation allocates persistent identities and positions.
use crate::ast::*;
use apiforge_core::{format::*, graph, model::*, operator::Operator, path::JsonPath};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
};

pub struct LowerContext<'a> {
    pub project: &'a Project,
    allow_unresolved_environment: bool,
    component: Option<&'a Component>,
}
impl<'a> LowerContext<'a> {
    /// Workspace-aware checking: unknown environment names are errors.
    pub fn new(project: &'a Project) -> Self {
        Self {
            project,
            allow_unresolved_environment: false,
            component: None,
        }
    }
    /// Editing/printing an existing document may retain intentionally unset
    /// environment names. Their values are never read or substituted here.
    pub fn document(project: &'a Project) -> Self {
        Self {
            project,
            allow_unresolved_environment: true,
            component: None,
        }
    }
    /// Pins the definition being edited/printed, including a historical revision.
    pub fn with_component(mut self, component: &'a Component) -> Self {
        self.component = Some(component);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Document {
    Scenario(Scenario),
    Component(Component),
}

#[derive(Clone, Debug)]
pub struct Lowered {
    /// Erroneous statements are omitted. Never commit this document when errors
    /// are present: it is useful only for editor diagnostics and completion.
    pub document: Option<Document>,
    pub diagnostics: Vec<Diagnostic>,
}
impl Lowered {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }
}

pub fn lower(tree: &SyntaxTree, context: &LowerContext<'_>) -> Lowered {
    let mut lower = Lower {
        project: context.project,
        diagnostics: tree.diagnostics.clone(),
        nodes: HashMap::new(),
        scopes: HashMap::new(),
        parameters: HashSet::new(),
        arguments: None,
        allow_unresolved_environment: context.allow_unresolved_environment,
        component: context.component,
        ranges: HashMap::new(),
        slot_ranges: HashMap::new(),
        handle: String::new(),
    };
    let document = tree.file.as_ref().map(|file| lower.file(file));
    if let Some(document) = &document {
        lower.type_warnings(document);
    }
    // Run the same structural, cycle and size validation as document imports.
    // Diagnostics deliberately omit the import error's path (it can contain names).
    if !lower
        .diagnostics
        .iter()
        .any(|d| d.severity == Severity::Error)
        && let Some(document) = &document
    {
        let mut project = context.project.clone();
        match document {
            Document::Scenario(scenario) => {
                project.scenarios = vec![scenario.clone()];
            }
            Document::Component(component) => {
                project
                    .components
                    .retain(|c| c.id != component.id || c.revision != component.revision);
                project.components.push(component.clone());
            }
        }
        if let Err(error) = apiforge_core::validate_project(&project) {
            let (range, message) = match &error {
                ImportError::InvalidDocument { path, message } => {
                    (located(path, document, &project, &lower.ranges), *message)
                }
                _ => (None, "Invalid lowered document"),
            };
            lower.error(range.unwrap_or(0..tree.source.len()), message);
        }
    }
    Lowered {
        document,
        diagnostics: lower.diagnostics,
    }
}

struct Lower<'a> {
    project: &'a Project,
    diagnostics: Vec<Diagnostic>,
    nodes: HashMap<String, Node>,
    scopes: HashMap<String, bool>, // true for cleanup
    parameters: HashSet<String>,
    arguments: Option<HashSet<String>>,
    allow_unresolved_environment: bool,
    component: Option<&'a Component>,
    /// Statement ranges by lowered node ID, to locate document validation errors.
    ranges: HashMap<String, Range<usize>>,
    /// Source of each slot (handle, input, operand path) for type warnings.
    slot_ranges: HashMap<(String, String, Vec<String>), Range<usize>>,
    /// Handle of the statement being lowered.
    handle: String,
}

/// `@config` replaces the whole configuration, so it must not silently override
/// what the statement itself says. Every setting written by the shorthand has
/// to be in `@config` with the same value, or be absent there while the
/// shorthand writes its default (the absent-versus-default case that `@config`
/// exists to preserve).
fn config_agrees(shorthand: &NodeConfig, exact: &NodeConfig) -> bool {
    let (Ok(Value::Object(shorthand)), Ok(Value::Object(exact))) =
        (serde_json::to_value(shorthand), serde_json::to_value(exact))
    else {
        return false;
    };
    let default = |key: &str| match key {
        "method" => Some(json!("GET")),
        "predicate" => serde_json::to_value(Predicate::default()).ok(),
        "tolerance" => Some(json!(0.0)),
        "sampleStatus" => Some(json!(200)),
        "type" => Some(json!("any")),
        "bodyFormat" => Some(json!("json")),
        "match" => Some(json!("all")),
        _ => None,
    };
    shorthand
        .iter()
        .filter(|(key, _)| !matches!(key.as_str(), "componentId" | "revision"))
        .all(|(key, value)| match exact.get(key) {
            Some(exact) => exact == value,
            None => default(key).as_ref() == Some(value),
        })
}

/// The statement behind a validation path such as `….graph.edges[3].target`.
/// An edge is reported at its target, whose statement declares the ordering.
fn located(
    path: &str,
    document: &Document,
    project: &Project,
    ranges: &HashMap<String, Range<usize>>,
) -> Option<Range<usize>> {
    let (graph, rest) = match document {
        Document::Scenario(scenario) => {
            if let Some((_, rest)) = path.split_once("scenarios[0].graph.") {
                (&scenario.graph, rest)
            } else {
                (
                    &scenario.cleanup,
                    path.split_once("scenarios[0].cleanup.")?.1,
                )
            }
        }
        Document::Component(component) => {
            let prefix = format!("components[{}].", project.components.len() - 1);
            (&component.graph, path.split_once(prefix.as_str())?.1)
        }
    };
    let (collection, rest) = rest.split_once('[')?;
    let index: usize = rest.split_once(']')?.0.parse().ok()?;
    let id = match collection {
        "nodes" => &graph.nodes.get(index)?.id,
        "edges" => &graph.edges.get(index)?.target,
        _ => return None,
    };
    ranges.get(id).cloned()
}
impl Lower<'_> {
    fn error(&mut self, range: Range<usize>, message: &'static str) {
        if self.diagnostics.len() < crate::MAX_DIAGNOSTICS {
            self.diagnostics.push(Diagnostic::error(range, message));
        } else if !self
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
        {
            self.diagnostics.pop();
            self.diagnostics.push(Diagnostic::error(range, message));
        }
    }
    fn file(&mut self, file: &File) -> Document {
        let component = matches!(file.kind, FileKind::Component { .. });
        self.parameters = if component {
            file.inputs.iter().map(|p| p.name.clone()).collect()
        } else {
            file.parameters
                .iter()
                .map(|(name, _)| name.clone())
                .collect()
        };
        self.collect(&file.statements, false);
        if let Some(cleanup) = &file.cleanup {
            self.collect(cleanup, true);
        }
        let graph = self.statements(&file.statements, false, component);
        match &file.kind {
            FileKind::Scenario { name } => {
                let parameters = file
                    .parameters
                    .iter()
                    .filter_map(|(name, value)| {
                        self.literal(value).map(|value| (name.clone(), value))
                    })
                    .collect();
                Document::Scenario(Scenario {
                    id: "script-scenario".into(),
                    name: name.clone(),
                    description: file.description.clone().unwrap_or_default(),
                    parameters,
                    error_policy: if file.on_error.as_deref() == Some("continue") {
                        ErrorPolicy::Continue
                    } else {
                        ErrorPolicy::Stop
                    },
                    graph,
                    cleanup: self.statements(
                        file.cleanup.as_deref().unwrap_or_default(),
                        true,
                        false,
                    ),
                })
            }
            FileKind::Component { handle, name } => {
                let inputs: Vec<_> = file.inputs.iter().map(|input| self.input(input)).collect();
                let action = file.action.as_ref().and_then(|name| {
                    let Some(port) = inputs.iter().find(|p| p.name == *name) else {
                        self.error(file.range.clone(), "Action input does not exist");
                        return None;
                    };
                    Some(ComponentAction {
                        input: name.clone(),
                        value_type: file
                            .action_type
                            .as_deref()
                            .map_or(port.value_type, value_type_from),
                        contract_id: if file.action_type.is_some() {
                            file.action_contract
                                .as_ref()
                                .and_then(|c| self.contract(c, file.range.clone()))
                        } else {
                            port.contract_id.clone()
                        },
                    })
                });
                let previous = self.component.filter(|c| c.handle == *handle).or_else(|| {
                    self.project
                        .components
                        .iter()
                        .filter(|c| c.handle == *handle)
                        .max_by_key(|c| c.revision)
                });
                Document::Component(Component {
                    id: previous.map_or_else(|| "script-component".into(), |c| c.id.clone()),
                    revision: previous.map_or(1, |c| c.revision),
                    handle: handle.clone(),
                    name: name.clone(),
                    description: file.description.clone().unwrap_or_default(),
                    inputs,
                    action,
                    graph,
                })
            }
        }
    }
    fn collect(&mut self, statements: &[Statement], cleanup: bool) {
        for statement in statements {
            if self.nodes.contains_key(&statement.handle) {
                self.error(statement.range.clone(), "Duplicate block handle");
                continue;
            }
            if matches!(statement.handle.as_str(), "env" | "param") {
                self.error(
                    statement.range.clone(),
                    "Block handle conflicts with a language namespace",
                );
            }
            let (kind, config) = match &statement.form {
                Form::Start => (NodeKind::Start, NodeConfig::default()),
                Form::Request { .. } => (NodeKind::Request, NodeConfig::default()),
                Form::Assert(_) => (NodeKind::Assert, NodeConfig::default()),
                Form::Condition { .. } => (NodeKind::Condition, NodeConfig::default()),
                Form::If { .. } => (NodeKind::If, NodeConfig::default()),
                Form::Export { .. } => (NodeKind::Export, NodeConfig::default()),
                Form::Value(expr) => {
                    if let ExprKind::Call { name, revision, .. } = &ungroup(expr).kind
                        && !name.starts_with("op.")
                        && self.project.components.iter().any(|c| c.handle == *name)
                    {
                        let definition = self
                            .project
                            .components
                            .iter()
                            .filter(|c| {
                                c.handle == *name && revision.is_none_or(|r| c.revision == r)
                            })
                            .max_by_key(|c| c.revision);
                        if let Some(definition) = definition {
                            (
                                NodeKind::Component,
                                NodeConfig {
                                    component_id: Some(definition.id.clone()),
                                    revision: Some(definition.revision),
                                    ..Default::default()
                                },
                            )
                        } else {
                            self.error(expr.range.clone(), "Component revision does not exist");
                            (NodeKind::Component, NodeConfig::default())
                        }
                    } else {
                        (NodeKind::Value, NodeConfig::default())
                    }
                }
            };
            self.scopes.insert(statement.handle.clone(), cleanup);
            self.nodes.insert(
                statement.handle.clone(),
                Node {
                    id: statement.handle.clone(),
                    handle: statement.handle.clone(),
                    kind,
                    config,
                    label: None,
                    note: statement.note.clone(),
                    inputs: Default::default(),
                    position: Position { x: 0.0, y: 0.0 },
                    auto_placed: true,
                },
            );
            if let Form::If {
                then, otherwise, ..
            } = &statement.form
            {
                self.collect(then, cleanup);
                self.collect(otherwise.as_deref().unwrap_or_default(), cleanup);
            }
        }
    }
    fn statements(&mut self, statements: &[Statement], cleanup: bool, component: bool) -> Graph {
        let mut result = Graph {
            nodes: vec![],
            edges: vec![],
        };
        self.walk(statements, cleanup, component, None, &mut result);
        result
    }
    fn walk(
        &mut self,
        statements: &[Statement],
        cleanup: bool,
        component: bool,
        parent: Option<(&str, Branch)>,
        graph: &mut Graph,
    ) {
        for statement in statements {
            self.arguments = if let Form::Request { items, .. } = &statement.form {
                Some(
                    items
                        .iter()
                        .filter_map(|item| {
                            if let RequestItem::Argument { name, .. } = item {
                                Some(name.clone())
                            } else {
                                None
                            }
                        })
                        .collect(),
                )
            } else {
                None
            };
            let before = self.diagnostics.len();
            self.handle.clone_from(&statement.handle);
            let Some(mut node) = self.nodes.get(&statement.handle).cloned() else {
                continue;
            };
            if graph.nodes.iter().any(|n| n.handle == statement.handle) {
                continue;
            }
            if statement.variable {
                if node.kind != NodeKind::Value {
                    self.error(
                        statement.range.clone(),
                        "var is allowed only on Value blocks",
                    );
                }
                node.config.variable = Some(true);
            }
            match &statement.form {
                Form::Start => {}
                Form::Value(value) if node.kind == NodeKind::Component => {
                    if let ExprKind::Call { arguments, .. } = &ungroup(value).kind {
                        let definition = graph::component_definition(self.project, &node);
                        for argument in arguments {
                            let Some(name) = &argument.name else {
                                self.error(
                                    argument.value.range.clone(),
                                    "Component arguments must be named",
                                );
                                continue;
                            };
                            if definition.is_none_or(|c| !c.inputs.iter().any(|p| p.name == *name))
                            {
                                self.error(argument.value.range.clone(), "Unknown component input");
                            }
                            self.slot(&mut node, name, &argument.value, cleanup);
                        }
                    }
                }
                Form::Value(value) => self.slot(&mut node, "value", value, cleanup),
                Form::Export { value_type, value } => {
                    if !component {
                        self.error(
                            statement.range.clone(),
                            "Export blocks belong in components",
                        );
                    }
                    node.config.output_type = Some(value_type_from(value_type));
                    self.slot(&mut node, "value", value, cleanup);
                }
                Form::Assert(check) | Form::If { check, .. } => {
                    node.config.predicate = Some(predicate(&check.predicate));
                    self.check(&mut node, check, "actual", "expected", cleanup);
                }
                Form::Condition {
                    mode,
                    checks,
                    value,
                } => {
                    node.config.match_mode = Some(if mode == "any" {
                        Match::Any
                    } else {
                        Match::All
                    });
                    node.config.predicates = Some(
                        checks
                            .iter()
                            .map(|c| ConditionPredicate {
                                predicate: predicate(&c.predicate),
                            })
                            .collect(),
                    );
                    for (index, check) in checks.iter().enumerate() {
                        let (actual, expected) = graph::condition_inputs(index + 1);
                        self.check(&mut node, check, &actual, &expected, cleanup);
                    }
                    if let Some(value) = value {
                        self.slot(&mut node, "value", value, cleanup);
                    }
                }
                Form::Request { method, url, items } => {
                    node.config.method = serde_json::from_value(Value::String(method.clone())).ok();
                    self.slot(&mut node, "url", url, cleanup);
                    for item in items {
                        match item {
                            RequestItem::Argument { name, value } => {
                                if !graph::is_request_argument(name) {
                                    self.error(
                                        value.range.clone(),
                                        "Request argument conflicts with an HTTP field",
                                    );
                                }
                                self.slot(&mut node, name, value, cleanup);
                            }
                            RequestItem::Headers(value) => {
                                self.slot(&mut node, "headers", value, cleanup)
                            }
                            RequestItem::Query(value) => {
                                self.slot(&mut node, "query", value, cleanup)
                            }
                            RequestItem::Body { text, value } => {
                                node.config.body_format = Some(if *text {
                                    BodyFormat::Text
                                } else {
                                    BodyFormat::Json
                                });
                                self.slot(&mut node, "body", value, cleanup);
                            }
                            RequestItem::Timeout { milliseconds } => {
                                node.config.timeout_ms = Some(*milliseconds)
                            }
                            RequestItem::Contract(name) => {
                                node.config.contract_id =
                                    self.contract(name, statement.range.clone())
                            }
                            RequestItem::Sample { status, value } => {
                                node.config.sample_status = Some(*status);
                                if let Some(value) = self.literal(value) {
                                    node.config.sample_response = OptionalJson::Present(value);
                                }
                            }
                        }
                    }
                }
            }
            let edge_start = graph.edges.len();
            if let Some((source, branch)) = parent {
                add_edge(graph, source, &node.id, Some(branch));
            }
            let mut seen = HashSet::new();
            for attribute in &statement.attributes {
                if !seen.insert(&attribute.name) {
                    self.error(attribute.range.clone(), "Duplicate block attribute");
                }
                match attribute.name.as_str() {
                    "label" => node.label = self.string_argument(attribute),
                    "note" => node.note = self.string_argument(attribute),
                    "contract" => {
                        if let Some(name) = self.string_argument(attribute) {
                            node.config.contract_id = self.contract(&name, attribute.range.clone());
                        }
                    }
                    "after" => {
                        if attribute.arguments.is_empty() {
                            self.error(attribute.range.clone(), "after needs an ordering source");
                        }
                        for argument in &attribute.arguments {
                            let (name, fields) = reference_parts(argument);
                            let branch = match fields.as_slice() {
                                [] => None,
                                [field] if field == "then" => Some(Branch::True),
                                [field] if field == "else" => Some(Branch::False),
                                _ => {
                                    self.error(argument.range.clone(), "Invalid ordering source");
                                    continue;
                                }
                            };
                            if let Some(source) = name.and_then(|name| self.nodes.get(name)) {
                                if self.scopes.get(&source.handle) != Some(&cleanup)
                                    || (branch.is_some() && source.kind != NodeKind::If)
                                {
                                    self.error(
                                        argument.range.clone(),
                                        "Ordering must stay in its graph and branches need an IF",
                                    );
                                } else {
                                    add_edge(graph, &source.id, &node.id, branch);
                                }
                            } else {
                                self.error(argument.range.clone(), "Unknown ordering source");
                            }
                        }
                    }
                    // Exact metadata for document fields with no shorthand. This
                    // never accepts identity/layout or raw reference identifiers.
                    "config" => {
                        if let [value] = attribute.arguments.as_slice()
                            && let Some(value) = self.literal(value)
                            && value.is_object()
                            && value.get("componentId").is_none()
                            && value.get("revision").is_none()
                            && let Ok(config) = serde_json::from_value::<NodeConfig>(value)
                        {
                            let identity = (node.config.component_id.clone(), node.config.revision);
                            if !config_agrees(&node.config, &config) {
                                self.error(
                                    attribute.range.clone(),
                                    "@config disagrees with this statement's settings; edit the value in @config or remove it",
                                );
                            }
                            node.config = config;
                            node.config.component_id = identity.0;
                            node.config.revision = identity.1;
                        } else {
                            self.error(
                                attribute.range.clone(),
                                "Expected configuration metadata without component identity",
                            );
                        }
                    }
                    "inputs" => {
                        if let [value] = attribute.arguments.as_slice()
                            && let Some(Binding::Object { fields }) = self.binding(value, cleanup)
                        {
                            node.inputs = fields;
                        } else {
                            self.error(
                                attribute.range.clone(),
                                "Expected an object of input bindings",
                            );
                        }
                    }
                    _ => self.error(
                        attribute.range.clone(),
                        "Unknown or misplaced block attribute",
                    ),
                }
            }
            if self.diagnostics[before..]
                .iter()
                .any(|d| d.severity == Severity::Error)
            {
                graph.edges.truncate(edge_start);
            } else {
                self.ranges.insert(node.id.clone(), statement.range.clone());
                graph.nodes.push(node);
            }
            if let Form::If {
                then, otherwise, ..
            } = &statement.form
            {
                self.walk(
                    then,
                    cleanup,
                    component,
                    Some((&statement.handle, Branch::True)),
                    graph,
                );
                self.walk(
                    otherwise.as_deref().unwrap_or_default(),
                    cleanup,
                    component,
                    Some((&statement.handle, Branch::False)),
                    graph,
                );
            }
        }
    }
    fn input(&mut self, input: &Input) -> Port {
        let mut port = Port::new(&input.name, value_type_from(&input.value_type));
        if let Some(value) = &input.default
            && let Some(value) = self.literal(value)
        {
            port.default_value = OptionalJson::Present(value);
        }
        let mut seen = HashSet::new();
        for attribute in &input.attributes {
            if !seen.insert(&attribute.name) {
                self.error(attribute.range.clone(), "Duplicate input attribute");
            }
            match attribute.name.as_str() {
                "label" => port.label = self.string_argument(attribute),
                "contract" => {
                    if let Some(name) = self.string_argument(attribute) {
                        port.contract_id = self.contract(&name, attribute.range.clone());
                    }
                }
                "optional" if attribute.arguments.is_empty() => port.required = Some(false),
                "hidden" if attribute.arguments.is_empty() => port.exposed = Some(false),
                "required" | "exposed" => {
                    if let [
                        Expr {
                            kind: ExprKind::Literal(Value::Bool(value)),
                            ..
                        },
                    ] = attribute.arguments.as_slice()
                    {
                        if attribute.name == "required" {
                            port.required = Some(*value);
                        } else {
                            port.exposed = Some(*value);
                        }
                    } else {
                        self.error(
                            attribute.range.clone(),
                            "Expected a boolean input attribute",
                        );
                    }
                }
                _ => self.error(
                    attribute.range.clone(),
                    "Unknown or misplaced input attribute",
                ),
            }
        }
        port
    }
    fn string_argument(&mut self, attribute: &Attribute) -> Option<String> {
        if let [
            Expr {
                kind: ExprKind::Literal(Value::String(value)),
                ..
            },
        ] = attribute.arguments.as_slice()
        {
            Some(value.clone())
        } else {
            self.error(
                attribute.range.clone(),
                "Expected one literal string argument",
            );
            None
        }
    }
    fn contract(&mut self, name: &str, range: Range<usize>) -> Option<String> {
        // IDs are stable and take precedence; names are accepted only if unique.
        let matches: Vec<_> = self
            .project
            .contracts
            .iter()
            .filter(|c| c.id == name)
            .collect();
        let matches = if matches.is_empty() {
            self.project
                .contracts
                .iter()
                .filter(|c| c.name == name)
                .collect()
        } else {
            matches
        };
        if let [contract] = matches.as_slice() {
            Some(contract.id.clone())
        } else {
            self.error(range, "Unknown or ambiguous contract");
            None
        }
    }
    fn literal(&mut self, expr: &Expr) -> Option<Value> {
        match &expr.kind {
            ExprKind::Literal(value) => Some(value.clone()),
            ExprKind::Group(value) => self.literal(value),
            ExprKind::Object(fields) => {
                let fields: Option<serde_json::Map<_, _>> = fields
                    .iter()
                    .map(|(k, v)| self.literal(v).map(|v| (k.clone(), v)))
                    .collect();
                fields.map(Value::Object)
            }
            ExprKind::Array(items) => items
                .iter()
                .map(|v| self.literal(v))
                .collect::<Option<Vec<_>>>()
                .map(Value::Array),
            _ => {
                self.error(expr.range.clone(), "Expected a literal JSON value");
                None
            }
        }
    }
    fn slot(&mut self, node: &mut Node, name: &str, value: &Expr, cleanup: bool) {
        if let Some(binding) = self.binding(value, cleanup) {
            self.record_slots(name, value, &binding);
            node.inputs.insert(name.into(), binding);
        }
    }
    /// Remembers where the input and, for an operator, each operand came from.
    fn record_slots(&mut self, input: &str, value: &Expr, binding: &Binding) {
        let key = |path: Vec<String>| (self.handle.clone(), input.to_owned(), path);
        self.slot_ranges.insert(key(vec![]), value.range.clone());
        let Binding::Expression { operator, .. } = binding else {
            return;
        };
        let operands = operator.spec().operands;
        let parts: Vec<(Option<&str>, &Expr)> = match &ungroup(value).kind {
            ExprKind::Binary { left, right, .. } => vec![(None, left), (None, right)],
            ExprKind::Not(base) => vec![(None, base)],
            ExprKind::Call { arguments, .. } => arguments
                .iter()
                .map(|argument| (argument.name.as_deref(), &argument.value))
                .collect(),
            _ => vec![],
        };
        for (index, (name, expr)) in parts.into_iter().enumerate() {
            if let Some(operand) = name.or_else(|| operands.get(index).map(|o| o.name)) {
                let slot = key(vec![operand.to_owned()]);
                self.slot_ranges.insert(slot, expr.range.clone());
            }
        }
    }
    fn warning(&mut self, range: Range<usize>, message: &'static str) {
        if self.diagnostics.len() < crate::MAX_DIAGNOSTICS {
            self.diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                ..Diagnostic::error(range, message)
            });
        }
    }
    /// The board's connection rule, applied to values written in code: a whole
    /// output must be compatible with the slot it fills, and a value with a
    /// statically known type (literal, operator, template, object, array) must
    /// have the slot's base type. The document model accepts mismatches, and runtime
    /// checks stay authoritative for dynamic values, so these are warnings.
    fn type_warnings(&mut self, document: &Document) {
        let graphs = match document {
            Document::Scenario(scenario) => vec![&scenario.graph, &scenario.cleanup],
            Document::Component(component) => vec![&component.graph],
        };
        let nodes: HashMap<&str, &Node> = graphs
            .iter()
            .flat_map(|graph| &graph.nodes)
            .map(|node| (node.id.as_str(), node))
            .collect();
        for node in graphs.iter().flat_map(|graph| &graph.nodes) {
            for slot in graph::input_slots(node, self.project) {
                let Some(binding) = node.inputs.get(&slot.input).and_then(|b| b.at(&slot.path))
                else {
                    continue;
                };
                let fits = match binding {
                    Binding::Reference {
                        node_id,
                        port,
                        path,
                    } if path.is_empty() => nodes.get(node_id.as_str()).is_none_or(|source| {
                        graph::output_ports(source, self.project)
                            .iter()
                            .find(|output| output.name == *port)
                            .is_none_or(|output| graph::compatible(output, &slot.port))
                    }),
                    Binding::Reference { .. } => true,
                    // Null and dynamic sources (parameters, environment,
                    // arguments) report `any`, as do operators returning any.
                    Binding::Literal { value } if value.is_null() => true,
                    _ => {
                        let found = graph::binding_type(binding);
                        found == ValueType::Any
                            || slot.port.value_type == ValueType::Any
                            || found == slot.port.value_type
                    }
                };
                if !fits {
                    let range = self
                        .slot_ranges
                        .get(&(node.handle.clone(), slot.input.clone(), slot.path.clone()))
                        .or_else(|| self.ranges.get(&node.id))
                        .cloned()
                        .unwrap_or_default();
                    self.warning(range, "The value's type does not match this input");
                }
            }
        }
    }
    fn check(
        &mut self,
        node: &mut Node,
        check: &Check,
        actual: &str,
        expected: &str,
        cleanup: bool,
    ) {
        self.slot(node, actual, &check.actual, cleanup);
        if let Some(value) = &check.expected {
            self.slot(node, expected, value, cleanup);
        }
        if let Some(value) = &check.tolerance {
            if let Some(number) = self.literal(value).and_then(|v| v.as_f64()) {
                if node
                    .config
                    .tolerance
                    .is_some_and(|previous| previous != number)
                {
                    self.error(value.range.clone(), "Condition checks share one tolerance");
                }
                node.config.tolerance = Some(number);
            } else {
                self.error(value.range.clone(), "Tolerance must be a number");
            }
        }
    }
    fn binding(&mut self, expr: &Expr, cleanup: bool) -> Option<Binding> {
        let value = match &expr.kind {
            ExprKind::Group(value) => return self.binding(value, cleanup),
            ExprKind::Missing => return None,
            ExprKind::Literal(value) => Binding::from_json(value.clone()),
            ExprKind::Object(fields) => Binding::Object {
                fields: fields
                    .iter()
                    .filter_map(|(k, v)| {
                        if matches!(ungroup(v).kind, ExprKind::Missing) {
                            self.error(v.range.clone(), "Object fields cannot be unconnected");
                        }
                        self.binding(v, cleanup).map(|v| (k.clone(), v))
                    })
                    .collect(),
            },
            ExprKind::Array(items) => Binding::Array {
                items: items
                    .iter()
                    .filter_map(|v| {
                        if matches!(ungroup(v).kind, ExprKind::Missing) {
                            self.error(v.range.clone(), "Array items cannot be unconnected");
                        }
                        self.binding(v, cleanup)
                    })
                    .collect(),
            },
            ExprKind::Template(parts) => Binding::Template {
                parts: parts
                    .iter()
                    .filter_map(|p| match p {
                        Part::Text(text) => Some(TemplatePart::Text { text: text.clone() }),
                        Part::Value(value) => {
                            if matches!(ungroup(value).kind, ExprKind::Missing) {
                                self.error(
                                    value.range.clone(),
                                    "Template values cannot be unconnected",
                                );
                            }
                            self.binding(value, cleanup)
                                .map(|binding| TemplatePart::Value { binding })
                        }
                    })
                    .collect(),
            },
            ExprKind::Name(_) | ExprKind::Field { .. } => return self.reference(expr, cleanup),
            ExprKind::Call {
                name,
                revision,
                arguments,
            } => {
                if revision.is_some() {
                    self.error(expr.range.clone(), "Operator calls cannot name a revision");
                }
                let Some(operator) = operator_named(name.strip_prefix("op.").unwrap_or(name))
                else {
                    self.error(
                        expr.range.clone(),
                        "Unknown operator or nested component call",
                    );
                    return None;
                };
                let spec = operator.spec();
                let mut inputs = Bindings::new();
                for (index, argument) in arguments.iter().enumerate() {
                    let name = argument
                        .name
                        .as_deref()
                        .or_else(|| spec.operands.get(index).map(|p| p.name));
                    let Some(name) = name.filter(|name| spec.operand(name).is_some()) else {
                        self.error(
                            argument.value.range.clone(),
                            "Unknown or excess operator operand",
                        );
                        continue;
                    };
                    if inputs.contains_key(name) {
                        self.error(argument.value.range.clone(), "Duplicate operator operand");
                    }
                    if let Some(value) = self.binding(&argument.value, cleanup) {
                        inputs.insert(name.into(), value);
                    }
                }
                Binding::Expression { operator, inputs }
            }
            ExprKind::Binary {
                operator,
                left,
                right,
            } => {
                let Some(operator) = binary_operator(operator) else {
                    self.error(expr.range.clone(), "Unknown binary operator");
                    return None;
                };
                let mut inputs = Bindings::new();
                if let Some(value) = self.binding(left, cleanup) {
                    inputs.insert("left".into(), value);
                }
                if let Some(value) = self.binding(right, cleanup) {
                    inputs.insert("right".into(), value);
                }
                Binding::Expression { operator, inputs }
            }
            ExprKind::Not(value) => Binding::Expression {
                operator: Operator::Not,
                inputs: self
                    .binding(value, cleanup)
                    .map(|v| ("left".into(), v))
                    .into_iter()
                    .collect(),
            },
        };
        Some(value.normalized())
    }
    fn reference(&mut self, expr: &Expr, cleanup: bool) -> Option<Binding> {
        let (name, mut fields) = reference_parts(expr);
        let Some(name) = name else {
            self.error(
                expr.range.clone(),
                "A path must start at a block, parameter or environment name",
            );
            return None;
        };
        if name == "input" {
            if fields.is_empty() {
                self.error(expr.range.clone(), "Expected a request argument name");
                return None;
            }
            let key = fields.remove(0);
            if self
                .arguments
                .as_ref()
                .is_none_or(|names| !names.contains(&key))
            {
                self.error(expr.range.clone(), "Unknown request argument");
                return None;
            }
            return Some(Binding::Argument {
                name: key,
                path: field_path(&fields),
            });
        }
        if name == "param" || name == "env" {
            if fields.is_empty() {
                self.error(
                    expr.range.clone(),
                    "Expected a parameter or environment key",
                );
                return None;
            }
            let key = fields.remove(0);
            if name == "param" {
                if !self.parameters.contains(&key) {
                    self.error(expr.range.clone(), "Unknown parameter");
                    return None;
                }
                return Some(Binding::parameter_field(key, field_path(&fields)));
            }
            if !fields.is_empty() {
                self.error(expr.range.clone(), "Environment values do not have fields");
                return None;
            }
            if !self.allow_unresolved_environment
                && key != "baseUrl"
                && !self.project.environment.variables.contains_key(&key)
                && !self.project.environment.secret_names.contains(&key)
            {
                self.error(expr.range.clone(), "Unknown environment name");
                return None;
            }
            return Some(Binding::environment(key));
        }
        let Some(node) = self.nodes.get(name) else {
            self.error(expr.range.clone(), "Unknown block handle");
            return None;
        };
        if !cleanup && self.scopes.get(name) == Some(&true) {
            self.error(expr.range.clone(), "Main flow cannot reference cleanup");
            return None;
        }
        let ports = graph::output_ports(node, self.project);
        let port = if let [port] = ports.as_slice() {
            port.name.clone()
        } else if !fields.is_empty() {
            fields.remove(0)
        } else {
            self.error(
                expr.range.clone(),
                "A block with multiple outputs needs an output name",
            );
            return None;
        };
        if !ports.iter().any(|p| p.name == port) {
            self.error(expr.range.clone(), "Unknown block output");
            return None;
        }
        Some(Binding::reference(name, port, field_path(&fields)))
    }
}
fn add_edge(graph: &mut Graph, source: &str, target: &str, branch: Option<Branch>) {
    if !graph
        .edges
        .iter()
        .any(|e| e.source == source && e.target == target && e.branch == branch)
    {
        graph.edges.push(OrderEdge {
            id: format!("order-{}", graph.edges.len()),
            source: source.into(),
            target: target.into(),
            branch,
        });
    }
}
pub(crate) fn ungroup(mut expr: &Expr) -> &Expr {
    while let ExprKind::Group(inner) = &expr.kind {
        expr = inner;
    }
    expr
}
fn reference_parts(expr: &Expr) -> (Option<&str>, Vec<String>) {
    let mut fields = Vec::new();
    let mut current = ungroup(expr);
    while let ExprKind::Field { base, key, .. } = &current.kind {
        fields.push(key.clone());
        current = ungroup(base);
    }
    fields.reverse();
    (
        if let ExprKind::Name(name) = &current.kind {
            Some(name.as_str())
        } else {
            None
        },
        fields,
    )
}
fn field_path(fields: &[String]) -> String {
    let mut path = JsonPath::root();
    for field in fields {
        path.push_key(field);
    }
    path.to_string()
}
pub(crate) fn value_type_from(name: &str) -> ValueType {
    match name {
        "string" => ValueType::String,
        "number" => ValueType::Number,
        "boolean" => ValueType::Boolean,
        "object" => ValueType::Object,
        "array" => ValueType::Array,
        _ => ValueType::Any,
    }
}
pub(crate) fn predicate(name: &str) -> Predicate {
    match name {
        "!=" => Predicate::NotEquals,
        ">" => Predicate::Greater,
        "<" => Predicate::Less,
        "exists" => Predicate::Exists,
        "nonempty" => Predicate::NotEmpty,
        "contains" => Predicate::Contains,
        "is" => Predicate::Type,
        "~=" => Predicate::Approximately,
        _ => Predicate::Equals,
    }
}
pub(crate) fn operator_named(name: &str) -> Option<Operator> {
    Operator::ALL.into_iter().find(|op| {
        serde_json::to_value(op)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .as_deref()
            == Some(name)
    })
}
pub(crate) fn binary_operator(name: &str) -> Option<Operator> {
    Some(match name {
        "+" => Operator::Add,
        "-" => Operator::Subtract,
        "*" => Operator::Multiply,
        "/" => Operator::Divide,
        "%" => Operator::Modulo,
        "==" => Operator::Equals,
        "!=" => Operator::NotEquals,
        ">" => Operator::Greater,
        "<" => Operator::Less,
        ">=" => Operator::AtLeast,
        "<=" => Operator::AtMost,
        "&&" => Operator::And,
        "||" => Operator::Or,
        _ => return None,
    })
}
