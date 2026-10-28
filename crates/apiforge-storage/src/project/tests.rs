use super::*;
use apiforge_core::format::Environment;
use std::fs;

pub(super) fn manifest() -> ProjectManifest {
    ProjectManifest {
        format_version: 1,
        id: "project-existing-id".into(),
        name: "Folder project".into(),
        environment: Environment {
            base_url: String::new(),
            variables: Default::default(),
            secret_names: vec![],
        },
        contracts: vec![],
        documents: vec![DocumentEntry {
            document_id: "c6103f71-9854-4e30-b009-1f796bd484ce".into(),
            kind: DocumentKind::Scenario,
            path: ProjectPath::working_source("scenarios/main.afs").unwrap(),
            entity_id: "scenario-existing-id".into(),
            base_revision: None,
            recovered: false,
        }],
        revisions: vec![],
    }
}

pub(super) fn path(value: &str) -> ProjectPath {
    ProjectPath::new(value).unwrap()
}

pub(super) fn write(value: &str, expected: FileState, bytes: &[u8]) -> ProjectChange {
    ProjectChange::Write {
        path: path(value),
        expected,
        bytes: bytes.to_vec(),
    }
}

pub(super) fn initialise(store: &ProjectStore) {
    store
        .commit(vec![
            ProjectChange::CreateDirectory {
                path: path("scenarios"),
            },
            write(
                "scenarios/main.afs",
                FileState::Missing,
                b"scenario \"Main\" {}\n",
            ),
            write(
                MANIFEST_PATH,
                FileState::Missing,
                &manifest().to_public_bytes().unwrap(),
            ),
        ])
        .unwrap();
}

#[test]
fn read_only_checkout_does_not_create_local_infrastructure() {
    let directory = tempfile::tempdir().unwrap();
    let store = ProjectStore::open_read_only(directory.path()).unwrap();
    assert!(store.is_read_only());
    assert_eq!(store.read(&[]).unwrap().generation, 0);
    assert!(matches!(store.commit(vec![]), Err(ProjectError::ReadOnly)));
    assert!(matches!(store.recover(), Err(ProjectError::ReadOnly)));
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn writer_lock_is_retained_by_clones_and_never_replaced() {
    let directory = tempfile::tempdir().unwrap();
    let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
    let worker = writer.clone();
    drop(writer);
    assert!(matches!(
        ProjectStore::open_exclusive(directory.path()),
        Err(ProjectError::Locked)
    ));
    let reader = ProjectStore::open(directory.path()).unwrap();
    assert!(reader.is_read_only());
    drop(worker);
    let next = ProjectStore::open_exclusive(directory.path()).unwrap();
    assert!(!next.is_read_only());
}

#[test]
fn saved_sources_round_trip_exactly_including_invalid_text() {
    let directory = tempfile::tempdir().unwrap();
    let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
    assert!(
        !directory.path().join(MANIFEST_PATH).exists(),
        "opening is not initialisation"
    );
    initialise(&writer);
    let (_, snapshot) = writer.load().unwrap();
    assert_eq!(snapshot.generation, 1);
    let source = path("scenarios/main.afs");
    let invalid = b"// unfinished work\nscenario \"Main\" {\n";
    assert_eq!(
        writer
            .commit(vec![write(
                source.as_str(),
                snapshot.state(&source).unwrap(),
                invalid
            )])
            .unwrap(),
        2
    );
    drop(writer);
    let reader = ProjectStore::open_read_only(directory.path()).unwrap();
    let (loaded, snapshot) = reader.load().unwrap();
    assert_eq!(loaded, manifest());
    assert_eq!(snapshot.files[&source].as_deref(), Some(invalid.as_slice()));
    assert_eq!(snapshot.generation, 2);
}

#[test]
fn equal_length_external_edits_and_missing_targets_stop_saves() {
    for external in [Some(b"scenario \"Else\" {}\n".as_slice()), None] {
        let directory = tempfile::tempdir().unwrap();
        let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
        initialise(&writer);
        let (_, snapshot) = writer.load().unwrap();
        let source = path("scenarios/main.afs");
        let file = directory.path().join(source.as_str());
        if let Some(bytes) = external {
            fs::write(&file, bytes).unwrap();
        } else {
            fs::remove_file(&file).unwrap();
        }
        assert!(matches!(
            writer.commit(vec![write(
                source.as_str(),
                snapshot.state(&source).unwrap(),
                b"local edit"
            )]),
            Err(ProjectError::Conflict { .. })
        ));
        assert_eq!(fs::read(&file).ok().as_deref(), external);
        assert_eq!(writer.read(&[]).unwrap().generation, 1);
    }
}

#[test]
fn schema_rejects_unknown_versions_fields_and_collisions() {
    let original = manifest();
    let bytes = original.to_public_bytes().unwrap();
    let duplicate_map = String::from_utf8(bytes.clone()).unwrap().replace(
        "\"variables\": {}",
        "\"variables\": {\"value\": \"first\", \"value\": \"second\"}",
    );
    assert!(matches!(
        ProjectManifest::from_bytes(duplicate_map.as_bytes()),
        Err(ProjectError::InvalidMetadata)
    ));
    assert_eq!(ProjectManifest::from_bytes(&bytes).unwrap(), original);
    let mut json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    json["formatVersion"] = 2.into();
    assert!(matches!(
        ProjectManifest::from_bytes(&serde_json::to_vec(&json).unwrap()),
        Err(ProjectError::UnsupportedVersion)
    ));
    json["formatVersion"] = 1.into();
    json["surprise"] = true.into();
    assert!(matches!(
        ProjectManifest::from_bytes(&serde_json::to_vec(&json).unwrap()),
        Err(ProjectError::InvalidMetadata)
    ));

    let mut duplicate = original.clone();
    let mut entry = duplicate.documents[0].clone();
    entry.path = path("scenarios/MAIN.afs");
    entry.document_id = "another-document".into();
    entry.entity_id = "another-scenario".into();
    duplicate.documents.push(entry);
    assert!(duplicate.validate().is_err());
    let mut empty = original;
    empty.documents.clear();
    empty.validate().unwrap();
}

#[test]
fn recovered_component_drafts_share_entity_but_not_document_identity() {
    let mut manifest = manifest();
    let entry = &mut manifest.documents[0];
    entry.kind = DocumentKind::Component;
    entry.path = path("components/working.afs");
    let mut recovered = entry.clone();
    recovered.document_id = "recovered-document".into();
    recovered.path = path("components/recovered.afs");
    recovered.recovered = true;
    manifest.documents.push(recovered);
    manifest.validate().unwrap();
    manifest.documents[1].recovered = false;
    assert!(manifest.validate().is_err());
    manifest.documents[1].recovered = true;
    manifest.documents[0].base_revision = Some(1);
    assert!(manifest.validate().is_err(), "base revision must exist");
}

#[test]
fn registry_configuration_uses_authoring_validation_before_any_new_files_are_staged() {
    let directory = tempfile::tempdir().unwrap();
    let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
    let mut invalid = manifest();
    invalid
        .environment
        .variables
        .insert("token".into(), "x".repeat(100_001));
    let bytes = serde_json::to_vec(&invalid).unwrap();
    assert!(matches!(
        writer.commit(vec![write(MANIFEST_PATH, FileState::Missing, &bytes)]),
        Err(ProjectError::InvalidMetadata)
    ));
    assert!(!directory.path().join(MANIFEST_PATH).exists());
    assert_eq!(
        fs::read_dir(directory.path().join(".apiforge/local/transactions"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn path_policy_rejects_portability_and_control_file_hazards() {
    for value in [
        "",
        "/tmp/x",
        "../x",
        "scenarios/../x.afs",
        "a//b",
        "a\\b",
        "C:/x",
        "a.",
        "a ",
        "NUL.afs",
        "dir/COM1.afs",
        "lpt².afs",
        ".git/config",
        "dir/.Git/config",
        ".apiforge/local/project.lock",
        ".apiforge/local/head.json",
        ".apiforge/local/transactions/id/transaction.json",
        ".APIFORGE/diagram.json",
        ".apiforge/revisions/x/.APIFORGE/a.afs",
    ] {
        assert!(ProjectPath::new(value).is_err(), "accepted {value}");
    }
    for value in [
        "scenarios/main.afs",
        "other/new.afs",
        ".gitignore",
        ".apiforge/diagram.json",
        ".apiforge/revisions/opaque-id/1.afs",
        ".apiforge/local/trash/id/source.afs",
    ] {
        ProjectPath::new(value).unwrap();
    }
    assert!(ProjectPath::working_source(".apiforge/revisions/x/1.afs").is_err());
    assert!(ProjectPath::working_source("notes.txt").is_err());
    assert!(serde_json::from_str::<ProjectPath>("\"../escape\"").is_err());
}

#[test]
fn case_collisions_and_corrupt_manifests_block_writes() {
    let directory = tempfile::tempdir().unwrap();
    let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
    initialise(&writer);
    assert!(matches!(
        writer.commit(vec![write(
            "scenarios/MAIN.afs",
            FileState::Missing,
            b"collision"
        )]),
        Err(ProjectError::UnsafePath { .. })
    ));
    fs::write(directory.path().join(MANIFEST_PATH), b"{broken").unwrap();
    assert!(matches!(
        writer.commit(vec![write("another.afs", FileState::Missing, b"text")]),
        Err(ProjectError::InvalidMetadata)
    ));
    assert!(!directory.path().join("another.afs").exists());
}

#[cfg(unix)]
#[test]
fn symlink_roots_parents_and_targets_are_rejected_without_touching_referents() {
    use std::os::unix::fs::symlink;
    let directory = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
    initialise(&writer);
    symlink(outside.path(), directory.path().join("linked")).unwrap();
    assert!(matches!(
        writer.commit(vec![write("linked/escape.afs", FileState::Missing, b"no")]),
        Err(ProjectError::UnsafePath { .. })
    ));
    let source = directory.path().join("scenarios/main.afs");
    fs::remove_file(&source).unwrap();
    fs::write(outside.path().join("source.afs"), b"foreign").unwrap();
    symlink(outside.path().join("source.afs"), &source).unwrap();
    assert!(writer.load().is_err());
    assert_eq!(
        fs::read(outside.path().join("source.afs")).unwrap(),
        b"foreign"
    );
    let root_link = directory.path().join("root-link");
    symlink(outside.path(), &root_link).unwrap();
    assert!(ProjectStore::open_exclusive(&root_link).is_err());
    assert!(!outside.path().join(".apiforge").exists());
}

#[test]
fn known_secrets_are_rejected_before_staging_and_not_echoed() {
    let directory = tempfile::tempdir().unwrap();
    let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
    initialise(&writer);
    let secret = "session-secret-\"\\\n";
    writer.set_known_secrets([secret.into(), String::new()]);
    for bytes in [
        secret.as_bytes().to_vec(),
        serde_json::to_vec(&vec![secret]).unwrap(),
    ] {
        let error = writer
            .commit(vec![write("secret.afs", FileState::Missing, &bytes)])
            .unwrap_err();
        assert!(matches!(error, ProjectError::Secret));
        assert!(!error.to_string().contains(secret));
    }
    assert_eq!(
        fs::read_dir(directory.path().join(".apiforge/local/transactions"))
            .unwrap()
            .count(),
        0
    );
    let mut manifest = manifest();
    manifest.environment.secret_names.push("token".into());
    manifest
        .environment
        .variables
        .insert("token".into(), secret.into());
    let public = manifest.to_public_bytes().unwrap();
    assert!(
        !String::from_utf8(public)
            .unwrap()
            .contains("session-secret")
    );
}

#[test]
fn published_sources_are_hash_checked_and_immutable() {
    let directory = tempfile::tempdir().unwrap();
    let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
    initialise(&writer);
    let mut manifest = manifest();
    let source = b"component Helper() {}\n";
    let FileState::File { sha256, .. } = FileState::of(source) else {
        unreachable!()
    };
    manifest.revisions.push(RevisionEntry {
        component_id: "component-helper".into(),
        handle: "Helper".into(),
        revision: 1,
        path: path(".apiforge/revisions/component-helper/1.afs"),
        source_hash: sha256,
    });
    let (_, snapshot) = writer.load().unwrap();
    writer
        .commit(vec![
            ProjectChange::CreateDirectory {
                path: path(".apiforge/revisions"),
            },
            ProjectChange::CreateDirectory {
                path: path(".apiforge/revisions/component-helper"),
            },
            write(
                ".apiforge/revisions/component-helper/1.afs",
                FileState::Missing,
                source,
            ),
            write(
                MANIFEST_PATH,
                snapshot.state(&path(MANIFEST_PATH)).unwrap(),
                &manifest.to_public_bytes().unwrap(),
            ),
        ])
        .unwrap();
    writer.load().unwrap();
    assert!(matches!(
        writer.commit(vec![write(
            ".apiforge/revisions/component-helper/1.afs",
            FileState::of(source),
            b"replacement"
        )]),
        Err(ProjectError::ImmutableRevision)
    ));
    assert!(matches!(
        writer.commit(vec![ProjectChange::RemoveFile {
            path: path(".apiforge/revisions/component-helper/1.afs"),
            expected: FileState::of(source)
        }]),
        Err(ProjectError::ImmutableRevision)
    ));
    fs::write(
        directory
            .path()
            .join(".apiforge/revisions/component-helper/1.afs"),
        b"external tampering",
    )
    .unwrap();
    assert!(matches!(writer.load(), Err(ProjectError::InvalidMetadata)));
}

#[test]
fn transaction_limits_fail_without_activation_or_source_changes() {
    let directory = tempfile::tempdir().unwrap();
    let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
    initialise(&writer);
    let oversized = vec![b' '; MAX_SOURCE_BYTES + 1];
    assert!(
        writer
            .commit(vec![write("too-big.afs", FileState::Missing, &oversized)])
            .is_err()
    );
    let operations = (0..=MAX_TRANSACTION_OPERATIONS)
        .map(|index| write(&format!("file-{index}.afs"), FileState::Missing, b""))
        .collect();
    assert!(matches!(
        writer.commit(operations),
        Err(ProjectError::Limit)
    ));
    assert_eq!(writer.read(&[]).unwrap().generation, 1);
    assert!(!directory.path().join("too-big.afs").exists());
    // The exact 1 MiB source limit is supported without truncation.
    writer
        .commit(vec![write(
            "limit.afs",
            FileState::Missing,
            &oversized[..MAX_SOURCE_BYTES],
        )])
        .unwrap();
    assert_eq!(
        writer.read(&[path("limit.afs")]).unwrap().files[&path("limit.afs")]
            .as_ref()
            .unwrap()
            .len(),
        MAX_SOURCE_BYTES
    );
}

#[test]
fn readers_can_probe_for_a_live_writer_without_creating_its_lock() {
    let directory = tempfile::tempdir().unwrap();
    let reader = ProjectStore::open_read_only(directory.path()).unwrap();
    assert!(!reader.has_live_writer().unwrap());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
    assert!(reader.has_live_writer().unwrap());
    drop(writer);
    assert!(!reader.has_live_writer().unwrap());
    // The probe never prevents a new writer.
    assert!(!ProjectStore::open(directory.path()).unwrap().is_read_only());
}

#[cfg(unix)]
#[test]
fn an_unwritable_untouched_folder_opens_read_only_without_local_data() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o555)).unwrap();
    // Privileged users bypass directory permissions; nothing to verify then.
    if fs::create_dir(directory.path().join("probe")).is_ok() {
        return;
    }
    let store = ProjectStore::open(directory.path()).unwrap();
    assert!(store.is_read_only());
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
}

// ----- MCP-07: the lock names its holder ----------------------------------------

const LOCK: &str = ".apiforge/local/project.lock";
const HOLDER: &str = ".apiforge/local/holder.json";

/// Criterion 1: the writer describes itself in one JSON line beside the lock,
/// and a reader returns that description while the writer lives. The lock
/// file itself stays empty: on Windows it cannot be read while it is held.
#[test]
fn mcp07_a_writer_describes_itself_and_a_reader_returns_the_description() {
    let directory = tempfile::tempdir().unwrap();
    let writer = ProjectStore::open_exclusive_as(
        directory.path(),
        &Holder::new(HolderRole::Agent, Some("Claude Code".into())),
    )
    .unwrap();
    let bytes = fs::read(directory.path().join(HOLDER)).unwrap();
    assert_eq!(bytes.iter().filter(|byte| **byte == b'\n').count(), 1);
    assert!(bytes.ends_with(b"\n"), "one JSON line");
    let line: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let object = line.as_object().unwrap();
    assert_eq!(
        object.keys().map(String::as_str).collect::<Vec<_>>(),
        ["version", "program", "role", "client", "since", "endpoint"]
    );
    assert_eq!(line["version"], 1);
    assert_eq!(line["program"], "apiforge");
    assert_eq!(line["role"], "agent");
    assert_eq!(line["client"], "Claude Code");
    assert_eq!(line["endpoint"], serde_json::Value::Null);
    let since = line["since"].as_str().unwrap();
    assert_eq!(since.len(), "2026-10-27T10:42:11Z".len(), "{since}");
    assert!(
        since.ends_with('Z') && since.as_bytes()[10] == b'T',
        "{since}"
    );
    // Held, so only its length: Windows refuses to read a locked file.
    assert_eq!(fs::metadata(directory.path().join(LOCK)).unwrap().len(), 0);

    let reader = ProjectStore::open_read_only(directory.path()).unwrap();
    let holder = reader
        .holder()
        .unwrap()
        .expect("a live writer is described");
    assert_eq!(holder.role(), Some(HolderRole::Agent));
    assert_eq!(holder.client(), Some("Claude Code"));
    assert_eq!(holder.since(), Some(since));
    assert_eq!(holder.endpoint(), None);
    assert!(!holder.is_unknown());
    drop(writer);
}

/// The window and the command line name themselves; `open_exclusive` keeps
/// describing a window for the callers that do not say who they are.
#[test]
fn mcp07_windows_and_commands_name_themselves() {
    let directory = tempfile::tempdir().unwrap();
    let reader = ProjectStore::open_read_only(directory.path()).unwrap();
    let writer =
        ProjectStore::open_exclusive_as(directory.path(), &Holder::command("apply")).unwrap();
    let holder = reader.holder().unwrap().unwrap();
    assert_eq!(holder.role(), Some(HolderRole::Command));
    assert_eq!(holder.client(), Some("apply"));
    drop(writer);
    let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
    let holder = reader.holder().unwrap().unwrap();
    assert_eq!(holder.role(), Some(HolderRole::Window));
    assert_eq!(holder.client(), None);
    drop(writer);
    let writer = ProjectStore::open(directory.path()).unwrap();
    assert!(!writer.is_read_only());
    assert_eq!(
        reader.holder().unwrap().unwrap().role(),
        Some(HolderRole::Window)
    );
}

/// A description left behind by a holder that is gone describes nobody.
#[test]
fn mcp07_a_holder_file_without_a_live_writer_is_ignored() {
    let directory = tempfile::tempdir().unwrap();
    let writer = ProjectStore::open_exclusive_as(
        directory.path(),
        &Holder::new(HolderRole::Agent, Some("Claude Code".into())),
    )
    .unwrap();
    drop(writer);
    assert!(directory.path().join(HOLDER).exists(), "left behind");
    let reader = ProjectStore::open_read_only(directory.path()).unwrap();
    assert_eq!(reader.holder().unwrap(), None);
    // The probe never prevents a new writer, which describes itself anew.
    let next = ProjectStore::open_exclusive_as(directory.path(), &Holder::command("run")).unwrap();
    assert_eq!(
        reader.holder().unwrap().unwrap().role(),
        Some(HolderRole::Command)
    );
    drop(next);
}

/// Criterion 2's storage half: a live writer from an older build wrote no
/// description, or one that cannot be read; the reader says only that some
/// process holds the project.
#[test]
fn mcp07_a_live_writer_without_a_readable_description_is_an_unknown_holder() {
    let directory = tempfile::tempdir().unwrap();
    let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
    let reader = ProjectStore::open_read_only(directory.path()).unwrap();
    fs::remove_file(directory.path().join(HOLDER)).unwrap();
    let holder = reader.holder().unwrap().unwrap();
    assert!(holder.is_unknown());
    assert_eq!(holder, Holder::unknown());
    assert_eq!(holder.role(), None);
    assert_eq!(holder.since(), None);
    for unreadable in [
        &b""[..],
        b"not json\n",
        b"{\"version\":2,\"program\":\"apiforge\",\"role\":\"agent\",\"client\":null,\"since\":\"2026-10-27T10:42:11Z\",\"endpoint\":null}\n",
        b"{\"version\":1,\"program\":\"other\",\"role\":\"agent\",\"client\":null,\"since\":\"2026-10-27T10:42:11Z\",\"endpoint\":null}\n",
        b"{\"version\":1,\"program\":\"apiforge\",\"role\":\"robot\",\"client\":null,\"since\":\"2026-10-27T10:42:11Z\",\"endpoint\":null}\n",
    ] {
        fs::write(directory.path().join(HOLDER), unreadable).unwrap();
        assert_eq!(
            reader.holder().unwrap(),
            Some(Holder::unknown()),
            "{}",
            String::from_utf8_lossy(unreadable)
        );
    }
    drop(writer);
    assert_eq!(reader.holder().unwrap(), None);
}

/// Asking who holds a project writes nothing, as the probe writes nothing.
#[test]
fn mcp07_reading_the_holder_creates_no_local_files() {
    let directory = tempfile::tempdir().unwrap();
    let reader = ProjectStore::open_read_only(directory.path()).unwrap();
    assert_eq!(reader.holder().unwrap(), None);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
}

/// The lock file is never replaced or written: a new holder replaces only
/// the description, and the lock keeps its identity across holders.
#[cfg(unix)]
#[test]
fn mcp07_describing_the_holder_never_replaces_the_lock_file() {
    use std::os::unix::fs::MetadataExt;
    let directory = tempfile::tempdir().unwrap();
    let lock = directory.path().join(LOCK);
    let first =
        ProjectStore::open_exclusive_as(directory.path(), &Holder::command("apply")).unwrap();
    let identity = fs::metadata(&lock).unwrap().ino();
    drop(first);
    let second =
        ProjectStore::open_exclusive_as(directory.path(), &Holder::new(HolderRole::Agent, None))
            .unwrap();
    assert_eq!(fs::metadata(&lock).unwrap().ino(), identity);
    assert_eq!(fs::metadata(&lock).unwrap().len(), 0);
    drop(second);
}

/// Review round 1 of #106: a description this user cannot read (no
/// permission, or another user's private file) is still a live writer whose
/// name is lost, never an error that would hide the bar. Unix only: Windows
/// permissions cannot be modelled with a mode here.
#[cfg(unix)]
#[test]
fn mcp07_an_unreadable_description_with_a_live_writer_is_an_unknown_holder() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    let writer =
        ProjectStore::open_exclusive_as(directory.path(), &Holder::command("apply")).unwrap();
    let description = directory.path().join(HOLDER);
    fs::set_permissions(&description, fs::Permissions::from_mode(0o000)).unwrap();
    // Privileged users read it regardless; then there is nothing to verify.
    if fs::read(&description).is_ok() {
        return;
    }
    let reader = ProjectStore::open_read_only(directory.path()).unwrap();
    assert_eq!(reader.holder().unwrap(), Some(Holder::unknown()));
    drop(writer);
    assert_eq!(reader.holder().unwrap(), None);
}

#[cfg(unix)]
#[test]
fn mcp12_the_uid_is_read_from_holder_file_metadata_and_is_not_serialized() {
    use std::os::unix::fs::MetadataExt;
    let directory = tempfile::tempdir().unwrap();
    let writer = ProjectStore::open_exclusive_as(directory.path(), &Holder::window()).unwrap();
    let path = directory.path().join(HOLDER);
    let before = fs::read(&path).unwrap();
    let found = writer.holder().unwrap().unwrap();
    assert_eq!(found.owner_uid(), Some(fs::metadata(&path).unwrap().uid()));
    assert_eq!(found.to_line().unwrap(), before);
    assert_eq!(Holder::unknown().owner_uid(), None);
    assert!(!String::from_utf8(before).unwrap().contains("uid"));
}
