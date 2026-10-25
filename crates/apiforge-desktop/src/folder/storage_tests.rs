use super::*;
use apiforge_project::StorageMode;
use std::{fs, sync::mpsc};

#[test]
fn slow_storage_keeps_frames_and_real_text_input_responsive() {
    let (directory, id) = named_folder("Responsive storage");
    let (context, mut app) = open(directory.path());
    let (started, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    app.storage_io(
        "Reading project…",
        move || {
            started.send(()).unwrap();
            resume.recv().unwrap();
        },
        |_, result| result.unwrap(),
    );
    frame(&mut app, &context, vec![]);
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(1));
        let _ = release.send(());
    });
    let before = Instant::now();
    type_into_code(&mut app, &context, "// typed while reading\n");
    assert!(
        before.elapsed() < Duration::from_millis(500),
        "a frame waited for storage"
    );
    assert!(text(&app, &id).starts_with("// typed while reading\n"));
    assert!(
        app.storage.is_busy(),
        "typing finished before the slow operation"
    );
    release.join().unwrap();
    wait_storage(&mut app, &context);
}

#[test]
fn a_slow_save_acknowledges_older_text_and_keeps_newer_input_dirty() {
    let (directory, id) = named_folder("Save versions");
    let (context, mut app) = open(directory.path());
    let earlier = text(&app, &id).replace('1', "2");
    app.session
        .as_mut()
        .unwrap()
        .edit_source(&id, &earlier, CODE)
        .unwrap();
    let (started, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let target = id.clone();
    app.storage_project(
        "Saving file…",
        StorageMode::PreserveEdits,
        false,
        move |session| {
            started.send(()).unwrap();
            resume.recv().unwrap();
            session.save(&target)
        },
        |_, result| {
            result.unwrap();
        },
    );
    frame(&mut app, &context, vec![]);
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    type_into_code(&mut app, &context, "// newer unsaved text\n");
    let latest = text(&app, &id);
    assert!(app.storage.is_busy());
    release.send(()).unwrap();
    wait_storage(&mut app, &context);
    assert_eq!(text(&app, &id), latest);
    assert!(
        app.session
            .as_ref()
            .unwrap()
            .document(&id)
            .unwrap()
            .is_dirty()
    );
    assert_eq!(
        fs::read_to_string(directory.path().join("main.afs")).unwrap(),
        earlier
    );
}

#[test]
fn a_queued_save_takes_current_text_and_committed_baselines_when_it_starts() {
    let (directory, id) = named_folder("Queued saves");
    let (context, mut app) = open(directory.path());
    let earlier = text(&app, &id).replace('1', "2");
    app.session
        .as_mut()
        .unwrap()
        .edit_source(&id, &earlier, CODE)
        .unwrap();
    let (started, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let target = id.clone();
    app.storage_project(
        "Saving file…",
        StorageMode::PreserveEdits,
        false,
        move |session| {
            started.send(()).unwrap();
            resume.recv().unwrap();
            session.save(&target)
        },
        |_, result| {
            result.unwrap();
        },
    );
    frame(&mut app, &context, vec![]);
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    app.save_document(&id).unwrap();
    type_into_code(&mut app, &context, "// captured at dequeue\n");
    let latest = text(&app, &id);
    release.send(()).unwrap();
    wait_storage(&mut app, &context);
    assert_eq!(
        fs::read_to_string(directory.path().join("main.afs")).unwrap(),
        latest
    );
    assert_eq!(text(&app, &id), latest);
    assert!(
        !app.session
            .as_ref()
            .unwrap()
            .document(&id)
            .unwrap()
            .is_dirty()
    );
}

#[test]
fn close_waits_for_storage_before_releasing_the_writer() {
    let (directory, _) = named_folder("Close ownership");
    let (context, mut app) = open(directory.path());
    let (started, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    app.storage_project(
        "Saving file…",
        StorageMode::PreserveEdits,
        false,
        move |session| {
            started.send(()).unwrap();
            resume.recv().unwrap();
            session.save_recovery()
        },
        |_, result| {
            result.unwrap();
        },
    );
    frame(&mut app, &context, vec![]);
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(!app.prepare_close());
    assert!(app.session.is_some() && app.save_pending());
    assert!(matches!(
        apiforge_storage::project::ProjectStore::open_exclusive(directory.path()),
        Err(apiforge_project::ProjectError::Locked)
    ));
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !app.prepare_close() && Instant::now() < deadline {
        frame(&mut app, &context, vec![]);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(app.session.is_none());
    assert!(apiforge_storage::project::ProjectStore::open_exclusive(directory.path()).is_ok());
}

#[test]
fn a_late_text_open_does_not_steal_the_users_newer_tab_selection() {
    let (directory, id) = named_folder("Late file open");
    fs::write(directory.path().join("notes.txt"), "notes\r\n").unwrap();
    let (context, mut app) = open(directory.path());
    let (release, resume) = mpsc::channel();
    app.storage_io(
        "Reading project…",
        move || resume.recv().unwrap(),
        |_, result| result.unwrap(),
    );
    frame(&mut app, &context, vec![]);
    app.open_text("notes.txt");
    app.activate_tab(&id);
    release.send(()).unwrap();
    wait_storage(&mut app, &context);
    assert_eq!(app.workbench.active(), Some(id.as_str()));
    assert!(app.workbench.tab_mut(&text_tab("notes.txt")).is_some());
    assert_eq!(
        app.session
            .as_ref()
            .unwrap()
            .text_file("notes.txt")
            .unwrap()
            .text(),
        "notes\r\n"
    );
}

#[test]
fn late_import_and_folder_chooser_results_cannot_replace_the_current_project() {
    let (alpha, _) = named_folder("Chooser Alpha");
    let (beta, _) = named_folder("Chooser Beta");
    let (context, mut app) = open(alpha.path());
    let identity = app.session.as_ref().unwrap().storage_identity();
    let generation = app.open_generation;
    app.request_switch(beta.path().into());
    wait_storage(&mut app, &context);
    let source = alpha.path().join("offered.afs");
    fs::write(&source, "scenario \"Offered\" {}\n").unwrap();
    app.picked_ready.push(Picked::Imports {
        identity,
        folder: String::new(),
        files: vec![source],
    });
    app.picked_ready.push(Picked::Folder {
        generation,
        path: Some(alpha.path().into()),
    });
    app.follow_dialogs();
    wait_storage(&mut app, &context);
    assert_eq!(app.root, beta.path());
    assert!(app.prompt.is_none());
    assert!(!beta.path().join("offered.afs").exists());
}

#[test]
fn keeping_the_window_invalidates_a_pending_close_export() {
    let (directory, _) = named_folder("Revoked close");
    let (context, mut app) = open(directory.path());
    let contents = app.session.as_ref().unwrap().export_unsaved().unwrap();
    let generation = app.persistence.departure_generation;
    app.cancel_switch();
    app.picked_ready.push(Picked::CloseExported {
        generation,
        result: Some(Ok(directory.path().join("export.json"))),
        contents,
    });
    app.follow_dialogs();
    wait_storage(&mut app, &context);
    assert!(app.session.is_some());
    assert!(!app.persistence.closing && !app.persistence.departure && !app.close_requested);
}

#[test]
fn renaming_a_dirty_text_tab_preserves_exact_text_widget_history_and_restart_recovery() {
    let (directory, _) = named_folder("Text rename");
    fs::write(directory.path().join("notes.txt"), "saved\r\n").unwrap();
    let (context, mut app) = open(directory.path());
    app.open_text("notes.txt");
    wait_storage(&mut app, &context);
    type_into_code(&mut app, &context, "draft ");
    let before = app
        .session
        .as_ref()
        .unwrap()
        .text_file("notes.txt")
        .unwrap()
        .text()
        .to_owned();
    app.relocate("notes.txt", "moved.txt").unwrap();
    wait_storage(&mut app, &context);
    assert_eq!(app.workbench.active(), Some(text_tab("moved.txt").as_str()));
    assert_eq!(
        app.session
            .as_ref()
            .unwrap()
            .text_file("moved.txt")
            .unwrap()
            .text(),
        before
    );
    context.memory_mut(|memory| memory.request_focus(editor_id(&text_tab("moved.txt"))));
    frame(&mut app, &context, vec![]);
    frame(&mut app, &context, vec![key(Key::Z, COMMAND)]);
    assert_eq!(
        app.session
            .as_ref()
            .unwrap()
            .text_file("moved.txt")
            .unwrap()
            .text(),
        "saved\r\n"
    );
    frame(&mut app, &context, vec![key(Key::Z, command_shift())]);
    assert_eq!(
        app.session
            .as_ref()
            .unwrap()
            .text_file("moved.txt")
            .unwrap()
            .text(),
        before
    );
    assert_eq!(
        fs::read(directory.path().join("moved.txt")).unwrap(),
        b"saved\r\n"
    );
    assert!(await_close(&mut app, &context));
    let (_, reopened) = open(directory.path());
    assert_eq!(
        reopened
            .session
            .as_ref()
            .unwrap()
            .text_file("moved.txt")
            .unwrap()
            .text(),
        before
    );
}
