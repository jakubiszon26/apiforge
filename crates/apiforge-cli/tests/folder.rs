use apiforge_project::{AnalysisState, DocumentKind, Origin, ProjectSession, prepare_project};
use std::{
    ffi::OsStr,
    fs,
    process::{Command, Output},
};

fn run(root: &std::path::Path, command: &str, rest: &[&OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_apiforge"))
        .arg(command)
        .arg("--folder")
        .arg(root)
        .args(rest)
        .output()
        .unwrap()
}

#[test]
fn cli_and_gui_share_saved_semantics_identities_and_explicit_revisions() {
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &apiforge_core::demos::shop_project()).unwrap();
    let session = ProjectSession::open(directory.path()).unwrap();
    for document in session.documents() {
        let path = document.entry().path.as_str();
        let output = run(
            directory.path(),
            "print",
            &[OsStr::new("--file"), OsStr::new(path)],
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let expected =
            apiforge_script::print_folder_document(session.context(), document.model().unwrap())
                .unwrap();
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
    assert!(run(directory.path(), "check", &[]).status.success());
    let scenario = session
        .documents()
        .iter()
        .find(|document| document.entry().kind == DocumentKind::Scenario)
        .unwrap();
    let id = scenario.id().to_owned();
    let path = scenario.entry().path.as_str().to_owned();
    let original = scenario.model().unwrap().clone();
    let input = directory.path().join("incoming.txt");
    let edited = scenario
        .text()
        .replacen("scenario \"", "scenario \"Changed ", 1);
    fs::write(&input, &edited).unwrap();
    assert!(
        !run(
            directory.path(),
            "apply",
            &[OsStr::new("--file"), OsStr::new(&path), input.as_os_str()]
        )
        .status
        .success(),
        "a GUI writer holds the project lock"
    );
    drop(session);
    assert!(
        run(
            directory.path(),
            "apply",
            &[OsStr::new("--file"), OsStr::new(&path), input.as_os_str()]
        )
        .status
        .success()
    );
    let session = ProjectSession::open(directory.path()).unwrap();
    let document = session.document(&id).unwrap();
    assert_eq!(document.text(), edited);
    assert_eq!(document.analysis(), AnalysisState::Valid);
    let apiforge_project::Document::Scenario(before) = original else {
        panic!()
    };
    let Some(apiforge_project::Document::Scenario(after)) = document.model() else {
        panic!()
    };
    assert_eq!(before.id, after.id);
    assert_eq!(before.graph, after.graph);
    assert_eq!(before.cleanup, after.cleanup);
}

#[test]
fn folder_cli_ignores_dirty_recovery_and_cache_and_never_executes_invalid_saved_source() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Saved").unwrap();
    let source = "scenario \"Saved\" {\n  answer = 1\n}\n";
    let id = session
        .create_document(DocumentKind::Scenario, "saved.afs", source)
        .unwrap();
    session
        .edit_source(&id, &source.replace('1', "2"), Origin::SESSION)
        .unwrap();
    session.settle(&id).unwrap();
    session.save_last_valid().unwrap();
    session.save_recovery().unwrap();
    let output = run(
        directory.path(),
        "print",
        &[OsStr::new("--file"), OsStr::new("saved.afs")],
    );
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("answer = 1")
    );
    drop(session);
    fs::write(
        directory.path().join("saved.afs"),
        "scenario \"Saved\" { answer = }",
    )
    .unwrap();
    assert!(!run(directory.path(), "check", &[]).status.success());
    assert!(
        !run(
            directory.path(),
            "print",
            &[OsStr::new("--file"), OsStr::new("saved.afs")]
        )
        .status
        .success()
    );
    assert!(
        !run(
            directory.path(),
            "run",
            &[OsStr::new("--scenario"), OsStr::new("saved.afs")]
        )
        .status
        .success()
    );
}

#[test]
fn invalid_apply_and_unqualified_calls_preserve_the_saved_project() {
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &apiforge_core::demos::shop_project()).unwrap();
    let session = ProjectSession::open(directory.path()).unwrap();
    let document = session
        .documents()
        .iter()
        .find(|d| d.entry().kind == DocumentKind::Scenario)
        .unwrap();
    let path = document.entry().path.as_str().to_owned();
    let original = document.text().to_owned();
    let input = directory.path().join("incoming.txt");
    let unpinned = original.replace("@1(", "(");
    assert_ne!(original, unpinned);
    drop(session);
    for text in ["scenario \"Broken\" { value = }", &unpinned] {
        fs::write(&input, text).unwrap();
        let output = run(
            directory.path(),
            "apply",
            &[OsStr::new("--file"), OsStr::new(&path), input.as_os_str()],
        );
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(
            fs::read_to_string(directory.path().join(&path)).unwrap(),
            original
        );
    }
}

#[test]
fn saved_folder_runs_match_gui_snapshots_and_ignore_an_unrelated_invalid_scenario() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Run").unwrap();
    session
        .create_document(
            DocumentKind::Scenario,
            "good.afs",
            "scenario \"Good\" {\n  answer = 7\n  equal = assert answer == 7\n}\n",
        )
        .unwrap();
    session
        .create_document(
            DocumentKind::Scenario,
            "bad.afs",
            "scenario \"Bad\" { value = }",
        )
        .unwrap();
    drop(session);
    let output = run(
        directory.path(),
        "run",
        &[
            OsStr::new("--scenario"),
            OsStr::new("good.afs"),
            OsStr::new("--json"),
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: apiforge_core::run::RunResult = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report.status, apiforge_core::run::RunStatus::Passed);
    assert_eq!(report.steps.len(), 2);
}

#[test]
fn folder_arguments_reject_ambiguous_modes_and_unsafe_or_unregistered_paths() {
    let directory = tempfile::tempdir().unwrap();
    let session = ProjectSession::create(directory.path(), "Arguments").unwrap();
    drop(session);
    for (command, args) in [
        ("print", vec![]),
        ("apply", vec!["--file", "x.afs"]),
        ("check", vec!["--folder", "duplicate"]),
        ("check", vec!["--workspace", "legacy.json"]),
    ] {
        let rest: Vec<_> = args.iter().map(OsStr::new).collect();
        assert_eq!(run(directory.path(), command, &rest).status.code(), Some(2));
    }
    for path in [
        "../outside.afs",
        ".apiforge/revisions/id/1.afs",
        "unregistered.afs",
    ] {
        assert_eq!(
            run(
                directory.path(),
                "print",
                &[OsStr::new("--file"), OsStr::new(path)]
            )
            .status
            .code(),
            Some(1)
        );
    }
}

#[test]
fn read_only_folder_commands_leave_an_untouched_checkout_without_local_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Checkout").unwrap();
    session
        .create_document(
            DocumentKind::Scenario,
            "saved.afs",
            "scenario \"Saved\" {\n  value = 1\n}\n",
        )
        .unwrap();
    drop(session);
    fs::remove_dir_all(directory.path().join(".apiforge/local")).unwrap();
    assert!(run(directory.path(), "check", &[]).status.success());
    assert!(
        run(
            directory.path(),
            "print",
            &[OsStr::new("--file"), OsStr::new("saved.afs")]
        )
        .status
        .success()
    );
    assert!(
        run(
            directory.path(),
            "run",
            &[OsStr::new("--scenario"), OsStr::new("saved.afs")]
        )
        .status
        .success()
    );
    assert!(!directory.path().join(".apiforge/local").exists());
}

#[test]
fn a_live_folder_run_requires_its_project_lock_and_a_writable_journal() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = ProjectSession::create(directory.path(), "Live").unwrap();
    session
        .create_document(
            DocumentKind::Scenario,
            "saved.afs",
            "scenario \"Saved\" {\n  value = 1\n}\n",
        )
        .unwrap();
    let arguments = [
        OsStr::new("--scenario"),
        OsStr::new("saved.afs"),
        OsStr::new("--live"),
    ];
    // MCP-15: the window that holds it has no channel, so it cannot be
    // asked, and the refusal says so instead of *another writer session*.
    let held = run(directory.path(), "run", &arguments);
    assert_eq!(held.status.code(), Some(1));
    let text = String::from_utf8_lossy(&held.stderr);
    assert!(
        text.contains("could not be asked; nothing was sent")
            && !text.contains("The folder project is open in another writer session"),
        "{text}"
    );
    drop(session);
    let journal = directory.path().join(".apiforge/local/journal.json");
    fs::create_dir(&journal).unwrap();
    assert!(!run(directory.path(), "run", &arguments).status.success());
    fs::remove_dir(&journal).unwrap();
    assert!(run(directory.path(), "run", &arguments).status.success());
    assert!(!journal.exists(), "a finished run removes its own record");
}

/// MCP-07: `apply` names itself while it holds the project, so a window
/// opened meanwhile can say *apiforge apply*. The description stays behind
/// when it exits; a reader ignores it once no writer is live.
#[test]
fn apply_describes_itself_as_the_apply_command() {
    let directory = tempfile::tempdir().unwrap();
    prepare_project(directory.path(), &apiforge_core::demos::shop_project()).unwrap();
    let session = ProjectSession::open_saved_read_only(directory.path()).unwrap();
    let scenario = session
        .documents()
        .iter()
        .find(|document| document.entry().kind == DocumentKind::Scenario)
        .unwrap();
    let path = scenario.entry().path.as_str().to_owned();
    let input = directory.path().join("incoming.txt");
    fs::write(&input, format!("{}// applied\n", scenario.text())).unwrap();
    let output = run(
        directory.path(),
        "apply",
        &[OsStr::new("--file"), OsStr::new(&path), input.as_os_str()],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let description =
        fs::read_to_string(directory.path().join(".apiforge/local/holder.json")).unwrap();
    assert!(
        description.contains("\"role\":\"command\",\"client\":\"apply\""),
        "{description}"
    );
    assert_eq!(session.store().holder().unwrap(), None, "apply has exited");
}

/// MCP-15 (#182): a live folder run asks the open window for the project
/// over the control channel, as the MCP server does, instead of refusing.
/// The window is scripted here: the test holds the project's lock as a
/// window naming its endpoint, listens on that endpoint with the channel's
/// own listener and, on `request`, lets the lock go and answers `granted`
/// as the window does. Live requests go to the demo API in this process.
mod mcp15 {
    use apiforge_channel::{Asker, Endpoint, Listener, Message, Received, RunMode, RunState};
    use apiforge_storage::project::{Holder, HolderRole, ProjectStore};
    use serde_json::{Value, json};
    use std::{
        collections::BTreeMap,
        fs,
        path::{Path, PathBuf},
        process::{Command, Output, Stdio},
        thread,
        time::{Duration, Instant},
    };

    const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
    const MANIFEST: &str = "apiforge.project.json";
    const JOURNAL: &str = ".apiforge/local/journal.json";
    /// The refusal before MCP-15, which a live folder run no longer gives.
    const OLD: &str = "The folder project is open in another writer session";
    const HANDED_OVER: &str = "The open API Forge window handed the project over for this run";
    /// The owner's decision of 2026-11-01 on #182: a neutral line, since
    /// the window may be busy rather than asking the person.
    const WAITING: &str = "Waiting for the API Forge window to answer";
    const KEPT: &str = "The window kept the project (you chose to keep editing); nothing was sent";
    /// The one line a lost channel leaves on stderr.
    const LOST: &str = "the window disconnected";
    /// The reason a window gives when the person keeps editing (MCP-08c).
    const KEEP_EDITING: &str = "the person chose to keep editing";
    const WSL: &str = "Windows/WSL";
    /// A scenario that passes against the demo API, with a cleanup.
    const PASSES: &str = "scenario \"MCP-15 live\" {\n  response = GET \"/shop/products/123\" { sample 200 { id: 123 } }\n  ok = assert response.status == 200\n  cleanup {\n    again = GET \"/shop/products/123\" { sample 200 { id: 123 } }\n  }\n}\n";
    /// The same against a product the demo API does not have: it fails.
    const FAILS: &str = "scenario \"MCP-15 live\" {\n  response = GET \"/shop/products/999\" { sample 200 { id: 999 } }\n  ok = assert response.status == 200\n  cleanup {\n    again = GET \"/shop/products/123\" { sample 200 { id: 123 } }\n  }\n}\n";

    /// The demo shop, its base URL `base_url` and its scenario `scenario`.
    fn shop(base_url: &str, scenario: &str) -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        apiforge_project::prepare_project(root, &apiforge_core::demos::shop_project()).unwrap();
        fs::remove_dir_all(root.join(".apiforge/local")).unwrap();
        let manifest = root.join(MANIFEST);
        let mut config: Value = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        config["environment"]["baseUrl"] = json!(base_url);
        fs::write(manifest, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
        fs::write(root.join(SCENARIO), scenario).unwrap();
        directory
    }

    fn command(root: &Path, live: bool) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_apiforge"));
        command
            .arg("run")
            .arg("--folder")
            .arg(root)
            .args(["--scenario", SCENARIO, "--json"]);
        if live {
            command.arg("--live");
        }
        command
    }

    /// Runs the command to its end; returns its output and its process id.
    fn run(root: &Path, live: bool) -> (Output, u32) {
        let child = command(root, live)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let pid = child.id();
        (child.wait_with_output().unwrap(), pid)
    }

    fn stderr(output: &Output) -> String {
        String::from_utf8_lossy(&output.stderr).into_owned()
    }

    fn report(output: &Output) -> apiforge_core::run::RunResult {
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "no report ({error}); stdout: {}; stderr: {}",
                String::from_utf8_lossy(&output.stdout),
                stderr(output)
            )
        })
    }

    /// How the scripted window answers a request.
    #[derive(Clone, Copy)]
    enum Answer {
        /// Lets the project go and observes over the connection.
        Grant,
        /// Lets it go, then closes the connection, as a window that closes.
        GrantThenClose,
        /// Keeps the project and says why.
        Refuse(&'static str),
    }

    #[derive(Clone, Copy)]
    struct Script {
        answer: Answer,
        /// How long the window takes to answer: the person at the prompt.
        delay: Duration,
        /// Says `permission { live: false }` after `granted` (criterion 4).
        permission_after_grant: bool,
    }

    impl Script {
        fn grant() -> Self {
            Self {
                answer: Answer::Grant,
                delay: Duration::ZERO,
                permission_after_grant: false,
            }
        }
    }

    /// What the scripted window saw.
    #[derive(Debug, Default)]
    struct Seen {
        connections: usize,
        /// Every message read, in order, after the window's own consent.
        messages: Vec<Message>,
        /// The connection ended after the last message.
        eof: bool,
        /// The window took the lock back as soon as it heard `released`.
        retaken_on_released: Option<bool>,
    }

    /// The test's window: it holds the project and listens on the endpoint
    /// its holder file names.
    struct Window {
        stop: Option<tokio::sync::oneshot::Sender<()>>,
        thread: Option<thread::JoinHandle<Seen>>,
    }

    impl Window {
        /// Holds `root` as a window with a channel. Without a script, any
        /// connection is a failure the test asserts afterwards.
        fn open(root: &Path, script: Option<Script>) -> Self {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let mut listener = {
                let _context = runtime.enter();
                Listener::bind().unwrap()
            };
            let holder =
                Holder::window().with_endpoint(Some(listener.endpoint().name().to_owned()));
            Self::serve(
                root,
                &holder,
                runtime,
                async move { listener.accept().await },
                script,
            )
        }

        /// Holds `root` as `holder` and listens on `listener`'s endpoint,
        /// which the holder names or not.
        fn serve(
            root: &Path,
            holder: &Holder,
            runtime: tokio::runtime::Runtime,
            accept: impl std::future::Future<Output = std::io::Result<apiforge_channel::Connection>>
            + Send
            + 'static,
            script: Option<Script>,
        ) -> Self {
            let owner = ProjectStore::open_exclusive_as(root, holder).unwrap();
            let root = root.to_owned();
            let (stop, mut stopped) = tokio::sync::oneshot::channel::<()>();
            let thread = thread::spawn(move || {
                runtime.block_on(async move {
                    let mut seen = Seen::default();
                    let mut owner = Some(owner);
                    let mut connection = tokio::select! {
                        _ = &mut stopped => return seen,
                        connection = accept => connection.unwrap(),
                    };
                    seen.connections = 1;
                    let Some(script) = script else {
                        return seen;
                    };
                    // A window says the person's consent first (MCP-09).
                    connection
                        .writer
                        .send(&Message::Permission { live: false })
                        .await
                        .unwrap();
                    match connection.reader.next().await.unwrap() {
                        Some(Received::Message(message)) => seen.messages.push(message),
                        other => panic!("the command asked nothing: {other:?}"),
                    }
                    tokio::time::sleep(script.delay).await;
                    match script.answer {
                        Answer::Refuse(reason) => {
                            connection
                                .writer
                                .send(&Message::Refused {
                                    reason: reason.into(),
                                })
                                .await
                                .unwrap();
                        }
                        Answer::Grant | Answer::GrantThenClose => {
                            drop(owner.take());
                            connection.writer.send(&Message::Granted).await.unwrap();
                            if script.permission_after_grant {
                                connection
                                    .writer
                                    .send(&Message::Permission { live: false })
                                    .await
                                    .unwrap();
                            }
                            if matches!(script.answer, Answer::GrantThenClose) {
                                drop(connection);
                                let _ = stopped.await;
                                return seen;
                            }
                        }
                    }
                    loop {
                        match connection.reader.next().await {
                            Ok(Some(Received::Message(message))) => {
                                if message == Message::Released {
                                    seen.retaken_on_released = Some(
                                        ProjectStore::open_exclusive_as(&root, &Holder::window())
                                            .is_ok(),
                                    );
                                }
                                seen.messages.push(message);
                            }
                            Ok(Some(Received::Ignored(why))) => panic!("ignored {why}"),
                            Ok(None) | Err(_) => {
                                seen.eof = true;
                                break;
                            }
                        }
                    }
                    let _ = stopped.await;
                    seen
                })
            });
            Self {
                stop: Some(stop),
                thread: Some(thread),
            }
        }

        /// Stops listening and returns what the window saw.
        fn close(mut self) -> Seen {
            let _ = self.stop.take().unwrap().send(());
            self.thread.take().unwrap().join().unwrap()
        }
    }

    /// The text without the line's end, once, at the start of a line.
    fn lines_with<'a>(text: &'a str, part: &str) -> Vec<&'a str> {
        text.lines().filter(|line| line.contains(part)).collect()
    }

    /// `text` contains `before`, an `HH:MM` wall-clock time, then `after`.
    fn names_holder(text: &str, before: &str, after: &str) {
        let start = text
            .find(before)
            .unwrap_or_else(|| panic!("{before:?} is not in {text:?}"));
        let rest = &text[start + before.len()..];
        let time: Vec<char> = rest.chars().take(5).collect();
        assert!(
            time.len() == 5
                && time[2] == ':'
                && [0, 1, 3, 4].iter().all(|&at| time[at].is_ascii_digit()),
            "no HH:MM after {before:?} in {text:?}"
        );
        assert!(
            rest[5..].starts_with(after),
            "{after:?} does not follow the time in {text:?}"
        );
    }

    /// Every file under `root` with its bytes; the writer's lock file by
    /// its length, since Windows cannot read a held lock file.
    fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        fn walk(root: &Path, directory: &Path, found: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(root, &path, found);
                } else {
                    let bytes = if path.file_name() == Some("project.lock".as_ref()) {
                        fs::metadata(&path).unwrap().len().to_le_bytes().to_vec()
                    } else {
                        fs::read(&path).unwrap()
                    };
                    found.insert(path.strip_prefix(root).unwrap().to_owned(), bytes);
                }
            }
        }
        let mut found = BTreeMap::new();
        walk(root, root, &mut found);
        found
    }

    fn api() -> apiforge_demo_api::DemoServerHandle {
        apiforge_demo_api::spawn("127.0.0.1:0".parse().unwrap()).unwrap()
    }

    /// Criterion 1: a clean window hands the project over; the requests are
    /// sent and journalled, the report printed, exit 0; the window is told
    /// `released` with the lock already free.
    #[test]
    fn mcp15_criterion_1_a_clean_window_hands_over_and_the_run_sends() {
        let api = api();
        let directory = shop(&api.base_url(), PASSES);
        let root = directory.path();
        let window = Window::open(root, Some(Script::grant()));
        let (output, _) = run(root, true);
        let seen = window.close();
        let text = stderr(&output);
        assert_eq!(output.status.code(), Some(0), "{text}");
        assert!(!text.contains(OLD), "{text}");
        assert_eq!(lines_with(&text, HANDED_OVER).len(), 1, "{text}");
        let report = report(&output);
        assert_eq!(report.mode, apiforge_core::run::RunMode::Live);
        assert_eq!(report.status, apiforge_core::run::RunStatus::Passed);
        // Sent, not simulated: the sample has no product name; the API's
        // answer does.
        assert!(
            serde_json::to_string(&report.steps)
                .unwrap()
                .contains("Whole bean coffee"),
            "the API answered: {:?}",
            report.steps
        );
        assert_eq!(seen.connections, 1);
        assert_eq!(seen.retaken_on_released, Some(true), "{seen:?}");
        assert!(
            !root.join(JOURNAL).exists(),
            "the finished run's entry is gone"
        );
    }

    /// Criterion 3: a window that answers after longer than the server's
    /// thirty seconds is still waited for and granted; the command says it
    /// waits, once.
    #[test]
    fn mcp15_criterion_3_the_command_waits_for_the_windows_answer() {
        let api = api();
        let directory = shop(&api.base_url(), PASSES);
        let root = directory.path();
        let window = Window::open(
            root,
            Some(Script {
                delay: Duration::from_secs(32),
                ..Script::grant()
            }),
        );
        let started = Instant::now();
        let (output, _) = run(root, true);
        let waited = started.elapsed();
        let seen = window.close();
        let text = stderr(&output);
        assert_eq!(output.status.code(), Some(0), "{text}");
        assert!(waited >= Duration::from_secs(32), "{waited:?}");
        assert_eq!(lines_with(&text, WAITING).len(), 1, "{text}");
        assert_eq!(
            report(&output).status,
            apiforge_core::run::RunStatus::Passed
        );
        assert_eq!(seen.retaken_on_released, Some(true), "{seen:?}");
    }

    /// Criterion 4: the window's `permission { live: false }`, before and
    /// after `granted`, changes nothing: the person's `--live` is the
    /// consent.
    #[test]
    fn mcp15_criterion_4_the_windows_live_toggle_does_not_govern_the_command() {
        let api = api();
        let directory = shop(&api.base_url(), PASSES);
        let root = directory.path();
        let window = Window::open(
            root,
            Some(Script {
                permission_after_grant: true,
                ..Script::grant()
            }),
        );
        let (output, _) = run(root, true);
        window.close();
        let text = stderr(&output);
        assert_eq!(output.status.code(), Some(0), "{text}");
        assert!(!text.contains("live requests are off"), "{text}");
        let report = report(&output);
        assert_eq!(report.mode, apiforge_core::run::RunMode::Live);
        assert_eq!(report.status, apiforge_core::run::RunStatus::Passed);
    }

    /// Criterion 5: an agent session holding the project, with an endpoint,
    /// is refused by name; nothing is sent and its endpoint hears nothing.
    #[test]
    fn mcp15_criterion_5_an_agent_session_is_refused_by_name_and_not_asked() {
        let directory = shop("http://127.0.0.1:9/demo", PASSES);
        let root = directory.path();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let mut listener = {
            let _context = runtime.enter();
            Listener::bind().unwrap()
        };
        let agent = Holder::new(HolderRole::Agent, Some("Claude Code".into()))
            .with_endpoint(Some(listener.endpoint().name().to_owned()));
        let window = Window::serve(
            root,
            &agent,
            runtime,
            async move { listener.accept().await },
            None,
        );
        let before = files(root);
        let (output, _) = run(root, true);
        let seen = window.close();
        let text = stderr(&output);
        assert_eq!(output.status.code(), Some(1), "{text}");
        names_holder(
            &text,
            "Claude Code via apiforge mcp has held this project since ",
            "; nothing was sent. Run again when it has finished editing",
        );
        assert!(!text.contains(OLD), "{text}");
        assert!(output.stdout.is_empty(), "no report");
        assert_eq!(seen.connections, 0, "the agent's server was not asked");
        assert_eq!(files(root), before, "nothing was journalled");
    }

    /// Criterion 6: every other holder is named; the Windows/WSL sentence
    /// belongs to an unreachable window under `/mnt/<drive>/` only (the
    /// unit tests of `apiforge_mcp::command` cover such a path).
    #[test]
    fn mcp15_criterion_6_other_holders_are_named() {
        let unreachable = Endpoint::random().name().to_owned();
        for (holder, before, after) in [
            (
                Holder::command("apply"),
                "apiforge apply has held this project since ",
                "; nothing was sent. Run again when it has finished",
            ),
            (
                Holder::window(),
                "The API Forge window has held this project since ",
                " and could not be asked; nothing was sent. Close the project there or update API Forge, then run again",
            ),
            (
                Holder::window().with_endpoint(Some(unreachable)),
                "The API Forge window has held this project since ",
                " and could not be asked; nothing was sent. Close the project there or update API Forge, then run again",
            ),
        ] {
            let directory = shop("http://127.0.0.1:9/demo", PASSES);
            let root = directory.path();
            let owner = ProjectStore::open_exclusive_as(root, &holder).unwrap();
            let (output, _) = run(root, true);
            let text = stderr(&output);
            assert_eq!(output.status.code(), Some(1), "{text}");
            names_holder(&text, before, after);
            assert!(!text.contains(OLD), "{text}");
            assert!(!text.contains(WSL), "{text}");
            drop(owner);
        }
        // A holder whose description cannot be read.
        let directory = shop("http://127.0.0.1:9/demo", PASSES);
        let root = directory.path();
        let owner = ProjectStore::open_exclusive_as(root, &Holder::window()).unwrap();
        fs::write(
            root.join(".apiforge/local/holder.json"),
            b"not a description",
        )
        .unwrap();
        let (output, _) = run(root, true);
        let text = stderr(&output);
        assert_eq!(output.status.code(), Some(1), "{text}");
        assert!(
            text.contains(
                "Another process holds this project; nothing was sent. Run again when it has let go"
            ),
            "{text}"
        );
        drop(owner);
    }

    /// Criteria 2 and 6: a window that keeps the project for the person is
    /// named with the person's choice; nothing is sent.
    #[test]
    fn mcp15_a_window_that_keeps_the_project_sends_nothing() {
        let directory = shop("http://127.0.0.1:9/demo", PASSES);
        let root = directory.path();
        let window = Window::open(
            root,
            Some(Script {
                answer: Answer::Refuse(KEEP_EDITING),
                ..Script::grant()
            }),
        );
        let (output, _) = run(root, true);
        let seen = window.close();
        let text = stderr(&output);
        assert_eq!(output.status.code(), Some(1), "{text}");
        assert!(text.contains(KEPT), "{text}");
        assert!(output.stdout.is_empty(), "no report");
        assert!(!root.join(JOURNAL).exists(), "nothing was journalled");
        assert_eq!(seen.connections, 1);
    }

    /// Criterion 8: the window closes after `granted`; the command finishes
    /// its run and exits 0; the lost channel is one line on stderr.
    #[test]
    fn mcp15_criterion_8_the_window_closing_mid_run_changes_nothing() {
        let api = api();
        let directory = shop(&api.base_url(), PASSES);
        let root = directory.path();
        let window = Window::open(
            root,
            Some(Script {
                answer: Answer::GrantThenClose,
                ..Script::grant()
            }),
        );
        let (output, _) = run(root, true);
        window.close();
        let text = stderr(&output);
        assert_eq!(output.status.code(), Some(0), "{text}");
        assert_eq!(
            report(&output).status,
            apiforge_core::run::RunStatus::Passed
        );
        assert_eq!(lines_with(&text, LOST).len(), 1, "{text}");
    }

    /// Criterion 9: a simulation with the window open connects to nothing,
    /// takes no lock and leaves the folder byte for byte as it was.
    #[test]
    fn mcp15_criterion_9_a_simulation_never_asks() {
        let directory = shop("http://127.0.0.1:9/demo", PASSES);
        let root = directory.path();
        let window = Window::open(root, None);
        let before = files(root);
        let (output, _) = run(root, false);
        let seen = window.close();
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        assert_eq!(
            report(&output).mode,
            apiforge_core::run::RunMode::Simulation
        );
        assert_eq!(seen.connections, 0, "a simulation asks no window");
        assert_eq!(files(root), before);
    }

    /// Criterion 10: the window hears `request` naming the command, then
    /// `run` when the run starts and when it ends, `released`, then the end
    /// of the connection; for a run that fails as for one that passes.
    #[test]
    fn mcp15_criterion_10_the_messages_in_order() {
        let api = api();
        // MCP-10: the end says how many steps ran and the first that did
        // not pass: the assertion of `FAILS` (step 2, `ok`, no label).
        for (scenario, code, state, failed) in [
            (PASSES, 0, RunState::Passed, None),
            (FAILS, 1, RunState::Failed, Some((2, "ok"))),
        ] {
            let directory = shop(&api.base_url(), scenario);
            let root = directory.path();
            let window = Window::open(root, Some(Script::grant()));
            let (output, pid) = run(root, true);
            let seen = window.close();
            assert_eq!(output.status.code(), Some(code), "{}", stderr(&output));
            assert_eq!(
                seen.messages,
                vec![
                    Message::Request {
                        client: Some("run --live".into()),
                        pid,
                        role: Some(Asker::Command),
                    },
                    Message::Run {
                        path: SCENARIO.into(),
                        mode: RunMode::Live,
                        state: RunState::Running,
                        step_count: None,
                        step: None,
                        label: None,
                    },
                    Message::Run {
                        path: SCENARIO.into(),
                        mode: RunMode::Live,
                        state,
                        step_count: Some(3),
                        step: failed.map(|(step, _)| step),
                        label: failed.map(|(_, label)| label.to_owned()),
                    },
                    Message::Released,
                ],
            );
            assert!(seen.eof, "the connection ends after released");
            assert_eq!(seen.retaken_on_released, Some(true));
        }
    }

    /// While the command holds the project it names itself as typed, so a
    /// window opened meanwhile and a third process read the same name.
    #[test]
    fn mcp15_the_command_names_itself_run_live_while_it_holds() {
        // A demo API that accepts and never answers holds the run open.
        let silent = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let directory = shop(
            &format!("http://{}/demo", silent.local_addr().unwrap()),
            PASSES,
        );
        let root = directory.path();
        let mut child = command(root, true)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let (_held, _) = silent.accept().unwrap();
        let holder = ProjectStore::open_read_only(root)
            .unwrap()
            .holder()
            .unwrap()
            .expect("the command holds the project");
        child.kill().unwrap();
        child.wait().unwrap();
        assert_eq!(holder.role(), Some(HolderRole::Command));
        assert_eq!(holder.client(), Some("run --live"));
        assert_eq!(holder.endpoint(), None, "a command does not listen");
    }
}
