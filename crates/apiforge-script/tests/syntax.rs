use apiforge_script::{MAX_DEPTH, MAX_SOURCE_BYTES, ast::*, format, lexer::TokenKind, parse};

fn valid(source: &str) -> SyntaxTree {
    let tree = parse(source);
    assert!(!tree.has_errors(), "{:#?}\n{source}", tree.diagnostics);
    tree
}

fn stable(source: &str) -> String {
    let output = format(source).unwrap_or_else(|errors| panic!("{errors:#?}\n{source}"));
    assert_eq!(
        format(&output).unwrap(),
        output,
        "formatter is not idempotent"
    );
    output
}

#[test]
fn checkout_example_covers_all_scenario_forms_and_ordering() {
    let source = r#"scenario "Checkout" {
      description "A complete example"
      on_error continue
      param quantity = 2
      begin = start
      // Fetch products.
      @after(begin)
      products = GET "/products" { headers { Accept: "application/json" } query { limit: 10 } timeout 5s sample 200 {items: []} }
      status_ok = assert products.status == 200
      var first = products.body.items[0]
      total = first.price * param.quantity
      cart = AddToCart@2(product: first, qty: param.quantity)
      @after(status_ok)
      @label("Place an order")
      order = POST "/orders" {body { productId: first.id, cartId: cart.id, total: total }}
      created = if order.status == 201 {
        receipt = GET "/orders/{{order.body.id}}"
      } else { failed = assert false }
      @after(created.then, failed)
      checks = condition any { exists order.body.id, order.status is "number", total ~= 10 within 0.01, } with order.body
      cleanup { removed = DELETE "/orders/{{order.body.id}}" }
    }"#;
    let output = stable(source);
    let tree = valid(&output);
    let file = tree.file.unwrap();
    assert_eq!(file.statements.len(), 9);
    assert_eq!(file.statements[1].note.as_deref(), Some("Fetch products."));
    assert!(matches!(
        &file.statements[5].form,
        Form::Value(Expr {
            kind: ExprKind::Call {
                revision: Some(2),
                ..
            },
            ..
        })
    ));
    assert_eq!(file.cleanup.unwrap().len(), 1);
}

#[test]
fn components_keep_input_attributes_defaults_actions_and_exports() {
    let source = r#"component Lookup "Look up a value" {
      description "Reusable"
      @label("Input record") @contract("Product") @optional @hidden
      input product: object = { id: 123, "path.with.dots": [true, null] }
      input "other key": string = "default"
      action product
      begin = start
      @after(begin) response = GET "/{{param.product.id}}"
      @contract("Product") export payload: object = response.body
      export code: number = response.status
    }"#;
    let output = stable(source);
    let file = valid(&output).file.unwrap();
    assert_eq!(file.inputs[0].attributes.len(), 4);
    assert_eq!(file.inputs[1].name, "other key");
    assert_eq!(file.action.as_deref(), Some("product"));
    assert!(matches!(file.statements[2].form, Form::Export { .. }));
}

#[test]
fn precedence_associativity_and_parenthesized_check_operands_survive() {
    for expression in [
        "1 + 2 * 3",
        "(1 + 2) * 3",
        "8 - (4 - 2)",
        "8 / (4 / 2)",
        "!true || false && true",
        "a == b || c != d",
        "a < b == c >= d",
        "-0.5e-2",
        "round(a + b, 2)",
        "op.add(left: 2)",
        "op.exists()",
        "?",
        "param[\"x y\"].id",
        "env[\"API-TOKEN\"]",
    ] {
        stable(&format!("scenario \"Example\" {{ value = {expression} }}"));
    }
    let output = stable(
        r#"scenario "Checks" {
        bare = assert a >= b
        nested = assert (a == b) == true
        present = assert exists(a)
        kind = assert a is "number"
        contains_value = assert a contains b
    }"#,
    );
    assert!(output.contains("bare = assert (a >= b) == true"));
    assert!(output.contains("nested = assert (a == b) == true"));
    assert!(parse(r#"scenario "Bad" { check = assert a == b contains c }"#).has_errors());
}

#[test]
fn templates_support_nested_objects_strings_braces_and_exact_ranges() {
    let source = r#"scenario "Unicode" {
      data = "żółć {{concat("a", { value: param.x })}} / {{env["API-TOKEN"]}}"
      escaped = "literal \{{braces}} and \\{{param.x}}"
    }"#;
    let tree = valid(source);
    let Form::Value(Expr {
        kind: ExprKind::Template(parts),
        ..
    }) = &tree.file.as_ref().unwrap().statements[0].form
    else {
        panic!()
    };
    let Part::Value(expr) = &parts[1] else {
        panic!()
    };
    assert_eq!(
        &source[expr.range.clone()],
        r#"concat("a", { value: param.x })"#
    );
    stable(source);
    let output = stable(r#"scenario "Plain" { data = "\{{literal}}" }"#);
    let file = valid(&output).file.unwrap();
    assert!(
        matches!(&file.statements[0].form, Form::Value(Expr {kind: ExprKind::Literal(serde_json::Value::String(s)),..}) if s == "{{literal}}")
    );
}

#[test]
fn multiline_strings_are_dedented_once_and_keep_interpolation() {
    let source = "scenario \"Body\" {\n  body = \"\"\"\n    first line\n    {{param.value}}\n    last line\n    \"\"\"\n}";
    let tree = valid(source);
    let Form::Value(Expr {
        kind: ExprKind::Template(parts),
        ..
    }) = &tree.file.as_ref().unwrap().statements[0].form
    else {
        panic!()
    };
    assert_eq!(parts[0], Part::Text("first line\n".into()));
    assert_eq!(parts[2], Part::Text("\nlast line".into()));
    stable(source);
    let source = "scenario \"Text\" { value = \"\"\"\n    \"quoted\"\n      indented\n    \"\"\" }";
    let output = stable(source);
    assert!(output.contains(r#""\"quoted\"\n  indented""#));
    let source = "scenario \"CRLF\" { value = \"\"\"\r\n    first\r\n    last\r\n    \"\"\" }";
    let output = stable(source);
    assert!(output.contains(r#""first\r\nlast""#));
}

#[test]
fn comments_attach_to_the_nearest_statement_and_floating_comments_warn() {
    let source = r#"// A floating file comment.
    scenario "Notes" {
      // First line
      //  Preserve a leading space
      @label("A") a = 1 // trailing
      // Parent
      gate = if true {
        // Child
        child = 2 // child tail
      } // parent tail
      // This comment has no statement.
    }"#;
    let tree = valid(source);
    assert_eq!(
        tree.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .count(),
        2
    );
    let file = tree.file.unwrap();
    assert_eq!(
        file.statements[0].note.as_deref(),
        Some("First line\n Preserve a leading space\ntrailing")
    );
    assert_eq!(
        file.statements[1].note.as_deref(),
        Some("Parent\nparent tail")
    );
    let Form::If { then, .. } = &file.statements[1].form else {
        panic!()
    };
    assert_eq!(then[0].note.as_deref(), Some("Child\nchild tail"));
    stable(source);
}

#[test]
fn malformed_statements_recover_to_later_statements_and_offer_insertion_fixes() {
    let source = "scenario \"Recover\" { bad = @broken\n good = 42\n later = GET \"/ok\" }";
    let tree = parse(source);
    assert!(tree.has_errors());
    assert_eq!(
        tree.file
            .unwrap()
            .statements
            .iter()
            .map(|s| s.handle.as_str())
            .collect::<Vec<_>>(),
        ["good", "later"]
    );
    let tree = parse("scenario \"Missing\" { value = [1, 2");
    assert!(tree.has_errors());
    assert!(
        tree.diagnostics
            .iter()
            .any(|d| d.fix.as_ref().is_some_and(|f| f.replacement == "]"))
    );
}

#[test]
fn invalid_json_numbers_duplicates_and_wrong_forms_are_diagnostics() {
    for body in [
        "x = 01",
        "x = 1e",
        "x = 1e999",
        "x = -other",
        "x = { a: 1, a: 2 }",
        "x = call(a: 1, a: 2)",
        "x = call(a: 1, 2)",
        "var x = GET \"/\"",
        "x = condition all {}",
        "x = GET \"/\" { timeout 0ms }",
        "x = GET \"/\" { timeout 1e308s }",
        "x = GET \"/\" { timeout 5 s }",
        "x = GET \"/\" { sample 99 {} }",
        "x = GET \"/\" { body 1 body 2 }",
        "x = \"bad\\q\"",
        "x = data[-1]",
        "x = data[1.5]",
        "x = Component@0()",
    ] {
        let source = format!("scenario \"Invalid\" {{ {body} }}");
        assert!(
            parse(&source).has_errors(),
            "accepted invalid syntax: {body}"
        );
    }
}

#[test]
fn diagnostics_never_quote_source_literals() {
    let source = "scenario \"canary-secret-value\" { x = \"canary-secret-value\\q\" }";
    let tree = parse(source);
    assert!(tree.has_errors());
    for diagnostic in tree.diagnostics {
        assert!(!diagnostic.message.contains("canary-secret-value"));
    }
}

#[test]
fn tokens_cover_every_byte_including_errors_comments_and_unicode() {
    let source = "scenario \"żółć\" { x=ż // notes\n y = \"ok\" }";
    let tree = parse(source);
    let mut at = 0;
    for token in &tree.tokens {
        assert_eq!(token.range.start, at);
        assert!(
            source.is_char_boundary(token.range.start) && source.is_char_boundary(token.range.end)
        );
        at = token.range.end;
    }
    assert_eq!(at, source.len());
    assert!(tree.tokens.iter().any(|t| t.kind == TokenKind::Comment));
    assert!(tree.tokens.iter().any(|t| t.kind == TokenKind::Error));
}

#[test]
fn depth_source_size_and_diagnostic_budgets_are_enforced() {
    for value in [
        format!(
            "{}0{}",
            "(".repeat(MAX_DEPTH * 2),
            ")".repeat(MAX_DEPTH * 2)
        ),
        format!("0{}", "+0".repeat(MAX_DEPTH * 2)),
        format!("{}true", "!".repeat(MAX_DEPTH * 2)),
    ] {
        assert!(parse(&format!("scenario \"Depth\" {{ x = {value} }}")).has_errors());
    }
    let oversized = " ".repeat(MAX_SOURCE_BYTES + 1);
    let tree = parse(&oversized);
    assert!(tree.has_errors());
    assert_eq!(tree.tokens[0].range, 0..oversized.len());
    assert!(parse(&"#".repeat(10_000)).diagnostics.len() <= apiforge_script::MAX_DIAGNOSTICS);
}

#[test]
fn generated_utf8_and_token_mutations_never_panic_or_produce_invalid_ranges() {
    let seeds = [
        r#"scenario "X" { x = [1, {a: "{{param.x}}"}] }"#,
        r#"component C "C" { export out: any = ? }"#,
    ];
    let mut state = 0x5eeda11cu32;
    let alphabet = [
        'a', 'ż', '🦀', '\0', '"', '\\', '{', '}', '[', ']', '(', ')', '@', '=', '/', '\n', ' ',
        '0', '+', '!', '?',
    ];
    for iteration in 0..5_000 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let mut source = if iteration % 2 == 0 {
            seeds[iteration % seeds.len()].to_owned()
        } else {
            String::new()
        };
        for _ in 0..(state as usize % 32) {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let boundaries = source
                .char_indices()
                .map(|(i, _)| i)
                .chain(std::iter::once(source.len()))
                .collect::<Vec<_>>();
            let at = boundaries[state as usize % boundaries.len()];
            source.insert(at, alphabet[(state >> 16) as usize % alphabet.len()]);
        }
        let tree = parse(&source);
        for diagnostic in &tree.diagnostics {
            assert!(
                diagnostic.range.start <= diagnostic.range.end
                    && diagnostic.range.end <= source.len(),
                "{source:?}: {diagnostic:?}"
            );
            assert!(
                source.is_char_boundary(diagnostic.range.start)
                    && source.is_char_boundary(diagnostic.range.end)
            );
        }
        if !tree.has_errors() {
            let _ = stable(&source);
        }
    }
}
