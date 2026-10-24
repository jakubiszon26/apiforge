use apiforge_core::{format::*, *};

fn project() -> Project {
    import_document(
        include_bytes!("../../../fixtures/workspace.json"),
        &ImportLimits::default(),
    )
    .unwrap()
    .into_workspace()
    .projects
    .remove(0)
}

#[test]
fn authoring_allows_empty_and_component_only_projects_without_weakening_imports() {
    let mut project = project();
    project.scenarios.clear();
    assert!(!project.components.is_empty());
    validate_authoring_project(&project).unwrap();
    assert!(validate_project(&project).is_err());
    assert!(
        import_document(
            &serde_json::to_vec(&project).unwrap(),
            &ImportLimits::default()
        )
        .is_err()
    );
    project.components.clear();
    validate_authoring_project(&project).unwrap();
    assert!(validate_project(&project).is_err());
}

#[test]
fn authoring_keeps_definition_count_identity_and_graph_validation() {
    let mut project = project();
    project.scenarios.clear();
    project.components.push(project.components[0].clone());
    assert!(validate_authoring_project(&project).is_err());
    project.components.pop();
    project.components[0].graph.nodes[0].position.x = f64::NAN;
    assert!(validate_authoring_project(&project).is_err());
}
