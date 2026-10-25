//! Real Git merge/checkout evidence and the uncoordinated-writer boundary.
use apiforge_core::format::{Position, ProxySource};
use apiforge_project::*;
use std::{fs, path::Path, process::Command, sync::mpsc};

fn git(root: &Path, args: &[&str]) -> Vec<u8> {
    let result = Command::new("git")
        .args([
            "-c",
            "user.name=Jakub Kołakowski",
            "-c",
            "user.email=jakubkolakowski6@protonmail.com",
        ])
        .args(["-c", "commit.gpgsign=false"])
        .args(["-C", root.to_str().unwrap()])
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    result.stdout
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn initialisation_appends_narrow_lf_rules_without_rewriting_sources_attributes_or_index() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let original = b"* text=auto\r\n*.json -text\r\n# retained without final newline";
    fs::write(root.join(".gitattributes"), original).unwrap();
    fs::write(root.join("README.md"), "existing\r\n").unwrap();
    git(root, &["init", "--quiet"]);
    git(root, &["add", ".gitattributes", "README.md"]);
    git(root, &["commit", "--quiet", "-m", "test fixture"]);
    let index = fs::read(root.join(".git/index")).unwrap();
    let source = "scenario \"Main\" { answer = 1 }\r\n";
    fs::write(root.join("main.afs"), source).unwrap();
    let mut session = ProjectSession::initialise(
        root,
        "Existing",
        &[Registration {
            path: "main.afs".into(),
            kind: DocumentKind::Scenario,
        }],
    )
    .unwrap();
    let attributes = fs::read(root.join(".gitattributes")).unwrap();
    assert!(attributes.starts_with(original));
    assert!(attributes.ends_with(b"*.afs text eol=lf\n/apiforge.project.json text eol=lf\n/.apiforge/diagram.json text eol=lf\n"));
    assert_eq!(fs::read(root.join(".git/index")).unwrap(), index);
    assert_eq!(fs::read(root.join("main.afs")).unwrap(), source.as_bytes());
    let id = session.documents()[0].id().to_owned();
    let edited = source.replace('1', "9");
    session.edit_source(&id, &edited, Origin(1)).unwrap();
    session.save(&id).unwrap();
    assert_eq!(fs::read(root.join("main.afs")).unwrap(), edited.as_bytes());
    assert_eq!(fs::read(root.join(".git/index")).unwrap(), index);
    drop(session);
    fs::write(
        root.join(".gitattributes"),
        b"user rules after creation\r\n",
    )
    .unwrap();
    drop(ProjectSession::open_saved_read_only(root).unwrap());
    assert_eq!(
        fs::read(root.join(".gitattributes")).unwrap(),
        b"user rules after creation\r\n"
    );
}

#[test]
fn actual_autocrlf_checkouts_keep_revisions_valid_only_when_effective_attributes_require_lf() {
    for policy in ["root-lf", "no-rules", "nested-crlf"] {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("original");
        fs::create_dir(&root).unwrap();
        let manifest = prepare_project(&root, &apiforge_core::demos::shop_project()).unwrap();
        let revision = manifest.revisions[0].path.as_str();
        let original = fs::read(root.join(revision)).unwrap();
        assert!(!original.contains(&b'\r'));
        match policy {
            "no-rules" => fs::remove_file(root.join(".gitattributes")).unwrap(),
            "nested-crlf" => fs::write(
                root.join(".apiforge/revisions/.gitattributes"),
                "*.afs text eol=crlf\n",
            )
            .unwrap(),
            _ => {}
        }
        git(&root, &["init", "--quiet"]);
        git(&root, &["config", "core.autocrlf", "false"]);
        git(
            &root,
            &[
                "add",
                "apiforge.project.json",
                ".apiforge/diagram.json",
                ".apiforge/revisions",
                "scenarios",
                "components",
            ],
        );
        if policy != "no-rules" {
            git(&root, &["add", ".gitattributes"]);
        }
        git(&root, &["commit", "--quiet", "-m", "test fixture"]);
        let checkout = directory.path().join("checkout");
        git(
            directory.path(),
            &[
                "clone",
                "--quiet",
                "--no-local",
                "--config",
                "core.autocrlf=true",
                "--config",
                "core.eol=crlf",
                root.to_str().unwrap(),
                checkout.to_str().unwrap(),
            ],
        );
        let bytes = fs::read(checkout.join(revision)).unwrap();
        if policy == "root-lf" {
            assert_eq!(bytes, original);
            let loaded = ProjectSession::open_saved_read_only(&checkout).unwrap();
            assert_eq!(loaded.context().components.len(), manifest.revisions.len());
            assert!(
                !fs::read(checkout.join("apiforge.project.json"))
                    .unwrap()
                    .contains(&b'\r')
            );
            assert!(
                !fs::read(checkout.join(".apiforge/diagram.json"))
                    .unwrap()
                    .contains(&b'\r')
            );
        } else {
            assert!(bytes.windows(2).any(|pair| pair == b"\r\n"), "{policy}");
            assert!(
                matches!(
                    ProjectSession::open_saved_read_only(&checkout),
                    Err(Error::Storage(ProjectError::InvalidMetadata))
                ),
                "{policy}: immutable integrity must fail explicitly"
            );
        }
        assert_eq!(fs::read(root.join(revision)).unwrap(), original);
    }
}

fn edit_document(root: &Path, id: &str, answer: &str, offset: f64) -> Document {
    let mut session = ProjectSession::open(root).unwrap();
    let text = session
        .document(id)
        .unwrap()
        .text()
        .replace("seed = 1", &format!("seed = {answer}"));
    session.edit_source(id, &text, Origin(1)).unwrap();
    session.settle(id).unwrap();
    let mut model = session.document(id).unwrap().model().unwrap().clone();
    let Document::Scenario(scenario) = &mut model else {
        panic!()
    };
    scenario.graph.nodes[0].position = Position { x: offset, y: 88.0 };
    scenario.cleanup.nodes[0].position = Position {
        x: offset,
        y: 144.0,
    };
    scenario.graph.nodes[0].config.variable = Some(true);
    scenario.graph = apiforge_core::edit::add_proxy(
        &scenario.graph,
        ProxySource::Variable {
            node_id: scenario.graph.nodes[0].id.clone(),
        },
        Position {
            x: offset + 240.0,
            y: 88.0,
        },
    )
    .unwrap();
    session.apply_model_edit(id, model, Origin(2)).unwrap();
    session.save(id).unwrap();
    session.document(id).unwrap().model().unwrap().clone()
}

#[test]
fn independent_source_identity_and_layout_edits_merge_in_one_checkpoint_file_and_reload_exactly() {
    let directory = tempfile::tempdir().unwrap();
    let base = directory.path().join("base");
    fs::create_dir(&base).unwrap();
    let mut session = ProjectSession::create(&base, "Merge").unwrap();
    let alpha = session.create_document(DocumentKind::Scenario, "alpha.afs", "scenario \"Alpha\" {\n  seed = 1\n  result = seed + 2\n  cleanup { cleaned = seed }\n}\n").unwrap();
    let beta = session.create_document(DocumentKind::Scenario, "beta.afs", "scenario \"Beta\" {\n  seed = 1\n  result = seed + 3\n  cleanup { cleaned = seed }\n}\n").unwrap();
    session.save(&alpha).unwrap();
    session.save(&beta).unwrap();
    drop(session);
    let ours = directory.path().join("ours");
    let theirs = directory.path().join("theirs");
    copy_tree(&base, &ours);
    copy_tree(&base, &theirs);
    let alpha_model = edit_document(&ours, &alpha, "7", 420.0);
    let beta_model = edit_document(&theirs, &beta, "9", 840.0);
    let diagram = ".apiforge/diagram.json";
    let merged = Command::new("git")
        .args(["merge-file", "-p"])
        .arg(ours.join(diagram))
        .arg(base.join(diagram))
        .arg(theirs.join(diagram))
        .output()
        .unwrap();
    assert!(
        merged.status.success(),
        "independent checkpoint merge failed: {:?}: {} {}",
        merged.status.code(),
        String::from_utf8_lossy(&merged.stdout),
        String::from_utf8_lossy(&merged.stderr)
    );
    fs::write(base.join(diagram), &merged.stdout).unwrap();
    fs::copy(ours.join("alpha.afs"), base.join("alpha.afs")).unwrap();
    fs::copy(theirs.join("beta.afs"), base.join("beta.afs")).unwrap();
    let records = apiforge_storage::project::DiagramFile::from_bytes(&merged.stdout).unwrap();
    assert_eq!(records.records.len(), 2);
    let loaded = ProjectSession::open_saved_read_only(&base).unwrap();
    assert_eq!(loaded.document(&alpha).unwrap().model(), Some(&alpha_model));
    assert_eq!(loaded.document(&beta).unwrap().model(), Some(&beta_model));
    assert_eq!(
        loaded.document(&alpha).unwrap().analysis(),
        AnalysisState::Valid
    );
    assert_eq!(
        loaded.document(&beta).unwrap().analysis(),
        AnalysisState::Valid
    );
}

#[test]
fn a_paused_uncoordinated_checkout_can_expose_a_stable_intermediate_tree_with_or_without_a_writer()
{
    for writer in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let mut created = ProjectSession::create(root, "External checkout").unwrap();
        let first = created
            .create_document(
                DocumentKind::Scenario,
                "first.afs",
                "scenario \"First\" { answer = 1 }\n",
            )
            .unwrap();
        let second = created
            .create_document(
                DocumentKind::Scenario,
                "second.afs",
                "scenario \"Second\" { answer = 2 }\n",
            )
            .unwrap();
        drop(created);
        let mut session = if writer {
            ProjectSession::open(root)
        } else {
            ProjectSession::open_saved_read_only(root)
        }
        .unwrap();
        let ticket = session.prepare_run(&first).unwrap();
        let head = fs::read(root.join(".apiforge/local/head.json")).unwrap();
        let (changed, paused) = mpsc::channel();
        let (resume, next) = mpsc::channel();
        let destination = root.to_owned();
        let checkout = std::thread::spawn(move || {
            fs::write(
                destination.join("first.afs"),
                "scenario \"First\" { answer = 7 }\n",
            )
            .unwrap();
            changed.send(()).unwrap();
            next.recv().unwrap();
            fs::write(
                destination.join("second.afs"),
                "scenario \"Second\" { answer = 9 }\n",
            )
            .unwrap();
        });
        paused.recv().unwrap();
        let changes = session.external_check().run().unwrap();
        let outcome = session.apply_external(changes).unwrap();
        assert_eq!(outcome.reloaded, std::slice::from_ref(&first));
        assert!(
            session
                .document(&first)
                .unwrap()
                .text()
                .contains("answer = 7")
        );
        assert!(
            session
                .document(&second)
                .unwrap()
                .text()
                .contains("answer = 2")
        );
        let stable_reader = ProjectSession::open_saved_read_only(root).unwrap();
        assert!(
            stable_reader
                .document(&first)
                .unwrap()
                .text()
                .contains("answer = 7")
        );
        assert!(
            stable_reader
                .document(&second)
                .unwrap()
                .text()
                .contains("answer = 2")
        );
        assert!(session.validate_run(&ticket).is_err());
        resume.send(()).unwrap();
        checkout.join().unwrap();
        let changes = session.external_check().run().unwrap();
        let outcome = session.apply_external(changes).unwrap();
        assert_eq!(outcome.reloaded, std::slice::from_ref(&second));
        assert!(
            session
                .document(&second)
                .unwrap()
                .text()
                .contains("answer = 9")
        );
        assert_eq!(
            fs::read(root.join(".apiforge/local/head.json")).unwrap(),
            head
        );
    }
}
