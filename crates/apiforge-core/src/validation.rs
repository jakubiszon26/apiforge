//! One semantic validator shared by every document entry point.
use crate::{format::*, model::*};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};

type Result = std::result::Result<(), ImportError>;
fn require(condition: bool, path: &str, message: &'static str) -> Result {
    if condition {
        Ok(())
    } else {
        Err(ImportError::invalid(path, message))
    }
}
fn id(value: &str, path: &str) -> Result {
    require(
        !value.is_empty()
            && value.encode_utf16().count() <= 200
            && !["__proto__", "constructor", "prototype"].contains(&value),
        path,
        "Invalid identifier",
    )
}
fn text(value: &str, path: &str) -> Result {
    require(
        value.encode_utf16().count() <= 100_000,
        path,
        "Text exceeds 100000 UTF-16 units",
    )
}
fn json(value: &Value, depth: usize, path: &str) -> Result {
    require(depth < 40, path, "JSON exceeds maximum nesting depth")?;
    match value {
        Value::String(value) => text(value, path),
        Value::Array(values) => {
            require(values.len() <= 10_000, path, "Too many JSON array entries")?;
            for value in values {
                json(value, depth + 1, path)?;
            }
            Ok(())
        }
        Value::Object(values) => {
            require(values.len() <= 10_000, path, "Too many JSON object fields")?;
            for (key, value) in values {
                text(key, path)?;
                require(
                    !["__proto__", "constructor", "prototype"].contains(&key.as_str()),
                    path,
                    "Reserved JSON key",
                )?;
                json(value, depth + 1, path)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
fn optional_json(value: &OptionalJson, path: &str) -> Result {
    if let OptionalJson::Present(value) = value {
        json(value, 0, path)?;
    }
    Ok(())
}
fn unique<'a>(values: impl IntoIterator<Item = &'a str>, path: &str) -> Result {
    let mut seen = HashSet::new();
    for value in values {
        id(value, path)?;
        require(seen.insert(value), path, "Duplicate identifier")?;
    }
    Ok(())
}
fn ports(values: &[Port], path: &str) -> Result {
    require(values.len() <= 100, path, "Too many ports")?;
    unique(values.iter().map(|port| port.name.as_str()), path)?;
    for port in values {
        if let Some(value) = &port.label {
            text(value, path)?;
        }
        if let Some(value) = &port.contract_id {
            id(value, path)?;
        }
        optional_json(&port.default_value, path)?;
    }
    Ok(())
}

pub(crate) fn workspace(workspace: &Workspace) -> Result {
    require(
        workspace.version == 1,
        "version",
        "Unsupported workspace version",
    )?;
    require(
        !workspace.projects.is_empty() && workspace.projects.len() <= 100,
        "projects",
        "Expected 1..100 projects",
    )?;
    unique(
        workspace.projects.iter().map(|project| project.id.as_str()),
        "projects",
    )?;
    require(
        workspace
            .projects
            .iter()
            .any(|project| project.id == workspace.active_id),
        "activeId",
        "Active project does not exist",
    )?;
    for (index, project) in workspace.projects.iter().enumerate() {
        validate_project(project, &format!("projects[{index}]"))?;
    }
    Ok(())
}
fn validate_project(project: &Project, path: &str) -> Result {
    require(project.version == 1, path, "Unsupported project version")?;
    text(&project.name, path)?;
    require(
        !project.scenarios.is_empty() && project.scenarios.len() <= 100,
        path,
        "Expected 1..100 scenarios",
    )?;
    require(
        project.components.len() <= 200 && project.contracts.len() <= 200,
        path,
        "Too many definitions",
    )?;
    unique(
        project
            .scenarios
            .iter()
            .map(|scenario| scenario.id.as_str()),
        path,
    )?;
    unique(
        project
            .contracts
            .iter()
            .map(|contract| contract.id.as_str()),
        path,
    )?;
    text(&project.environment.base_url, path)?;
    unique(
        project.environment.secret_names.iter().map(String::as_str),
        path,
    )?;
    for (name, value) in &project.environment.variables {
        id(name, path)?;
        text(value, path)?;
    }
    for contract in &project.contracts {
        text(&contract.name, path)?;
        json(&contract.sample, 0, path)?;
    }
    let mut revisions = HashSet::new();
    for (index, component) in project.components.iter().enumerate() {
        let path = format!("{path}.components[{index}]");
        id(&component.id, &path)?;
        text(&component.name, &path)?;
        text(&component.description, &path)?;
        require(
            component.revision > 0 && revisions.insert((&component.id, component.revision)),
            &path,
            "Duplicate or invalid component revision",
        )?;
        ports(&component.inputs, &path)?;
        require(component.outputs.len() <= 100, &path, "Too many outputs")?;
        unique(
            component.outputs.iter().map(|output| output.name.as_str()),
            &path,
        )?;
        let parameters: HashSet<&str> = component
            .inputs
            .iter()
            .map(|input| input.name.as_str())
            .collect();
        graph(&component.graph, &[], &parameters, project, &path)?;
        let nodes: HashMap<&str, &Node> = component
            .graph
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect();
        for output in &component.outputs {
            if let Some(label) = &output.label {
                text(label, &path)?;
            }
            if let Some(contract) = &output.contract_id {
                id(contract, &path)?;
            }
            optional_json(&output.default_value, &path)?;
            binding(
                &output.binding,
                &nodes,
                &parameters,
                project,
                0,
                &mut 0,
                &path,
            )?;
        }
        if let Some(action) = &component.action {
            require(
                parameters.contains(action.input.as_str()),
                &path,
                "Action input does not exist",
            )?;
            if let Some(contract) = &action.contract_id {
                id(contract, &path)?;
            }
        }
    }
    for (index, scenario) in project.scenarios.iter().enumerate() {
        let path = format!("{path}.scenarios[{index}]");
        text(&scenario.name, &path)?;
        text(&scenario.description, &path)?;
        require(
            scenario.parameters.len() <= 100,
            &path,
            "Too many parameters",
        )?;
        for (name, value) in &scenario.parameters {
            id(name, &path)?;
            json(value, 0, &path)?;
        }
        let parameters = scenario.parameters.keys().map(String::as_str).collect();
        graph(
            &scenario.graph,
            &[],
            &parameters,
            project,
            &format!("{path}.graph"),
        )?;
        graph(
            &scenario.cleanup,
            &scenario.graph.nodes,
            &parameters,
            project,
            &format!("{path}.cleanup"),
        )?;
    }
    let mut visiting = HashSet::new();
    let mut done = HashMap::new();
    for index in 0..project.components.len() {
        component_dag(index, project, &mut visiting, &mut done, 0, path)?;
    }
    Ok(())
}
fn component_dag(
    index: usize,
    project: &Project,
    visiting: &mut HashSet<usize>,
    heights: &mut HashMap<usize, usize>,
    depth: usize,
    path: &str,
) -> std::result::Result<usize, ImportError> {
    require(
        !visiting.contains(&index),
        path,
        "Recursive component definitions",
    )?;
    require(depth < 16, path, "Component nesting exceeds 16 levels")?;
    if let Some(&height) = heights.get(&index) {
        require(
            depth + height <= 16,
            path,
            "Component nesting exceeds 16 levels",
        )?;
        return Ok(height);
    }
    visiting.insert(index);
    let mut height = 1;
    for node in &project.components[index].graph.nodes {
        if node.kind == NodeKind::Component {
            let target = project.components.iter().position(|component| {
                Some(&component.id) == node.config.component_id.as_ref()
                    && Some(component.revision) == node.config.revision
            });
            if let Some(target) = target {
                height = height
                    .max(1 + component_dag(target, project, visiting, heights, depth + 1, path)?);
            }
        }
    }
    visiting.remove(&index);
    heights.insert(index, height);
    Ok(height)
}
fn output_exists(node: &Node, port: &str, project: &Project) -> bool {
    match node.kind {
        NodeKind::Trigger => false,
        NodeKind::Request => ["body", "status", "headers", "duration"].contains(&port),
        NodeKind::If | NodeKind::Assert | NodeKind::Condition => {
            ["value", "passed"].contains(&port)
        }
        NodeKind::Component => project.components.iter().any(|component| {
            Some(&component.id) == node.config.component_id.as_ref()
                && Some(component.revision) == node.config.revision
                && component.outputs.iter().any(|output| output.name == port)
        }),
        _ => port == "value",
    }
}
fn binding(
    value: &Binding,
    nodes: &HashMap<&str, &Node>,
    parameters: &HashSet<&str>,
    project: &Project,
    depth: usize,
    count: &mut usize,
    path: &str,
) -> Result {
    *count += 1;
    require(
        depth < 12 && *count <= 200,
        path,
        "Binding exceeds nesting or item limit",
    )?;
    match value {
        Binding::Literal { value } => json(value, 0, path),
        Binding::Expression { operator, inputs } => {
            require(
                inputs
                    .keys()
                    .all(|key| operator.spec().operand(key).is_some()),
                path,
                "Unknown operator argument",
            )?;
            for child in inputs.values() {
                binding(child, nodes, parameters, project, depth + 1, count, path)?;
            }
            Ok(())
        }
        Binding::Reference {
            node_id,
            port,
            path: field,
        } => {
            id(node_id, path)?;
            id(port, path)?;
            text(field, path)?;
            require(
                nodes
                    .get(node_id.as_str())
                    .is_some_and(|source| output_exists(source, port, project)),
                path,
                "Reference source or output does not exist",
            )
        }
        Binding::Parameter { name } => {
            id(name, path)?;
            require(
                parameters.contains(name.as_str()),
                path,
                "Parameter does not exist",
            )
        }
        Binding::Environment { name } => id(name, path),
    }
}
fn graph(
    graph: &Graph,
    external: &[Node],
    parameters: &HashSet<&str>,
    project: &Project,
    path: &str,
) -> Result {
    require(
        graph.nodes.len() <= 200 && graph.edges.len() <= 1000,
        path,
        "Graph exceeds node or edge limit",
    )?;
    unique(
        graph
            .nodes
            .iter()
            .chain(external)
            .map(|node| node.id.as_str()),
        path,
    )?;
    unique(graph.edges.iter().map(|edge| edge.id.as_str()), path)?;
    require(
        graph
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Trigger)
            .count()
            <= 1,
        path,
        "Only one Start is allowed",
    )?;
    let nodes: HashMap<&str, &Node> = graph
        .nodes
        .iter()
        .chain(external)
        .map(|node| (node.id.as_str(), node))
        .collect();
    let local: HashSet<&str> = graph.nodes.iter().map(|node| node.id.as_str()).collect();
    let mut dependencies: HashMap<&str, HashSet<&str>> =
        local.iter().map(|id| (*id, HashSet::new())).collect();
    for (index, node) in graph.nodes.iter().enumerate() {
        let node_path = format!("{path}.nodes[{index}]");
        text(&node.name, &node_path)?;
        require(
            node.position.x.is_finite() && node.position.y.is_finite(),
            &node_path,
            "Non-finite position",
        )?;
        require(node.inputs.len() <= 100, &node_path, "Too many inputs")?;
        if node.kind == NodeKind::Trigger {
            require(
                node.inputs.is_empty(),
                &node_path,
                "Start cannot have inputs",
            )?;
        }
        config(node, project, &node_path)?;
        for (name, value) in &node.inputs {
            id(name, &node_path)?;
            binding(value, &nodes, parameters, project, 0, &mut 0, &node_path)?;
            let mut references = Vec::new();
            value.references(&mut references);
            for (source, _) in references {
                if local.contains(source) {
                    dependencies.entry(&node.id).or_default().insert(source);
                }
            }
        }
    }
    for (index, edge) in graph.edges.iter().enumerate() {
        let edge_path = format!("{path}.edges[{index}]");
        id(&edge.source, &edge_path)?;
        id(&edge.target, &edge_path)?;
        require(
            nodes.contains_key(edge.source.as_str()) && local.contains(edge.target.as_str()),
            &edge_path,
            "Control edge endpoint does not exist",
        )?;
        if let Some(source) = nodes.get(edge.source.as_str()) {
            require(
                (source.kind == NodeKind::If) == edge.branch.is_some(),
                &edge_path,
                "Only IF outputs require a true/false branch",
            )?;
        }
        require(
            nodes
                .get(edge.target.as_str())
                .is_none_or(|node| node.kind != NodeKind::Trigger),
            &edge_path,
            "Start cannot have incoming edges",
        )?;
        if local.contains(edge.source.as_str()) {
            dependencies
                .entry(&edge.target)
                .or_default()
                .insert(&edge.source);
        }
    }
    // Iterative topological validation keeps adversarial input off the call stack.
    let mut ready: VecDeque<&str> = dependencies
        .iter()
        .filter_map(|(id, deps)| deps.is_empty().then_some(*id))
        .collect();
    let mut completed = HashSet::new();
    while let Some(id) = ready.pop_front() {
        if !completed.insert(id) {
            continue;
        }
        for (target, deps) in &mut dependencies {
            if deps.remove(id) && deps.is_empty() {
                ready.push_back(target);
            }
        }
    }
    require(
        completed.len() == graph.nodes.len(),
        path,
        "Graph contains a data/control cycle",
    )
}
fn config(node: &Node, project: &Project, path: &str) -> Result {
    let c = &node.config;
    for value in [&c.url, &c.path, &c.contract_id, &c.component_id]
        .into_iter()
        .flatten()
    {
        text(value, path)?;
    }
    for map in [&c.headers, &c.query].into_iter().flatten() {
        require(map.len() <= 100, path, "Too many request fields")?;
        for (key, value) in map {
            text(key, path)?;
            text(value, path)?;
        }
    }
    if let Some(timeout) = c.timeout_ms {
        require(
            timeout.is_finite() && timeout > 0.0 && timeout <= 60_000.0,
            path,
            "Request timeout must be in (0, 60000] ms",
        )?;
    }
    if let Some(status) = c.sample_status {
        require(
            (100..=599).contains(&status),
            path,
            "Invalid sample HTTP status",
        )?;
    }
    if let Some(tolerance) = c.tolerance {
        require(
            tolerance.is_finite() && tolerance >= 0.0,
            path,
            "Invalid tolerance",
        )?;
    }
    optional_json(&c.sample_response, path)?;
    if let Some(values) = &c.input_ports {
        ports(values, path)?;
    }
    if let Some(values) = &c.template_inputs {
        ports(values, path)?;
    }
    if let Some(predicates) = &c.predicates {
        require(predicates.len() <= 100, path, "Too many predicates")?;
        for predicate in predicates {
            id(&predicate.input, path)?;
            id(&predicate.expected, path)?;
            if let Some(field) = &predicate.path {
                text(field, path)?;
            }
        }
    }
    if node.kind == NodeKind::Component {
        require(
            project.components.iter().any(|component| {
                Some(&component.id) == c.component_id.as_ref()
                    && Some(component.revision) == c.revision
            }),
            path,
            "Component revision does not exist",
        )?;
    }
    Ok(())
}
