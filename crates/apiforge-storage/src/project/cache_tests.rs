use super::*;
use crate::project::tests::{initialise, manifest, path, write};
use apiforge_core::format::{Position, Proxy, ProxySource};
use std::fs;

const SOURCE: &str =
    "scenario \"Main\" {\r\n var amount = 2\r\n cleanup { var other = 3 }\r\n}\r\n";

fn context() -> Project {
    let manifest = manifest();
    Project {
        version: apiforge_core::format::FORMAT_VERSION,
        id: manifest.id,
        name: manifest.name,
        environment: manifest.environment,
        contracts: manifest.contracts,
        components: Vec::new(),
        scenarios: Vec::new(),
    }
}

pub(in crate::project) fn snapshot() -> LastValidSnapshot {
    let context = context();
    let lowered = apiforge_script::lower(
        &apiforge_script::parse(SOURCE),
        &LowerContext::folder(&context),
    );
    assert!(!lowered.has_errors(), "{:?}", lowered.diagnostics);
    let Some(Document::Scenario(mut scenario)) = lowered.document else {
        panic!("scenario")
    };
    scenario.id = manifest().documents[0].entity_id.clone();
    scenario.graph.nodes[0].position = Position { x: 42.5, y: -280.0 };
    scenario.graph.nodes[0].auto_placed = true;
    scenario.graph.proxies.push(Proxy {
        id: "main-proxy".into(),
        source: ProxySource::Variable {
            node_id: scenario.graph.nodes[0].id.clone(),
        },
        position: Position { x: 50.0, y: 100.0 },
    });
    scenario.cleanup.proxies.push(Proxy {
        id: "cleanup-proxy".into(),
        source: ProxySource::Variable {
            node_id: scenario.cleanup.nodes[0].id.clone(),
        },
        position: Position { x: 200.0, y: 300.0 },
    });
    LastValidSnapshot::capture(
        DocumentKey::Working {
            document_id: manifest().documents[0].document_id.clone(),
        },
        SOURCE,
        CacheProvenance {
            session_epoch: "session-one".into(),
            source_version: 7,
            saved_source_hash: Some(transaction::hash(b"scenario \"Main\" {}\n")),
        },
        IdentityBase::Scenario(scenario),
        &context,
    )
    .unwrap()
}

fn setup() -> (tempfile::TempDir, ProjectStore, LastValidRead) {
    let directory = tempfile::tempdir().unwrap();
    let store = ProjectStore::open_exclusive(directory.path()).unwrap();
    initialise(&store);
    let read = store.load_last_valid_cache(&manifest().id).unwrap();
    (directory, store, read)
}

#[test]
fn cache_round_trip_keeps_exact_source_provenance_identity_layout_and_cleanup_proxies() {
    let (directory, store, read) = setup();
    let expected = snapshot();
    assert!(read.snapshots().is_empty());
    assert_eq!(read.notice(), None);
    store
        .save_last_valid_cache(&read, std::slice::from_ref(&expected))
        .unwrap();
    drop(store);
    let reader = ProjectStore::open_read_only(directory.path()).unwrap();
    let loaded = reader.load_last_valid_cache(&manifest().id).unwrap();
    assert_eq!(loaded.notice(), None);
    let actual = &loaded.snapshots()[0];
    assert_eq!(actual.source(), SOURCE);
    assert_eq!(actual.source_hash(), transaction::hash(SOURCE.as_bytes()));
    assert_eq!(actual.document(), expected.document());
    assert_eq!(actual.context(), expected.context());
    assert_eq!(actual.provenance(), expected.provenance());
    assert_eq!(actual.snapshot_digest(), expected.snapshot_digest());
    assert!(actual.matches_source(SOURCE));
    assert!(!actual.matches_source(&SOURCE.replace("\r\n", "\n")));
    assert_eq!(
        fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
        b"scenario \"Main\" {}\n"
    );
    assert_eq!(reader.read(&[]).unwrap().generation, 1);
}

#[test]
fn invalid_current_source_never_gets_replaced_and_last_valid_keeps_its_own_configuration() {
    let (directory, store, read) = setup();
    let mut expected = snapshot();
    let mut old_context = context();
    old_context.environment.base_url = "https://old.invalid".into();
    expected = LastValidSnapshot::capture(
        expected.key().clone(),
        SOURCE,
        expected.provenance().clone(),
        expected.document().clone(),
        &old_context,
    )
    .unwrap();
    store.save_last_valid_cache(&read, &[expected]).unwrap();
    let (mut registry, disk) = store.load().unwrap();
    registry.environment.base_url = "https://new.invalid".into();
    store
        .commit(vec![
            write(
                "scenarios/main.afs",
                FileState::of(b"scenario \"Main\" {}\n"),
                b"scenario { unfinished",
            ),
            write(
                MANIFEST_PATH,
                disk.state(&path(MANIFEST_PATH)).unwrap(),
                &registry.to_public_bytes().unwrap(),
            ),
        ])
        .unwrap();
    let cache = store.load_last_valid_cache(&manifest().id).unwrap();
    assert_eq!(cache.notice(), None);
    let candidate = &cache.snapshots()[0];
    assert_eq!(
        candidate.context().environment.base_url,
        "https://old.invalid"
    );
    let mut current = context();
    current.environment = registry.environment;
    assert!(!candidate.matches_context(&current));
    assert!(!candidate.matches_source("scenario { unfinished"));
    assert_eq!(
        fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
        b"scenario { unfinished"
    );
}

#[test]
fn changed_hashes_models_layout_source_and_provenance_are_ignored_and_preserved() {
    for field in [
        "sourceHash",
        "contextDigest",
        "semanticDigest",
        "snapshotDigest",
        "source",
        "layout",
        "model",
        "provenance",
    ] {
        let (directory, store, read) = setup();
        store.save_last_valid_cache(&read, &[snapshot()]).unwrap();
        let file = directory.path().join(CACHE_PATH);
        let mut json: serde_json::Value =
            serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        match field {
            "source" => json["entries"][0]["source"] = SOURCE.replace("= 2", "= 4").into(),
            "layout" => {
                json["entries"][0]["identityBase"]["document"]["graph"]["nodes"][0]["position"]["x"] =
                    999.into()
            }
            "model" => {
                json["entries"][0]["identityBase"]["document"]["graph"]["nodes"][0]["inputs"]["value"]
                    ["value"] = 4.into()
            }
            "provenance" => json["entries"][0]["provenance"]["sourceVersion"] = 999.into(),
            _ => json["entries"][0][field] = "0".repeat(64).into(),
        }
        let bytes = serde_json::to_vec(&json).unwrap();
        fs::write(&file, &bytes).unwrap();
        let cache = store.load_last_valid_cache(&manifest().id).unwrap();
        assert_eq!(cache.notice(), Some(CacheNotice::Invalid), "{field}");
        assert!(cache.snapshots().is_empty());
        assert!(matches!(
            store.save_last_valid_cache(&cache, &[snapshot()]),
            Err(ProjectError::CacheReplacementRequired)
        ));
        assert_eq!(fs::read(&file).unwrap(), bytes);
        store.load().unwrap();
    }
}

#[test]
fn unsupported_foreign_missing_and_oversized_cache_do_not_break_authoritative_storage_reads() {
    let (directory, store, read) = setup();
    store.save_last_valid_cache(&read, &[snapshot()]).unwrap();
    let file = directory.path().join(CACHE_PATH);
    let original = fs::read(&file).unwrap();
    for field in ["formatVersion", "modelVersion", "cacheVersion"] {
        let mut json: serde_json::Value = serde_json::from_slice(&original).unwrap();
        if field == "formatVersion" {
            json[field] = 99.into();
        } else {
            json["entries"][0][field] = 99.into();
        }
        let bytes = serde_json::to_vec(&json).unwrap();
        fs::write(&file, &bytes).unwrap();
        assert_eq!(
            store
                .load_last_valid_cache(&manifest().id)
                .unwrap()
                .notice(),
            Some(CacheNotice::Unsupported)
        );
        assert_eq!(fs::read(&file).unwrap(), bytes);
        store.load().unwrap();
    }
    let mut foreign = snapshot().entry;
    foreign.project_id = "another-project".into();
    foreign.context.id = foreign.project_id.clone();
    foreign.context_digest = context_digest(&foreign.context).unwrap();
    foreign.snapshot_digest = snapshot_digest(&foreign).unwrap();
    fs::write(
        &file,
        super::super::codec::encode(
            &CacheFile {
                format_version: 1,
                entries: vec![foreign],
            },
            MAX_JSON_BYTES,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        store
            .load_last_valid_cache(&manifest().id)
            .unwrap()
            .notice(),
        Some(CacheNotice::Foreign)
    );
    fs::write(&file, vec![b' '; MAX_JSON_BYTES + 1]).unwrap();
    assert_eq!(
        store
            .load_last_valid_cache(&manifest().id)
            .unwrap()
            .notice(),
        Some(CacheNotice::TooLarge)
    );
    store.load().unwrap();
    fs::remove_file(&file).unwrap();
    let missing = store.load_last_valid_cache(&manifest().id).unwrap();
    assert_eq!(missing.notice(), None);
    assert!(missing.snapshots().is_empty());
}

#[test]
fn source_model_disagreement_and_unqualified_calls_cannot_be_sealed_as_last_valid() {
    let snapshot = snapshot();
    assert!(matches!(
        LastValidSnapshot::capture(
            snapshot.key().clone(),
            &SOURCE.replace("= 2", "= 5"),
            snapshot.provenance().clone(),
            snapshot.document().clone(),
            snapshot.context()
        ),
        Err(ProjectError::InvalidCache)
    ));
    let project = apiforge_core::import_document(
        include_bytes!("../../../../fixtures/workspace.json"),
        &apiforge_core::ImportLimits::default(),
    )
    .unwrap()
    .into_workspace()
    .projects
    .remove(0);
    let scenario = &project.scenarios[0];
    let source = apiforge_script::print_scenario(&project, scenario).unwrap();
    let snapshot = LastValidSnapshot::capture(
        DocumentKey::Working {
            document_id: "fixture-document".into(),
        },
        &source,
        snapshot.provenance().clone(),
        IdentityBase::Scenario(scenario.clone()),
        &project,
    )
    .unwrap();
    assert!(!snapshot.context().components.is_empty());
    let component = &snapshot.context().components[0];
    let unpinned = source.replace(
        &format!("{}@{}", component.handle, component.revision),
        &component.handle,
    );
    assert_ne!(source, unpinned);
    assert!(
        LastValidSnapshot::capture(
            snapshot.key().clone(),
            &unpinned,
            snapshot.provenance().clone(),
            snapshot.document().clone(),
            &project
        )
        .is_err()
    );
}

#[test]
fn component_only_authoring_and_a_working_component_calling_its_older_revision_need_no_placeholder()
{
    let mut context = context();
    let source = "component Helper \"Helper\" { export result: number = 2 }";
    let lowered = apiforge_script::lower(
        &apiforge_script::parse(source),
        &LowerContext::folder(&context),
    );
    assert!(!lowered.has_errors());
    let Some(Document::Component(mut published)) = lowered.document else {
        panic!("component")
    };
    published.id = "helper".into();
    let key = DocumentKey::Working {
        document_id: "helper-working".into(),
    };
    let provenance = snapshot().provenance().clone();
    let first = LastValidSnapshot::capture(
        key.clone(),
        source,
        provenance.clone(),
        IdentityBase::Component(published.clone()),
        &context,
    )
    .unwrap();
    assert!(first.context().scenarios.is_empty());
    assert!(first.context().components.is_empty());
    context.components.push(published.clone());
    let working_source = "component Helper \"Helper\" { previous = Helper@1() export result: number = previous.result }";
    let lowered = apiforge_script::lower(
        &apiforge_script::parse(working_source),
        &LowerContext::folder(&context).with_component(&published),
    );
    assert!(!lowered.has_errors(), "{:?}", lowered.diagnostics);
    let Some(Document::Component(working)) = lowered.document else {
        panic!("working")
    };
    let cached = LastValidSnapshot::capture(
        key,
        working_source,
        provenance,
        IdentityBase::Component(working.clone()),
        &context,
    )
    .unwrap();
    assert_eq!(cached.document(), &IdentityBase::Component(working));
    assert_eq!(cached.context().components, vec![published]);
    assert!(cached.context().scenarios.is_empty());
}

#[test]
fn read_only_and_changed_cache_or_manifest_baselines_never_acknowledge_a_save() {
    let (directory, _store, read) = setup();
    let reader = ProjectStore::open(directory.path()).unwrap();
    assert!(reader.is_read_only());
    assert!(matches!(
        reader.save_last_valid_cache(&read, &[snapshot()]),
        Err(ProjectError::ReadOnly)
    ));
    for target in [CACHE_PATH, MANIFEST_PATH] {
        let (directory, store, read) = setup();
        fs::write(directory.path().join(target), b"foreign").unwrap();
        assert!(store.save_last_valid_cache(&read, &[snapshot()]).is_err());
        assert_eq!(fs::read(directory.path().join(target)).unwrap(), b"foreign");
    }
    assert!(!directory.path().join(CACHE_PATH).exists());
}

#[test]
fn known_session_secrets_are_stripped_from_context_and_refused_in_source_or_model() {
    let (directory, store, read) = setup();
    let snapshot = snapshot();
    let mut context = context();
    context.environment.secret_names.push("token".into());
    context
        .environment
        .variables
        .insert("token".into(), "runtime-secret-value".into());
    let cached = LastValidSnapshot::capture(
        snapshot.key().clone(),
        SOURCE,
        snapshot.provenance().clone(),
        snapshot.document().clone(),
        &context,
    )
    .unwrap();
    assert!(!cached.context().environment.variables.contains_key("token"));
    store.set_known_secrets(["runtime-secret-value".into()]);
    let saved = store.save_last_valid_cache(&read, &[cached]).unwrap();
    assert!(
        !String::from_utf8(fs::read(directory.path().join(CACHE_PATH)).unwrap())
            .unwrap()
            .contains("runtime-secret-value")
    );
    store.set_known_secrets(["Main".into()]);
    assert!(matches!(
        store.save_last_valid_cache(&saved, &[snapshot]),
        Err(ProjectError::Secret)
    ));
}

#[test]
fn independent_dependency_versions_and_configuration_are_inside_the_snapshot_digest() {
    let project = apiforge_core::import_document(
        include_bytes!("../../../../fixtures/workspace.json"),
        &apiforge_core::ImportLimits::default(),
    )
    .unwrap()
    .into_workspace()
    .projects
    .remove(0);
    let document = &project.scenarios[0];
    let source = apiforge_script::print_scenario(&project, document).unwrap();
    let cached = LastValidSnapshot::capture(
        DocumentKey::Working {
            document_id: "fixture".into(),
        },
        &source,
        snapshot().provenance().clone(),
        IdentityBase::Scenario(document.clone()),
        &project,
    )
    .unwrap();
    assert!(cached.matches_context(&project));
    let mut new_context = project.clone();
    new_context.environment.base_url.push_str("/changed");
    assert!(!cached.matches_context(&new_context));
    let mut newer = project.components[0].clone();
    newer.revision = 999;
    newer.name.push_str(" newer");
    new_context = project.clone();
    new_context.components.push(newer);
    assert!(
        cached.matches_context(&new_context),
        "unreferenced publication does not change pinned closure"
    );
    new_context.components.retain(|component| {
        component.id != cached.context().components[0].id
            || component.revision != cached.context().components[0].revision
    });
    assert!(
        !cached.matches_context(&new_context),
        "missing exact dependency never selects latest"
    );
}

#[test]
fn explicit_cache_rebuild_preserves_opaque_old_bytes_and_checks_the_preview() {
    for original in [
        b"invalid cache".as_slice(),
        b"{\"formatVersion\":999,\"future\":true}".as_slice(),
    ] {
        let (directory, store, _) = setup();
        fs::write(directory.path().join(CACHE_PATH), original).unwrap();
        let preview = store.load_last_valid_cache(&manifest().id).unwrap();
        assert!(preview.notice().is_some());
        let rebuilt = store
            .replace_last_valid_cache(&preview, &[snapshot()])
            .unwrap();
        assert_eq!(rebuilt.notice(), None);
        assert_eq!(rebuilt.snapshots().len(), 1);
        assert_eq!(
            fs::read(
                directory
                    .path()
                    .join(rebuilt.preserved_copy().unwrap().as_str())
            )
            .unwrap(),
            original
        );
        assert_eq!(store.read(&[]).unwrap().generation, 1);
        assert!(
            store
                .replace_last_valid_cache(&preview, &[snapshot()])
                .is_err()
        );
    }
}

#[test]
fn duplicate_entries_bad_keys_and_source_limits_cannot_be_saved() {
    let (directory, store, read) = setup();
    let snapshot = snapshot();
    assert!(matches!(
        store.save_last_valid_cache(&read, &[snapshot.clone(), snapshot.clone()]),
        Err(ProjectError::InvalidCache)
    ));
    let mut key = snapshot.key().clone();
    if let DocumentKey::Working { document_id } = &mut key {
        *document_id = "foreign-document".into();
    }
    let foreign = LastValidSnapshot::capture(
        key,
        SOURCE,
        snapshot.provenance().clone(),
        snapshot.document().clone(),
        snapshot.context(),
    )
    .unwrap();
    assert!(store.save_last_valid_cache(&read, &[foreign]).is_err());
    let oversized = " ".repeat(MAX_SOURCE_BYTES + 1);
    assert!(matches!(
        LastValidSnapshot::capture(
            snapshot.key().clone(),
            &oversized,
            snapshot.provenance().clone(),
            snapshot.document().clone(),
            snapshot.context()
        ),
        Err(ProjectError::Limit)
    ));
    assert!(!directory.path().join(CACHE_PATH).exists());
}

#[test]
fn encoded_cache_limit_reports_failure_without_truncation_or_source_changes() {
    let (directory, store, read) = setup();
    let original = snapshot();
    let saved = store
        .save_last_valid_cache(&read, std::slice::from_ref(&original))
        .unwrap();
    let bytes = fs::read(directory.path().join(CACHE_PATH)).unwrap();
    let mut context = context();
    for index in 0..28 {
        context
            .environment
            .variables
            .insert(format!("large_{index}"), "x".repeat(100_000));
    }
    let large = LastValidSnapshot::capture(
        original.key().clone(),
        SOURCE,
        original.provenance().clone(),
        original.document().clone(),
        &context,
    )
    .unwrap();
    assert!(matches!(
        store.save_last_valid_cache(&saved, &vec![large; 6]),
        Err(ProjectError::Limit)
    ));
    assert_eq!(fs::read(directory.path().join(CACHE_PATH)).unwrap(), bytes);
    assert_eq!(
        fs::read(directory.path().join("scenarios/main.afs")).unwrap(),
        b"scenario \"Main\" {}\n"
    );
}
