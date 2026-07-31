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
    pub folders: u64,
    pub files_seen: u64,
    pub items: i64,
    pub pending: i64,
    pub running: i64,
    pub failed: i64,
    pub completed: u64,
    pub last_error: Option<String>,
}

pub struct QueueInner {
    pub paths: LibraryPaths,
    pub tools: Tools,
    app: AppHandle,
    db_path: PathBuf,
    stop: AtomicBool,
    walking: AtomicBool,
    folders: AtomicU64,
    files_seen: AtomicU64,
    completed: AtomicU64,
}

impl QueueInner {
    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    pub fn set_walking(&self, walking: bool) {
        self.walking.store(walking, Ordering::Relaxed);
    }

    pub fn report_walk(&self, folders: u64, files: u64) {
        self.folders.store(folders, Ordering::Relaxed);
        self.files_seen.store(files, Ordering::Relaxed);
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
            folders: self.folders.load(Ordering::Relaxed),
            files_seen: self.files_seen.load(Ordering::Relaxed),
            items: db::items::count(conn)?,
            pending: counts.pending,
            running: counts.running,
            failed: counts.failed,
            completed: self.completed.load(Ordering::Relaxed),
            last_error: db::jobs::last_error(conn)?,
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
    ) -> Result<JobQueue> {
        let inner = Arc::new(QueueInner {
            paths,
            tools,
            app,
            db_path,
            stop: AtomicBool::new(false),
            walking: AtomicBool::new(false),
            folders: AtomicU64::new(0),
            files_seen: AtomicU64::new(0),
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

pub fn enqueue_hash(conn: &Connection, folder_id: i64, disk_name: &str) -> Result<()> {
    let payload = serde_json::to_string(&kinds::HashPayload {
        folder_id,
        disk_name: disk_name.to_string(),
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
