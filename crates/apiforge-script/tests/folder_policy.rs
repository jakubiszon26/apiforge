use apiforge_core::format::*;
use apiforge_script::*;

fn context() -> Project {
    Project {
        version: FORMAT_VERSION,
        id: "folder".into(),
        name: "Folder".into(),
        scenarios: Vec::new(),
        components: Vec::new(),
        contracts: Vec::new(),
        environment: Environment {
            base_url: String::new(),
            variables: Default::default(),
            secret_names: Vec::new(),
        },
    }
}

#[test]
fn folder_component_only_validation_does_not_weaken_legacy_json_policy() {
    let project = context();
    let tree = parse("component Helper \"Helper\" { export result: number = 2 }");
    assert!(!lower(&tree, &LowerContext::folder(&project)).has_errors());
    assert!(lower(&tree, &LowerContext::new(&project)).has_errors());
    assert!(apiforge_core::validate_project(&project).is_err());
    apiforge_core::validate_authoring_project(&project).unwrap();
}

#[test]
fn unqualified_folder_calls_have_an_explicit_pin_fix_while_legacy_keeps_latest() {
    let mut project = context();
    let Some(Document::Component(component)) = lower(
        &parse("component Helper \"Helper\" { export result: number = 2 }"),
        &LowerContext::folder(&project),
    )
    .document
    else {
        panic!("component")
    };
    let mut newer = component.clone();
    newer.revision = 2;
    project.components = vec![component, newer];
    let source = "scenario \"Main\" { call = (Helper()) }";
    let tree = parse(source);
    let legacy = lower(&tree, &LowerContext::new(&project));
    assert!(!legacy.has_errors());
    let Some(Document::Scenario(scenario)) = legacy.document else {
        panic!("scenario")
    };
    assert_eq!(scenario.graph.nodes[0].config.revision, Some(2));
    let folder = lower(&tree, &LowerContext::folder(&project));
    assert!(folder.has_errors());
    let fix = folder
        .diagnostics
        .iter()
        .find_map(|diagnostic| diagnostic.fix.as_ref())
        .unwrap();
    let mut fixed = source.to_owned();
    fixed.replace_range(fix.range.clone(), &fix.replacement);
    assert!(fixed.contains("Helper@2()"));
    assert!(!lower(&parse(&fixed), &LowerContext::folder(&project)).has_errors());
    let old = fixed.replace("Helper@2", "Helper@1");
    let old = lower(&parse(&old), &LowerContext::folder(&project));
    assert!(!old.has_errors());
    let Some(Document::Scenario(scenario)) = old.document else {
        panic!("scenario")
    };
    assert_eq!(scenario.graph.nodes[0].config.revision, Some(1));
}
