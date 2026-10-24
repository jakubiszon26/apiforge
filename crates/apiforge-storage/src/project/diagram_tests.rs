use super::*;
use apiforge_core::{
    ImportLimits,
    format::{OrderEdge, Position, Proxy, ProxySource},
    import_document,
};

pub(crate) fn fixture() -> (Project, DiagramRecord) {
    let mut project = import_document(
        include_bytes!("../../../../fixtures/workspace.json"),
        &ImportLimits::default(),
    )
    .unwrap()
    .into_workspace()
    .projects
    .remove(0);
    let scenario = &mut project.scenarios[0];
    let variable = scenario
        .graph
        .nodes
        .iter_mut()
        .find(|node| node.kind == apiforge_core::model::NodeKind::Value)
        .unwrap();
    variable.config.variable = Some(true);
    variable.auto_placed = true;
    variable.position = Position {
        x: 77.25,
        y: -123.5,
    };
    scenario.graph.proxies.push(Proxy {
        id: "proxy-main".into(),
        source: ProxySource::Variable {
            node_id: variable.id.clone(),
        },
        position: Position {
            x: 520.0,
            y: -180.0,
        },
    });
    scenario.graph.edges.push(OrderEdge {
        id: "order-main".into(),
        source: scenario.graph.nodes[0].id.clone(),
        target: scenario.graph.nodes[1].id.clone(),
        branch: None,
    });
    scenario.cleanup.edges.push(OrderEdge {
        id: "order-cleanup".into(),
        source: scenario.cleanup.nodes[0].id.clone(),
        target: scenario.cleanup.nodes[1].id.clone(),
        branch: None,
    });
    project
        .environment
        .variables
        .insert("proxyEnv".into(), "public".into());
    scenario.cleanup.proxies.push(Proxy {
        id: "proxy-cleanup".into(),
        source: ProxySource::Environment {
            name: "proxyEnv".into(),
        },
        position: Position { x: 940.0, y: 480.0 },
    });
    let record = DiagramRecord {
        key: DocumentKey::Working {
            document_id: "document-main".into(),
        },
        source_hash: "a".repeat(64),
        context_digest: "b".repeat(64),
        model_version: FORMAT_VERSION,
        cache_version: 1,
        semantic_digest: "c".repeat(64),
        identity_base: IdentityBase::Scenario(scenario.clone()),
    };
    (project, record)
}

#[test]
fn checkpoint_round_trip_keeps_main_cleanup_ids_positions_ordering_and_variable_proxies() {
    let (project, record) = fixture();
    record.validate_in_context(&project).unwrap();
    let file = DiagramFile {
        format_version: 1,
        records: vec![record],
    };
    let loaded = DiagramFile::from_bytes(&file.to_bytes().unwrap()).unwrap();
    assert_eq!(loaded, file);
    loaded.records[0].validate_in_context(&project).unwrap();
}

#[test]
fn working_and_published_component_layouts_have_distinct_keys_and_checked_revisions() {
    let (project, _) = fixture();
    let component = project.components[0].clone();
    let mut published = DiagramRecord {
        key: DocumentKey::Revision {
            component_id: component.id.clone(),
            revision: component.revision,
        },
        source_hash: "a".repeat(64),
        context_digest: "b".repeat(64),
        model_version: FORMAT_VERSION,
        cache_version: 1,
        semantic_digest: "c".repeat(64),
        identity_base: IdentityBase::Component(component),
    };
    published.validate_in_context(&project).unwrap();
    let mut working = published.clone();
    working.key = DocumentKey::Working {
        document_id: "working-component".into(),
    };
    let file = DiagramFile {
        format_version: 1,
        records: vec![published.clone(), working],
    };
    assert_eq!(
        DiagramFile::from_bytes(&file.to_bytes().unwrap())
            .unwrap()
            .records
            .len(),
        2
    );
    if let DocumentKey::Revision { revision, .. } = &mut published.key {
        *revision += 1;
    }
    assert!(matches!(
        published.validate_metadata(),
        Err(ProjectError::InvalidCheckpoint)
    ));
}

#[test]
fn serialized_record_order_is_deterministic_and_duplicate_keys_are_rejected() {
    let (_, first) = fixture();
    let mut second = first.clone();
    second.key = DocumentKey::Working {
        document_id: "another-document".into(),
    };
    let file = DiagramFile {
        format_version: 1,
        records: vec![first.clone(), second.clone()],
    };
    let reverse = DiagramFile {
        format_version: 1,
        records: vec![second, first.clone()],
    };
    assert_eq!(file.to_bytes().unwrap(), reverse.to_bytes().unwrap());
    let duplicate = DiagramFile {
        format_version: 1,
        records: vec![first.clone(), first],
    };
    assert!(matches!(
        duplicate.to_bytes(),
        Err(ProjectError::InvalidCheckpoint)
    ));
}

#[test]
fn foreign_document_bad_digest_unknown_version_and_invalid_models_cannot_be_used_as_checkpoints() {
    let (project, record) = fixture();
    let entry = DocumentEntry {
        document_id: "document-main".into(),
        kind: DocumentKind::Scenario,
        path: ProjectPath::working_source("scenario.afs").unwrap(),
        entity_id: record.identity_base.entity_id().into(),
        base_revision: None,
        recovered: false,
    };
    assert!(record.matches_document(&entry));
    let mut foreign = entry.clone();
    foreign.document_id = "other".into();
    assert!(!record.matches_document(&foreign));
    foreign = entry.clone();
    foreign.entity_id = "another-scenario".into();
    assert!(!record.matches_document(&foreign));
    foreign = entry;
    foreign.kind = DocumentKind::Component;
    assert!(!record.matches_document(&foreign));
    let mut invalid = record.clone();
    invalid.source_hash = "A".repeat(64);
    assert!(invalid.validate_metadata().is_err());
    let mut obsolete = record.clone();
    obsolete.model_version += 1;
    assert!(matches!(
        obsolete.validate_metadata(),
        Err(ProjectError::UnsupportedVersion)
    ));
    let mut invalid = record.clone();
    if let IdentityBase::Scenario(scenario) = &mut invalid.identity_base {
        scenario.graph.edges[0].source = "nonexistent-node".into();
    }
    assert!(matches!(
        invalid.validate_in_context(&project),
        Err(ProjectError::InvalidCheckpoint)
    ));
    let mut invalid = record;
    if let IdentityBase::Scenario(scenario) = &mut invalid.identity_base {
        scenario.graph.nodes[0].position.x = f64::INFINITY;
    }
    assert!(
        DiagramFile {
            format_version: 1,
            records: vec![invalid]
        }
        .to_bytes()
        .is_err()
    );
}

#[test]
fn malformed_and_future_diagram_files_are_reported_without_accepting_partial_records() {
    assert!(matches!(
        DiagramFile::from_bytes(b"{\"formatVersion\":2,\"future\":true}"),
        Err(ProjectError::UnsupportedVersion)
    ));
    let (_, record) = fixture();
    let mut json: serde_json::Value = serde_json::from_slice(
        &DiagramFile {
            format_version: 1,
            records: vec![record],
        }
        .to_bytes()
        .unwrap(),
    )
    .unwrap();
    json["records"][0]["unexpected"] = true.into();
    assert!(matches!(
        DiagramFile::from_bytes(&serde_json::to_vec(&json).unwrap()),
        Err(ProjectError::InvalidCheckpoint)
    ));
}

#[test]
fn captured_checkpoints_describe_only_their_exact_source_context_and_semantics() {
    let (mut project, _) = fixture();
    let scenario = project.scenarios.remove(0);
    let source = apiforge_script::print_folder_document(
        &project,
        &apiforge_script::Document::Scenario(scenario.clone()),
    )
    .unwrap();
    let base = IdentityBase::Scenario(scenario.clone());
    let record = DiagramRecord::capture(
        DocumentKey::Working {
            document_id: "document".into(),
        },
        &source,
        base.clone(),
        &project,
    )
    .unwrap();
    assert_eq!(record.identity_base, base);
    assert!(record.describes(&source, &base, &project));
    assert!(!record.describes(&format!("{source} "), &base, &project));
    let mut renamed = scenario.clone();
    renamed.name.push_str(" renamed");
    assert!(!record.describes(&source, &IdentityBase::Scenario(renamed), &project));
    let mut configured = project.clone();
    configured.environment.base_url.push_str("/other");
    assert!(!record.describes(&source, &base, &configured));
    // Unrelated components are outside the closed dependency context.
    let mut unrelated = project.clone();
    let mut extra = unrelated.components[0].clone();
    extra.id = "unrelated".into();
    extra.handle = "unrelated".into();
    unrelated.components.push(extra);
    let called: Vec<_> = scenario
        .graph
        .nodes
        .iter()
        .chain(&scenario.cleanup.nodes)
        .filter_map(|node| node.config.component_id.clone())
        .collect();
    assert!(!called.contains(&"unrelated".to_owned()));
    assert!(record.describes(&source, &base, &unrelated));
}
