//! A background runner for GUI threads.
//!
//! The service owns a Tokio runtime on its own worker threads. A GUI starts runs and
//! polls their handles while painting; it never blocks on I/O. Snapshots go through a
//! latest-value slot rather than a queue: a slow reader skips intermediate snapshots
//! but always sees the newest one, including the final report.

use crate::{
    engine::run_validated,
    http::HttpTransport,
    request::{RunError, RunLimits, RunOptions, RunRequest, validate},
    support::{SeededRandom, rfc3339_millis},
};
use apiforge_core::{
    journal::JournalSink,
    run::{RunResult, RunStatus},
};
use std::{
    io,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime},
};
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

/// Runs that may execute at the same time.
pub const MAX_ACTIVE_RUNS: usize = 4;

/// Called whenever a run publishes a new snapshot, e.g. to request a repaint.
pub type Notify = Arc<dyn Fn() + Send + Sync>;

/// Executes runs in the background and hands out their snapshots.
pub struct RunnerService {
    runtime: Option<Runtime>,
    transport: HttpTransport,
    limits: RunLimits,
    /// Journals live runs; see [`RunOptions::journal`].
    journal: Option<Arc<dyn JournalSink>>,
    notify: Notify,
    active: Arc<AtomicUsize>,
    runs: Mutex<Vec<Arc<Shared>>>,
}

impl RunnerService {
    pub fn new(notify: Notify) -> io::Result<Self> {
        Self::with_limits(notify, RunLimits::default())
    }

    pub fn with_limits(notify: Notify, limits: RunLimits) -> io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("apiforge-runner")
            .enable_all()
            .build()?;
        let transport = HttpTransport::new().map_err(io::Error::other)?;
        Ok(Self {
            runtime: Some(runtime),
            transport,
            limits,
            journal: None,
            notify,
            active: Arc::new(AtomicUsize::new(0)),
            runs: Mutex::new(Vec::new()),
        })
    }

    /// Records live runs of this service in `journal`.
    #[must_use]
    pub fn with_journal(mut self, journal: Arc<dyn JournalSink>) -> Self {
        self.journal = Some(journal);
        self
    }

    /// Whether live runs of this service are journalled.
    pub fn is_journalled(&self) -> bool {
        self.journal.is_some()
    }

    /// Validates and starts a run. Nothing executes when an error is returned.
    pub fn start(&self, request: RunRequest) -> Result<RunHandle, RunError> {
        validate(&request)?;
        let runtime = self
            .runtime
            .as_ref()
            .ok_or(RunError::TooManyActiveRuns(0))?;
        self.active
            .try_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                (active < MAX_ACTIVE_RUNS).then_some(active + 1)
            })
            .map_err(|_| RunError::TooManyActiveRuns(MAX_ACTIVE_RUNS))?;
        let shared = Arc::new(Shared::default());
        {
            let mut runs = self.runs.lock().unwrap_or_else(PoisonError::into_inner);
            runs.retain(|run| !run.finished.load(Ordering::Acquire));
            runs.push(Arc::clone(&shared));
        }
        let options = RunOptions {
            limits: self.limits.clone(),
            transport: self.transport.clone(),
            random: Box::new(SeededRandom::from_entropy()),
            journal: self.journal.clone(),
        };
        let guard = ActiveGuard(Arc::clone(&self.active));
        let task_shared = Arc::clone(&shared);
        let notify = Arc::clone(&self.notify);
        let fallback = RunResult {
            id: apiforge_core::ids::new_id("run"),
            scenario_id: request.scenario_id.clone(),
            mode: request.mode,
            started_at: rfc3339_millis(SystemTime::now()),
            finished_at: None,
            status: RunStatus::Failed,
            steps: Vec::new(),
            diagnostic: request.target_node_id.is_some() || request.single_node_id.is_some(),
            snapshot: None,
            error: Some(
                "Internal runner error: the worker terminated before finishing the run.".to_owned(),
            ),
            notice: None,
        };
        let finish = FinishGuard::new(Arc::clone(&shared), Arc::clone(&notify), fallback);
        runtime.spawn(async move {
            let _guard = guard;
            let publish = |run: &RunResult| {
                task_shared.publish(run.clone());
                notify();
            };
            let cancel = task_shared.cancel.clone();
            match run_validated(request, options, cancel, publish).await {
                Ok(run) => finish.complete(run),
                Err(error) => finish.fail_with(format!("Internal runner error: {error}")),
            }
        });
        Ok(RunHandle { shared })
    }

    /// Runs that have not finished yet.
    pub fn active_runs(&self) -> usize {
        self.active.load(Ordering::Acquire)
    }

    /// Cancels every run and waits up to `timeout` for them to finish, cleanup
    /// included (cleanup has its own budget and ignores cancellation).
    pub fn shutdown(mut self, timeout: Duration) {
        self.cancel_all();
        if let Some(runtime) = self.runtime.take() {
            let active = Arc::clone(&self.active);
            runtime.block_on(async move {
                let _ = tokio::time::timeout(timeout, async {
                    while active.load(Ordering::Acquire) > 0 {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                })
                .await;
            });
            runtime.shutdown_background();
        }
    }

    fn cancel_all(&self) {
        let runs = self.runs.lock().unwrap_or_else(PoisonError::into_inner);
        for run in runs.iter() {
            run.cancel.cancel();
        }
    }
}

impl Drop for RunnerService {
    /// Dropping without [`RunnerService::shutdown`] cancels runs and abandons them,
    /// cleanup included; it never blocks the calling thread.
    fn drop(&mut self) {
        self.cancel_all();
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

/// State shared between a run task and its handle.
#[derive(Default)]
struct Shared {
    latest: Mutex<Snapshot>,
    cancel: CancellationToken,
    finished: AtomicBool,
}

/// Retain the last redacted snapshot even after the UI consumes its notification.
#[derive(Default)]
struct Snapshot {
    report: Option<RunResult>,
    unread: bool,
}

impl Shared {
    fn publish(&self, run: RunResult) {
        let mut slot = self.latest.lock().unwrap_or_else(PoisonError::into_inner);
        slot.report = Some(run);
        slot.unread = true;
    }

    fn publish_terminal_failure(&self, mut fallback: RunResult) {
        let now = rfc3339_millis(SystemTime::now());
        let mut slot = self.latest.lock().unwrap_or_else(PoisonError::into_inner);
        match slot.report.as_mut() {
            Some(existing) if existing.status == RunStatus::Running => {
                existing.status = RunStatus::Failed;
                existing.finished_at.get_or_insert(now);
                existing.error = fallback.error;
            }
            None => {
                fallback.finished_at.get_or_insert(now);
                slot.report = Some(fallback);
            }
            Some(_) => {}
        }
        slot.unread = true;
    }
}

/// Ensures `finished` is marked and a terminal `RunResult` is published even if the
/// background worker task unwinds on a panic or is aborted.
struct FinishGuard {
    shared: Arc<Shared>,
    notify: Notify,
    fallback: Option<RunResult>,
}

impl FinishGuard {
    fn new(shared: Arc<Shared>, notify: Notify, fallback: RunResult) -> Self {
        Self {
            shared,
            notify,
            fallback: Some(fallback),
        }
    }

    fn complete(mut self, run: RunResult) {
        self.fallback = None;
        self.shared.publish(run);
        self.shared.finished.store(true, Ordering::Release);
        (self.notify)();
    }

    fn fail_with(mut self, message: String) {
        if let Some(mut fallback) = self.fallback.take() {
            fallback.error = Some(message);
            self.shared.publish_terminal_failure(fallback);
        }
        self.shared.finished.store(true, Ordering::Release);
        (self.notify)();
    }
}

impl Drop for FinishGuard {
    fn drop(&mut self) {
        if let Some(mut fallback) = self.fallback.take() {
            if std::thread::panicking() {
                fallback.error = Some("Internal runner error: worker task panicked.".to_owned());
            }
            self.shared.publish_terminal_failure(fallback);
            self.shared.finished.store(true, Ordering::Release);
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (self.notify)()));
        }
    }
}

/// Releases an active-run slot when the run task ends, however it ends.
struct ActiveGuard(Arc<AtomicUsize>);

impl Drop for ActiveGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// A started run. Dropping the handle does not cancel the run.
pub struct RunHandle {
    shared: Arc<Shared>,
}

impl RunHandle {
    /// The newest redacted snapshot, if one arrived since the previous call.
    pub fn latest(&self) -> Option<RunResult> {
        let mut slot = self
            .shared
            .latest
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if !std::mem::take(&mut slot.unread) {
            return None;
        }
        slot.report.clone()
    }

    /// Requests cancellation of the main phase; cleanup still runs.
    pub fn cancel(&self) {
        self.shared.cancel.cancel();
    }

    /// Whether the run has ended. The final report is available from
    /// [`RunHandle::latest`] once this is true (unless it was already taken).
    pub fn is_finished(&self) -> bool {
        self.shared.finished.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apiforge_core::run::RunMode;

    #[test]
    fn finish_guard_marks_run_failed_and_notifies_on_panic() {
        let shared = Arc::new(Shared::default());
        let notified = Arc::new(AtomicBool::new(false));
        let notified_clone = Arc::clone(&notified);
        let notify: Notify = Arc::new(move || {
            notified_clone.store(true, Ordering::SeqCst);
        });
        let fallback = RunResult {
            id: "run-1".into(),
            scenario_id: "scen-1".into(),
            mode: RunMode::Simulation,
            started_at: "2026-10-20T00:00:00.000Z".into(),
            finished_at: None,
            status: RunStatus::Failed,
            steps: Vec::new(),
            diagnostic: false,
            snapshot: None,
            error: Some("Internal runner error".into()),
            notice: None,
        };
        let shared_for_panic = Arc::clone(&shared);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _finish = FinishGuard::new(shared_for_panic, notify, fallback);
            panic!("simulated worker panic");
        }));
        assert!(result.is_err());
        let handle = RunHandle { shared };
        assert!(handle.is_finished());
        assert!(notified.load(Ordering::SeqCst));
        let report = handle
            .latest()
            .expect("terminal failure report is published");
        assert_eq!(report.status, RunStatus::Failed);
        assert!(
            report
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("panicked")
        );
    }
    #[test]
    fn worker_panic_after_ui_poll_preserves_run_identity_and_releases_slot() {
        use std::sync::mpsc;
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let release_rx = Mutex::new(release_rx);
        let first = AtomicBool::new(true);
        let notify: Notify = Arc::new(move || {
            if first.swap(false, Ordering::SeqCst) {
                entered_tx.send(()).unwrap();
                release_rx
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap();
                panic!("injected worker observer panic");
            }
        });
        let service = RunnerService::new(notify).unwrap();
        let project = apiforge_core::demos::shop_project();
        let request = RunRequest::new(
            project.clone(),
            project.scenarios[0].id.clone(),
            RunMode::Simulation,
        );
        let handle = service.start(request).unwrap();
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let first = handle.latest().unwrap();
        assert_eq!(first.status, RunStatus::Running);
        assert!(handle.latest().is_none());
        release_tx.send(()).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !handle.is_finished() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(handle.is_finished());
        let failed = handle.latest().unwrap();
        assert_eq!(failed.id, first.id);
        assert!(!failed.id.is_empty());
        assert_eq!(failed.status, RunStatus::Failed);
        assert!(failed.finished_at.is_some());
        assert!(failed.error.unwrap().contains("panicked"));
        service.shutdown(Duration::from_secs(1));
    }
}
