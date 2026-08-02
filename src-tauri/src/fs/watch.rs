//! The filesystem watcher — M1.8, replacing the "Re-index" button.
//! `notify::recommended_watcher` on Windows is `ReadDirectoryChangesW`: one
//! recursive handle on the library root, so this is push notifications, not
//! polling.
//!
//! Four things need care here, each covered by a test below:
//!
//! - **Settling.** A file just copied in from Explorer emits events long
//!   before it is whole. `Pending` tracks size and mtime per in-flight path
//!   and only hands it to the indexer once both stop changing for `SETTLE` —
//!   see `check_settled`.
//! - **Self-suppression.** `.gallery/` is never watched content (`should_ignore`),
//!   and paths the app is itself mid-write on are registered with
//!   `Suppressor` — `fs::import::rename_on_arrival` and
//!   `fs::relocate::retitle_folder` — so the watcher never feeds its own
//!   writes back to itself as arrivals or deletions.
//! - **Overflow or error.** `notify`'s Windows backend does not distinguish a
//!   dropped-events buffer overflow from any other backend failure — both
//!   surface identically as an `Err` on the event channel — so both are
//!   handled the same way: `handle_watch_error` sets `rescanning` and queues
//!   a full reconcile walk rather than letting the database silently drift
//!   from disk. `Progress.rescanning` (see `jobs`) is what lets the readout
//!   say so instead of looking like an ordinary index.
//! - **Identity.** A changed file is routed through the exact same
//!   `walk::queue_file` → `enqueue_hash` → `worker::run_hash` →
//!   `db::items::upsert` path the walker itself uses, which matches on
//!   folder + disk name — so a modification updates the existing row in
//!   place rather than inserting a second one.
//!
//! A fifth, added in M2.2: **directory renames.** Windows always reports a
//! rename as a `RenameMode::From` event immediately followed by a
//! `RenameMode::To` for the same operation (documented `ReadDirectoryChangesW`
//! behaviour), so `handle_event` pairs them via `pending_rename_from` rather
//! than treating each half as an independent remove/create — otherwise a
//! folder renamed in Explorer would have every item beneath it retired and
//! then re-indexed as unrelated, losing tags and favorites. `handle_dir_renamed`
//! updates the existing folder row's title and path in place instead. See
//! docs/DESIGN.md §1 "Folder names".

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use notify::event::{ModifyKind, RenameMode};
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
/// currently arriving or changing — never the library at large.
const SETTLE_TICK: Duration = Duration::from_millis(200);

/// How long a suppressed path stays suppressed. Generous next to how long a
/// single rename actually takes, so a slightly delayed event still lands
/// inside the window; short enough that the map never holds much.
const SUPPRESS_TTL: Duration = Duration::from_secs(5);

// --- self-suppression -------------------------------------------------------

/// Paths the app is currently writing to itself. Checked before the watcher
/// acts on any event, so its own writes are never reported back to it as
/// arrivals or deletions.
///
/// Keyed by the same normalised, lower-cased relative path the database
/// uses (`LibraryPaths::to_rel`) rather than the raw `PathBuf` a caller
/// happens to construct: the app's own internal paths are built through
/// `LibraryPaths::to_abs`, which normalises the folder portion to lower
/// case, while Windows reports watch events with whatever case the on-disk
/// entries actually carry. A real library almost always has mixed-case
/// folder names, so comparing raw paths would miss the very events this
/// exists to catch.
#[derive(Clone, Default)]
pub struct Suppressor(Arc<Mutex<HashMap<String, Instant>>>);

impl Suppressor {
    /// Register `abs` as app-written, starting now.
    pub fn suppress(&self, paths: &LibraryPaths, abs: &Path) {
        let Ok(rel) = paths.to_rel(abs) else { return };
        if let Ok(mut map) = self.0.lock() {
            map.insert(rel, Instant::now());
        }
    }

    /// True if `abs` was suppressed within the last `SUPPRESS_TTL`. Expired
    /// entries are pruned as a side effect, so the map never grows unbounded
    /// across a long-running watch.
    pub fn is_suppressed(&self, paths: &LibraryPaths, abs: &Path) -> bool {
        let Ok(rel) = paths.to_rel(abs) else {
            return false;
        };
        let Ok(mut map) = self.0.lock() else {
            return false;
        };
        map.retain(|_, at| at.elapsed() < SUPPRESS_TTL);
        map.contains_key(&rel)
    }
}

/// Shared with `jobs::QueueInner` so the watcher and the job queue's
/// `Progress` report agree on whether a reconcile is running, without the
/// watcher needing the whole queue — and the `AppHandle` building one
/// requires — just to flip a flag.
pub type Rescanning = Arc<AtomicBool>;

/// True if any path component from the library root down to `abs` is
/// `.gallery` or starts with `.` — the same OS-litter and app-storage
/// exclusion `walk::is_skipped_dir` applies during a full walk, applied here
/// to a single arbitrary path instead of a filtered directory entry.
fn is_hidden(paths: &LibraryPaths, abs: &Path) -> bool {
    if paths.is_gallery_dir(abs) {
        return true;
    }
    match paths.to_rel(abs) {
        Ok(rel) => rel.split('/').any(|seg| seg.starts_with('.')),
        Err(_) => true, // outside the root entirely — never this watcher's business
    }
}

fn should_ignore(paths: &LibraryPaths, suppressor: &Suppressor, abs: &Path) -> bool {
    is_hidden(paths, abs) || suppressor.is_suppressed(paths, abs)
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

/// Start watching `paths.root()` recursively. The processing thread opens its
/// own database connection — nothing here runs on the Tauri command thread,
/// consistent with every other long-lived worker in the app.
pub fn start(
    paths: LibraryPaths,
    db_path: PathBuf,
    suppressor: Suppressor,
    rescanning: Rescanning,
) -> Result<Watch> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = tx.send(event);
    })?;
    watcher.watch(paths.root(), RecursiveMode::Recursive)?;

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
        run(&paths, &mut conn, &suppressor, &rescanning, &rx, &stop_loop);
    });

    Ok(Watch {
        _watcher: watcher,
        stop,
        thread: Some(thread),
    })
}

/// One path the watcher is waiting to stop changing before it touches it.
struct Pending {
    last_stat: Option<(u64, i64)>,
    last_changed: Instant,
}

impl Pending {
    fn new() -> Self {
        Pending {
            last_stat: None,
            last_changed: Instant::now(),
        }
    }
}

fn run(
    paths: &LibraryPaths,
    conn: &mut Connection,
    suppressor: &Suppressor,
    rescanning: &Rescanning,
    rx: &std::sync::mpsc::Receiver<notify::Result<notify::Event>>,
    stop: &Arc<AtomicBool>,
) {
    let mut pending: HashMap<PathBuf, Pending> = HashMap::new();
    // A `RenameMode::From` stashed here, with when it arrived, while its
    // matching `RenameMode::To` is awaited — see `handle_event`'s doc
    // comment. Flushed early by a subsequent `From` (a good sign the first
    // was never getting a pair), and otherwise by `flush_stale_rename_from`
    // once it has waited past `SETTLE` — the directory it named may have
    // left the watched tree entirely, which fires no `To` at all.
    let mut pending_rename_from: Option<(PathBuf, Instant)> = None;

    while !stop.load(Ordering::Relaxed) {
        match rx.recv_timeout(SETTLE_TICK) {
            Ok(Ok(event)) => handle_event(
                paths,
                conn,
                suppressor,
                &mut pending,
                &mut pending_rename_from,
                event,
            ),
            Ok(Err(err)) => {
                // Real events already in flight for paths we were waiting to
                // settle are still worth trusting once the reconcile below
                // catches everything anyway — but there is no way to tell
                // which of them the dropped batch belonged to, so start
                // clean rather than settle-checking paths that may already
                // be gone.
                pending.clear();
                pending_rename_from = None;
                handle_watch_error(conn, rescanning, &err);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        check_settled(paths, conn, &mut pending);
        flush_stale_rename_from(paths, conn, &mut pending_rename_from);
    }
}

/// A stashed `From` that has waited past `SETTLE` with no matching `To` is
/// not going to get one — most likely the directory (or file) moved outside
/// the watched tree. Retire it, the same as an ordinary removal.
fn flush_stale_rename_from(
    paths: &LibraryPaths,
    conn: &Connection,
    pending_rename_from: &mut Option<(PathBuf, Instant)>,
) {
    let stale = matches!(pending_rename_from, Some((_, since)) if since.elapsed() >= SETTLE);
    if !stale {
        return;
    }
    let (old, _) = pending_rename_from.take().expect("checked Some above");
    if let Err(err) = handle_removed(paths, conn, &old) {
        eprintln!("watcher could not retire {}: {err}", old.display());
    }
}

/// `notify`'s Windows backend does not surface a dropped-events buffer
/// overflow as anything more specific than a generic backend error — see the
/// module docs — so every error here gets the same response: assume events
/// were missed, and reconcile by walking the whole library again rather than
/// letting the database silently diverge from disk. Queued (not run inline)
/// so a large reconcile does not block this thread from processing whatever
/// events keep arriving while it runs.
fn handle_watch_error(conn: &Connection, rescanning: &Rescanning, err: &notify::Error) {
    eprintln!("watcher error, queuing a full reconcile: {err}");
    rescanning.store(true, Ordering::Relaxed);
    if let Err(err) = jobs::enqueue_index(conn) {
        eprintln!("could not queue the reconcile walk: {err}");
    }
}

/// Classify one notify event. Removals act immediately — there is nothing to
/// wait for. Arrivals and changes only ever join the settle queue here; the
/// actual stat happens in `check_settled`, so a create followed by a burst of
/// modify events for the same path costs one hashmap insert, not one stat per
/// event.
///
/// `RenameMode::From`/`RenameMode::To` need more care than a plain
/// remove-then-create: Windows' `ReadDirectoryChangesW` always reports a
/// rename as `FILE_ACTION_RENAMED_OLD_NAME` immediately followed by
/// `FILE_ACTION_RENAMED_NEW_NAME` for that same operation (that ordering is
/// documented Win32 behaviour, not a heuristic) — `notify`'s Windows backend
/// carries neither a tracker nor a cookie to correlate them explicitly, so
/// `pending_rename_from` stashes the old path and the very next `To` is
/// assumed to be its pair. If a directory renamed in Explorer went through
/// the ordinary remove-then-create path instead, `handle_removed` would
/// retire every item beneath it and the settle path would re-index the new
/// location as an unrelated arrival — losing tags, favorites and manual
/// tags on every item in the subtree. `handle_dir_renamed` avoids that by
/// updating the existing folder row in place. See docs/DESIGN.md §1
/// "Folder names".
fn handle_event(
    paths: &LibraryPaths,
    conn: &Connection,
    suppressor: &Suppressor,
    pending: &mut HashMap<PathBuf, Pending>,
    pending_rename_from: &mut Option<(PathBuf, Instant)>,
    event: notify::Event,
) {
    for path in &event.paths {
        if should_ignore(paths, suppressor, path) {
            continue;
        }
        match event.kind {
            EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
                // An unmatched From (e.g. the previous rename's target lay
                // outside the watched tree, so no To ever arrived) is a real
                // removal once a new From shows it is not getting one —
                // `flush_stale_rename_from` catches the case where no
                // further event ever arrives at all.
                if let Some((old, _)) = pending_rename_from.take() {
                    if let Err(err) = handle_removed(paths, conn, &old) {
                        eprintln!("watcher could not retire {}: {err}", old.display());
                    }
                }
                pending.remove(path);
                *pending_rename_from = Some((path.clone(), Instant::now()));
            }
            EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
                let Some((old, _)) = pending_rename_from.take() else {
                    // No matching From — arrived from outside the watched
                    // tree (a cross-volume move behaves like a plain copy).
                    pending.entry(path.clone()).or_insert_with(Pending::new);
                    continue;
                };
                // The old path no longer exists to `stat`; the new one does,
                // and that is enough to tell a renamed directory from a
                // renamed file.
                let is_dir = std::fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false);
                if is_dir {
                    if let Err(err) = handle_dir_renamed(paths, conn, &old, path) {
                        eprintln!(
                            "watcher could not process the rename {} -> {}: {err}",
                            old.display(),
                            path.display()
                        );
                    }
                } else {
                    // File renames are not this milestone's concern —
                    // fall back to the pre-existing behaviour.
                    if let Err(err) = handle_removed(paths, conn, &old) {
                        eprintln!("watcher could not retire {}: {err}", old.display());
                    }
                    pending.entry(path.clone()).or_insert_with(Pending::new);
                }
            }
            EventKind::Remove(_) => {
                pending.remove(path);
                if let Err(err) = handle_removed(paths, conn, path) {
                    eprintln!("watcher could not retire {}: {err}", path.display());
                }
            }
            EventKind::Create(_)
            | EventKind::Modify(ModifyKind::Data(_))
            | EventKind::Modify(ModifyKind::Any) => {
                pending.entry(path.clone()).or_insert_with(Pending::new);
            }
            _ => {}
        }
    }
}

/// A directory renamed **outside the app** — Explorer, a script, anything
/// that did not go through `fs::relocate::retitle_folder` (which suppresses
/// its own rename, so the watcher never sees it as external in the first
/// place). Updates the title to match the new name, unless the current
/// title already sanitises to it — in which case only the derived name
/// changed and the title is left alone. See docs/DESIGN.md §1
/// "Folder names".
fn handle_dir_renamed(paths: &LibraryPaths, conn: &Connection, old_abs: &Path, new_abs: &Path) -> Result<()> {
    let Ok(old_rel) = paths.to_rel(old_abs) else {
        return Ok(());
    };
    let Ok(new_rel) = paths.to_rel(new_abs) else {
        return Ok(());
    };
    if old_rel.is_empty() || new_rel.is_empty() {
        return Ok(()); // the library root itself — not this milestone's problem
    }

    let Some(folder_id) = db::folders::id_for_rel(conn, &old_rel)? else {
        // Never indexed under its old name (e.g. created and renamed before
        // ever being walked) — nothing to update in place. Index it fresh
        // at the new location, the same way a brand-new directory settles.
        ensure_folder_chain(paths, conn, new_abs)?;
        walk::index_subtree(paths, conn, new_abs)?;
        return Ok(());
    };

    let current_title: String =
        conn.query_row("SELECT title FROM folder WHERE id = ?1", [folder_id], |r| r.get(0))?;
    let new_name = new_abs
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();

    // One rename in Explorer is one undoable step, so the title change and
    // the path change share a batch.
    let batch = db::journal::new_batch();
    if crate::fs::relocate::sanitise_folder_name(&current_title) != new_name {
        db::folders::set_title(conn, folder_id, &new_name, &batch)?;
    }

    db::folders::set_rel_path(conn, folder_id, &new_rel)?;
    jobs::enqueue_rename_folder_subtree(conn, &old_rel, &new_rel)?;
    db::journal::record_folder_rename_dir(conn, &batch, folder_id, &old_rel, &new_rel)?;
    Ok(())
}

/// Soft-delete whatever `abs` was — an item if it names a file that was
/// indexed, everything under it if it names a folder. Both are tried: a
/// removed path can't be both, but which one it was is no longer answerable
/// once it is gone, and each lookup is a cheap, specific key match rather
/// than a scan.
fn handle_removed(paths: &LibraryPaths, conn: &Connection, abs: &Path) -> Result<()> {
    let Ok(rel) = paths.to_rel(abs) else {
        return Ok(());
    };
    if rel.is_empty() {
        return Ok(()); // the library root itself disappearing is not this milestone's problem
    }

    let (folder_rel, name) = match rel.rsplit_once('/') {
        Some((folder, name)) => (folder.to_string(), name.to_string()),
        None => (String::new(), rel.clone()),
    };
    if let Some(folder_id) = db::folders::id_for_rel(conn, &folder_rel)? {
        db::items::retire_one(conn, folder_id, &name)?;
    }
    db::items::retire_folder(conn, &rel)?;
    Ok(())
}

/// Recheck every in-flight path; anything whose size and mtime have not
/// moved for `SETTLE` is handed to the indexer. The only polling in the
/// watcher, and it only ever looks at paths already known to be arriving or
/// changing.
fn check_settled(paths: &LibraryPaths, conn: &Connection, pending: &mut HashMap<PathBuf, Pending>) {
    let now = Instant::now();
    let mut ready: Vec<(PathBuf, bool)> = Vec::new();

    for (path, state) in pending.iter_mut() {
        match std::fs::metadata(path) {
            Err(_) => ready.push((path.clone(), false)), // vanished before it ever settled
            Ok(meta) => {
                let stat = (meta.len(), walk::mtime_secs(&meta));
                if Some(stat) != state.last_stat {
                    state.last_stat = Some(stat);
                    state.last_changed = now;
                } else if now.duration_since(state.last_changed) >= SETTLE {
                    ready.push((path.clone(), true));
                }
            }
        }
    }

    for (path, still_there) in ready {
        pending.remove(&path);
        if !still_there {
            continue;
        }
        if let Err(err) = handle_settled(paths, conn, &path) {
            eprintln!("watcher could not index {}: {err}", path.display());
        }
    }
}

/// A path that stopped changing: index it. A directory is walked as a
/// subtree (see `walk::index_subtree`'s docs for why); a file is queued
/// exactly the way the full walk queues one, so a modification is
/// indistinguishable from indexing it the first time except that the row
/// already exists.
fn handle_settled(paths: &LibraryPaths, conn: &Connection, abs: &Path) -> Result<()> {
    let meta = match std::fs::metadata(abs) {
        Ok(meta) => meta,
        Err(_) => return Ok(()), // gone again between the settle check and now
    };

    if meta.is_dir() {
        ensure_folder_chain(paths, conn, abs)?;
        walk::index_subtree(paths, conn, abs)?;
        return Ok(());
    }
    if !meta.is_file() {
        return Ok(());
    }

    let Some(name) = abs.file_name().and_then(|n| n.to_str()) else {
        return Ok(());
    };
    if walk::IGNORED_FILES.contains(&name.to_lowercase().as_str()) {
        return Ok(());
    }
    let Some(parent) = abs.parent() else {
        return Ok(());
    };

    let folder_id = ensure_folder_chain(paths, conn, parent)?;
    walk::queue_file(paths, conn, folder_id, name, &meta)?;
    Ok(())
}

/// Ensure every folder from the library root down to `dir` (inclusive)
/// exists, creating any that are missing, and return `dir`'s own folder id.
///
/// A folder pasted or moved in can be several levels deep with none of them
/// known yet (`NewSet/2024/Trip`), unlike the full walk, which always visits
/// a directory's parent before the directory itself and so never needs to
/// look one up. Titles are taken from `dir`'s own path components, which
/// carry whatever case is actually on disk — not from the lower-cased
/// `rel_path` a database round-trip would give back.
fn ensure_folder_chain(paths: &LibraryPaths, conn: &Connection, dir: &Path) -> Result<i64> {
    let mut id = db::folders::upsert(conn, "", &walk::root_title(paths))?;

    let Ok(rel) = dir.strip_prefix(paths.root()) else {
        return Ok(id);
    };

    let mut built = String::new();
    for component in rel.components() {
        let Component::Normal(part) = component else {
            continue;
        };
        let part = part.to_string_lossy();
        built = if built.is_empty() {
            part.to_string()
        } else {
            format!("{built}/{part}")
        };
        id = db::folders::upsert(conn, &crate::fs::paths::normalise_rel(&built), &part)?;
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::items::Scope;
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

    /// A real, decodable PNG — the thumbnail job actually opens whatever is
    /// on disk, so a placeholder byte string fails at `run_thumb` rather than
    /// at the point a test cares about.
    fn write_png(path: &Path, width: u32, height: u32) {
        let mut image = image::RgbImage::new(width, height);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = image::Rgb([(x % 256) as u8, (y % 256) as u8, 128]);
        }
        image.save(path).expect("write test png");
    }

    /// Drain the job queue the way a worker pool would, using the real
    /// `run_hash`/`run_thumb` so identity and rename behaviour match
    /// production exactly.
    fn drain(paths: &LibraryPaths, conn: &mut Connection, suppressor: &Suppressor) {
        let tools = crate::sidecar::Tools::default();
        while let Some(job) = db::jobs::claim(conn).unwrap() {
            let outcome = match job.kind.as_str() {
                kinds::HASH => crate::jobs::worker::run_hash(
                    paths,
                    &tools,
                    &HashMap::new(),
                    suppressor,
                    conn,
                    serde_json::from_str(&job.payload).unwrap(),
                ),
                kinds::THUMB => crate::jobs::worker::run_thumb(
                    paths,
                    &tools,
                    conn,
                    serde_json::from_str(&job.payload).unwrap(),
                ),
                other => panic!("unexpected job {other}"),
            };
            outcome.unwrap();
            db::jobs::complete(conn, job.id).unwrap();
        }
    }

    // --- 1. settling ---------------------------------------------------

    #[test]
    fn a_file_still_changing_is_not_indexed() {
        let root = scratch("watch-settle-pending");
        let (paths, conn) = open_db(&root);
        let file = root.join("video.mp4");
        std::fs::write(&file, b"partial bytes").unwrap();

        let mut pending = HashMap::new();
        pending.insert(
            file.clone(),
            Pending {
                last_stat: None,
                // Just observed — nowhere near SETTLE yet.
                last_changed: Instant::now(),
            },
        );

        check_settled(&paths, &conn, &mut pending);

        assert!(
            pending.contains_key(&file),
            "still waiting — not enough time has passed since the last change"
        );
        assert_eq!(
            db::jobs::counts(&conn).unwrap().pending,
            0,
            "nothing queued for a file still being written"
        );
    }

    #[test]
    fn a_file_unchanged_past_settle_gets_queued() {
        let root = scratch("watch-settle-ready");
        let (paths, conn) = open_db(&root);
        let file = root.join("photo.jpg");
        std::fs::write(&file, b"final bytes").unwrap();
        let size = std::fs::metadata(&file).unwrap().len();
        let mtime = walk::mtime_secs(&std::fs::metadata(&file).unwrap());

        let mut pending = HashMap::new();
        pending.insert(
            file.clone(),
            Pending {
                last_stat: Some((size, mtime)),
                // Already sitting well past SETTLE with no change recorded.
                last_changed: Instant::now() - SETTLE - Duration::from_millis(200),
            },
        );

        check_settled(&paths, &conn, &mut pending);

        assert!(
            !pending.contains_key(&file),
            "settled — removed from the queue"
        );
        assert_eq!(
            db::jobs::counts(&conn).unwrap().pending,
            1,
            "a hash job is queued for the now-settled file"
        );
    }

    #[test]
    fn a_still_growing_file_keeps_resetting_its_timer() {
        let root = scratch("watch-settle-growing");
        let (paths, conn) = open_db(&root);
        let file = root.join("still-copying.mp4");
        std::fs::write(&file, b"1234567890").unwrap();

        let mut pending = HashMap::new();
        pending.insert(
            file.clone(),
            Pending {
                // Stale stat: the file has since grown, so this tick must
                // notice the mismatch and restart the clock rather than
                // trusting how long ago `last_changed` claims to be.
                last_stat: Some((3, 0)),
                last_changed: Instant::now() - SETTLE - Duration::from_secs(10),
            },
        );

        check_settled(&paths, &conn, &mut pending);

        assert!(
            pending.contains_key(&file),
            "the stat moved this tick, so it is not settled no matter how old last_changed was"
        );
        assert_eq!(db::jobs::counts(&conn).unwrap().pending, 0);
    }

    #[test]
    fn a_nested_folder_that_appears_gets_its_whole_chain_and_contents() {
        // Simulates a folder moved in from elsewhere on the same volume in one
        // atomic rename — `ReadDirectoryChangesW` reports the top directory
        // but not necessarily the files already inside it, so settling a
        // brand-new, several-levels-deep directory must walk its contents,
        // not just create one folder row.
        let root = scratch("watch-settle-new-folder");
        let (paths, conn) = open_db(&root);
        std::fs::create_dir_all(root.join("Imported/2024/Trip")).unwrap();
        write_png(&root.join("Imported/2024/Trip/photo.png"), 12, 12);

        let dir = root.join("Imported");
        let dir_meta = std::fs::metadata(&dir).unwrap();
        let mut pending = HashMap::new();
        pending.insert(
            dir.clone(),
            Pending {
                // Already stat'd once at its current value — otherwise the
                // very first check would (correctly) treat the missing prior
                // stat as "just changed" and reset the clock regardless of
                // how far in the past `last_changed` claims to be.
                last_stat: Some((dir_meta.len(), walk::mtime_secs(&dir_meta))),
                last_changed: Instant::now() - SETTLE - Duration::from_millis(200),
            },
        );

        check_settled(&paths, &conn, &mut pending);

        assert!(pending.is_empty());
        assert!(
            db::folders::id_for_rel(&conn, "imported/2024/trip")
                .unwrap()
                .is_some(),
            "the whole ancestor chain exists, not just the top folder"
        );
        assert_eq!(
            db::jobs::counts(&conn).unwrap().pending,
            1,
            "the file already inside the new folder was picked up by the same settle"
        );
    }

    // --- 2. self-suppression -------------------------------------------

    #[test]
    fn gallery_dir_is_always_ignored() {
        let root = scratch("watch-ignore-gallery");
        let paths = LibraryPaths::new(&root);
        let suppressor = Suppressor::default();

        assert!(should_ignore(&paths, &suppressor, &paths.db_path(),));
    }

    #[test]
    fn a_suppressed_path_is_ignored_until_it_expires() {
        let root = scratch("watch-suppress");
        let paths = LibraryPaths::new(&root);
        let suppressor = Suppressor::default();
        let file = root.join("photo.jpg");

        assert!(
            !should_ignore(&paths, &suppressor, &file),
            "not suppressed yet"
        );

        suppressor.suppress(&paths, &file);
        assert!(should_ignore(&paths, &suppressor, &file));

        // A different case for the same on-disk path must still match — the
        // whole reason suppression is keyed by normalised rel path rather
        // than the raw PathBuf notify happens to report.
        let differently_cased = root.join("PHOTO.JPG");
        assert!(should_ignore(&paths, &suppressor, &differently_cased));
    }

    #[test]
    fn rename_on_arrival_suppresses_both_its_paths() {
        let root = scratch("watch-suppress-rename-on-arrival");
        std::fs::write(root.join("newphoto.jpg"), b"fresh bytes").unwrap();
        let (paths, conn) = open_db(&root);
        let root_id = db::folders::upsert(&conn, "", "Library").unwrap();
        let uuid = uuid::Uuid::new_v4().to_string();
        let item_id = db::items::upsert(
            &conn,
            &crate::db::items::NewItem {
                uuid: uuid.clone(),
                folder_id: root_id,
                disk_name: "newphoto.jpg".to_string(),
                ext: "jpg".to_string(),
                orig_name: "newphoto.jpg".to_string(),
                hash: "deadbeef".to_string(),
                size_bytes: 11,
                mtime: 0,
                kind: "image".to_string(),
                width: None,
                height: None,
                duration_ms: None,
                codec: None,
                bitrate: None,
                captured_at: None,
                captured_src: None,
            },
        )
        .unwrap();

        let suppressor = Suppressor::default();
        crate::fs::import::rename_on_arrival(&paths, &conn, item_id, &suppressor).unwrap();

        assert!(
            should_ignore(&paths, &suppressor, &root.join("newphoto.jpg")),
            "the old path the rename deleted must be suppressed"
        );
        assert!(
            should_ignore(&paths, &suppressor, &root.join(format!("{uuid}.jpg"))),
            "the new path the rename created must be suppressed"
        );
    }

    // --- 3. overflow / error ---------------------------------------------

    #[test]
    fn a_watcher_error_sets_rescanning_and_queues_a_reconcile() {
        let root = scratch("watch-error-reconcile");
        let (_, conn) = open_db(&root);
        let rescanning: Rescanning = Arc::new(AtomicBool::new(false));

        handle_watch_error(
            &conn,
            &rescanning,
            &notify::Error::generic("simulated overflow"),
        );

        assert!(
            rescanning.load(Ordering::Relaxed),
            "the readout must say a rescan is running"
        );
        assert_eq!(
            db::jobs::counts(&conn).unwrap().pending,
            1,
            "a full reconcile walk is queued"
        );
    }

    #[test]
    fn a_reconcile_walk_catches_up_on_whatever_the_watcher_missed() {
        // Files present on disk but never reported to the watcher (the actual
        // consequence of a dropped overflow) must still surface once the
        // reconcile this queues actually runs.
        let root = scratch("watch-error-catches-up");
        let (paths, mut conn) = open_db(&root);
        write_png(&root.join("missed.png"), 8, 8);

        let rescanning: Rescanning = Arc::new(AtomicBool::new(false));
        handle_watch_error(&conn, &rescanning, &notify::Error::generic("simulated"));

        // Run the queued INDEX job the way a worker would.
        let job = db::jobs::claim(&mut conn)
            .unwrap()
            .expect("index job queued");
        assert_eq!(job.kind, kinds::INDEX);
        walk::index(&paths, &mut conn, &mut |_, _| {}).unwrap();
        db::jobs::complete(&conn, job.id).unwrap();
        drain(&paths, &mut conn, &Suppressor::default());

        assert_eq!(
            db::items::count(&conn).unwrap(),
            1,
            "the missed file is indexed"
        );
    }

    // --- 4. identity on modification ---------------------------------------

    #[test]
    fn a_modified_file_updates_the_existing_row_instead_of_creating_another() {
        let root = scratch("watch-identity-modify");
        let (paths, mut conn) = open_db(&root);
        let file = root.join("photo.png");
        write_png(&file, 10, 10);

        walk::index(&paths, &mut conn, &mut |_, _| {}).unwrap();
        let suppressor = Suppressor::default();
        drain(&paths, &mut conn, &suppressor);

        assert_eq!(db::items::count(&conn).unwrap(), 1);
        let before = db::items::list(&conn, &Scope::default()).unwrap();
        let (before_id, before_hash) = (before[0].id, before[0].thumb.clone());

        // Simulate the watcher settling on the same path after its content
        // changed — exactly what `handle_settled` does for a file, without
        // needing a real notify event round trip.
        write_png(&file, 40, 20);
        let meta = std::fs::metadata(&file).unwrap();
        walk::queue_file(
            &paths,
            &conn,
            db::folders::id_for_rel(&conn, "").unwrap().unwrap(),
            "photo.png",
            &meta,
        )
        .unwrap();
        drain(&paths, &mut conn, &suppressor);

        assert_eq!(
            db::items::count(&conn).unwrap(),
            1,
            "still exactly one row — anchored on path, not appended"
        );
        let after = db::items::list(&conn, &Scope::default()).unwrap();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].id, before_id, "same identity");
        assert_eq!(
            after[0].thumb, before_hash,
            "thumb path is derived from uuid alone, which does not change"
        );
    }

    #[test]
    fn a_removed_file_is_retired_not_left_behind() {
        let root = scratch("watch-removed");
        let (paths, mut conn) = open_db(&root);
        let file = root.join("gone.png");
        write_png(&file, 8, 8);

        walk::index(&paths, &mut conn, &mut |_, _| {}).unwrap();
        drain(&paths, &mut conn, &Suppressor::default());
        assert_eq!(db::items::count(&conn).unwrap(), 1);

        std::fs::remove_file(&file).unwrap();
        handle_removed(&paths, &conn, &file).unwrap();

        assert_eq!(
            db::items::count(&conn).unwrap(),
            0,
            "retired, not left behind"
        );
    }

    // --- 5. directory renames (M2.2) --------------------------------------

    fn rename_event(kind_mode: RenameMode, path: &Path) -> notify::Event {
        notify::Event::new(EventKind::Modify(ModifyKind::Name(kind_mode))).add_path(path.to_path_buf())
    }

    #[test]
    fn a_paired_directory_rename_updates_the_existing_folder_in_place() {
        // The dumb remove-then-create path would retire every item beneath
        // the old name and re-index the new one as unrelated, losing tags
        // and favorites on the whole subtree — this is the bug the From/To
        // pairing in `handle_event` exists to avoid.
        let root = scratch("watch-dir-rename-title-updates");
        let (paths, conn) = open_db(&root);
        db::folders::upsert(&conn, "", "Library").unwrap();
        let ana = db::folders::upsert(&conn, "ana", "Ana").unwrap();
        std::fs::create_dir_all(root.join("Ana")).unwrap();
        std::fs::rename(root.join("Ana"), root.join("Anastasia")).unwrap();

        let mut pending = HashMap::new();
        let mut pending_rename_from = None;
        let suppressor = Suppressor::default();

        handle_event(
            &paths,
            &conn,
            &suppressor,
            &mut pending,
            &mut pending_rename_from,
            rename_event(RenameMode::From, &root.join("Ana")),
        );
        assert!(pending_rename_from.is_some(), "the From half is stashed, not acted on yet");

        handle_event(
            &paths,
            &conn,
            &suppressor,
            &mut pending,
            &mut pending_rename_from,
            rename_event(RenameMode::To, &root.join("Anastasia")),
        );
        assert!(pending_rename_from.is_none());

        let detail = db::folders::get_detail(&conn, ana).unwrap().unwrap();
        assert_eq!(detail.title, "Anastasia", "the title follows the external rename");
        assert_eq!(detail.rel_path, "anastasia");
    }

    #[test]
    fn a_paired_directory_rename_leaves_the_title_alone_when_it_already_sanitises_to_the_new_name() {
        let root = scratch("watch-dir-rename-title-unchanged");
        let (paths, conn) = open_db(&root);
        db::folders::upsert(&conn, "", "Library").unwrap();
        // Title already sanitises to "Ana-Trip" — simulating drift between
        // the title and what's actually on disk (e.g. a database rebuilt
        // from library.jsonl after the title was set but before the
        // directory rename that should have followed it).
        let folder = db::folders::upsert(&conn, "ana", "Ana/Trip").unwrap();
        std::fs::create_dir_all(root.join("Ana")).unwrap();
        std::fs::rename(root.join("Ana"), root.join("Ana-Trip")).unwrap();

        let mut pending = HashMap::new();
        let mut pending_rename_from = None;
        let suppressor = Suppressor::default();

        handle_event(
            &paths,
            &conn,
            &suppressor,
            &mut pending,
            &mut pending_rename_from,
            rename_event(RenameMode::From, &root.join("Ana")),
        );
        handle_event(
            &paths,
            &conn,
            &suppressor,
            &mut pending,
            &mut pending_rename_from,
            rename_event(RenameMode::To, &root.join("Ana-Trip")),
        );

        let detail = db::folders::get_detail(&conn, folder).unwrap().unwrap();
        assert_eq!(
            detail.title, "Ana/Trip",
            "only the derived name changed — the title already explains it"
        );
        assert_eq!(detail.rel_path, "ana-trip");
    }

    #[test]
    fn a_new_from_flushes_an_earlier_unmatched_one_immediately() {
        let root = scratch("watch-dir-rename-flush-on-next-from");
        let (paths, mut conn) = open_db(&root);
        db::folders::upsert(&conn, "", "Library").unwrap();
        std::fs::create_dir_all(root.join("Gone")).unwrap();
        write_png(&root.join("Gone/photo.png"), 4, 4);
        walk::index(&paths, &mut conn, &mut |_, _| {}).unwrap();
        drain(&paths, &mut conn, &Suppressor::default());
        assert_eq!(db::items::count(&conn).unwrap(), 1);

        let mut pending = HashMap::new();
        let mut pending_rename_from = None;
        let suppressor = Suppressor::default();

        // "Gone" leaves the watched tree entirely — no paired To will ever
        // arrive for it.
        std::fs::remove_dir_all(root.join("Gone")).unwrap();
        handle_event(
            &paths,
            &conn,
            &suppressor,
            &mut pending,
            &mut pending_rename_from,
            rename_event(RenameMode::From, &root.join("Gone")),
        );
        assert!(pending_rename_from.is_some());

        // A second, unrelated From is what proves the first was never
        // getting a pair — it must flush immediately rather than wait out
        // the settle timeout.
        handle_event(
            &paths,
            &conn,
            &suppressor,
            &mut pending,
            &mut pending_rename_from,
            rename_event(RenameMode::From, &root.join("also-unrelated")),
        );

        assert_eq!(
            db::items::count(&conn).unwrap(),
            0,
            "the item beneath the departed folder was retired"
        );
    }

    #[test]
    fn an_unpaired_from_is_flushed_once_it_has_waited_past_settle() {
        // A directory moved *out* of the watched tree only ever fires the
        // From half — nothing this app watches ever sees a matching To, and
        // nothing may ever rename anything else to trigger the immediate
        // flush either. `flush_stale_rename_from` is the safety net.
        let root = scratch("watch-dir-rename-timeout-flush");
        let (paths, mut conn) = open_db(&root);
        db::folders::upsert(&conn, "", "Library").unwrap();
        std::fs::create_dir_all(root.join("Gone")).unwrap();
        write_png(&root.join("Gone/photo.png"), 4, 4);
        walk::index(&paths, &mut conn, &mut |_, _| {}).unwrap();
        drain(&paths, &mut conn, &Suppressor::default());
        assert_eq!(db::items::count(&conn).unwrap(), 1);

        std::fs::remove_dir_all(root.join("Gone")).unwrap();
        // Backdated rather than actually slept for — same trick the settle
        // tests above use.
        let mut pending_rename_from = Some((
            root.join("Gone"),
            Instant::now() - SETTLE - Duration::from_millis(200),
        ));

        flush_stale_rename_from(&paths, &conn, &mut pending_rename_from);

        assert!(pending_rename_from.is_none());
        assert_eq!(
            db::items::count(&conn).unwrap(),
            0,
            "the item beneath the departed folder was retired"
        );
    }

    #[test]
    fn a_fresh_unmatched_from_is_not_flushed_before_settle() {
        let root = scratch("watch-dir-rename-not-yet-stale");
        let (paths, conn) = open_db(&root);
        let mut pending_rename_from = Some((root.join("Gone"), Instant::now()));

        flush_stale_rename_from(&paths, &conn, &mut pending_rename_from);

        assert!(pending_rename_from.is_some(), "not held long enough yet");
    }
}
