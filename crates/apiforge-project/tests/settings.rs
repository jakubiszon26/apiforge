use apiforge_project::*;
use apiforge_storage::project::{FileState, MANIFEST_PATH};

fn project() -> (tempfile::TempDir, ProjectSession) {
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &apiforge_core::demos::shop_project()).unwrap();
    let session = ProjectSession::open(directory.path()).unwrap();
    (directory, session)
}

#[test]
fn settings_cannot_remove_dependencies_of_immutable_revisions() {
    let (_directory, mut session) = project();
    let before = session.manifest().clone();
    assert!(matches!(
        session.edit_configuration(|manifest| manifest.contracts.clear(), Origin::SESSION),
        Err(Error::InvalidRevision { .. })
    ));
    assert_eq!(session.manifest(), &before);
    assert!(!session.is_manifest_dirty());
}

#[test]
fn configuration_validation_uses_actual_published_source_including_contract_name_aliases() {
    let (directory, session) = project();
    let mut manifest = session.manifest().clone();
    drop(session);
    let contract = manifest
        .contracts
        .iter_mut()
        .find(|contract| contract.id == "Product")
        .unwrap();
    contract.name = "FriendlyProduct".into();
    let mut changed = false;
    for revision in &mut manifest.revisions {
        let path = directory.path().join(revision.path.as_str());
        let original = std::fs::read_to_string(&path).unwrap();
        let source = original.replace("contract \"Product\"", "contract \"FriendlyProduct\"");
        if source != original {
            changed = true;
            std::fs::write(path, &source).unwrap();
            let FileState::File { sha256, .. } = FileState::of(source.as_bytes()) else {
                unreachable!()
            };
            revision.source_hash = sha256;
        }
    }
    assert!(
        changed,
        "the fixture has a published request with a Product contract"
    );
    std::fs::write(
        directory.path().join(MANIFEST_PATH),
        manifest.to_public_bytes().unwrap(),
    )
    .unwrap();
    let mut session = ProjectSession::open(directory.path()).unwrap();
    assert!(matches!(
        session.edit_configuration(
            |manifest| manifest
                .contracts
                .iter_mut()
                .find(|contract| contract.id == "Product")
                .unwrap()
                .name = "AnotherName".into(),
            Origin::SESSION
        ),
        Err(Error::InvalidRevision { .. })
    ));
    assert_eq!(session.manifest(), &manifest);
}
