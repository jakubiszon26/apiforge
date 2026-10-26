//! Acceptance tests for this crate's sprint tickets. Each test names the
//! criterion it covers in `docs/tickets/<ID>.md`.
use apiforge_storage::project::ProjectStore;
use std::time::{Duration, Instant};

/// KTLO-21, criteria 1 and 2: a probe never reports a writer that was already
/// dropped. `Command::spawn` forks this process, and the child keeps a
/// duplicate of the writer's lock descriptor until its exec, so the lock still
/// looks held after the only `File` for it has been closed. The spawning thread
/// creates that window deliberately; without it the race cannot occur at all.
#[cfg(unix)]
#[test]
fn a_reader_never_sees_a_writer_that_was_already_dropped() {
    use std::{
        process::{Command, Stdio},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };
    const ROUNDS: u32 = 600;

    let stop = Arc::new(AtomicBool::new(false));
    let spawning = {
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            let mut spawned = 0u32;
            while !stop.load(Ordering::Relaxed) {
                let mut child = Command::new("/bin/true")
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .expect("spawn /bin/true");
                child.wait().expect("wait for /bin/true");
                spawned += 1;
            }
            spawned
        })
    };

    let directory = tempfile::tempdir().expect("temporary folder");
    let reader = ProjectStore::open_read_only(directory.path()).expect("read-only store");

    // A live writer is seen once, outside the loop: confirming a would-block
    // costs the confirmation window, and the released lock is what this test is
    // about.
    let writer = ProjectStore::open_exclusive(directory.path()).expect("writer");
    assert!(
        reader.has_live_writer().expect("probe"),
        "a live writer was not seen"
    );
    drop(writer);

    let mut rounds = 0;
    let mut stale = 0;
    // Stops at the first stale answer: one is already a defect, and the run
    // before the fix stays short.
    while rounds < ROUNDS && stale == 0 {
        rounds += 1;
        let writer = ProjectStore::open_exclusive(directory.path()).expect("writer");
        drop(writer);
        if reader.has_live_writer().expect("probe") {
            stale += 1;
        }
        // A probe that reports a writer must not contradict opening one.
        assert!(
            !ProjectStore::open(directory.path())
                .expect("writer after the probe")
                .is_read_only(),
            "round {rounds}: the probe kept the next writer out"
        );
    }
    stop.store(true, Ordering::Relaxed);
    let spawned = spawning.join().expect("spawning thread");
    assert_eq!(
        stale, 0,
        "a dropped writer was still reported as live in round {rounds} of {ROUNDS}, \
         alongside {spawned} process spawns"
    );
}

/// KTLO-21, criterion 3: the session holding the writer lock answers from what
/// it already knows, so confirming a would-block never makes a writer wait for
/// its own lock. Passes before the fix as well; it bounds the wait the fix adds.
#[test]
fn a_writer_reports_its_own_lock_without_waiting() {
    let directory = tempfile::tempdir().expect("temporary folder");
    let writer = ProjectStore::open_exclusive(directory.path()).expect("writer");
    let start = Instant::now();
    assert!(writer.has_live_writer().expect("probe"));
    let waited = start.elapsed();
    assert!(
        waited < Duration::from_millis(50),
        "the writer waited {waited:?} for its own lock"
    );
}
