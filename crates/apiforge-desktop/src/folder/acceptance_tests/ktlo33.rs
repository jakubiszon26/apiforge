//! KTLO-33 (#122), decision 2 = A (owner, 2026-11-10): an observing window
//! whose take-back fails for a passing reason tries again at its next check;
//! the notice appears only after three failed checks in a row, and the
//! window keeps trying.
//!
//! The failure is injected through the storage worker's fault seam at the
//! first file operation `ProjectStore::open_exclusive_as` reports after it
//! has taken the lock: the flush of `.apiforge/local`, just before the
//! holder description is replaced. That is where a reader or a scanner
//! holding a file for a moment fails a take-back on Windows (flake 4).
use super::*;
use apiforge_storage::project::{
    Holder, HolderRole, ProjectError, ProjectStore,
    faults::{Boundary, Event, Phase},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

/// The notice of a take-back that failed, as the window words it.
const NOTICE: &str = "This window could not take the project back";
/// Generous: a check runs once a second, so only a broken window needs it.
const STUCK: Duration = Duration::from_secs(20);

fn read_only(harness: &FolderHarness) -> bool {
    folder(harness).session.as_ref().unwrap().is_read_only()
}

fn noticed(harness: &FolderHarness) -> bool {
    harness.query_by_label_contains(NOTICE).is_some()
}

/// The first flush the take-back makes once it holds the lock.
fn after_the_lock(event: &Event) -> bool {
    event.boundary == Boundary::DirectorySync
        && event.phase == Phase::Before
        && event.path.ends_with(".apiforge/local")
}

/// Fails the take-back's first flush while `failing` holds, counting each
/// failure in `failed`.
fn fail_take_backs(
    harness: &mut FolderHarness,
    failing: impl Fn(usize) -> bool + Send + Sync + 'static,
) -> Arc<AtomicUsize> {
    let failed = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&failed);
    app(harness).storage.fault = Some(Arc::new(move |event| {
        if after_the_lock(event) && failing(counter.load(Ordering::Acquire)) {
            counter.fetch_add(1, Ordering::AcqRel);
            return Err(ProjectError::Io {
                path: event.path.clone(),
                source: std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "injected: a reader held the file",
                ),
            });
        }
        Ok(())
    }));
    failed
}

/// The window observing an agent that holds the demo shop; dropping the
/// returned store releases the project.
fn observing() -> (tempfile::TempDir, ProjectStore, FolderHarness) {
    let directory = shop();
    let owner = ProjectStore::open_exclusive_as(
        directory.path(),
        &Holder::new(HolderRole::Agent, Some("Claude Code".into())),
    )
    .unwrap();
    let harness = folder_harness(directory.path(), WIDE);
    assert!(read_only(&harness), "the window observes");
    (directory, owner, harness)
}

/// Steps the window in real time until `done` holds; `during` is asserted
/// after every frame.
fn step_until(
    harness: &mut FolderHarness,
    what: &str,
    done: impl Fn(&FolderHarness) -> bool,
    during: impl Fn(&FolderHarness),
) {
    let deadline = Instant::now() + STUCK;
    while !done(harness) {
        assert!(Instant::now() < deadline, "the window never showed {what}");
        harness.step();
        during(harness);
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Criterion 4, first half: the first take-back fails; the window tries
/// again at its next check and becomes the writer, with no notice.
#[test]
fn ktlo33_a_take_back_that_fails_once_is_tried_again_at_the_next_check() {
    let (_directory, owner, mut harness) = observing();
    let failed = fail_take_backs(&mut harness, |failed| failed < 1);
    drop(owner);
    step_until(
        &mut harness,
        "the project taken back after one failure",
        |harness| !read_only(harness),
        |harness| assert!(!noticed(harness), "one failure says nothing"),
    );
    assert_eq!(
        failed.load(Ordering::Acquire),
        1,
        "the first attempt failed"
    );
    settle(&mut harness);
    assert!(!noticed(&harness), "no notice once taken back");
    assert!(!has_label(&harness, "Observing"));
}

/// Criterion 4, second half: three failed checks in a row show the notice;
/// the window keeps trying and takes the project back once the reason has
/// passed.
#[test]
fn ktlo33_three_failed_take_backs_in_a_row_show_the_notice_and_the_window_keeps_trying() {
    let (_directory, owner, mut harness) = observing();
    let passing = Arc::new(AtomicBool::new(false));
    let passed = Arc::clone(&passing);
    let failed = fail_take_backs(&mut harness, move |_| !passed.load(Ordering::Acquire));
    let count = Arc::clone(&failed);
    let fewer_than_three_say_nothing = move |harness: &FolderHarness| {
        if noticed(harness) {
            assert!(
                count.load(Ordering::Acquire) >= 3,
                "the notice after {} failure(s)",
                count.load(Ordering::Acquire)
            );
        }
    };
    drop(owner);
    step_until(
        &mut harness,
        "the notice",
        noticed,
        &fewer_than_three_say_nothing,
    );
    assert!(read_only(&harness), "still observing");
    // Still trying: a fourth check tries again.
    let tried = Arc::clone(&failed);
    step_until(
        &mut harness,
        "a fourth attempt",
        move |_| tried.load(Ordering::Acquire) >= 4,
        |_| {},
    );
    assert!(noticed(&harness), "the notice stays while it fails");
    passing.store(true, Ordering::Release);
    step_until(
        &mut harness,
        "the project taken back once the reason passed",
        |harness| !read_only(harness),
        |_| {},
    );
}
