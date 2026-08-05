//! The filesystem watcher — narrowed by PLAN.md §M2.6 to `<root>/inbox/` and
//! the root's own top level, replacing the whole-tree watch M1.8 built.
//! `notify::recommended_watcher` on Windows is `ReadDirectoryChangesW`: two
//! handles on the same watcher, one recursive and one not, so this is still
//! push notifications, not polling.
//!
//! Three things still need care here:
//!
//! - **Settling.** A file just copied in from Explorer emits events long
//!   before it is whole. `Pending` tracks size and mtime per in-flight path
//!   and only hands it to the indexer once both stop changing for `SETTLE`.
//! - **The root's own top level is watched too, but only one directory deep
//!   (`RecursiveMode::NonRecursive`).** `inbox/` is the one place a user is
//!   meant to put files, but nothing stops one landing in the root instead —
//!   so a settled top-level entry there is not indexed directly, it is swept
//!   into `inbox/` (`handle_root_arrival`, sharing `fs::walk::
//!   sweep_root_into_inbox` with the startup reconcile) and picked up from
//!   there exactly like any other arrival. This is deliberately shallow —
//!   watching the whole tree recursively is exactly what decision 30 ended,
//!   and a shallow watch on one directory costs nothing extra.
//! - **Overflow or error.** `notify`'s Windows backend does not distinguish a
//!   dropped-events buffer overflow from any other backend failure, so both
//!   are handled the same way: `handle_watch_error` sets `rescanning` and
//!   queues a full reconcile (`fs::walk::reconcile`) rather than letting
//!   `inbox/` silently drift from what has actually been queued.
//!
//! **What M1.8 needed and this no longer does.** Self-suppression existed
//! because the app used to rename directories inside the watched tree and
//! had to recognise its own writes to avoid feeding them back to itself.
//! Nothing the app does now touches `inbox/` at all except moving a settled
//! arrival *out* of it — and an unsuppressed `Remove` for a path nothing was
//! ever indexed under is harmless, since arrivals are keyed by uuid the
//! moment they're indexed, never by their transient inbox path. Directory
//! *rename* pairing is gone for the same reason folders lost their
//! directories entirely: there is no tree left to rename inside.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use rusqlite::Connection;

use crate::db;
use crate::error::Result;
use crate::fs::paths::LibraryPaths;
use crate::fs::walk;
use crate::jobs;

/// How long a file's size and mtime must sit still before it is treated as
/// fully written. A large video copied in from Explorer emits events for
/// most of the copy; touching it before this elapses would hash something
/// that will not exist a second later.
const SETTLE: Duration = Duration::from_millis(1000);

/// How often in-flight paths are rechecked. This is the only polling
/// anywhere in the watcher, and it only ever looks at the handful of paths
/// currently arriving in `inbox/` — never the library at large.
const SETTLE_TICK: Duration = Duration::from_millis(200);

/// Shared with `jobs::QueueInner` so the watcher and the job queue's
/// `Progress` report agree on whether a reconcile is running, without the
/// watcher needing the whole queue — and the `AppHandle` building one
/// requires — just to flip a flag.
pub type Rescanning = Arc<AtomicBool>;

fn is_ignored_name(name: &str) -> bool {
    walk::IGNORED_FILES.contains(&name.to_lowercase().as_str()) || name.starts_with('.')
}

// --- lifecycle ---------------------------------------------------------------

/// Owns the live `notify` watcher and the thread processing its events. The
/// watcher stops the moment this is dropped or `stop` is called explicitly.
pub struct Watch {
    _watcher: RecommendedWatcher,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Watch {
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Start watching `paths.inbox_dir()` recursively — a folder of files dropped
/// in should still all land in the Sorting Box — and `paths.root()` itself,
/// one level deep, so a file dropped directly in the root rather than
/// `inbox/` is not silently invisible to the app. The processing thread opens
/// its own database connection — nothing here runs on the Tauri command
/// thread, consistent with every other long-lived worker in the app.
pub fn start(paths: LibraryPaths, db_path: PathBuf, rescanning: Rescanning) -> Result<Watch> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = tx.send(event);
    })?;
    watcher.watch(&paths.inbox_dir(), RecursiveMode::Recursive)?;
    watcher.watch(paths.root(), RecursiveMode::NonRecursive)?;

    let stop = Arc::new(AtomicBool::new(false));
    let stop_loop = Arc::clone(&stop);

    let thread = std::thread::spawn(move || {
        let mut conn = match db::open(&db_path) {
            Ok(conn) => conn,
            Err(err) => {
                eprintln!("watcher could not open the database: {err}");
                return;
            }
        };
        run(&paths, &mut conn, &rescanning, &rx, &stop_loop);
    });

    Ok(Watch {
        _watcher: watcher,
        stop,
        thread: Some(thread),
    })
}

/// What a pending path resolves to once it settles — decided once, at the
/// moment it is first seen, from which of the two watched trees it is under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingKind {
    /// Under `inbox/` — ready to hash once settled.
    InboxArrival,
    /// A direct child of the root itself — swept into `inbox/` once settled,
    /// not hashed directly (see `handle_root_arrival`).
    RootArrival,
}

/// One path the watcher is waiting to stop changing before it touches it.
struct Pending {
    kind: PendingKind,
    last_stat: Option<(u64, i64)>,
    last_changed: Instant,
}

impl Pending {
    fn new(kind: PendingKind) -> Self {
        Pending {
            kind,
            last_stat: None,
            last_changed: Instant::now(),
        }
    }
}

/// Which of the two watched trees `path` belongs to, or `None` for something
/// neither side cares about — OS litter, a hidden entry, or the app's own
/// `.gallery`/`files`/`inbox` showing up in the *root*'s shallow watch (the
/// same names are legitimate content once they are nested under `inbox/`
/// instead, which is why this is not just a name check on its own).
fn classify(paths: &LibraryPaths, path: &Path) -> Option<PendingKind> {
    let name = path.file_name()?.to_str()?;
    if is_ignored_name(name) {
        return None;
    }
    if path.parent() == Some(paths.root()) {
        if crate::fs::paths::is_reserved_top_level(name) {
            return None;
        }
        return Some(PendingKind::RootArrival);
    }
    Some(PendingKind::InboxArrival)
}

fn run(
    paths: &LibraryPaths,
    conn: &mut Connection,
    rescanning: &Rescanning,
    rx: &std::sync::mpsc::Receiver<notify::Result<notify::Event>>,
    stop: &Arc<AtomicBool>,
) {
    let mut pending: HashMap<PathBuf, Pending> = HashMap::new();

    while !stop.load(Ordering::Relaxed) {
        match rx.recv_timeout(SETTLE_TICK) {
            Ok(Ok(event)) => handle_event(paths, &mut pending, event),
            Ok(Err(err)) => {
                // Real events already in flight for paths we were waiting to
                // settle are still worth trusting once the reconcile below
                // catches everything anyway — but there is no way to tell
                // which of them the dropped batch belonged to, so start
                // clean rather than settle-checking paths that may already
                // be gone.
                pending.clear();
                handle_watch_error(conn, rescanning, &err);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        check_settled(paths, conn, &mut pending);
    }
}

/// `notify`'s Windows backend does not surface a dropped-events buffer
/// overflow as anything more specific than a generic backend error — so
/// every error here gets the same response: assume events were missed, and
/// reconcile (`fs::walk::reconcile`) rather than letting `inbox/` silently
/// diverge from what has been queued. Queued (not run inline) so a large
/// reconcile does not block this thread from processing whatever events
/// keep arriving while it runs.
fn handle_watch_error(conn: &Connection, rescanning: &Rescanning, err: &notify::Error) {
    eprintln!("watcher error, queuing a reconcile: {err}");
    rescanning.store(true, Ordering::Relaxed);
    if let Err(err) = jobs::enqueue_index(conn) {
        eprintln!("could not queue the reconcile: {err}");
    }
}

/// Classify one notify event. Removals and the `From` half of a rename just
/// drop the path from `pending` — nothing is indexed by a transient inbox
/// path, so there is nothing to retire. Everything else (create, the `To`
/// half of a rename, a data/metadata change) joins the settle queue; the
/// actual stat happens in `check_settled`.
fn handle_event(paths: &LibraryPaths, pending: &mut HashMap<PathBuf, Pending>, event: notify::Event) {
    for path in &event.paths {
        let Some(kind) = classify(paths, path) else { continue };

        match event.kind {
            EventKind::Remove(_) | EventKind::Modify(notify::event::ModifyKind::Name(notify::event::RenameMode::From)) => {
                pending.remove(path);
            }
            EventKind::Create(_)
            | EventKind::Modify(notify::event::ModifyKind::Name(notify::event::RenameMode::To))
            | EventKind::Modify(notify::event::ModifyKind::Data(_))
            | EventKind::Modify(notify::event::ModifyKind::Any) => {
                pending.entry(path.clone()).or_insert_with(|| Pending::new(kind));
            }
            _ => {}
        }
    }
}

/// Recheck every in-flight path; anything whose size and mtime have not
/// moved for `SETTLE` is handed off — to the indexer for an inbox arrival, or
/// swept into `inbox/` for a root arrival.
fn check_settled(paths: &LibraryPaths, conn: &Connection, pending: &mut HashMap<PathBuf, Pending>) {
    let now = Instant::now();
    let mut ready: Vec<(PathBuf, bool, PendingKind)> = Vec::new();

    for (path, state) in pending.iter_mut() {
        match std::fs::metadata(path) {
            Err(_) => ready.push((path.clone(), false, state.kind)), // vanished before it ever settled
            Ok(meta) => {
                let stat = (meta.len(), walk::mtime_secs(&meta));
                if Some(stat) != state.last_stat {
                    state.last_stat = Some(stat);
                    state.last_changed = now;
                } else if now.duration_since(state.last_changed) >= SETTLE {
                    ready.push((path.clone(), true, state.kind));
                }
            }
        }
    }

    for (path, still_there, kind) in ready {
        pending.remove(&path);
        if !still_there {
            continue;
        }
        let result = match kind {
            PendingKind::InboxArrival => handle_settled(paths, conn, &path),
            PendingKind::RootArrival => handle_root_arrival(paths, &path),
        };
        if let Err(err) = result {
            eprintln!("watcher could not handle {}: {err}", path.display());
        }
    }
}

/// A path that stopped changing: queue it. A directory dropped straight into
/// `inbox/` is walked flat — every file beneath it, regardless of depth,
/// lands in the Sorting Box exactly like a single dropped file (PLAN.md
/// decision 30 — `inbox/` carries no organisational meaning).
fn handle_settled(paths: &LibraryPaths, conn: &Connection, abs: &Path) -> Result<()> {
    let meta = match std::fs::metadata(abs) {
        Ok(meta) => meta,
        Err(_) => return Ok(()), // gone again between the settle check and now
    };

    let inbox = paths.inbox_dir();
    if meta.is_dir() {
        for entry in walkdir::WalkDir::new(abs).follow_links(false).into_iter().flatten() {
            if !entry.file_type().is_file() {
                continue;
            }
            let Some(name) = entry.file_name().to_str() else { continue };
            if is_ignored_name(name) {
                continue;
            }
            queue_inbox_file(conn, &inbox, entry.path())?;
        }
        return Ok(());
    }
    if !meta.is_file() {
        return Ok(());
    }
    queue_inbox_file(conn, &inbox, abs)
}

/// A top-level root entry that stopped changing: sweep it into `inbox/` — one
/// `rename`, carrying everything beneath it if it is a directory — and leave
/// it for the recursive `inbox/` watch to pick up from there, the same as if
/// it had been dropped in directly. Nothing is indexed here directly; this
/// only ever relocates.
fn handle_root_arrival(paths: &LibraryPaths, abs: &Path) -> Result<()> {
    if !abs.exists() {
        return Ok(()); // gone again between the settle check and now
    }
    let Some(name) = abs.file_name() else { return Ok(()) };
    let dest = paths.inbox_dir().join(name);
    if dest.exists() {
        eprintln!("could not sweep {} into inbox: {} already exists there", abs.display(), dest.display());
        return Ok(());
    }
    std::fs::rename(abs, &dest)?;
    Ok(())
}

fn queue_inbox_file(conn: &Connection, inbox: &Path, abs: &Path) -> Result<()> {
    let Ok(rel) = abs.strip_prefix(inbox) else { return Ok(()) };
    let rel = rel.to_string_lossy().replace('\\', "/");
    jobs::enqueue_hash(conn, &rel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::kinds;

    fn scratch(name: &str) -> PathBuf {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-libraries")
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create scratch library");
        root
    }

    fn open_db(root: &Path) -> (LibraryPaths, Connection) {
        let paths = LibraryPaths::new(root);
        paths.ensure_dirs().unwrap();
        let mut conn = db::open(&paths.db_path()).unwrap();
        db::migrate(&mut conn).unwrap();
        (paths, conn)
    }

    // --- settling ---------------------------------------------------

    #[test]
    fn a_file_still_changing_is_not_queued() {
        let root = scratch("watch-settle-pending");
        let (paths, conn) = open_db(&root);
        let file = paths.inbox_dir().join("video.mp4");
        std::fs::write(&file, b"partial bytes").unwrap();

        let mut pending = HashMap::new();
        pending.insert(
            file.clone(),
            Pending { kind: PendingKind::InboxArrival, last_stat: None, last_changed: Instant::now() },
        );

        check_settled(&paths, &conn, &mut pending);

        assert!(pending.contains_key(&file), "still waiting");
        assert_eq!(db::jobs::counts(&conn).unwrap().pending, 0);
    }

    #[test]
    fn a_file_unchanged_past_settle_gets_queued() {
        let root = scratch("watch-settle-ready");
        let (paths, conn) = open_db(&root);
        let file = paths.inbox_dir().join("photo.jpg");
        std::fs::write(&file, b"final bytes").unwrap();
        let size = std::fs::metadata(&file).unwrap().len();
        let mtime = walk::mtime_secs(&std::fs::metadata(&file).unwrap());

        let mut pending = HashMap::new();
        pending.insert(
            file.clone(),
            Pending {
                kind: PendingKind::InboxArrival,
                last_stat: Some((size, mtime)),
                last_changed: Instant::now() - SETTLE - Duration::from_millis(200),
            },
        );

        check_settled(&paths, &conn, &mut pending);

        assert!(!pending.contains_key(&file), "settled — removed from the queue");
        assert_eq!(db::jobs::counts(&conn).unwrap().pending, 1);
    }

    #[test]
    fn a_still_growing_file_keeps_resetting_its_timer() {
        let root = scratch("watch-settle-growing");
        let (paths, conn) = open_db(&root);
        let file = paths.inbox_dir().join("still-copying.mp4");
        std::fs::write(&file, b"1234567890").unwrap();

        let mut pending = HashMap::new();
        pending.insert(
            file.clone(),
            Pending {
                kind: PendingKind::InboxArrival,
                last_stat: Some((3, 0)),
                last_changed: Instant::now() - SETTLE - Duration::from_secs(10),
            },
        );

        check_settled(&paths, &conn, &mut pending);

        assert!(pending.contains_key(&file), "the stat moved this tick");
        assert_eq!(db::jobs::counts(&conn).unwrap().pending, 0);
    }

    #[test]
    fn a_folder_dropped_into_inbox_queues_every_file_beneath_it_flat() {
        let root = scratch("watch-settle-dir-drop");
        let (paths, conn) = open_db(&root);
        std::fs::create_dir_all(paths.inbox_dir().join("Photos/2024")).unwrap();
        std::fs::write(paths.inbox_dir().join("Photos/a.jpg"), b"a").unwrap();
        std::fs::write(paths.inbox_dir().join("Photos/2024/b.jpg"), b"b").unwrap();

        let dir = paths.inbox_dir().join("Photos");
        let dir_meta = std::fs::metadata(&dir).unwrap();
        let mut pending = HashMap::new();
        pending.insert(
            dir.clone(),
            Pending {
                kind: PendingKind::InboxArrival,
                last_stat: Some((dir_meta.len(), walk::mtime_secs(&dir_meta))),
                last_changed: Instant::now() - SETTLE - Duration::from_millis(200),
            },
        );

        check_settled(&paths, &conn, &mut pending);

        assert_eq!(db::jobs::counts(&conn).unwrap().pending, 2, "both nested files queued, regardless of depth");
    }

    // --- the root's own top level --------------------------------------

    #[test]
    fn classify_treats_an_ordinary_root_entry_as_a_root_arrival() {
        let root = scratch("watch-classify-root");
        let (paths, _conn) = open_db(&root);

        assert_eq!(
            classify(&paths, &paths.root().join("cover.png")),
            Some(PendingKind::RootArrival),
        );
    }

    #[test]
    fn classify_ignores_the_apps_own_reserved_root_names() {
        let root = scratch("watch-classify-reserved");
        let (paths, _conn) = open_db(&root);

        assert_eq!(classify(&paths, &paths.gallery_dir()), None);
        assert_eq!(classify(&paths, &paths.files_dir()), None);
        assert_eq!(classify(&paths, &paths.inbox_dir()), None);
    }

    #[test]
    fn classify_treats_a_nested_inbox_path_as_an_inbox_arrival() {
        let root = scratch("watch-classify-inbox");
        let (paths, _conn) = open_db(&root);

        assert_eq!(
            classify(&paths, &paths.inbox_dir().join("photo.jpg")),
            Some(PendingKind::InboxArrival),
        );
    }

    #[test]
    fn a_settled_root_file_is_swept_into_inbox_not_indexed_directly() {
        let root = scratch("watch-root-sweep-file");
        let (paths, conn) = open_db(&root);
        let file = paths.root().join("cover.png");
        std::fs::write(&file, b"cover bytes").unwrap();
        let meta = std::fs::metadata(&file).unwrap();

        let mut pending = HashMap::new();
        pending.insert(
            file.clone(),
            Pending {
                kind: PendingKind::RootArrival,
                last_stat: Some((meta.len(), walk::mtime_secs(&meta))),
                last_changed: Instant::now() - SETTLE - Duration::from_millis(200),
            },
        );

        check_settled(&paths, &conn, &mut pending);

        assert!(!file.exists(), "moved out of the root");
        assert!(paths.inbox_dir().join("cover.png").is_file());
        assert_eq!(
            db::jobs::counts(&conn).unwrap().pending,
            0,
            "swept, not hashed directly — the inbox watch queues it from its new spot",
        );
    }

    #[test]
    fn a_settled_root_directory_is_swept_into_inbox_whole() {
        let root = scratch("watch-root-sweep-dir");
        let (paths, conn) = open_db(&root);
        std::fs::create_dir_all(paths.root().join("People/Ana")).unwrap();
        std::fs::write(paths.root().join("People/Ana/holiday.jpg"), b"bytes").unwrap();

        let dir = paths.root().join("People");
        let dir_meta = std::fs::metadata(&dir).unwrap();
        let mut pending = HashMap::new();
        pending.insert(
            dir.clone(),
            Pending {
                kind: PendingKind::RootArrival,
                last_stat: Some((dir_meta.len(), walk::mtime_secs(&dir_meta))),
                last_changed: Instant::now() - SETTLE - Duration::from_millis(200),
            },
        );

        check_settled(&paths, &conn, &mut pending);

        assert!(!dir.exists());
        assert!(paths.inbox_dir().join("People/Ana/holiday.jpg").is_file());
    }

    // --- overflow / error ---------------------------------------------

    #[test]
    fn a_watcher_error_sets_rescanning_and_queues_a_reconcile() {
        let root = scratch("watch-error-reconcile");
        let (_, conn) = open_db(&root);
        let rescanning: Rescanning = Arc::new(AtomicBool::new(false));

        handle_watch_error(&conn, &rescanning, &notify::Error::generic("simulated overflow"));

        assert!(rescanning.load(Ordering::Relaxed), "the readout must say a rescan is running");
        assert_eq!(db::jobs::counts(&conn).unwrap().pending, 1, "a reconcile is queued");
    }

    #[test]
    fn a_reconcile_catches_up_on_whatever_the_watcher_missed() {
        let root = scratch("watch-error-catches-up");
        let (paths, mut conn) = open_db(&root);
        std::fs::write(paths.inbox_dir().join("missed.png"), b"bytes").unwrap();

        let rescanning: Rescanning = Arc::new(AtomicBool::new(false));
        handle_watch_error(&conn, &rescanning, &notify::Error::generic("simulated"));

        let job = db::jobs::claim(&mut conn).unwrap().expect("index job queued");
        assert_eq!(job.kind, kinds::INDEX);
        walk::reconcile(&paths, &conn, &mut |_, _| {}).unwrap();
        db::jobs::complete(&conn, job.id).unwrap();

        assert_eq!(db::jobs::counts(&conn).unwrap().pending, 1, "the missed file's hash job is queued");
    }

    #[test]
    fn removing_a_pending_arrival_drops_it_without_touching_the_database() {
        let root = scratch("watch-removed-before-settle");
        let (paths, conn) = open_db(&root);
        let file = paths.inbox_dir().join("gone.png");
        std::fs::write(&file, b"bytes").unwrap();

        let mut pending = HashMap::new();
        handle_event(
            &paths,
            &mut pending,
            notify::Event::new(EventKind::Create(notify::event::CreateKind::File)).add_path(file.clone()),
        );
        assert!(pending.contains_key(&file));

        std::fs::remove_file(&file).unwrap();
        handle_event(
            &paths,
            &mut pending,
            notify::Event::new(EventKind::Remove(notify::event::RemoveKind::File)).add_path(file.clone()),
        );

        assert!(!pending.contains_key(&file));
        assert_eq!(db::items::count(&conn).unwrap(), 0);
    }
}
