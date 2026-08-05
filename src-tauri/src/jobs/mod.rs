//! The persistent job queue's runner.
//!
//! Every worker is a plain OS thread with its own SQLite connection. Nothing
//! here runs on a Tauri command thread: heavy work inside a `#[tauri::command]`
//! blocks the native window message pump and Windows marks the app "Not
//! Responding" — see docs/ENGINEERING-NOTES.md.

pub mod kinds;
pub mod worker;

use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::db;
use crate::error::Result;
use crate::fs::paths::LibraryPaths;
use crate::sidecar::Tools;

/// Emitted to the frontend on a fixed tick while anything is happening, and
/// once more when the queue drains.
pub const PROGRESS_EVENT: &str = "job-progress";

const TICK: Duration = Duration::from_millis(500);
const IDLE_SLEEP: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// idle | walking | working
    pub phase: &'static str,
    /// Existing items whose shard file the current reconcile has confirmed
    /// present, so far.
    pub items_checked: u64,
    /// Inbox arrivals the current reconcile has queued for hashing.
    pub queued: u64,
    pub items: i64,
    pub pending: i64,
    pub running: i64,
    pub failed: i64,
    pub completed: u64,
    pub last_error: Option<String>,
    /// True while a reconcile is running because the filesystem watcher
    /// overflowed or errored, rather than because of the ordinary startup
    /// pass. Distinguishes the two in the readout so a rescan says so
    /// instead of looking like an ordinary first index — see `fs::watch`.
    pub rescanning: bool,
}

pub struct QueueInner {
    pub paths: LibraryPaths,
    pub tools: Tools,
    /// Shared with the filesystem watcher, which sets this before queuing a
    /// reconcile after an overflow or error. Cleared here, by
    /// `worker::run_index`, once that reconcile (or the ordinary startup
    /// pass) actually finishes.
    pub rescanning: Arc<AtomicBool>,
    app: AppHandle,
    db_path: PathBuf,
    stop: AtomicBool,
    walking: AtomicBool,
    items_checked: AtomicU64,
    queued: AtomicU64,
    completed: AtomicU64,
}

impl QueueInner {
    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    pub fn set_walking(&self, walking: bool) {
        self.walking.store(walking, Ordering::Relaxed);
    }

    pub fn report_walk(&self, items_checked: u64, queued: u64) {
        self.items_checked.store(items_checked, Ordering::Relaxed);
        self.queued.store(queued, Ordering::Relaxed);
    }

    pub fn progress(&self, conn: &Connection) -> Result<Progress> {
        let counts = db::jobs::counts(conn)?;
        let walking = self.walking.load(Ordering::Relaxed);
        let busy = counts.pending > 0 || counts.running > 0;

        Ok(Progress {
            phase: if walking {
                "walking"
            } else if busy {
                "working"
            } else {
                "idle"
            },
            items_checked: self.items_checked.load(Ordering::Relaxed),
            queued: self.queued.load(Ordering::Relaxed),
            items: db::items::count(conn)?,
            pending: counts.pending,
            running: counts.running,
            failed: counts.failed,
            completed: self.completed.load(Ordering::Relaxed),
            last_error: db::jobs::last_error(conn)?,
            rescanning: self.rescanning.load(Ordering::Relaxed),
        })
    }
}

pub struct JobQueue {
    inner: Arc<QueueInner>,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

impl JobQueue {
    pub fn start(
        app: AppHandle,
        paths: LibraryPaths,
        tools: Tools,
        db_path: PathBuf,
        rescanning: Arc<AtomicBool>,
    ) -> Result<JobQueue> {
        let inner = Arc::new(QueueInner {
            paths,
            tools,
            rescanning,
            app,
            db_path,
            stop: AtomicBool::new(false),
            walking: AtomicBool::new(false),
            items_checked: AtomicU64::new(0),
            queued: AtomicU64::new(0),
            completed: AtomicU64::new(0),
        });

        // Leave a core for the UI and the webview's own decoding.
        let count = num_cpus::get().saturating_sub(1).clamp(2, 8);
        let mut threads = Vec::with_capacity(count + 1);
        for _ in 0..count {
            let inner = Arc::clone(&inner);
            threads.push(std::thread::spawn(move || work(inner)));
        }
        let ticker = Arc::clone(&inner);
        threads.push(std::thread::spawn(move || tick(ticker)));

        Ok(JobQueue {
            inner,
            threads: Mutex::new(threads),
        })
    }

    pub fn inner(&self) -> &Arc<QueueInner> {
        &self.inner
    }

    /// Signals every thread and waits for them. Workers check the flag between
    /// jobs, so a long hash or transcode finishes rather than being torn off
    /// mid-write.
    pub fn stop(&self) {
        self.inner.stop.store(true, Ordering::Relaxed);
        if let Ok(mut threads) = self.threads.lock() {
            for handle in threads.drain(..) {
                let _ = handle.join();
            }
        }
    }
}

fn work(inner: Arc<QueueInner>) {
    let mut conn = match db::open(&inner.db_path) {
        Ok(conn) => conn,
        Err(err) => {
            eprintln!("job worker could not open the database: {err}");
            return;
        }
    };

    while !inner.stopped() {
        match db::jobs::claim(&mut conn) {
            Ok(Some(job)) => {
                // A decoder panicking on a malformed file must cost one job,
                // not the whole application. `panic = "abort"` is deliberately
                // not set in the release profile so this can catch.
                let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    worker::execute(&inner, &mut conn, &job)
                }));

                match outcome {
                    Ok(Ok(())) => {
                        let _ = db::jobs::complete(&conn, job.id);
                        inner.completed.fetch_add(1, Ordering::Relaxed);
                    }
                    Ok(Err(err)) => {
                        let retry =
                            job.attempts < kinds::MAX_ATTEMPTS && worker::is_transient(&err);
                        let _ = db::jobs::fail(&conn, job.id, &err.to_string(), retry);
                    }
                    Err(_) => {
                        let _ =
                            db::jobs::fail(&conn, job.id, "worker panicked on this file", false);
                    }
                }
            }
            Ok(None) => std::thread::sleep(IDLE_SLEEP),
            Err(err) => {
                eprintln!("job worker could not claim a job: {err}");
                std::thread::sleep(Duration::from_millis(500));
            }
        }
    }
}

/// Coalesces progress into one event per tick. Individual job completions are
/// far too frequent to emit — a full index is hundreds of thousands of them.
///
/// The comparison against the previous snapshot is what makes the queue go
/// quiet when there is nothing happening, and — more importantly — what makes
/// a queue that drained inside a single tick still report itself. Emitting
/// only "while busy" misses a small library entirely.
fn tick(inner: Arc<QueueInner>) {
    let conn = match db::open(&inner.db_path) {
        Ok(conn) => conn,
        Err(err) => {
            eprintln!("progress reporter could not open the database: {err}");
            return;
        }
    };

    let mut previous: Option<Progress> = None;
    let mut elapsed = Duration::ZERO;

    while !inner.stopped() {
        std::thread::sleep(IDLE_SLEEP);
        elapsed += IDLE_SLEEP;
        if elapsed < TICK {
            continue;
        }
        elapsed = Duration::ZERO;

        let Ok(progress) = inner.progress(&conn) else {
            continue;
        };
        if previous.as_ref() == Some(&progress) {
            continue;
        }
        let _ = inner.app.emit(PROGRESS_EVENT, &progress);
        previous = Some(progress);
    }
}

pub fn enqueue_index(conn: &Connection) -> Result<()> {
    if db::jobs::is_queued(conn, kinds::INDEX, "{}")? {
        return Ok(());
    }
    db::jobs::enqueue(conn, kinds::INDEX, "{}", kinds::PRIORITY_INDEX)?;
    Ok(())
}

// The per-file enqueues below do not check for an existing job on purpose.
// `is_queued` is a scan of the job table, and calling it once per file turns a
// walk of 100k files quadratic. Duplicate work is prevented one level up
// instead: only one index job can be queued at a time, and a walk enqueues
// each file exactly once.

pub fn enqueue_hash(conn: &Connection, inbox_rel: &str) -> Result<()> {
    let payload = serde_json::to_string(&kinds::HashPayload {
        inbox_rel: inbox_rel.to_string(),
    })?;
    db::jobs::enqueue(conn, kinds::HASH, &payload, kinds::PRIORITY_HASH)?;
    Ok(())
}

pub fn enqueue_thumb(conn: &Connection, item_id: i64) -> Result<()> {
    let payload = serde_json::to_string(&kinds::ItemPayload { item_id })?;
    db::jobs::enqueue(conn, kinds::THUMB, &payload, kinds::PRIORITY_THUMB)?;
    Ok(())
}

pub fn enqueue_sprite(conn: &Connection, item_id: i64) -> Result<()> {
    let payload = serde_json::to_string(&kinds::ItemPayload { item_id })?;
    db::jobs::enqueue(conn, kinds::SPRITE, &payload, kinds::PRIORITY_SPRITE)?;
    Ok(())
}

/// Fan out a folder-level tag edit into `item_effective_tag` across its
/// subtree — a folder's own tags changed, an archetype was applied, or (with
/// `folder_id: None`) the whole library needs rebuilding. Also what a folder
/// *move* enqueues: `parent_id` changed, so ancestry (and every descendant
/// item's inherited tags) did too. A plain rename enqueues nothing — it
/// never changes `parent_id`, so no descendant's ancestry changed, only the
/// title tag's own text, which `db::folders::set_title_unjournalled` updates
/// directly.
pub fn enqueue_retag_folder(conn: &Connection, folder_id: Option<i64>) -> Result<()> {
    let payload = serde_json::to_string(&kinds::RetagFolderPayload { folder_id })?;
    db::jobs::enqueue(conn, kinds::RETAG_FOLDER, &payload, kinds::PRIORITY_RETAG)?;
    Ok(())
}

/// Recompute one item's effective tags — item-level manual tag changes.
/// A brand-new item's initial cache is built inline in `jobs::worker::run_hash`
/// instead of through this queue; see that function's doc comment.
pub fn enqueue_retag_item(conn: &Connection, item_id: i64) -> Result<()> {
    let payload = serde_json::to_string(&kinds::ItemPayload { item_id })?;
    db::jobs::enqueue(conn, kinds::RETAG_ITEM, &payload, kinds::PRIORITY_RETAG)?;
    Ok(())
}
