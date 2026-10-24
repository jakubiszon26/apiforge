//! Project text files: exact text, saving, recovery, external changes and the
//! boundaries that keep them from becoming definitions or managed data.
use apiforge_project::*;
use apiforge_storage::project::TextSelection;

const SCENARIO: &str = "scenario \"Main\" {\n  total = 2\n}\n";

fn project() -> (tempfile::TempDir, ProjectSession) {
    let directory = tempfile::tempdir().unwrap();
    let session = ProjectSession::create(directory.path(), "Folder").unwrap();
    (directory, session)
}

fn write(directory: &tempfile::TempDir, path: &str, text: &str) {
    std::fs::write(directory.path().join(path), text).unwrap();
}

fn follow_disk(session: &mut ProjectSession) -> ExternalOutcome {
    let changes = session.external_check().run().unwrap();
    session.apply_external(changes).unwrap()
}

#[test]
fn a_text_file_opens_with_its_exact_bytes_and_saves_them_back() {
    let (directory, mut session) = project();
    write(&directory, ".gitignore", "/.apiforge/local/\r\n# keep\n");
    let file = session.open_text_file(".gitignore").unwrap();
    assert_eq!(file.text(), "/.apiforge/local/\r\n# keep\n");
    assert!(!file.is_dirty());
    // Opening it again is the same open file, not a second one.
    session.open_text_file(".gitignore").unwrap();
    assert_eq!(session.text_files().len(), 1);

    session
        .edit_text(
            ".gitignore",
            "/.apiforge/local/\r\n# keep\ntarget\n",
            Origin(7),
        )
        .unwrap();
    assert!(session.text_file(".gitignore").unwrap().is_dirty());
    assert!(session.save_text_file(".gitignore").unwrap().is_some());
    assert_eq!(
        std::fs::read_to_string(directory.path().join(".gitignore")).unwrap(),
        "/.apiforge/local/\r\n# keep\ntarget\n",
        "exact bytes, including CRLF"
    );
    assert!(!session.text_file(".gitignore").unwrap().is_dirty());
    assert!(session.save_text_file(".gitignore").unwrap().is_none());
}

#[test]
fn managed_data_registered_documents_and_unreadable_files_are_not_text_files() {
    let (directory, mut session) = project();
    let id = session
        .create_document(DocumentKind::Scenario, "scenarios/main.afs", SCENARIO)
        .unwrap();
    session.save(&id).unwrap();
    assert!(session.open_text_file(".apiforge/diagram.json").is_err());
    assert!(session.open_text_file(".apiforge/local/head.json").is_err());
    assert!(matches!(
        session.open_text_file("apiforge.project.json"),
        Err(Error::ManagedData)
    ));
    assert!(session.open_text_file("../outside.txt").is_err());
    std::fs::create_dir_all(directory.path().join(".git")).unwrap();
    write(&directory, ".git/config", "[core]\n");
    assert!(session.open_text_file(".git/config").is_err());
    assert!(matches!(
        session.open_text_file("scenarios/main.afs"),
        Err(Error::RegisteredDocument)
    ));
    std::fs::create_dir_all(directory.path().join("docs")).unwrap();
    assert!(session.open_text_file("docs").is_err());
    std::fs::write(directory.path().join("logo.bin"), [0xff, 0xfe, 0x00]).unwrap();
    assert!(matches!(
        session.open_text_file("logo.bin"),
        Err(Error::NotText(_))
    ));
    std::fs::write(
        directory.path().join("big.txt"),
        vec![b'x'; 1024 * 1024 + 1],
    )
    .unwrap();
    assert!(matches!(
        session.open_text_file("big.txt"),
        Err(Error::SourceTooLarge)
    ));
    assert!(session.text_files().is_empty());
}

#[test]
fn unsaved_text_blocks_close_until_it_is_recovered_and_returns_on_reopening() {
    let (directory, mut session) = project();
    write(&directory, "README.md", "# Shop\n");
    session.open_text_file("README.md").unwrap();
    session
        .edit_text("README.md", "# Shop\n\nNotes.\n", Origin(1))
        .unwrap();
    session
        .set_text_selection(
            "README.md",
            TextSelection {
                anchor: 8,
                caret: 15,
            },
        )
        .unwrap();
    assert!(session.save_recovery().unwrap());
    assert!(session.close().is_ok());
    assert_eq!(
        std::fs::read_to_string(directory.path().join("README.md")).unwrap(),
        "# Shop\n",
        "recovery never saves the file"
    );

    let mut session = ProjectSession::open(directory.path()).unwrap();
    // The draft is preserved until its file is opened, then restored.
    assert!(session.text_files().is_empty());
    let file = session.open_text_file("README.md").unwrap();
    assert_eq!(file.text(), "# Shop\n\nNotes.\n");
    assert_eq!(file.selection().caret, 15);
    assert!(file.is_dirty() && !file.is_held());
    session.save_text_file("README.md").unwrap();
    assert!(session.close().is_ok());
    let session = ProjectSession::open(directory.path()).unwrap();
    assert!(session.text_files().is_empty());
}

#[test]
fn a_draft_of_a_changed_file_is_held_until_an_explicit_decision() {
    let (directory, mut session) = project();
    write(&directory, "notes.txt", "one\n");
    session.open_text_file("notes.txt").unwrap();
    session
        .edit_text("notes.txt", "one\ntwo\n", Origin(1))
        .unwrap();
    session.save_recovery().unwrap();
    drop(session);
    write(&directory, "notes.txt", "replaced\n");

    let mut session = ProjectSession::open(directory.path()).unwrap();
    let file = session.open_text_file("notes.txt").unwrap();
    assert!(file.is_held(), "the baseline is no longer the file on disk");
    assert_eq!(file.text(), "one\ntwo\n");
    assert!(matches!(
        session.edit_text("notes.txt", "three\n", Origin(1)),
        Err(Error::DraftDecisionRequired)
    ));
    assert!(matches!(
        session.save_text_file("notes.txt"),
        Err(Error::DraftDecisionRequired)
    ));
    session.discard_text_draft("notes.txt").unwrap();
    let file = session.text_file("notes.txt").unwrap();
    assert_eq!(file.text(), "replaced\n");
    assert!(!file.is_dirty() && !file.is_held());
}

#[test]
fn an_external_change_follows_a_clean_file_and_conflicts_with_unsaved_work() {
    let (directory, mut session) = project();
    write(&directory, "notes.txt", "one\n");
    write(&directory, "other.txt", "a\n");
    session.open_text_file("notes.txt").unwrap();
    session.open_text_file("other.txt").unwrap();
    session.edit_text("other.txt", "a\nb\n", Origin(1)).unwrap();

    write(&directory, "notes.txt", "one\ntwo\n");
    write(&directory, "other.txt", "elsewhere\n");
    let outcome = follow_disk(&mut session);
    assert_eq!(outcome.text_conflicts, ["other.txt"]);
    assert_eq!(session.text_file("notes.txt").unwrap().text(), "one\ntwo\n");
    assert_eq!(session.text_file("other.txt").unwrap().text(), "a\nb\n");
    assert_eq!(
        session.text_file("other.txt").unwrap().conflict(),
        Some(ExternalConflict::Changed)
    );
    assert!(matches!(
        session.save_text_file("other.txt"),
        Err(Error::ExternalConflict)
    ));
    session
        .accept_external_text("other.txt", Origin(1))
        .unwrap();
    let file = session.text_file("other.txt").unwrap();
    assert_eq!(file.text(), "elsewhere\n");
    assert!(file.conflict().is_none());

    // Deletion outside API Forge: the local text can be kept and recreated.
    session.edit_text("other.txt", "mine\n", Origin(1)).unwrap();
    std::fs::remove_file(directory.path().join("other.txt")).unwrap();
    let outcome = follow_disk(&mut session);
    assert_eq!(outcome.text_conflicts, ["other.txt"]);
    assert_eq!(
        session.text_file("other.txt").unwrap().conflict(),
        Some(ExternalConflict::Deleted)
    );
    session.keep_deleted_text("other.txt").unwrap();
    session.save_text_file("other.txt").unwrap();
    assert_eq!(
        std::fs::read_to_string(directory.path().join("other.txt")).unwrap(),
        "mine\n"
    );
}

#[test]
fn a_second_session_can_read_text_files_but_cannot_save_them() {
    let (directory, mut session) = project();
    write(&directory, "notes.txt", "one\n");
    session.open_text_file("notes.txt").unwrap();
    let mut reader = ProjectSession::open(directory.path()).unwrap();
    assert!(reader.is_read_only());
    assert_eq!(reader.open_text_file("notes.txt").unwrap().text(), "one\n");
    reader.edit_text("notes.txt", "two\n", Origin(1)).unwrap();
    assert!(reader.save_text_file("notes.txt").is_err());
    assert!(reader.save_recovery().is_err());
    // The read-only session returns itself so the caller can offer export.
    assert!(reader.close().is_err());
}

#[test]
fn an_explorer_move_carries_an_open_file_and_a_delete_keeps_its_unsaved_work() {
    let (directory, mut session) = project();
    write(&directory, "notes.txt", "one\n");
    session.open_text_file("notes.txt").unwrap();
    session
        .edit_text("notes.txt", "one\ntwo\n", Origin(1))
        .unwrap();
    session.create_folder("docs").unwrap();
    session
        .relocate("notes.txt", "docs/notes.txt", Origin(1))
        .unwrap();
    assert!(session.text_file("notes.txt").is_none());
    let file = session.text_file("docs/notes.txt").unwrap();
    assert_eq!(
        file.text(),
        "one\ntwo\n",
        "unsaved work moved with the file"
    );
    assert!(file.is_dirty());

    let preview = session.preview_delete(&["docs"]).unwrap();
    assert_eq!(preview.unsaved_texts(), ["docs/notes.txt"]);
    session.delete(&preview, Origin(1)).unwrap();
    let file = session.text_file("docs/notes.txt").unwrap();
    assert_eq!(
        file.conflict(),
        Some(ExternalConflict::Deleted),
        "unsaved work stays in the session after its file is trashed"
    );

    // A clean file simply closes with its file.
    write(&directory, "clean.txt", "x\n");
    session.open_text_file("clean.txt").unwrap();
    let preview = session.preview_delete(&["clean.txt"]).unwrap();
    assert!(preview.unsaved_texts().is_empty());
    session.delete(&preview, Origin(1)).unwrap();
    assert!(session.text_file("clean.txt").is_none());
}

#[test]
fn closing_a_text_file_needs_a_decision_about_unsaved_work() {
    let (directory, mut session) = project();
    write(&directory, "notes.txt", "one\n");
    session.open_text_file("notes.txt").unwrap();
    session
        .edit_text("notes.txt", "one\ntwo\n", Origin(1))
        .unwrap();
    assert!(matches!(
        session.close_text_file("notes.txt"),
        Err(Error::UnsavedTextFile)
    ));
    session.revert_text_file("notes.txt", Origin(1)).unwrap();
    assert_eq!(session.text_file("notes.txt").unwrap().text(), "one\n");
    session.close_text_file("notes.txt").unwrap();
    assert!(session.text_files().is_empty());
    // Reverting refuses to overwrite an external change silently.
    session.open_text_file("notes.txt").unwrap();
    session.edit_text("notes.txt", "mine\n", Origin(1)).unwrap();
    write(&directory, "notes.txt", "theirs\n");
    assert!(matches!(
        session.revert_text_file("notes.txt", Origin(1)),
        Err(Error::ExternalConflict)
    ));
}
