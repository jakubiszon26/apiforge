use super::*;

/// A provider with a fixed shape, as a host's contract or last run would give.
struct Shapes(
    Vec<(
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        ValueType,
    )>,
);

impl ShapeProvider for Shapes {
    fn segments(&self, handle: &str, port: &str, prefix: &str) -> Vec<ShapePath> {
        self.0
            .iter()
            .filter(|(h, p, pre, ..)| *h == handle && *p == port && *pre == prefix)
            .map(|(.., segment, value_type)| ShapePath {
                segment: (*segment).to_owned(),
                value_type: *value_type,
                source: Some("contract".to_owned()),
            })
            .collect()
    }
}

fn handles() -> Vec<Handle> {
    vec![
        Handle::new("products", "request").with_ports(default_ports("request")),
        {
            let mut first = Handle::new("first", "value").with_ports(default_ports("value"));
            first.variable = true;
            first
        },
    ]
}

fn components() -> Vec<ComponentOffer> {
    vec![ComponentOffer {
        handle: "AddToCart".into(),
        name: "Add to cart".into(),
        inputs: vec!["product".into(), "qty".into()],
    }]
}

/// Completes at the `|` in the source, which is removed before the request runs.
fn at(source: &str, mode: Mode, shapes: &dyn ShapeProvider) -> Vec<Completion> {
    let cursor = source.find('|').expect("the test marks the cursor with |");
    let source = source.replace('|', "");
    let handles = handles();
    let components = components();
    let environment = ["token".to_owned(), "baseUrl".to_owned()];
    let parameters = ["quantity".to_owned()];
    let inputs = ["product".to_owned()];
    complete(&Request {
        source: &source,
        cursor,
        mode,
        scope: Scope {
            handles: &handles,
            environment: &environment,
            parameters: &parameters,
            inputs: &inputs,
            components: &components,
            shapes,
        },
    })
}

fn script(source: &str) -> Vec<Completion> {
    at(source, Mode::Script, &NoShapes)
}

fn labels(found: &[Completion]) -> Vec<&str> {
    found.iter().map(|found| found.label.as_str()).collect()
}

fn has(found: &[Completion], label: &str) -> bool {
    found.iter().any(|found| found.label == label)
}

#[test]
fn a_statement_offers_the_words_a_statement_begins_with() {
    let found = script("scenario \"S\" {\n  |\n}");
    assert!(has(&found, "var"), "{:?}", labels(&found));
    assert!(has(&found, "export"));
    assert!(has(&found, "cleanup"));
    assert!(has(&found, "GET"));
    assert!(has(&found, "assert"));
    // Request items belong inside a request block, not here.
    assert!(!has(&found, "headers"));
}

#[test]
fn the_right_hand_side_of_a_statement_offers_values_and_forms() {
    let found = script("scenario \"S\" {\n  total = |\n}");
    assert!(has(&found, "first"), "{:?}", labels(&found));
    assert!(has(&found, "products"));
    assert!(has(&found, "round"));
    assert!(has(&found, "AddToCart"));
    assert!(has(&found, "env.token"));
    assert!(has(&found, "param.quantity"));
    // `var` introduces a statement; it cannot follow `=`.
    assert!(!has(&found, "var"));
}

#[test]
fn variables_come_before_other_blocks() {
    let found = script("scenario \"S\" {\n  total = |\n}");
    let variable = found.iter().position(|found| found.label == "first");
    let other = found.iter().position(|found| found.label == "products");
    assert!(variable < other, "{:?}", labels(&found));
}

#[test]
fn a_request_block_offers_its_own_items() {
    let found = script("scenario \"S\" {\n  order = POST \"/orders\" {\n    |\n  }\n}");
    assert!(has(&found, "headers"), "{:?}", labels(&found));
    assert!(has(&found, "query"));
    assert!(has(&found, "timeout"));
    assert!(has(&found, "sample"));
    assert!(!has(&found, "var"));
}

#[test]
fn a_dot_after_a_block_offers_its_ports() {
    let found = script("scenario \"S\" {\n  ok = products.|\n}");
    assert_eq!(labels(&found), ["body", "duration", "headers", "status"]);
    assert_eq!(found[0].kind, Kind::Port);
}

#[test]
fn a_block_with_one_port_is_read_straight_through_it() {
    let shapes = Shapes(vec![
        ("first", "value", "", "price", ValueType::Number),
        ("first", "value", "", "id", ValueType::String),
    ]);
    let found = at(
        "scenario \"S\" {\n  total = first.|\n}",
        Mode::Script,
        &shapes,
    );
    assert_eq!(labels(&found), ["id", "price"]);
    assert_eq!(found[0].kind, Kind::Path);
    assert_eq!(found[0].detail.as_deref(), Some("string · contract"));
}

#[test]
fn a_path_continues_under_the_port_already_named() {
    let shapes = Shapes(vec![("products", "body", "items", "id", ValueType::String)]);
    let found = at(
        "scenario \"S\" {\n  x = products.body.items.|\n}",
        Mode::Script,
        &shapes,
    );
    assert_eq!(labels(&found), ["id"]);
}

#[test]
fn an_index_in_a_path_does_not_become_a_name() {
    let shapes = Shapes(vec![(
        "products",
        "body",
        "items.[]",
        "price",
        ValueType::Number,
    )]);
    let found = at(
        "scenario \"S\" {\n  x = products.body.items[0].|\n}",
        Mode::Script,
        &shapes,
    );
    assert_eq!(labels(&found), ["price"]);
}

#[test]
fn env_param_and_input_offer_their_names_after_a_dot() {
    assert_eq!(
        labels(&script("scenario \"S\" {\n  x = env.|\n}")),
        ["baseUrl", "token"]
    );
    assert_eq!(
        labels(&script("scenario \"S\" {\n  x = param.|\n}")),
        ["quantity"]
    );
    assert_eq!(
        labels(&script("component c \"C\" {\n  x = input.|\n}")),
        ["product"]
    );
}

#[test]
fn a_secret_value_is_never_offered_only_its_name() {
    let found = script("scenario \"S\" {\n  x = env.|\n}");
    for completion in &found {
        let detail = completion.detail.clone().unwrap_or_default();
        assert_eq!(detail, "an environment variable");
        assert!(!detail.contains("secret"));
    }
}

#[test]
fn a_component_is_offered_with_its_arguments() {
    let found = script("scenario \"S\" {\n  cart = Add|\n}");
    let component = found
        .iter()
        .find(|found| found.label == "AddToCart")
        .expect("the component is offered");
    assert_eq!(component.insert, "AddToCart(product: , qty: )");
    assert_eq!(component.kind, Kind::Component);
    // The caret lands inside the parentheses, ready for the first argument.
    assert!(component.caret > "AddToCart(".len());
}

#[test]
fn an_operator_carries_its_signature() {
    let found = script("scenario \"S\" {\n  x = rou|\n}");
    let round = found
        .iter()
        .find(|found| found.label == "round")
        .expect("round is offered");
    assert_eq!(round.insert, "round()");
    assert_eq!(round.caret, "round(".len());
    let detail = round.detail.clone().unwrap();
    // The signature names the slots the language writes, as `op.round(left: …)`.
    assert!(
        detail.contains("left") && detail.contains("number"),
        "{detail}"
    );
}

#[test]
fn a_check_offers_the_predicates() {
    let found = script("scenario \"S\" {\n  ok = assert products.status |\n}");
    assert!(has(&found, "exists"), "{:?}", labels(&found));
    assert!(has(&found, "contains"));
    assert!(has(&found, "is"));
}

#[test]
fn typing_filters_and_ranks_by_how_well_a_name_matches() {
    let found = script("scenario \"S\" {\n  x = pro|\n}");
    assert_eq!(
        found.first().map(|found| found.label.as_str()),
        Some("products")
    );
    assert!(!has(&found, "first"), "{:?}", labels(&found));
    // A case-insensitive match is offered, after the exact one.
    let found = script("scenario \"S\" {\n  x = PRO|\n}");
    assert!(has(&found, "products"));
}

#[test]
fn the_word_being_typed_is_what_gets_replaced() {
    let found = script("scenario \"S\" {\n  x = prod|\n}");
    let products = found
        .iter()
        .find(|found| found.label == "products")
        .unwrap();
    let source = "scenario \"S\" {\n  x = prod\n}";
    assert_eq!(&source[products.replace.clone()], "prod");
}

#[test]
fn a_block_written_in_the_source_is_offered_before_the_document_knows_it() {
    let found = script("scenario \"S\" {\n  var subtotal = 1\n  total = sub|\n}");
    assert!(has(&found, "subtotal"), "{:?}", labels(&found));
    let subtotal = found
        .iter()
        .find(|found| found.label == "subtotal")
        .unwrap();
    assert_eq!(subtotal.kind, Kind::Variable);
}

#[test]
fn a_parameter_is_not_mistaken_for_a_block() {
    let declared = declarations("scenario \"S\" {\n  param quantity = 2\n  var total = 1\n}");
    let names: Vec<&str> = declared.iter().map(|handle| handle.name.as_str()).collect();
    assert_eq!(names, ["total"]);
    assert!(declared[0].variable);
}

#[test]
fn declarations_read_the_form_of_each_statement() {
    let declared = declarations(
        "scenario \"S\" {\n  begin = start\n  p = GET \"/p\"\n  ok = assert p.status == 200\n}",
    );
    let kinds: Vec<&str> = declared.iter().map(|handle| handle.kind).collect();
    assert_eq!(kinds, ["start", "request", "assert"]);
    let request = &declared[1];
    assert!(request.ports.iter().any(|port| port.name == "status"));
}

#[test]
fn text_outside_an_interpolation_completes_nothing() {
    let found = at("GET \"/orders/x|\"", Mode::Script, &NoShapes);
    assert!(found.is_empty(), "{:?}", labels(&found));
}

#[test]
fn an_interpolation_completes_the_expression_inside_it() {
    let found = at(
        "scenario \"S\" {\n  x = GET \"/orders/{{pro|}}\"\n}",
        Mode::Script,
        &NoShapes,
    );
    assert!(has(&found, "products"), "{:?}", labels(&found));
    assert!(
        !has(&found, "GET"),
        "a method cannot appear in an expression"
    );
}

#[test]
fn a_template_field_completes_only_inside_its_braces() {
    assert!(at("/orders/|", Mode::Template, &NoShapes).is_empty());
    let found = at("/orders/{{fir|}}", Mode::Template, &NoShapes);
    assert!(has(&found, "first"), "{:?}", labels(&found));
}

#[test]
fn an_object_body_completes_values_but_not_statements() {
    let found = at("productId: fir|", Mode::ObjectBody, &NoShapes);
    assert!(has(&found, "first"), "{:?}", labels(&found));
    assert!(!has(&found, "var"));
    assert!(!has(&found, "GET"));
}

#[test]
fn a_member_in_an_object_body_follows_its_port() {
    let shapes = Shapes(vec![("first", "value", "", "id", ValueType::String)]);
    let found = at("productId: first.|", Mode::ObjectBody, &shapes);
    assert_eq!(labels(&found), ["id"]);
}

#[test]
fn completion_is_bounded_and_never_panics_on_odd_input() {
    for source in [
        "",
        "|",
        "scenario",
        "scenario \"S\" { x = ",
        "}}}}{{{{",
        "x = products.....",
        "\u{1F600} = 1",
    ] {
        let cursor = source.len();
        let found = complete(&Request {
            source,
            cursor,
            mode: Mode::Script,
            scope: Scope::default(),
        });
        assert!(found.len() <= MAX_COMPLETIONS);
    }
    // A cursor inside a character is refused rather than panicking.
    let found = complete(&Request {
        source: "ą = 1",
        cursor: 1,
        mode: Mode::Script,
        scope: Scope::default(),
    });
    assert!(found.is_empty());
}

#[test]
fn hover_says_what_a_name_is_and_where_it_comes_from() {
    let source = "scenario \"S\" {\n  var subtotal = 1\n  total = subtotal + 2\n}";
    let cursor = source.rfind("subtotal").unwrap() + 3;
    let request = Request {
        source,
        cursor,
        mode: Mode::Script,
        scope: Scope::default(),
    };
    let described = describe(&request).expect("the name is known");
    assert_eq!(described.name, "subtotal");
    assert_eq!(described.kind, Kind::Variable);
    assert!(described.detail.contains("value"));
    // Go to definition lands on the declaration, not on the use.
    let definition = definition(&request).expect("the block is declared here");
    assert_eq!(&source[definition.clone()], "subtotal");
    assert!(definition.start < cursor - 3);
}

#[test]
fn hover_describes_an_operator_and_a_parameter() {
    let source = "x = round(1)";
    let described = describe(&Request {
        source,
        cursor: 6,
        mode: Mode::Script,
        scope: Scope::default(),
    })
    .unwrap();
    assert_eq!(described.kind, Kind::Operator);
    assert!(described.detail.contains("number"));

    let parameters = ["quantity".to_owned()];
    let described = describe(&Request {
        source: "x = param.quantity",
        cursor: 12,
        mode: Mode::Script,
        scope: Scope {
            parameters: &parameters,
            ..Scope::default()
        },
    })
    .unwrap();
    assert_eq!(described.kind, Kind::Parameter);
    assert!(described.definition.is_none());
}

#[test]
fn the_outline_locates_every_statement_including_nested_ones() {
    let source = "scenario \"S\" {\n  begin = start\n  ok = if begin.passed {\n    inner = 1\n  } else {\n    other = 2\n  }\n  cleanup {\n    gone = 3\n  }\n}";
    let handles: Vec<String> = outline(source)
        .into_iter()
        .map(|(handle, _)| handle)
        .collect();
    assert_eq!(handles, ["begin", "ok", "inner", "other", "gone"]);

    // The cursor names the innermost block it is inside.
    let at = source.find("inner").unwrap() + 2;
    assert_eq!(statement_at(source, at).as_deref(), Some("inner"));
    let at = source.find("begin = start").unwrap() + 1;
    assert_eq!(statement_at(source, at).as_deref(), Some("begin"));
    // Between statements there is nothing to name.
    assert_eq!(statement_at(source, 0), None);

    let range = statement_range(source, "other").expect("the block is written here");
    assert!(source[range].starts_with("other"));
    assert_eq!(statement_range(source, "absent"), None);
}

#[test]
fn an_outline_of_unfinished_text_names_what_it_can() {
    // A half-written statement still has a handle and a place.
    let source = "scenario \"S\" {\n  first = 1\n  second =\n}";
    let handles: Vec<String> = outline(source)
        .into_iter()
        .map(|(handle, _)| handle)
        .collect();
    assert!(handles.contains(&"first".to_owned()), "{handles:?}");
    assert!(outline("").is_empty());
    assert!(statement_at("", 0).is_none());
}
