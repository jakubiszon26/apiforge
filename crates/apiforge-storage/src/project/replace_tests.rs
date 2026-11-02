//! MCP-19 acceptance: a commit beside a reader of its targets
//! (`docs/tickets/MCP-19.md`, criteria 1 to 3). The reader is synchronised
//! through the I/O fault seam: the commit stops at `Boundary::Replace`
//! until the reader holds the target, so the collision is certain rather
//! than likely. The seam is this crate's own, so the tests live beside it.
use super::io::{
    Boundary, Phase,
    testing::{Event, with_hook},
};
use super::tests::{initialise, path, write};
use super::*;
use std::{
    cell::Cell,
    fs::{self, File},
    io::{self, Read as _},
    path::Path,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{RecvTimeoutError, channel},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const MAIN: &str = "scenarios/main.afs";
const OLD: &[u8] = b"scenario \"Main\" {}\n";
const NEW: &[u8] = b"scenario \"Saved\" {}\n";
/// The ticket's bound on the wait for a handle that does not share delete.
#[cfg(windows)]
const BOUND: Duration = Duration::from_secs(1);
/// Long enough that a scan which is not held has answered.
const NOT_HELD: Duration = Duration::from_millis(500);
/// Generous: only reached when something is broken.
const STUCK: Duration = Duration::from_secs(20);

fn save() -> Vec<ProjectChange> {
    vec![write(MAIN, FileState::of(OLD), NEW)]
}

fn is_replace_of(event: &Event, phase: Phase, target: &Path) -> bool {
    event.boundary == Boundary::Replace && event.phase == phase && event.path == target
}

/// When the reader thread lets the target go.
#[derive(Clone, Copy)]
enum Release {
    /// When the commit has replaced it.
    AfterTheReplace,
    /// This long after the commit first tries to replace it.
    #[cfg_attr(not(windows), allow(dead_code, reason = "Windows-only criterion 2"))]
    After(Duration),
}

/// What happened to a commit beside a reader.
struct Beside {
    result: Result<u64>,
    /// From the commit's first attempt to replace the target to its answer.
    elapsed: Duration,
    /// Whether the reader still held the target when the commit answered.
    #[cfg_attr(not(windows), allow(dead_code, reason = "Windows-only criterion 2"))]
    held_at_answer: bool,
    /// The bytes the reader read through its handle, once it let go.
    reader: JoinHandle<Vec<u8>>,
}

/// Commits `save()` on this thread while another thread holds `MAIN` open
/// with `open` from just before the commit replaces it until `release`.
/// The reader's hold starts when the commit records its first attempt, so a
/// slow scheduler cannot close the handle before the attempt.
fn commit_beside_a_reader(
    writer: &ProjectStore,
    open: impl FnOnce(&Path) -> io::Result<File> + Send + 'static,
    release: Release,
) -> Beside {
    let target = writer.root().join(MAIN);
    let (open_now, opening) = channel::<()>();
    let (holding, held) = channel::<()>();
    let (let_go, letting_go) = channel::<()>();
    let (attempting, attempt) = channel::<()>();
    let closed = Arc::new(AtomicBool::new(false));
    let reader = {
        let target = target.clone();
        let closed = Arc::clone(&closed);
        thread::spawn(move || {
            if opening.recv().is_err() {
                return Vec::new();
            }
            let mut file = open(&target).expect("the reader opens the target");
            holding.send(()).unwrap();
            match release {
                // A commit that failed drops the hook, and with it the sender.
                Release::AfterTheReplace => drop(letting_go.recv()),
                Release::After(hold) => {
                    if attempt.recv().is_ok() {
                        thread::sleep(hold);
                    }
                }
            }
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)
                .expect("the reader reads its handle");
            closed.store(true, Ordering::SeqCst);
            drop(file);
            bytes
        })
    };
    let attempted = Rc::new(Cell::new(None::<Instant>));
    let result = with_hook(
        {
            let attempted = Rc::clone(&attempted);
            let mut open_now = Some(open_now);
            move |event| {
                if is_replace_of(event, Phase::Before, &target)
                    && let Some(open_now) = open_now.take()
                {
                    open_now.send(()).unwrap();
                    held.recv_timeout(STUCK)
                        .expect("the reader holds the target");
                    attempted.set(Some(Instant::now()));
                    let _ = attempting.send(());
                }
                if is_replace_of(event, Phase::After, &target) {
                    let _ = let_go.send(());
                }
                Ok(())
            }
        },
        || writer.commit(save()),
    );
    let elapsed = attempted
        .get()
        .expect("the commit reached the target's replace")
        .elapsed();
    Beside {
        result,
        elapsed,
        held_at_answer: !closed.load(Ordering::SeqCst),
        reader,
    }
}

fn store() -> (tempfile::TempDir, ProjectStore) {
    let directory = tempfile::tempdir().unwrap();
    let writer = ProjectStore::open_exclusive(directory.path()).unwrap();
    initialise(&writer);
    (directory, writer)
}

/// The generation a stable read in another store sees, and the target.
fn stable_read(root: &Path) -> (u64, Option<Vec<u8>>) {
    let mut snapshot = ProjectStore::open_read_only(root)
        .unwrap()
        .read(&[path(MAIN)])
        .expect("a stable read after the commit");
    (
        snapshot.generation,
        snapshot.files.remove(&path(MAIN)).flatten(),
    )
}

/// Criterion 1: a reader with Rust's default share mode, as this window,
/// another window, the MCP server or the command line opens a file, holds
/// the target for the whole replace. The commit succeeds and the reader
/// keeps the bytes it opened.
#[test]
fn mcp19_criterion_1_a_commit_replaces_a_target_a_reader_holds_open() {
    let (directory, writer) = store();
    let beside = commit_beside_a_reader(
        &writer,
        |target| File::open(target),
        Release::AfterTheReplace,
    );
    let generation = beside.result.unwrap_or_else(|error| {
        panic!(
            "the commit beside a reader failed after {:?}: {error}",
            beside.elapsed
        )
    });
    assert_eq!(
        beside.reader.join().unwrap(),
        OLD,
        "the reader keeps the bytes it opened"
    );
    assert_eq!(
        stable_read(directory.path()),
        (generation, Some(NEW.to_vec()))
    );
    assert_eq!(writer.head().unwrap().active_transaction_id, None);
}

/// A reader that shares reading only, as a program other than API Forge may
/// open a file: the rename cannot replace the target while it is open.
#[cfg(windows)]
fn open_sharing_read_only(target: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt as _;
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(target)
}

/// Criterion 2, within the bound: the reader closes 200 ms after the first
/// attempt to replace; the commit waits for it and succeeds.
#[cfg(windows)]
#[test]
fn mcp19_criterion_2_a_commit_waits_for_a_reader_that_does_not_share_delete() {
    let (directory, writer) = store();
    let beside = commit_beside_a_reader(
        &writer,
        open_sharing_read_only,
        Release::After(Duration::from_millis(200)),
    );
    let elapsed = beside.elapsed;
    let generation = beside.result.unwrap_or_else(|error| {
        panic!("the commit failed {elapsed:?} after its first attempt: {error}")
    });
    assert!(
        elapsed < BOUND,
        "the commit succeeded {elapsed:?} after its first attempt, beyond the bound"
    );
    assert!(
        !beside.held_at_answer,
        "the commit answered after {elapsed:?} while the reader still held the target"
    );
    assert_eq!(beside.reader.join().unwrap(), OLD);
    assert_eq!(
        stable_read(directory.path()),
        (generation, Some(NEW.to_vec()))
    );
    assert_eq!(writer.head().unwrap().active_transaction_id, None);
}

/// Criterion 2, beyond the bound: a reader that holds the target for two
/// seconds. The commit gives up after the bound, after activation, as
/// today; once the handle is closed, recovery finishes it.
#[cfg(windows)]
#[test]
fn mcp19_criterion_2_a_reader_held_beyond_the_bound_leaves_recoverable_intent() {
    let (directory, writer) = store();
    let beside = commit_beside_a_reader(
        &writer,
        open_sharing_read_only,
        Release::After(Duration::from_secs(2)),
    );
    let elapsed = beside.elapsed;
    assert!(
        matches!(beside.result, Err(ProjectError::Io { .. })),
        "the commit answered {:?} after {elapsed:?}",
        beside.result
    );
    assert!(
        beside.held_at_answer,
        "the commit waited {elapsed:?}, until the reader let go"
    );
    assert!(
        elapsed >= BOUND,
        "the commit gave up {elapsed:?} after its first attempt, before the bound"
    );
    let head = writer.head().unwrap();
    assert!(
        head.active_transaction_id.is_some(),
        "the failure came after activation"
    );
    assert_eq!(beside.reader.join().unwrap(), OLD);
    let generation = writer
        .recover()
        .expect("recovery once the handle is closed")
        .expect("the transaction was still active");
    assert_eq!(generation, head.generation + 1);
    assert_eq!(
        stable_read(directory.path()),
        (generation, Some(NEW.to_vec()))
    );
}

/// Criterion 3: the window's check scans through a clone of the writer's
/// store while its storage worker commits. The scan waits for the commit
/// and sees its generation, so it never hashes a file the commit replaces
/// or removes. A read-only store's scan is not held.
#[test]
fn mcp19_criterion_3_a_scan_waits_behind_this_writers_commit() {
    let (directory, writer) = store();
    let target = writer.root().join(MAIN);
    let (stopped, at_replace) = channel::<()>();
    let (resume, resumed) = channel::<()>();
    let committer = writer.clone();
    let commit = thread::spawn(move || {
        with_hook(
            move |event| {
                if is_replace_of(event, Phase::Before, &target) {
                    stopped.send(()).unwrap();
                    resumed
                        .recv_timeout(STUCK)
                        .expect("the test resumes the commit");
                }
                Ok(())
            },
            || committer.commit(save()),
        )
    });
    at_replace
        .recv_timeout(STUCK)
        .expect("the commit reached the replace");

    let observer = ProjectStore::open_read_only(directory.path()).unwrap();
    let (observed, observing) = channel();
    thread::spawn(move || observed.send(observer.scan_tree().map(|scan| scan.generation)));
    let watcher = writer.clone();
    let (started, starting) = channel::<()>();
    let (scanned, scanning) = channel();
    thread::spawn(move || {
        started.send(()).unwrap();
        scanned.send(watcher.scan_tree().map(|scan| scan.generation))
    });
    // The scan has started before the commit can go on: what follows is
    // its answer during the commit, or its wait.
    starting
        .recv_timeout(STUCK)
        .expect("the writer's scan starts");

    let other = observing
        .recv_timeout(STUCK)
        .expect("a read-only store's scan is not held behind the writer");
    assert!(
        matches!(other, Err(ProjectError::Busy)),
        "another process's scan meets a commit in progress: {other:?}"
    );
    match scanning.recv_timeout(NOT_HELD) {
        Err(RecvTimeoutError::Timeout) => {}
        early => panic!("the writer's scan answered during its own commit: {early:?}"),
    }
    resume.send(()).unwrap();
    let generation = commit.join().unwrap().expect("the commit");
    let scan = scanning
        .recv_timeout(STUCK)
        .expect("the scan answers after the commit");
    assert_eq!(
        scan.expect("the scan after the commit"),
        generation,
        "the scan sees the commit's generation"
    );
    assert_eq!(fs::read(directory.path().join(MAIN)).unwrap(), NEW);
}
