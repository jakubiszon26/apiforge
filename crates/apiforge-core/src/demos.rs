//! Built-in demo projects and empty documents for new work.
//!
//! The demos match the project fixtures node for node: the same kinds, bindings,
//! ordering edges, components and contracts, with English texts. Their requests target the local demo API (`apiforge-demo-api`) on
//! `127.0.0.1:4318`. Structural drift is caught by tests against the fixtures.

use crate::{
    format::{
        Binding, Component, ComponentAction, ConditionPredicate, Contract, Environment, Graph,
        Node, NodeConfig, OptionalJson, OrderEdge, Port, Position, Project, Scenario, TemplatePart,
    },
    graph::{create_node, export_position},
    handle::handle_base,
    ids::new_id,
    model::{Branch, ErrorPolicy, HttpMethod, NodeKind, Predicate, ValueType},
    operator::Operator,
};
use indexmap::IndexMap;
use serde_json::{Value, json};

/// Base URL of the demo shop served by the local demo API.
pub const SHOP_BASE_URL: &str = "http://127.0.0.1:4318/demo/shop";
/// Base URL of the demo ticket system served by the local demo API.
pub const SUPPORT_BASE_URL: &str = "http://127.0.0.1:4318/demo/support";
/// Name of a scenario created without an explicit name.
pub const DEFAULT_SCENARIO_NAME: &str = "New scenario";
/// Name of a new, empty project.
pub const DEFAULT_PROJECT_NAME: &str = "My API project";

/// Every demo project, in library order.
pub fn demo_projects() -> Vec<Project> {
    vec![
        shop_project(),
        tickets_project(),
        blocks_project(),
        workflow_project(),
    ]
}

/// An empty scenario holding only a Start node.
pub fn empty_scenario(name: impl Into<String>) -> Scenario {
    Scenario {
        id: new_id("scenario"),
        name: name.into(),
        description: String::new(),
        graph: Graph {
            nodes: vec![create_node(NodeKind::Start, at(80.0, 60.0), None)],
            edges: vec![],
        },
        cleanup: empty_graph(),
        parameters: IndexMap::new(),
        error_policy: ErrorPolicy::Continue,
    }
}

/// A new project with one empty scenario and no environment.
pub fn empty_project() -> Project {
    Project {
        version: crate::format::FORMAT_VERSION,
        id: new_id("project"),
        name: DEFAULT_PROJECT_NAME.into(),
        scenarios: vec![empty_scenario(DEFAULT_SCENARIO_NAME)],
        components: vec![],
        contracts: vec![],
        environment: environment(""),
    }
}

/// A shop scenario built from reusable components: product lookup, an action that
/// checks the promotion and a cart component, followed by price assertions and a
/// cleanup phase that deletes the cart.
pub fn shop_project() -> Project {
    let product_component = Definition {
        id: "get-product",
        revision: 1,
        name: "Product",
        description: "Fetch a product by ID together with its promotion.",
        inputs: vec![Port {
            default_value: OptionalJson::Present(json!(123)),
            exposed: Some(true),
            ..Port::new("id", ValueType::Number)
                .labelled("Product ID")
                .required(true)
        }],
        outputs: vec![output(
            "product",
            Some("Product"),
            ValueType::Object,
            Some("Product"),
            reference("fetch", "body", ""),
        )],
        graph: graph(vec![request(
            "fetch",
            "Fetch product",
            "/products/:id",
            vec![("id", parameter("id"))],
            product_sample(),
            HttpMethod::Get,
            at(80.0, 80.0),
        )]),
        action: None,
    }
    .build();
    let active_component = Definition {
        id: "active-promotion",
        revision: 1,
        name: "Active promotion",
        description: "Check that the product has an active promotion.",
        inputs: vec![
            contract_port("product", "Product")
                .labelled("Product")
                .required(true),
        ],
        outputs: vec![output(
            "product",
            None,
            ValueType::Object,
            Some("Product"),
            parameter("product"),
        )],
        graph: graph(vec![
            value(
                "active",
                "Promotion status",
                at(80.0, 80.0),
                Binding::parameter_field("product", "promotion.active"),
            ),
            node(
                "check",
                NodeKind::Condition,
                "Promotion must be active",
                at(400.0, 80.0),
                vec![
                    ("actual1", reference("active", "value", "")),
                    ("expected1", literal(json!(true))),
                ],
                NodeConfig {
                    predicates: Some(vec![ConditionPredicate {
                        predicate: Predicate::Equals,
                    }]),
                    ..NodeConfig::default()
                },
            ),
        ]),
        action: Some(object_action("product", "Product")),
    }
    .build();
    let cart_sample = json!({ "id": "sample-cart", "productId": 123, "quantity": 1, "total": 80 });
    let cart_component = Definition {
        id: "product-cart",
        revision: 1,
        name: "Add to cart",
        description: "Create a cart with the product and fetch its calculated price.",
        inputs: vec![
            contract_port("product", "Product").required(true),
            Port {
                default_value: OptionalJson::Present(json!(1)),
                exposed: Some(true),
                ..Port::new("quantity", ValueType::Number)
                    .labelled("Quantity")
                    .required(true)
            },
        ],
        outputs: vec![output(
            "cart",
            Some("Cart"),
            ValueType::Object,
            Some("Cart"),
            reference("read", "body", ""),
        )],
        graph: graph(vec![
            value(
                "id",
                "Product ID",
                at(50.0, 90.0),
                Binding::parameter_field("product", "id"),
            ),
            value(
                "body",
                "Cart item",
                at(350.0, 90.0),
                object(vec![
                    ("productId", reference("id", "value", "")),
                    ("quantity", parameter("quantity")),
                ]),
            ),
            request(
                "create",
                "Create cart",
                "/carts",
                vec![("body", reference("body", "value", ""))],
                cart_sample.clone(),
                HttpMethod::Post,
                at(650.0, 90.0),
            ),
            request(
                "read",
                "Fetch cart",
                "/carts/:id",
                vec![("id", reference("create", "body", "id"))],
                cart_sample,
                HttpMethod::Get,
                at(950.0, 90.0),
            ),
        ]),
        action: Some(object_action("product", "Product")),
    }
    .build();

    let nodes = vec![
        component_node(
            "product",
            "Product",
            "get-product",
            at(40.0, 160.0),
            vec![("id", literal(json!(123)))],
        ),
        component_node(
            "active",
            "Active promotion",
            "active-promotion",
            at(360.0, 30.0),
            vec![("product", reference("product", "product", ""))],
        ),
        value(
            "expected",
            "Expected price",
            at(360.0, 290.0),
            op(
                Operator::Multiply,
                literal(json!(80)),
                Some(literal(json!(1))),
            ),
        ),
        component_node(
            "cart",
            "Add to cart",
            "product-cart",
            at(680.0, 30.0),
            vec![
                ("product", reference("active", "product", "")),
                ("quantity", literal(json!(1))),
            ],
        ),
        node(
            "price",
            NodeKind::Assert,
            "Promotional price",
            at(1000.0, 160.0),
            vec![
                ("actual", reference("cart", "cart", "total")),
                ("expected", reference("expected", "value", "")),
            ],
            NodeConfig {
                predicate: Some(Predicate::Approximately),
                tolerance: Some(0.005),
                ..NodeConfig::default()
            },
        ),
        node(
            "field",
            NodeKind::Assert,
            "Field inna_nazwa",
            at(680.0, 290.0),
            vec![(
                "actual",
                reference("product", "product", "promotion.inna_nazwa"),
            )],
            predicate(Predicate::NotEmpty),
        ),
    ];
    let cleanup = vec![
        request(
            "delete-cart",
            "Delete cart",
            "/carts/:id",
            vec![("id", reference("cart", "cart", "id"))],
            json!({ "deleted": true }),
            HttpMethod::Delete,
            at(80.0, 80.0),
        ),
        node(
            "verify-cleanup",
            NodeKind::Assert,
            "Cart was deleted",
            at(400.0, 80.0),
            vec![
                ("actual", reference("delete-cart", "status", "")),
                ("expected", literal(json!(200))),
            ],
            predicate(Predicate::Equals),
        ),
    ];

    Project {
        version: crate::format::FORMAT_VERSION,
        id: "shop".into(),
        name: "Demo shop".into(),
        contracts: vec![
            contract("Product", "Product", product_sample()),
            contract("Cart", "Cart", json!({ "id": "cart-1", "total": 80 })),
        ],
        components: vec![product_component, active_component, cart_component],
        environment: environment(SHOP_BASE_URL),
        scenarios: vec![Scenario {
            id: "promotion".into(),
            name: "Promotion and cart price".into(),
            description: "Check the promotion, the final price and an extra field. \
                          The demo data can be run against the local demo API."
                .into(),
            graph: graph(nodes),
            cleanup: graph(cleanup),
            parameters: IndexMap::new(),
            error_policy: ErrorPolicy::Continue,
        }],
    }
}

/// A second project with the same building blocks against different endpoints:
/// a ticket lookup component and an action that checks the ticket status.
pub fn tickets_project() -> Project {
    let sample = ticket_sample();
    let component = Definition {
        id: "get-ticket",
        revision: 1,
        name: "Ticket",
        description: "Read a ticket by its identifier.",
        inputs: vec![Port {
            default_value: OptionalJson::Present(json!(7)),
            exposed: Some(true),
            ..Port::new("id", ValueType::Number).required(true)
        }],
        outputs: vec![output(
            "ticket",
            None,
            ValueType::Object,
            Some("Ticket"),
            reference("fetch", "body", ""),
        )],
        graph: graph(vec![request(
            "fetch",
            "Fetch ticket",
            "/tickets/:id",
            vec![("id", parameter("id"))],
            sample.clone(),
            HttpMethod::Get,
            at(80.0, 80.0),
        )]),
        action: None,
    }
    .build();
    let action = Definition {
        id: "ticket-open",
        revision: 1,
        name: "Check open ticket",
        description: "Check the ticket status.",
        inputs: vec![contract_port("ticket", "Ticket").required(true)],
        outputs: vec![output(
            "passed",
            None,
            ValueType::Boolean,
            None,
            reference("check", "passed", ""),
        )],
        graph: graph(vec![
            value(
                "status",
                "Read status",
                at(60.0, 60.0),
                Binding::parameter_field("ticket", "status"),
            ),
            node(
                "check",
                NodeKind::Assert,
                "Status is open",
                at(380.0, 60.0),
                vec![
                    ("actual", reference("status", "value", "")),
                    ("expected", literal(json!("open"))),
                ],
                predicate(Predicate::Equals),
            ),
        ]),
        action: Some(object_action("ticket", "Ticket")),
    }
    .build();
    let nodes = vec![
        component_node(
            "ticket",
            "Ticket",
            "get-ticket",
            at(60.0, 160.0),
            vec![("id", literal(json!(7)))],
        ),
        component_node(
            "open",
            "Check open ticket",
            "ticket-open",
            at(410.0, 60.0),
            vec![("ticket", reference("ticket", "ticket", ""))],
        ),
        node(
            "priority",
            NodeKind::Assert,
            "Priority is high",
            at(410.0, 310.0),
            vec![
                ("actual", reference("ticket", "ticket", "priority")),
                ("expected", literal(json!("high"))),
            ],
            predicate(Predicate::Equals),
        ),
    ];

    Project {
        version: crate::format::FORMAT_VERSION,
        id: "tickets".into(),
        name: "Ticket system".into(),
        environment: environment(SUPPORT_BASE_URL),
        contracts: vec![contract("Ticket", "Ticket", sample)],
        components: vec![component, action],
        scenarios: vec![Scenario {
            id: "ticket-status".into(),
            name: "Ticket status and priority".into(),
            description: "A second project: the same operations, different endpoints \
                          and its own actions."
                .into(),
            parameters: IndexMap::new(),
            error_policy: ErrorPolicy::Continue,
            cleanup: empty_graph(),
            graph: graph(nodes),
        }],
    }
}

/// A complete test assembled from value-producing operator blocks, with one generic
/// comparison per rule.
pub fn blocks_project() -> Project {
    let field = |path: &str| reference("response", "body", path);
    let source = request(
        "response",
        "Read API data",
        "/products/:code",
        vec![("code", literal(json!(123)))],
        product_sample(),
        HttpMethod::Get,
        at(60.0, 170.0),
    );
    let named = node(
        "name-check",
        NodeKind::Assert,
        "Name is a non-empty string",
        at(470.0, 30.0),
        vec![
            (
                "actual",
                op(
                    Operator::And,
                    op(
                        Operator::Equals,
                        op(Operator::TypeOf, field("name"), None),
                        Some(literal(json!("string"))),
                    ),
                    Some(op(
                        Operator::Greater,
                        op(Operator::Length, field("name"), None),
                        Some(literal(json!(0))),
                    )),
                ),
            ),
            ("expected", literal(json!(true))),
        ],
        predicate(Predicate::Equals),
    );
    let discount = node(
        "discount-check",
        NodeKind::Assert,
        "Active promotion takes at least 10% off",
        at(470.0, 350.0),
        vec![
            (
                "actual",
                op(
                    Operator::And,
                    field("promotion.active"),
                    Some(op(
                        Operator::AtMost,
                        field("promotion.price"),
                        Some(op(
                            Operator::Multiply,
                            field("price"),
                            Some(literal(json!(0.9))),
                        )),
                    )),
                ),
            ),
            ("expected", literal(json!(true))),
        ],
        predicate(Predicate::Equals),
    );

    Project {
        version: crate::format::FORMAT_VERSION,
        id: "blocks-poc".into(),
        name: "Operator blocks".into(),
        components: vec![],
        contracts: vec![],
        environment: environment(SHOP_BASE_URL),
        scenarios: vec![Scenario {
            id: "blocks-demo".into(),
            name: "Test built from nested blocks".into(),
            description: "API read → name type and length, plus a price rule → comparison \
                          with true. Green blocks return values; Comparison passes the test."
                .into(),
            parameters: IndexMap::new(),
            error_policy: ErrorPolicy::Continue,
            graph: graph(vec![source, named, discount]),
            cleanup: empty_graph(),
        }],
    }
}

/// Ordering edges enforce prices → create cart → add products and an IF node picks
/// the branch; references carry the current values between steps.
pub fn workflow_project() -> Project {
    let start = Node {
        id: "flow-start".into(),
        ..create_node(NodeKind::Start, at(40.0, 100.0), None)
    };
    let prices = request(
        "flow-prices",
        "Fetch product prices",
        "/products/123",
        vec![],
        product_sample(),
        HttpMethod::Get,
        at(370.0, 100.0),
    );
    let branch = Node {
        id: "flow-if".into(),
        handle: handle_base("IF response = 200"),
        label: Some("IF response = 200".into()),
        inputs: bindings(vec![
            ("actual", reference(&prices.id, "status", "")),
            ("expected", literal(json!(200))),
        ]),
        ..create_node(NodeKind::If, at(710.0, 100.0), None)
    };
    let mut cart = request(
        "flow-cart",
        "Create an empty cart",
        "/carts",
        vec![("body", literal(json!({})))],
        json!({ "id": "demo-cart", "total": 0, "quantity": 0, "items": [] }),
        HttpMethod::Post,
        at(1050.0, 20.0),
    );
    cart.config.sample_status = Some(201);
    let body = value(
        "flow-body",
        "Products to add",
        at(1050.0, 540.0),
        object(vec![
            ("productId", reference(&prices.id, "body", "id")),
            ("quantity", literal(json!(2))),
        ]),
    );
    let mut add = request(
        "flow-add",
        "Add products to the cart",
        "/carts/{{cartId}}/items",
        vec![
            ("cartId", reference(&cart.id, "body", "id")),
            ("body", literal(json!({}))),
        ],
        json!({
            "id": "demo-cart",
            "total": 160,
            "quantity": 2,
            "items": [{ "productId": 123, "quantity": 2, "price": 80 }],
        }),
        HttpMethod::Post,
        at(1390.0, 20.0),
    );
    add.inputs
        .insert("body".into(), reference(&body.id, "value", ""));
    let check = Node {
        id: "flow-check".into(),
        handle: handle_base("Compare the total with the fetched price"),
        label: Some("Compare the total with the fetched price".into()),
        inputs: bindings(vec![
            ("actual", reference(&add.id, "body", "total")),
            (
                "expected",
                op(
                    Operator::Multiply,
                    reference(&prices.id, "body", "promotion.price"),
                    Some(literal(json!(2))),
                ),
            ),
        ]),
        ..create_node(NodeKind::Assert, at(1730.0, 20.0), None)
    };
    let otherwise = Node {
        id: "flow-else".into(),
        handle: handle_base("Report missing prices without creating a cart"),
        label: Some("Report missing prices without creating a cart".into()),
        inputs: bindings(vec![
            ("actual", reference(&prices.id, "status", "")),
            ("expected", literal(json!(200))),
        ]),
        ..create_node(NodeKind::Assert, at(1050.0, 920.0), None)
    };
    let cleanup = request(
        "flow-cleanup",
        "Delete the created cart",
        "/carts/{{cartId}}",
        vec![("cartId", reference(&cart.id, "body", "id"))],
        json!({ "deleted": true }),
        HttpMethod::Delete,
        at(80.0, 80.0),
    );

    let mut edges: Vec<OrderEdge> = [
        (&start.id, &prices.id),
        (&prices.id, &branch.id),
        (&cart.id, &add.id),
        (&add.id, &check.id),
    ]
    .into_iter()
    .map(|(source, target)| OrderEdge {
        id: new_id("order"),
        source: source.clone(),
        target: target.clone(),
        branch: None,
    })
    .collect();
    edges.extend([
        branch_edge("flow-yes", &branch.id, &cart.id, Branch::True),
        branch_edge("flow-no", &branch.id, &otherwise.id, Branch::False),
    ]);

    Project {
        version: crate::format::FORMAT_VERSION,
        id: "workflow-poc".into(),
        name: "Workflow: prices and cart".into(),
        components: vec![],
        contracts: vec![],
        environment: environment(SHOP_BASE_URL),
        scenarios: vec![Scenario {
            id: "workflow-cart".into(),
            name: "Prices → cart → products".into(),
            description: "Manual Start, request ordering and IF / ELSE. Change the product \
                          endpoint to /products/999 to exercise the No branch."
                .into(),
            parameters: IndexMap::new(),
            error_policy: ErrorPolicy::Continue,
            graph: Graph {
                nodes: vec![start, prices, branch, cart, body, add, check, otherwise],
                edges,
            },
            cleanup: graph(vec![cleanup]),
        }],
    }
}

/// The product returned by the demo API for ID 123.
fn product_sample() -> Value {
    json!({
        "id": 123,
        "name": "Whole bean coffee",
        "price": 100,
        "promotion": {
            "type": "fixed_price",
            "active": true,
            "price": 80,
            "inna_nazwa": "Autumn offer",
        },
    })
}

/// The ticket returned by the demo API for ID 7.
fn ticket_sample() -> Value {
    json!({ "id": 7, "title": "Cannot sign in", "status": "open", "priority": "high" })
}

fn at(x: f64, y: f64) -> Position {
    Position { x, y }
}

fn empty_graph() -> Graph {
    graph(vec![])
}

fn graph(nodes: Vec<Node>) -> Graph {
    Graph {
        nodes,
        edges: vec![],
    }
}

fn environment(base_url: &str) -> Environment {
    Environment {
        base_url: base_url.into(),
        variables: IndexMap::new(),
        secret_names: vec![],
    }
}

/// A constant in canonical form: structured values become object and array bindings.
fn literal(value: Value) -> Binding {
    Binding::from_json(value)
}

fn reference(node_id: &str, port: &str, path: &str) -> Binding {
    Binding::reference(node_id, port, path)
}

fn parameter(name: &str) -> Binding {
    Binding::parameter(name)
}

/// An operator expression; unary operators take only `left`.
fn op(operator: Operator, left: Binding, right: Option<Binding>) -> Binding {
    let mut inputs = bindings(vec![("left", left)]);
    if let Some(right) = right {
        inputs.insert("right".into(), right);
    }
    Binding::Expression { operator, inputs }
}

fn bindings(entries: Vec<(&str, Binding)>) -> IndexMap<String, Binding> {
    entries
        .into_iter()
        .map(|(name, binding)| (name.to_owned(), binding))
        .collect()
}

fn predicate(predicate: Predicate) -> NodeConfig {
    NodeConfig {
        predicate: Some(predicate),
        ..NodeConfig::default()
    }
}

/// A Value block computing `binding`.
fn value(id: &str, name: &str, position: Position, binding: Binding) -> Node {
    node(
        id,
        NodeKind::Value,
        name,
        position,
        vec![("value", binding)],
        NodeConfig::default(),
    )
}

fn object(fields: Vec<(&str, Binding)>) -> Binding {
    Binding::Object {
        fields: bindings(fields),
    }
}

fn node(
    id: &str,
    kind: NodeKind,
    name: &str,
    position: Position,
    inputs: Vec<(&str, Binding)>,
    config: NodeConfig,
) -> Node {
    Node {
        id: id.into(),
        handle: handle_base(name),
        label: Some(name.into()),
        note: None,
        kind,
        position,
        auto_placed: false,
        inputs: bindings(inputs),
        config,
    }
}

/// A request. `:name` and `{{name}}` in `url` take the input of that name; the
/// `body` input is the request body.
fn request(
    id: &str,
    name: &str,
    url: &str,
    inputs: Vec<(&str, Binding)>,
    sample_response: Value,
    method: HttpMethod,
    position: Position,
) -> Node {
    let config = NodeConfig {
        method: Some(method),
        timeout_ms: Some(10_000.0),
        sample_response: OptionalJson::Present(sample_response),
        ..NodeConfig::default()
    };
    let arguments: IndexMap<&str, Binding> = inputs.into_iter().collect();
    let mut node_inputs = vec![("url", url_template(url, &arguments))];
    if let Some(body) = arguments.get("body") {
        node_inputs.push(("body", body.clone()));
    }
    node(id, NodeKind::Request, name, position, node_inputs, config)
}

/// A URL whose `:name` and `{{name}}` placeholders are values of `arguments`.
fn url_template(url: &str, arguments: &IndexMap<&str, Binding>) -> Binding {
    let mut parts = Vec::new();
    let mut text = String::new();
    let mut rest = url;
    while !rest.is_empty() {
        let placeholder = if let Some(after) = rest.strip_prefix("/:") {
            let length = after
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
                .unwrap_or(after.len());
            text.push('/');
            Some((&after[..length], &after[length..]))
        } else if let Some(after) = rest.strip_prefix("{{") {
            let end = after.find("}}").expect("closed placeholder");
            Some((after[..end].trim(), &after[end + 2..]))
        } else {
            None
        };
        match placeholder {
            Some((name, after)) => {
                if !text.is_empty() {
                    parts.push(TemplatePart::Text {
                        text: std::mem::take(&mut text),
                    });
                }
                parts.push(TemplatePart::Value {
                    binding: arguments[name].clone(),
                });
                rest = after;
            }
            None => {
                let next = rest.chars().next().expect("non-empty");
                text.push(next);
                rest = &rest[next.len_utf8()..];
            }
        }
    }
    if parts.is_empty() {
        return literal(json!(text));
    }
    if !text.is_empty() {
        parts.push(TemplatePart::Text { text });
    }
    Binding::Template { parts }
}

/// A node using revision 1 of a project component.
fn component_node(
    id: &str,
    name: &str,
    component_id: &str,
    position: Position,
    inputs: Vec<(&str, Binding)>,
) -> Node {
    let config = NodeConfig {
        component_id: Some(component_id.into()),
        revision: Some(1),
        ..NodeConfig::default()
    };
    node(id, NodeKind::Component, name, position, inputs, config)
}

fn contract_port(name: &str, contract_id: &str) -> Port {
    Port {
        contract_id: Some(contract_id.into()),
        ..Port::new(name, ValueType::Object)
    }
}

/// A component of a demo: its outputs become Export blocks to the right of its
/// graph, and its handle derives from its name.
struct Definition {
    id: &'static str,
    revision: u32,
    name: &'static str,
    description: &'static str,
    inputs: Vec<Port>,
    outputs: Vec<Output>,
    graph: Graph,
    action: Option<ComponentAction>,
}

struct Output {
    name: &'static str,
    label: Option<&'static str>,
    value_type: ValueType,
    contract_id: Option<&'static str>,
    binding: Binding,
}

impl Definition {
    fn build(self) -> Component {
        let mut graph = self.graph;
        let exports: Vec<Node> = self
            .outputs
            .into_iter()
            .enumerate()
            .map(|(index, output)| {
                let mut export =
                    create_node(NodeKind::Export, export_position(&graph.nodes, index), None);
                export.id = format!("{}-export-{}", self.id, output.name);
                export.handle = output.name.into();
                export.label = output.label.map(Into::into);
                export.config.output_type =
                    (output.value_type != ValueType::Any).then_some(output.value_type);
                export.config.contract_id = output.contract_id.map(Into::into);
                export.inputs.insert("value".into(), output.binding);
                export
            })
            .collect();
        graph.nodes.extend(exports);
        Component {
            id: self.id.into(),
            revision: self.revision,
            handle: handle_base(self.name),
            name: self.name.into(),
            description: self.description.into(),
            inputs: self.inputs,
            graph,
            action: self.action,
        }
    }
}

fn output(
    name: &'static str,
    label: Option<&'static str>,
    value_type: ValueType,
    contract_id: Option<&'static str>,
    binding: Binding,
) -> Output {
    Output {
        name,
        label,
        value_type,
        contract_id,
        binding,
    }
}

fn object_action(input: &str, contract_id: &str) -> ComponentAction {
    ComponentAction {
        input: input.into(),
        value_type: ValueType::Object,
        contract_id: Some(contract_id.into()),
    }
}

fn contract(id: &str, name: &str, sample: Value) -> Contract {
    Contract {
        id: id.into(),
        name: name.into(),
        sample,
    }
}

fn branch_edge(id: &str, source: &str, target: &str, branch: Branch) -> OrderEdge {
    OrderEdge {
        id: id.into(),
        source: source.into(),
        target: target.into(),
        branch: Some(branch),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURES: [(&str, &[u8]); 4] = [
        (
            "shop",
            include_bytes!("../../../fixtures/projects/shop.json"),
        ),
        (
            "tickets",
            include_bytes!("../../../fixtures/projects/tickets.json"),
        ),
        (
            "blocks",
            include_bytes!("../../../fixtures/projects/blocks.json"),
        ),
        (
            "workflow",
            include_bytes!("../../../fixtures/projects/workflow.json"),
        ),
    ];

    /// Removes what may legitimately differ from the reference: translated texts
    /// (entity names, descriptions, labels, sample strings) and random identifiers.
    /// Everything else, including array order, must match exactly.
    fn structure(project: &Project) -> Value {
        fn strip(value: &mut Value, in_sample: bool) {
            match value {
                Value::String(text) if in_sample || is_random_id(text) => text.clear(),
                Value::Array(items) => items.iter_mut().for_each(|item| strip(item, in_sample)),
                Value::Object(map) => {
                    let entity = map.contains_key("id") && !in_sample;
                    for (key, child) in map.iter_mut() {
                        match key.as_str() {
                            // Handles derive from the translated labels.
                            "description" | "label" | "handle" => *child = Value::Null,
                            "name" if entity => *child = Value::Null,
                            "sample" | "sampleResponse" => strip(child, true),
                            _ => strip(child, in_sample),
                        }
                    }
                }
                _ => {}
            }
        }
        fn is_random_id(text: &str) -> bool {
            text.split_once('-').is_some_and(|(prefix, suffix)| {
                matches!(prefix, "node" | "order" | "scenario" | "project")
                    && suffix.len() == 8
                    && suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        }
        let mut value = serde_json::to_value(project).expect("serialize project");
        strip(&mut value, false);
        value
    }

    fn graphs(project: &Project) -> Vec<&Graph> {
        let scenarios = project
            .scenarios
            .iter()
            .flat_map(|scenario| [&scenario.graph, &scenario.cleanup]);
        let components = project.components.iter().map(|component| &component.graph);
        scenarios.chain(components).collect()
    }

    fn fixture(bytes: &[u8]) -> Project {
        serde_json::from_slice(bytes).expect("fixture project")
    }

    #[test]
    fn every_demo_validates() {
        for project in demo_projects() {
            crate::validate_project(&project)
                .unwrap_or_else(|error| panic!("{}: {error}", project.id));
        }
        crate::validate_project(&empty_project()).expect("empty project");
    }

    #[test]
    fn demos_match_the_reference_structure() {
        for (project, (slug, bytes)) in demo_projects().iter().zip(FIXTURES) {
            let reference = fixture(bytes);
            assert_eq!(project.scenarios.len(), reference.scenarios.len(), "{slug}");
            let (ours, theirs) = (graphs(project), graphs(&reference));
            assert_eq!(ours.len(), theirs.len(), "{slug}");
            for (ours, theirs) in ours.iter().zip(&theirs) {
                let kinds = |graph: &Graph| graph.nodes.iter().map(|n| n.kind).collect::<Vec<_>>();
                assert_eq!(kinds(ours), kinds(theirs), "{slug}: node kinds");
                assert_eq!(ours.edges.len(), theirs.edges.len(), "{slug}: edges");
            }
            assert_eq!(structure(project), structure(&reference), "{slug}");
        }
    }

    #[test]
    fn demos_are_in_english() {
        let polish = |c: char| "ąćęłńóśźżĄĆĘŁŃÓŚŹŻ".contains(c);
        for project in demo_projects() {
            let text = serde_json::to_string(&project).expect("serialize project");
            assert!(!text.chars().any(polish), "{}", project.id);
        }
    }

    #[test]
    fn empty_documents_start_with_a_start_node() {
        let scenario = empty_scenario("Checkout");
        assert_eq!(scenario.name, "Checkout");
        assert!(scenario.id.starts_with("scenario-"));
        let kinds: Vec<_> = scenario.graph.nodes.iter().map(|node| node.kind).collect();
        assert_eq!(kinds, [NodeKind::Start]);
        assert!(scenario.graph.edges.is_empty() && scenario.cleanup.nodes.is_empty());

        let project = empty_project();
        assert_eq!(project.name, DEFAULT_PROJECT_NAME);
        assert_eq!(project.scenarios[0].name, DEFAULT_SCENARIO_NAME);
        assert_ne!(project.id, empty_project().id);
    }

    #[test]
    fn workflow_order_edges_get_fresh_identifiers() {
        let edges = &workflow_project().scenarios[0].graph.edges;
        assert!(edges[..4].iter().all(|edge| edge.id.starts_with("order-")));
        assert_eq!(edges[4].branch, Some(Branch::True));
        assert_eq!(edges[5].branch, Some(Branch::False));
    }
}
