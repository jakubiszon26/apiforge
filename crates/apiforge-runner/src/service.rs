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
    support::SeededRandom,
};
use apiforge_core::run::RunResult;
use std::{
    io,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
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
            notify,
            active: Arc::new(AtomicUsize::new(0)),
            runs: Mutex::new(Vec::new()),
        })
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
        };
        let guard = ActiveGuard(Arc::clone(&self.active));
        let task_shared = Arc::clone(&shared);
        let notify = Arc::clone(&self.notify);
        runtime.spawn(async move {
            let _guard = guard;
            let publish = |run: &RunResult| {
                task_shared.publish(run.clone());
                notify();
            };
            let cancel = task_shared.cancel.clone();
            let result = run_validated(request, options, cancel, publish).await;
            if let Ok(run) = result {
                task_shared.publish(run);
            }
            task_shared.finished.store(true, Ordering::Release);
            notify();
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
    latest: Mutex<Option<RunResult>>,
    cancel: CancellationToken,
    finished: AtomicBool,
}

impl Shared {
    fn publish(&self, run: RunResult) {
        *self.latest.lock().unwrap_or_else(PoisonError::into_inner) = Some(run);
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
        self.shared
            .latest
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
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
