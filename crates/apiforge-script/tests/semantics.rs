use apiforge_core::{ImportLimits, format::*, import_document};
use apiforge_script::{
    Document, LowerContext, lower, parse, print_component, print_scenario, reconcile, semantic,
};

fn workspace() -> Workspace {
    import_document(
        include_bytes!("../../../fixtures/workspace.json"),
        &ImportLimits::default(),
    )
    .unwrap()
    .into_workspace()
}
fn lowered(source: &str, project: &Project) -> Document {
    let result = lower(&parse(source), &LowerContext::new(project));
    assert!(!result.has_errors(), "{:#?}\n{source}", result.diagnostics);
    result.document.unwrap()
}
#[test]
fn every_workspace_scenario_and_component_round_trips() {
    let workspace = workspace();
    for project in &workspace.projects {
        for scenario in &project.scenarios {
            let text = print_scenario(project, scenario)
                .unwrap_or_else(|e| panic!("{}: {e:#?}", scenario.name));
            let document = lowered(&text, project);
            assert_eq!(
                semantic(&document, project),
                semantic(&Document::Scenario(scenario.clone()), project)
            );
            let Document::Scenario(new) = document else {
                panic!()
            };
            assert_eq!(print_scenario(project, &new).unwrap(), text);
        }
        for component in &project.components {
            let text = print_component(project, component)
                .unwrap_or_else(|e| panic!("{}: {e:#?}", component.name));
            let document = lowered(&text, project);
            assert_eq!(
                semantic(&document, project),
                semantic(&Document::Component(component.clone()), project)
            );
            let Document::Component(new) = document else {
                panic!()
            };
            assert_eq!(print_component(project, &new).unwrap(), text);
        }
    }
}
#[test]
fn examples_lower_with_branches_and_cleanup_provenance() {
    let workspace = workspace();
    let project = &workspace.projects[0];
    let Document::Scenario(scenario) = lowered(
        include_str!("../../../fixtures/scripts/checkout.afs"),
        project,
    ) else {
        panic!()
    };
    assert_eq!(scenario.graph.edges.len(), 4);
    assert_eq!(scenario.cleanup.nodes.len(), 1);
    let component = lowered(
        include_str!("../../../fixtures/scripts/lookup.afs"),
        project,
    );
    assert!(matches!(component, Document::Component(_)));
}
#[test]
fn identity_is_preserved_only_for_stable_handles_or_one_complete_rename() {
    let workspace = workspace();
    let project = &workspace.projects[0];
    let old = lowered(
        "scenario \"Identity\" { a = 1 b = a + 2 cleanup { c = a } }",
        project,
    );
    let initial = lower(
        &parse("scenario \"Identity\" { a = 1 b = a + 2 cleanup { c = a } }"),
        &LowerContext::new(project),
    );
    let old = reconcile(&old, &initial).unwrap().document;
    let renamed = lower(
        &parse("scenario \"Identity\" { renamed = 1 b = renamed + 2 cleanup { c = renamed } }"),
        &LowerContext::new(project),
    );
    let new = reconcile(&old, &renamed).unwrap();
    assert!(new.unpositioned.is_empty());
    let (Document::Scenario(old), Document::Scenario(new)) = (old, new.document) else {
        panic!()
    };
    assert_eq!(old.graph.nodes[0].id, new.graph.nodes[0].id);
    assert_eq!(
        new.graph.nodes[1].inputs["value"].references()[0].node_id,
        new.graph.nodes[0].id
    );
    assert_eq!(
        new.cleanup.nodes[0].inputs["value"].references()[0].node_id,
        new.graph.nodes[0].id
    );
}
#[test]
fn unknown_names_attributes_and_invalid_dependencies_are_errors_without_source_literals() {
    let workspace = workspace();
    let project = &workspace.projects[0];
    for source in [
        "scenario \"Private\" { x = missing }",
        "scenario \"Private\" { x = env.missing }",
        "scenario \"Private\" { x = param.missing }",
        "scenario \"Private\" { @private(\"canary-secret\") x = 1 }",
        "scenario \"Private\" { x = b cleanup { b = 1 } }",
        "scenario \"Private\" { x = y y = x }",
        "scenario \"Private\" { var x = GET \"/\" }",
        "scenario \"Private\" { export x: any = 1 }",
        "scenario \"Private\" { x = [?, 1] }",
        "scenario \"Private\" { x = op.add(unknown: 1) }",
    ] {
        let result = lower(&parse(source), &LowerContext::new(project));
        assert!(result.has_errors(), "{source}");
        assert!(
            result
                .diagnostics
                .iter()
                .all(|d| !d.message.contains("canary-secret"))
        );
        assert!(reconcile(&Document::Scenario(project.scenarios[0].clone()), &result).is_err());
    }
}

#[test]
fn request_argument_syntax_round_trips_with_body_and_url_sources() {
    let workspace = workspace();
    let project = &workspace.projects[0];
    let Document::Scenario(scenario) = lowered(
        r#"scenario "Arguments" {
        param product = { id: 123, name: "żółć" }
        response = POST "/products/{{input.productId}}" {
            argument productId = input.product.id
            argument product = param.product
            headers { "X-Product": "id-{{input.productId}}" }
            query { id: input.productId }
            body { product: input.product }
            sample 201 null
        }
    }"#,
        project,
    ) else {
        panic!()
    };
    let text = print_scenario(project, &scenario).unwrap();
    let Document::Scenario(new) = lowered(&text, project) else {
        panic!()
    };
    assert_eq!(
        semantic(&Document::Scenario(new.clone()), project),
        semantic(&Document::Scenario(scenario), project)
    );
    assert_eq!(print_scenario(project, &new).unwrap(), text);
    for text in [
        r#"scenario "Bad" { r = GET "{{input.missing}}" }"#,
        r#"scenario "Bad" { r = GET "/" { argument a = input.a } }"#,
        r#"scenario "Bad" { r = GET "/" { argument body = 1 } }"#,
        r#"scenario "Bad" { v = input.product }"#,
    ] {
        assert!(lower(&parse(text), &LowerContext::new(project)).has_errors());
    }
}

#[test]
fn all_frozen_project_and_run_documents_round_trip() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    for directory in ["projects", "runs"] {
        for entry in std::fs::read_dir(root.join(directory)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            let project_bytes = if directory == "runs" {
                serde_json::to_vec(&value["request"]["project"]).unwrap()
            } else {
                bytes
            };
            if path
                .file_stem()
                .is_some_and(|name| name == "component-depth")
            {
                // A frozen negative fixture: the importer must reject it before
                // either the language or runner can accept its recursive depth.
                assert!(import_document(&project_bytes, &ImportLimits::default()).is_err());
                continue;
            }
            let workspace = import_document(&project_bytes, &ImportLimits::default())
                .unwrap()
                .into_workspace();
            for project in &workspace.projects {
                for scenario in &project.scenarios {
                    let text = print_scenario(project, scenario)
                        .unwrap_or_else(|e| panic!("{path:?}: {e:#?}"));
                    let lowered = lower(&parse(&text), &LowerContext::document(project));
                    assert!(!lowered.has_errors(), "{path:?}: {:?}", lowered.diagnostics);
                    let actual = lowered.document.unwrap();
                    assert_eq!(
                        semantic(&actual, project),
                        semantic(&Document::Scenario(scenario.clone()), project),
                        "{path:?}"
                    );
                }
                for component in &project.components {
                    let text = print_component(project, component)
                        .unwrap_or_else(|e| panic!("{path:?}: {e:#?}"));
                    let lowered = lower(&parse(&text), &LowerContext::document(project));
                    assert!(!lowered.has_errors(), "{path:?}: {:?}", lowered.diagnostics);
                    let actual = lowered.document.unwrap();
                    assert_eq!(
                        semantic(&actual, project),
                        semantic(&Document::Component(component.clone()), project),
                        "{path:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn ambiguous_renames_kind_changes_swaps_and_insertions_do_not_exchange_identity() {
    let workspace = workspace();
    let project = &workspace.projects[0];
    let mut old = lowered("scenario \"Identity\" { a = 1 b = 2 }", project);
    let Document::Scenario(s) = &mut old else {
        panic!()
    };
    s.graph.nodes[0].id = "old-a".into();
    s.graph.nodes[0].position.x = 123.0;
    s.graph.nodes[0].auto_placed = false;
    s.graph.nodes[1].id = "old-b".into();
    for (source, fresh, handles) in [
        (
            "scenario \"Identity\" { b = 2 a = 9 }",
            0,
            vec![("a", "old-a"), ("b", "old-b")],
        ),
        (
            "scenario \"Identity\" { a = 2 b = 1 }",
            0,
            vec![("a", "old-a"), ("b", "old-b")],
        ),
        (
            "scenario \"Identity\" { a = 1 inserted = 3 b = 2 }",
            1,
            vec![("a", "old-a"), ("b", "old-b")],
        ),
        (
            "scenario \"Identity\" { a = GET \"/\" b = 2 }",
            1,
            vec![("b", "old-b")],
        ),
        (
            "scenario \"Identity\" { different = 3 b = 2 }",
            1,
            vec![("b", "old-b")],
        ),
        (
            "scenario \"Identity\" { renamed_a = 1 renamed_b = 2 }",
            2,
            vec![],
        ),
    ] {
        let lowered = lower(&parse(source), &LowerContext::new(project));
        let result = reconcile(&old, &lowered).unwrap();
        assert_eq!(result.unpositioned.len(), fresh, "{source}");
        let Document::Scenario(s) = result.document else {
            panic!()
        };
        for (handle, id) in handles {
            assert_eq!(
                s.graph
                    .nodes
                    .iter()
                    .find(|n| n.handle == handle)
                    .unwrap()
                    .id,
                id
            );
        }
        if let Some(a) = s.graph.nodes.iter().find(|n| n.id == "old-a") {
            assert_eq!(a.position.x, 123.0);
            assert!(!a.auto_placed);
        }
    }
}

#[test]
fn generated_documents_keep_every_optional_config_field_and_unconnected_slot() {
    use apiforge_core::{model::*, operator::Operator};
    let workspace = workspace();
    let project = &workspace.projects[0];
    for seed in 0..250 {
        let Document::Scenario(mut scenario) = lowered(
            r#"scenario "Generated" {
            begin = start
            @after(begin) response = POST "/" { body null sample 201 null }
            ok = if response.status == 201 { a = 1 } else { b = 2 }
            checks = condition any { exists response.body, response.status ~= 200 within 0.5 } with response.body
            value = op.add(left: 1)
            cleanup { removed = DELETE "/{{response.status}}" }
        }"#,
            project,
        ) else {
            panic!()
        };
        let request = &mut scenario.graph.nodes[1];
        request.config.method = (seed % 2 == 0).then_some(HttpMethod::Post);
        request.config.body_format = match seed % 3 {
            0 => None,
            1 => Some(BodyFormat::Json),
            _ => Some(BodyFormat::Text),
        };
        request.config.timeout_ms = (seed % 3 != 0).then_some(10_000.0);
        request.config.sample_status = (seed % 2 == 0).then_some(201);
        request.config.sample_response = if seed % 2 == 0 {
            OptionalJson::Missing
        } else {
            OptionalJson::Present(serde_json::json!(null))
        };
        request.config.variable = (seed % 2 == 0).then_some(false); // valid optional metadata even outside Value
        request.note = Some(
            [
                "",
                "  leading and trailing  ",
                "first\n\nlast",
                "żółć\r\nsecond",
                "ordinary note",
            ][seed % 5]
                .into(),
        );
        let value = scenario
            .graph
            .nodes
            .iter_mut()
            .find(|n| n.handle == "value")
            .unwrap();
        value.config.variable = match seed % 3 {
            0 => None,
            1 => Some(false),
            _ => Some(true),
        };
        let operator = Operator::ALL[seed % Operator::ALL.len()];
        value.inputs.clear();
        if seed % 4 != 0 {
            value.inputs.insert(
                "value".into(),
                Binding::Expression {
                    operator,
                    inputs: operator
                        .spec()
                        .operands
                        .first()
                        .map(|p| (p.name.to_owned(), Binding::literal(serde_json::json!(1))))
                        .into_iter()
                        .collect(),
                },
            );
        }
        let text =
            print_scenario(project, &scenario).unwrap_or_else(|e| panic!("seed={seed}: {e:?}"));
        let document = lowered(&text, project);
        assert_eq!(
            semantic(&document, project),
            semantic(&Document::Scenario(scenario), project),
            "seed={seed}"
        );
        let Document::Scenario(document) = document else {
            panic!()
        };
        assert_eq!(
            print_scenario(project, &document).unwrap(),
            text,
            "seed={seed}"
        );
    }
}

#[test]
fn component_identity_follows_handle_and_unambiguous_rename_rules() {
    let workspace = workspace();
    let project = &workspace.projects[0];
    let mut current = lowered(
        "component Original \"Component\" { export result: number = 1 }",
        project,
    );
    let Document::Component(component) = &mut current else {
        panic!()
    };
    component.id = "preserved-component".into();
    component.revision = 3;
    for (source, preserved) in [
        (
            "component Original \"Changed\" { export result: number = 2 }",
            true,
        ),
        (
            "component Renamed \"Component\" { export result: number = 1 }",
            true,
        ),
        (
            "component Different \"Component\" { export result: number = 2 }",
            false,
        ),
    ] {
        let result = reconcile(
            &current,
            &lower(&parse(source), &LowerContext::new(project)),
        )
        .unwrap();
        let Document::Component(component) = result.document else {
            panic!()
        };
        assert_eq!(component.id == "preserved-component", preserved);
        assert_eq!(component.revision, if preserved { 3 } else { 1 });
    }
}

#[test]
fn printing_a_historical_component_keeps_latest_instances_bound_to_their_exports() {
    let mut project = apiforge_core::demos::empty_project();
    let Document::Component(mut old) = lowered(
        "component Lookup \"Lookup\" { export old_result: number = 1 }",
        &project,
    ) else {
        panic!()
    };
    old.id = "lookup".into();
    let mut latest = old.clone();
    latest.revision = 2;
    latest.graph.nodes[0].handle = "new_result".into();
    project.components = vec![old.clone(), latest.clone()];
    let Document::Scenario(scenario) = lowered(
        "scenario \"Use\" { instance = Lookup@2() result = instance }",
        &project,
    ) else {
        panic!()
    };
    project.scenarios = vec![scenario];
    for component in [&old, &latest] {
        let text = print_component(&project, component).unwrap();
        let compiled = lower(
            &parse(&text),
            &LowerContext::new(&project).with_component(component),
        );
        assert!(!compiled.has_errors(), "{:?}", compiled.diagnostics);
        assert_eq!(
            semantic(compiled.document.as_ref().unwrap(), &project),
            semantic(&Document::Component(component.clone()), &project)
        );
    }
}

#[test]
fn printing_is_a_fixed_point_when_a_branch_reads_a_later_statement() {
    // `b` sits in the ELSE block but reads `later`, which follows the IF. A global
    // order filtered per block swapped `b` and `c` on every second print.
    let workspace = workspace();
    let project = &workspace.projects[0];
    let Document::Scenario(scenario) = lowered(
        "scenario \"Order\" { later = x.value x = if true { a = 1 } else { b = later + 1 c = 2 } }",
        project,
    ) else {
        panic!()
    };
    let first = print_scenario(project, &scenario).unwrap();
    let Document::Scenario(again) = lowered(&first, project) else {
        panic!()
    };
    assert_eq!(print_scenario(project, &again).unwrap(), first);
    assert!(
        first.find("b = ").unwrap() < first.find("c = ").unwrap(),
        "{first}"
    );
}

#[test]
fn extra_component_instance_inputs_print_as_exact_metadata() {
    // Found by fuzz_semantics: the call shorthand accepts declared inputs only.
    let workspace = workspace();
    let project = &workspace.projects[0];
    let Document::Scenario(scenario) = lowered(
        "scenario \"Extra\" { a = 1 @inputs({ value: a, id: 5 }) p = produkt(id: 1) }",
        project,
    ) else {
        panic!()
    };
    let text = print_scenario(project, &scenario).unwrap();
    assert!(text.contains("@inputs"), "{text}");
    let document = lowered(&text, project);
    assert_eq!(
        semantic(&document, project),
        semantic(&Document::Scenario(scenario), project)
    );
}

#[test]
fn document_validation_errors_point_at_their_statement() {
    let workspace = workspace();
    let project = &workspace.projects[0];
    for (source, statement) in [
        (
            "scenario \"Located\" {\n  a = 1\n  x = if a == 1 {\n    b = start\n  }\n}",
            "b = start",
        ),
        (
            "scenario \"Located\" {\n  x = if true {\n  }\n  @after(x)\n  y = 2\n}",
            "@after(x)\n  y = 2",
        ),
    ] {
        let result = lower(&parse(source), &LowerContext::new(project));
        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == apiforge_script::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "{:?}", result.diagnostics);
        assert_eq!(&source[errors[0].range.clone()], statement);
    }
}

#[test]
fn type_mismatches_the_board_would_refuse_are_located_warnings() {
    let workspace = workspace();
    let project = &workspace.projects[0];
    // Every stored document was built through the board's connection rule.
    for project in &workspace.projects {
        for scenario in &project.scenarios {
            let text = print_scenario(project, scenario).unwrap();
            let result = lower(&parse(&text), &LowerContext::new(project));
            assert!(
                result.diagnostics.is_empty(),
                "{}: {:?}",
                scenario.name,
                result.diagnostics
            );
        }
    }
    let source = r#"scenario "Types" {
  r = GET "/products"
  sum = r.headers + 1
  joined = concat("a", "b") - 2
  number = produkt(id: "seven")
  promoted = aktywna_promocja@1(product: r.body)
  fine = r.status + r.body.price
  later = early + 1
  early = { a: 1 }
}"#;
    let result = lower(&parse(source), &LowerContext::new(project));
    assert!(!result.has_errors(), "{:?}", result.diagnostics);
    let warnings: Vec<_> = result
        .diagnostics
        .iter()
        .map(|d| {
            assert_eq!(d.severity, apiforge_script::Severity::Warning);
            &source[d.range.clone()]
        })
        .collect();
    // Order follows the document; a forward reference (`early`) is typed too.
    assert_eq!(
        warnings,
        [
            "r.headers",
            "concat(\"a\", \"b\")",
            "\"seven\"",
            "r.body",
            "early"
        ]
    );
}

#[test]
fn replacing_a_request_by_a_different_one_does_not_transfer_identity() {
    let workspace = workspace();
    let project = &workspace.projects[0];
    let mut old = lowered(
        "scenario \"Replace\" { r = GET \"/a\" { timeout 5s } s = r.status }",
        project,
    );
    let Document::Scenario(scenario) = &mut old else {
        panic!()
    };
    scenario.graph.nodes[0].id = "old-request".into();
    scenario.graph.nodes[1].id = "old-reader".into();
    for source in [
        // A different request under a new handle, read by the same statement.
        "scenario \"Replace\" { q = POST \"/b\" s = q.status }",
        // The same request moved to a new handle while its reader changed too.
        "scenario \"Replace\" { q = GET \"/a\" { timeout 5s } s = q.body }",
    ] {
        let result = reconcile(&old, &lower(&parse(source), &LowerContext::new(project))).unwrap();
        let Document::Scenario(new) = result.document else {
            panic!()
        };
        let request = new.graph.nodes.iter().find(|n| n.handle == "q").unwrap();
        assert_ne!(request.id, "old-request", "{source}");
        assert_eq!(
            result.unpositioned,
            std::slice::from_ref(&request.id),
            "{source}"
        );
        let reader = new.graph.nodes.iter().find(|n| n.handle == "s").unwrap();
        assert_eq!(reader.id, "old-reader");
    }
}

#[test]
fn exact_config_never_silently_overrides_visible_settings() {
    let workspace = workspace();
    let project = &workspace.projects[0];
    let ok = |source: &str| !lower(&parse(source), &LowerContext::new(project)).has_errors();
    // Printed form: @config keeps sampleStatus absent, the rest agrees.
    let printed = r#"scenario "C" {
  @config({ method: "DELETE", timeoutMs: 10000.0, sampleResponse: { deleted: true } })
  r = DELETE "/carts" {
    timeout 10s
    sample 200 { deleted: true }
  }
}"#;
    assert!(ok(printed));
    for (from, to) in [
        ("timeout 10s", "timeout 3s"),
        ("r = DELETE", "r = POST"),
        ("sample 200", "sample 201"),
        ("{ deleted: true }\n", "{ deleted: false }\n"),
    ] {
        let edited = printed.replacen(from, to, 1);
        assert_ne!(edited, printed);
        assert!(!ok(&edited), "{to}");
    }
    assert!(!ok(
        "scenario \"C\" { @config({ method: \"GET\" }) r = POST \"/\" }"
    ));
    assert!(!ok(
        "scenario \"C\" { @config({ predicate: \"notEquals\" }) a = assert 1 == 1 }"
    ));
}
