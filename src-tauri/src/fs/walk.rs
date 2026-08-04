//! The library indexer.
//!
//! **Strictly read-only over the library.** The walk opens directories and
//! reads file metadata; it does not rename, move, delete or write anything
//! outside `.gallery/`. Reading file contents happens later, in hash jobs, so
//! that the walk stays fast enough to give the user a shape of the library
//! within seconds of pointing at it.

use std::collections::HashMap;
use std::fs::Metadata;
use std::path::Path;
use std::time::UNIX_EPOCH;

use rusqlite::Connection;
use walkdir::{DirEntry, WalkDir};

use crate::db;
use crate::error::Result;
use crate::fs::paths::LibraryPaths;
use crate::jobs;

/// Rows per transaction. Small enough that job workers writing item rows are
/// never locked out for long, large enough that a 100k-file walk is not 100k
/// fsyncs.
const BATCH: u64 = 500;

/// Windows and macOS litter; never library content.
///
/// `pub(crate)` — the M1.7 pre-import filesystem scan in `fs::import`
/// filters by the same list before any database exists to check against.
pub(crate) const IGNORED_FILES: &[&str] = &["thumbs.db", "desktop.ini", ".ds_store"];

#[derive(Debug, Clone, Copy, Default)]
pub struct WalkReport {
    pub folders: u64,
    pub files: u64,
    pub queued: u64,
    pub vanished: usize,
}

/// Directory name to fall back on when the root has none (e.g. a drive
/// root). Shared with `fs::watch`, which resolves the same root folder row
/// on demand while ensuring a new arrival's folder chain exists.
pub(crate) fn root_title(paths: &LibraryPaths) -> String {
    paths
        .root()
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Library".to_string())
}

pub fn index(
    paths: &LibraryPaths,
    conn: &mut Connection,
    on_progress: &mut dyn FnMut(u64, u64),
) -> Result<WalkReport> {
    let mut report = WalkReport::default();
    let mut folder_ids: HashMap<String, i64> = HashMap::new();

    db::items::begin_sweep(conn)?;

    let root_id = db::folders::upsert(conn, "", &root_title(paths))?;
    folder_ids.insert(String::new(), root_id);
    report.folders = 1;

    walk_tree(
        paths,
        conn,
        paths.root(),
        &mut folder_ids,
        &mut report,
        true,
        on_progress,
    )?;

    // Anything in the database that the walk did not see is gone from disk.
    // This is database bookkeeping only — no file is touched — and a file that
    // comes back clears the mark when it is re-indexed.
    report.vanished = db::items::finish_sweep(conn)?;

    Ok(report)
}

/// Walk one subtree that just appeared under an already-open, already-indexed
/// library — the filesystem watcher's response to a whole folder arriving in
/// one atomic move. `ReadDirectoryChangesW` reports a single event for the
/// top directory in that case, with no guarantee of separate events for
/// whatever was already inside it, so the watcher walks it once here rather
/// than assuming per-file events will follow.
///
/// Unlike `index`, there is no reconciliation sweep: only `dir` is in scope,
/// so "not seen during this walk" says nothing about anything outside it —
/// running one would incorrectly retire every other item in the library.
/// The caller is expected to have already ensured `dir`'s own ancestor chain
/// of folder rows exists (see `fs::watch::ensure_folder_chain`); this walk
/// only ever creates folder rows for `dir` and whatever is beneath it.
pub fn index_subtree(paths: &LibraryPaths, conn: &Connection, dir: &Path) -> Result<WalkReport> {
    let mut report = WalkReport::default();
    let mut folder_ids: HashMap<String, i64> = HashMap::new();
    walk_tree(
        paths,
        conn,
        dir,
        &mut folder_ids,
        &mut report,
        false,
        &mut |_, _| {},
    )?;
    Ok(report)
}

/// The shared walk loop: queues work for every file under `start`, creating
/// folder rows for directories it has not seen yet. `folder_ids` is seeded by
/// the caller with whatever ancestors are already known — `index` seeds it
/// with the root; `index_subtree` starts it empty because `start` itself and
/// everything below it is new. `mark_seen` is only meaningful alongside a
/// sweep, so it is skipped entirely for a subtree walk, which has no sweep to
/// feed.
fn walk_tree(
    paths: &LibraryPaths,
    conn: &Connection,
    start: &Path,
    folder_ids: &mut HashMap<String, i64>,
    report: &mut WalkReport,
    mark_seen: bool,
    on_progress: &mut dyn FnMut(u64, u64),
) -> Result<()> {
    db::begin_batch(conn)?;
    let mut in_batch = 0u64;

    let walker = WalkDir::new(start)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !is_skipped_dir(paths, entry));

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            // An unreadable directory is worth continuing past, not aborting
            // an index of 300GB over.
            Err(err) => {
                eprintln!("skipping unreadable entry: {err}");
                continue;
            }
        };

        if entry.file_type().is_dir() {
            let rel = paths.to_rel(entry.path())?;
            if folder_ids.contains_key(&rel) {
                continue; // the root (index) or start (index_subtree), already inserted
            }
            let title = entry.file_name().to_string_lossy().to_string();
            let id = db::folders::upsert(conn, &rel, &title)?;
            folder_ids.insert(rel, id);
            report.folders += 1;
        } else if entry.file_type().is_file() {
            let name = entry.file_name().to_string_lossy().to_string();
            if IGNORED_FILES.contains(&name.to_lowercase().as_str()) {
                continue;
            }

            let parent_rel = entry
                .path()
                .parent()
                .map(|p| paths.to_rel(p))
                .transpose()?
                .unwrap_or_default();
            let folder_id = match folder_ids.get(&parent_rel) {
                Some(id) => *id,
                None => continue, // parent was skipped, so this file is too
            };

            let Ok(meta) = entry.metadata() else {
                continue;
            };
            report.files += 1;

            if mark_seen {
                db::items::mark_seen(conn, folder_id, &name)?;
            }
            if queue_file(paths, conn, folder_id, &name, &meta)? {
                report.queued += 1;
            }

            in_batch += 1;
            if in_batch >= BATCH {
                db::commit_batch(conn)?;
                db::begin_batch(conn)?;
                in_batch = 0;
                on_progress(report.folders, report.files);
            }
        }
    }

    db::commit_batch(conn)?;
    on_progress(report.folders, report.files);
    Ok(())
}

/// Enqueue work for one file, or nothing if it is already indexed and
/// unchanged. Returns whether a job was queued.
///
/// `pub(crate)` — the filesystem watcher calls this directly for a single
/// settled file, the same way this walk calls it for every file it visits,
/// so a modified file is re-hashed through the identical path that indexes a
/// new one rather than the watcher growing its own copy.
pub(crate) fn queue_file(
    paths: &LibraryPaths,
    conn: &Connection,
    folder_id: i64,
    name: &str,
    meta: &Metadata,
) -> Result<bool> {
    let size = meta.len() as i64;
    let mtime = mtime_secs(meta);

    if let Some(existing) = db::items::existing(conn, folder_id, name)? {
        let unchanged = !existing.deleted && existing.size_bytes == size && existing.mtime == mtime;
        if unchanged {
            // Still worth a thumbnail if the cache was cleared or the last run
            // was interrupted — the cache is explicitly safe to delete.
            if !paths.thumb_path(&existing.uuid).exists() {
                jobs::enqueue_thumb(conn, existing.id)?;
                return Ok(true);
            }
            return Ok(false);
        }
    }

    jobs::enqueue_hash(conn, folder_id, name)?;
    Ok(true)
}

/// `.gallery` is the app's own storage, and dot-directories are not library
/// content. Everything else under the root is fair game.
///
/// `pub(crate)` for the same reason as `IGNORED_FILES` above.
pub(crate) fn is_skipped_dir(paths: &LibraryPaths, entry: &DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }
    if paths.is_gallery_dir(entry.path()) {
        return true;
    }
    entry.depth() > 0 && entry.file_name().to_string_lossy().starts_with('.')
}

pub fn mtime_secs(meta: &Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The filesystem's own creation time — NTFS always records one, unlike the
/// POSIX systems `Metadata::created()` is also defined for. This is the
/// fallback `captured_at` uses (`media::probe`) when a file carries no EXIF
/// or container date: the moment the file actually came to exist, not the
/// moment it was last touched, which is what `mtime_secs` above answers
/// instead and is a worse stand-in for "when was this taken".
pub fn created_secs(meta: &Metadata, fallback: i64) -> i64 {
    meta.created()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(fallback)
}
