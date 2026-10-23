use apiforge_core::{ImportError, ImportLimits, import_document};
use serde_json::{Value, json};

const SHOP: &[u8] = include_bytes!("../../../fixtures/compatibility/projects/shop.json");
const WORKFLOW: &[u8] = include_bytes!("../../../fixtures/compatibility/projects/workflow.json");
const CORPUS: &[&[u8]] = &[
    SHOP,
    WORKFLOW,
    include_bytes!("../../../fixtures/compatibility/projects/tickets.json"),
    include_bytes!("../../../fixtures/compatibility/projects/blocks.json"),
    include_bytes!("../../../fixtures/compatibility/workspace-v1.json"),
];
fn parse(value: &Value) -> Result<apiforge_core::ValidatedDocument, ImportError> {
    import_document(
        &serde_json::to_vec(value).expect("serialize test"),
        &ImportLimits::default(),
    )
}
fn workflow() -> Value {
    serde_json::from_slice(WORKFLOW).expect("fixture")
}
// v1 uses JavaScript numbers: serializing typed f64 metadata may emit 100.0
// instead of 100. Compare numeric values, not serde_json's integer/float tags.
fn assert_semantic_json(actual: &Value, expected: &Value) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => assert_eq!(a.as_f64(), b.as_f64()),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                assert_semantic_json(a, b);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len());
            for (key, expected) in b {
                assert_semantic_json(
                    a.get(key).unwrap_or_else(|| panic!("missing field: {key}")),
                    expected,
                );
            }
        }
        _ => assert_eq!(actual, expected),
    }
}
#[test]
fn every_typescript_export_round_trips_without_dropping_configuration() {
    for source in CORPUS {
        let document =
            import_document(source, &ImportLimits::default()).expect("valid exported document");
        let actual: Value =
            serde_json::from_slice(&document.to_json().expect("export")).expect("JSON");
        let expected: Value = serde_json::from_slice(source).expect("source JSON");
        assert_semantic_json(&actual, &expected);
    }
}
#[test]
fn missing_json_fields_and_present_null_have_distinct_round_trips() {
    let mut value = workflow();
    value["scenarios"][0]["graph"]["nodes"][1]["config"]["sampleResponse"] = Value::Null;
    value["scenarios"][0]["graph"]["nodes"][1]["config"]["inputPorts"] = json!([
        {"name":"nullable", "type":"any", "defaultValue": null},
        {"name":"missing", "type":"any"}
    ]);
    let document = parse(&value).expect("valid nulls");
    let actual: Value = serde_json::from_slice(&document.to_json().expect("export")).expect("JSON");
    assert_semantic_json(&actual, &value);
    let mut absent = value;
    absent["scenarios"][0]["graph"]["nodes"][1]["config"]
        .as_object_mut()
        .expect("config")
        .remove("sampleResponse");
    let actual: Value =
        serde_json::from_slice(&parse(&absent).expect("missing").to_json().expect("export"))
            .expect("JSON");
    assert_semantic_json(&actual, &absent);
}
#[test]
fn preserves_named_argument_order_and_json_unicode_values() {
    let mut value = workflow();
    value["scenarios"][0]["graph"]["nodes"][4]["inputs"] = serde_json::from_str(r#"{"z":{"kind":"literal","value":"Zażółć 🦀"},"a":{"kind":"literal","value":false},"0":{"kind":"literal","value":0}}"#).expect("JSON");
    let document = parse(&value).expect("valid order");
    let keys: Vec<_> = document.workspace().projects[0].scenarios[0].graph.nodes[4]
        .inputs
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["z", "a", "0"]);
}
#[test]
fn rejects_future_versions_unknown_fields_and_null_typed_metadata() {
    for field in ["version", "futureField", "method"] {
        let mut value = workflow();
        match field {
            "version" => value["version"] = json!(2),
            "futureField" => value["futureField"] = json!(true),
            _ => value["scenarios"][0]["graph"]["nodes"][1]["config"]["method"] = Value::Null,
        }
        assert!(parse(&value).is_err(), "{field}");
    }
}
#[test]
fn rejects_duplicate_keys_even_inside_literals_and_does_not_echo_credentials() {
    for source in [
        br#"{"version":1,"version":2,"secret":"credential-canary"}"#.as_slice(),
        br#"{"version":1,"secret":{"a":"credential-canary","a":0}}"#.as_slice(),
        br#"{"version":1,"secret":"credential-canary",INVALID}"#.as_slice(),
    ] {
        let error = import_document(source, &ImportLimits::default())
            .err()
            .expect("invalid");
        assert!(!error.to_string().contains("credential-canary"));
    }
}
#[test]
fn rejects_dangling_nested_references_and_missing_output_ports() {
    for (node_id, port) in [("missing", "body"), ("flow-prices", "missing")] {
        let mut value = workflow();
        value["scenarios"][0]["graph"]["nodes"][6]["inputs"]["expected"] = json!({"kind":"expression", "operator":"multiply", "inputs": {
            "left": {"kind":"reference", "nodeId":node_id, "port":port, "path":""}, "right":{"kind":"literal","value":2}
        }});
        let error = parse(&value).err().expect("invalid ref").to_string();
        assert!(error.contains("nodes[6]"));
        assert!(error.contains("Reference"));
    }
}
#[test]
fn validates_control_branches_start_and_mixed_cycles() {
    let mut unknown_branch = workflow();
    unknown_branch["scenarios"][0]["graph"]["edges"][4]["branch"] = json!("maybe");
    assert!(parse(&unknown_branch).is_err());
    let mut wrong_source = workflow();
    wrong_source["scenarios"][0]["graph"]["edges"][0]["branch"] = json!("true");
    assert!(parse(&wrong_source).is_err());
    let mut incoming_start = workflow();
    incoming_start["scenarios"][0]["graph"]["edges"][0]["target"] = json!("flow-start");
    assert!(parse(&incoming_start).is_err());
    let mut cycle = workflow();
    cycle["scenarios"][0]["graph"]["nodes"][1]["inputs"]["body"] =
        json!({"kind":"reference","nodeId":"flow-add","port":"body","path":""});
    assert!(
        parse(&cycle)
            .err()
            .expect("cycle")
            .to_string()
            .contains("cycle")
    );
}
#[test]
fn enforces_byte_node_expression_and_text_limits() {
    assert!(matches!(
        import_document(
            WORKFLOW,
            &ImportLimits {
                max_bytes: WORKFLOW.len() - 1
            }
        ),
        Err(ImportError::TooLarge { .. })
    ));
    let mut nodes = workflow();
    let template = nodes["scenarios"][0]["graph"]["nodes"][1].clone();
    let values = nodes["scenarios"][0]["graph"]["nodes"]
        .as_array_mut()
        .expect("nodes");
    for index in 0..201 {
        let mut node = template.clone();
        node["id"] = json!(format!("extra-{index}"));
        values.push(node);
    }
    assert!(parse(&nodes).is_err());
    let mut nested = workflow();
    let mut expression = json!({"kind":"literal","value":1});
    // Bindings nest at most 32 levels, including objects, arrays and templates.
    for _ in 0..32 {
        expression = json!({"kind":"expression","operator":"abs","inputs":{"left":expression}});
    }
    nested["scenarios"][0]["graph"]["nodes"][6]["inputs"]["expected"] = expression;
    assert!(parse(&nested).is_err());
    let mut text = workflow();
    text["name"] = json!("🦀".repeat(50_001));
    assert!(parse(&text).is_err());
}
#[test]
fn validates_component_identity_recursion_and_depth_even_when_subgraphs_are_cached() {
    let component = |id: &str, next: Option<&str>| {
        json!({
            "id":id,"revision":1,"name":id,"description":"","inputs":[],"outputs":[],
            "graph":{"nodes":next.map(|next| vec![json!({"id":"child","kind":"component","name":"child","position":{"x":0,"y":0},"inputs":{},"config":{"componentId":next,"revision":1}})]).unwrap_or_default(),"edges":[]}
        })
    };
    let mut value = workflow();
    value["components"] = json!([component("self", Some("self"))]);
    assert!(
        parse(&value)
            .err()
            .expect("recursive")
            .to_string()
            .contains("Recursive")
    );
    let mut components = Vec::new();
    for index in (0..17).rev() {
        let next = format!("c{}", index + 1);
        components.push(component(
            &format!("c{index}"),
            (index < 16).then_some(next.as_str()),
        ));
    }
    value["components"] = json!(components);
    assert!(
        parse(&value)
            .err()
            .expect("deep")
            .to_string()
            .contains("nesting")
    );
    let mut missing: Value = serde_json::from_slice(SHOP).expect("shop");
    missing["components"] = json!([]);
    assert!(parse(&missing).is_err());
}
#[test]
fn validates_workspace_envelope_and_reserved_keys() {
    let mut workspace = json!({"version":1,"activeId":"missing","projects":[workflow()]});
    assert!(parse(&workspace).is_err());
    workspace["activeId"] = json!("workflow-poc");
    assert!(parse(&workspace).is_ok());
    workspace["projects"]
        .as_array_mut()
        .expect("projects")
        .push(workflow());
    assert!(parse(&workspace).is_err());
    let mut value = workflow();
    value["scenarios"][0]["graph"]["nodes"][4]["inputs"]["constructor"] =
        json!({"kind":"literal","value":1});
    assert!(parse(&value).is_err());
}
