//! REL-04 (#205), criterion 5: a crash when the program has no console. The
//! output routing of `src/windows_console.rs` is compiled into this test.
//! Each test starts the helper below as a child process. The helper installs
//! it on a temporary log, as if the parent had given no handle, with the
//! dialog replaced by a file. It prints a line on each stream and panics on
//! the thread that installed the routing (the window's thread in the
//! program) or on another one.
#![cfg(windows)]

#[allow(
    dead_code,
    reason = "the program's start and its real dialog are not exercised here"
)]
#[path = "../src/windows_console.rs"]
mod windows_console;

use std::{
    env, fs,
    path::PathBuf,
    process::{Command, Output},
    thread,
};

const LOG: &str = "APIFORGE_REL04_LOG";
const DIALOG: &str = "APIFORGE_REL04_DIALOG";
const THREAD: &str = "APIFORGE_REL04_THREAD";

struct Crash {
    output: Output,
    log: PathBuf,
    text: String,
    dialog: Option<String>,
}

fn crash(thread: &str) -> Crash {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("logs").join("apiforge-desktop.log");
    let dialog = dir.path().join("dialog.txt");
    let output = Command::new(env::current_exe().unwrap())
        .args([
            "crash_helper",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(LOG, &log)
        .env(DIALOG, &dialog)
        .env(THREAD, thread)
        .env_remove("RUST_BACKTRACE")
        .output()
        .unwrap();
    Crash {
        output,
        text: fs::read_to_string(&log).unwrap_or_default(),
        dialog: fs::read_to_string(&dialog).ok(),
        log,
    }
}

#[test]
fn a_panic_on_the_window_thread_is_logged_and_shown() {
    let crash = crash("window");
    assert_eq!(crash.output.status.code(), Some(101), "{:?}", crash.output);
    let text = &crash.text;
    let start = format!("--- apiforge-desktop {} ", env!("CARGO_PKG_VERSION"));
    assert!(text.starts_with(&start), "{text}");
    assert!(
        text.contains("\nREL-04: printed\nREL-04: reported\n"),
        "{text}"
    );
    assert!(text.contains("panicked at"), "{text}");
    assert!(text.contains("REL-04: the window thread panics"), "{text}");
    assert_eq!(
        crash.dialog.as_deref(),
        Some(windows_console::dialog_text(&crash.log).as_str())
    );
}

#[test]
fn a_panic_on_a_worker_thread_is_logged_only() {
    let crash = crash("worker");
    assert_eq!(crash.output.status.code(), Some(0), "{:?}", crash.output);
    let text = &crash.text;
    assert!(text.contains("panicked at"), "{text}");
    assert!(text.contains("REL-04: a worker thread panics"), "{text}");
    assert_eq!(crash.dialog, None);
}

#[test]
#[ignore = "launched by crash_report"]
fn crash_helper() {
    let log = PathBuf::from(env::var_os(LOG).expect("crash() supplies the log"));
    let dialog = PathBuf::from(env::var_os(DIALOG).expect("crash() supplies the dialog file"));
    windows_console::install(
        log,
        |_| false,
        move |text: &str| fs::write(&dialog, text).unwrap(),
    );
    windows_console::write(
        windows_console::Stream::Out,
        format_args!("REL-04: printed"),
    );
    windows_console::write(
        windows_console::Stream::Err,
        format_args!("REL-04: reported"),
    );
    if env::var(THREAD).as_deref() == Ok("worker") {
        let worker = thread::spawn(|| panic!("REL-04: a worker thread panics"));
        assert!(worker.join().is_err());
    } else {
        panic!("REL-04: the window thread panics");
    }
}
