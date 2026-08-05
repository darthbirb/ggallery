//! The startup reconcile pass.
//!
//! PLAN.md §M2.6 deleted this module's whole reason for being a *walker*:
//! folders are no longer directories, so there is nothing left to discover
//! by walking one. What used to be "index the library root" is now two much
//! smaller, uuid-driven jobs, both run once at startup by [`reconcile`] and
//! otherwise handled live by `fs::watch`:
//!
//! 1. **Sweep the root.** `inbox/` is the only place a user is meant to put
//!    files (PLAN.md decision 30), but nothing stops one landing directly in
//!    the library root instead — by hand, or because `fs::watch`'s live
//!    root watch was not running to catch it. [`sweep_root_into_inbox`] moves
//!    every top-level entry that is not the app's own into `inbox/`, one
//!    `rename` per entry, so this covers whatever the watcher missed.
//! 2. **Does every item's shard file still exist?** A cheap existence check
//!    per row — `files/` is never itself walked, since the uuid already says
//!    exactly where to look — soft-deleting anything that doesn't resolve.
//! 3. **Drain `inbox/`.** Anything sitting there when the app starts (dropped
//!    in while it was closed, or just swept in by step 1) is queued for
//!    hashing exactly like a live arrival the watcher catches.

use std::fs::Metadata;
use std::time::UNIX_EPOCH;

use rusqlite::Connection;
use walkdir::WalkDir;

use crate::db;
use crate::error::Result;
use crate::fs::paths::LibraryPaths;
use crate::jobs;

/// Windows and macOS litter; never library content.
pub(crate) const IGNORED_FILES: &[&str] = &["thumbs.db", "desktop.ini", ".ds_store"];

#[derive(Debug, Clone, Copy, Default)]
pub struct WalkReport {
    /// Live items whose shard file was confirmed present.
    pub items_checked: u64,
    /// Inbox arrivals newly queued for hashing.
    pub queued: u64,
    /// Items retired because their shard file no longer resolves.
    pub vanished: usize,
}

/// One entry `sweep_root_into_inbox` could not move — the name it found at
/// the root, and why.
#[derive(Debug, Clone)]
pub struct RootSweepError {
    pub name: String,
    pub error: String,
}

#[derive(Debug, Clone, Default)]
pub struct RootSweepReport {
    pub moved: i64,
    pub errors: Vec<RootSweepError>,
}

/// Move every top-level entry in the library root — apart from the app's own
/// `.gallery`, `files` and `inbox` — into `inbox/`. One `rename` per entry,
/// so a whole pre-existing directory tree moves in a single atomic step
/// regardless of how many files are nested inside it. Idempotent for free:
/// an entry no longer at the root (already swept by an earlier pass) is
/// simply not there to find, not an error.
///
/// Shared by three callers that all mean the same thing by "a file showed up
/// somewhere it shouldn't stay": `fs::import`'s first-import ceremony (the
/// whole existing tree, once), this module's own startup `reconcile` (catch-up
/// for whatever `fs::watch` missed while the app was closed), and
/// `fs::watch`'s live root watch (one entry, as it settles).
pub fn sweep_root_into_inbox(
    paths: &LibraryPaths,
    on_progress: &mut dyn FnMut(i64, i64),
) -> Result<RootSweepReport> {
    paths.ensure_dirs()?;

    let mut entries = Vec::new();
    for entry in std::fs::read_dir(paths.root())? {
        let entry = entry?;
        let name = entry.file_name();
        if crate::fs::paths::is_reserved_top_level(&name.to_string_lossy()) {
            continue;
        }
        entries.push(name);
    }

    let total = entries.len() as i64;
    let mut report = RootSweepReport::default();

    for name in entries {
        let src = paths.root().join(&name);
        let dest = paths.inbox_dir().join(&name);
        let name_str = name.to_string_lossy().to_string();

        if !src.exists() {
            report.moved += 1;
        } else if dest.exists() {
            report.errors.push(RootSweepError {
                name: name_str,
                error: format!("{} already exists in inbox", dest.display()),
            });
        } else {
            match std::fs::rename(&src, &dest) {
                Ok(()) => report.moved += 1,
                Err(err) => report.errors.push(RootSweepError { name: name_str, error: err.to_string() }),
            }
        }

        on_progress(report.moved, total);
    }

    Ok(report)
}

/// Reconcile the database against disk, then drain `inbox/`. Run once at
/// startup, and again whenever the watcher overflows or errors and can no
/// longer trust that it saw everything.
pub fn reconcile(
    paths: &LibraryPaths,
    conn: &Connection,
    on_progress: &mut dyn FnMut(u64, u64),
) -> Result<WalkReport> {
    let mut report = WalkReport::default();

    // Catch up on anything that landed directly in the root — by hand, or
    // because the live root watch (`fs::watch`) was not running to sweep it
    // the moment it settled — before the inbox drain below, so it is picked
    // up in this same pass rather than waiting for the next one.
    match sweep_root_into_inbox(paths, &mut |_, _| {}) {
        Ok(swept) => {
            for err in &swept.errors {
                eprintln!("reconcile could not sweep {} into inbox: {}", err.name, err.error);
            }
        }
        Err(err) => eprintln!("reconcile could not sweep the library root: {err}"),
    }

    db::items::begin_sweep(conn)?;
    let mut stmt = conn.prepare("SELECT uuid, ext FROM item WHERE deleted_at IS NULL")?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    drop(stmt);
    for (uuid, ext) in &rows {
        if paths.item_path(uuid, ext).is_file() {
            db::items::mark_seen(conn, uuid)?;
        }
        report.items_checked += 1;
    }
    report.vanished = db::items::finish_sweep(conn)?;
    on_progress(report.items_checked, report.queued);

    let inbox = paths.inbox_dir();
    for entry in WalkDir::new(&inbox).follow_links(false).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if IGNORED_FILES.contains(&name.to_lowercase().as_str()) {
            continue;
        }
        let Ok(inbox_rel) = entry.path().strip_prefix(&inbox) else { continue };
        let inbox_rel = inbox_rel.to_string_lossy().replace('\\', "/");

        // A hash job for this exact arrival may already be queued from a
        // previous reconcile or from the watcher having just settled it —
        // `inbox/` is expected to hold at most a handful of files at once
        // (unlike a 300GB library), so a per-file `is_queued` check here
        // costs nothing the way it would over the whole library.
        let payload = serde_json::to_string(&jobs::kinds::HashPayload { inbox_rel: inbox_rel.clone() })?;
        if db::jobs::is_queued(conn, jobs::kinds::HASH, &payload)? {
            continue;
        }
        jobs::enqueue_hash(conn, &inbox_rel)?;
        report.queued += 1;
    }
    on_progress(report.items_checked, report.queued);

    Ok(report)
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
/// or container date.
pub fn created_secs(meta: &Metadata, fallback: i64) -> i64 {
    meta.created()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::items::NewItem;

    fn scratch(name: &str) -> std::path::PathBuf {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-libraries")
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create scratch library");
        root
    }

    fn open_db(root: &std::path::Path) -> (LibraryPaths, Connection) {
        let paths = LibraryPaths::new(root);
        paths.ensure_dirs().unwrap();
        let mut conn = db::open(&paths.db_path()).unwrap();
        db::migrate(&mut conn).unwrap();
        (paths, conn)
    }

    fn seed_item(conn: &Connection, uuid: &str, ext: &str) -> i64 {
        db::items::upsert(
            conn,
            &NewItem {
                uuid: uuid.to_string(),
                folder_id: None,
                disk_name: format!("{uuid}.{ext}"),
                ext: ext.to_string(),
                orig_name: "a.jpg".to_string(),
                hash: "h".to_string(),
                size_bytes: 1,
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
        .unwrap()
    }

    #[test]
    fn reconcile_retires_an_item_whose_shard_file_is_gone() {
        let root = scratch("walk-reconcile-vanished");
        let (paths, conn) = open_db(&root);
        seed_item(&conn, "a3f2c1d4-e29b-41d4-a716-446655440000", "jpg");
        // Never actually written to `files/` — simulates a file removed
        // outside the app.

        let report = reconcile(&paths, &conn, &mut |_, _| {}).unwrap();
        assert_eq!(report.vanished, 1);
        assert_eq!(db::items::count(&conn).unwrap(), 0);
    }

    #[test]
    fn reconcile_leaves_a_present_shard_file_alone() {
        let root = scratch("walk-reconcile-present");
        let (paths, conn) = open_db(&root);
        let uuid = "a3f2c1d4-e29b-41d4-a716-446655440000";
        seed_item(&conn, uuid, "jpg");
        let dest = paths.item_path(uuid, "jpg");
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(&dest, b"bytes").unwrap();

        let report = reconcile(&paths, &conn, &mut |_, _| {}).unwrap();
        assert_eq!(report.vanished, 0);
        assert_eq!(report.items_checked, 1);
        assert_eq!(db::items::count(&conn).unwrap(), 1);
    }

    #[test]
    fn reconcile_queues_whatever_is_waiting_in_inbox() {
        let root = scratch("walk-reconcile-inbox");
        let (paths, conn) = open_db(&root);
        std::fs::write(paths.inbox_dir().join("photo.jpg"), b"bytes").unwrap();
        std::fs::write(paths.inbox_dir().join("Thumbs.db"), b"litter").unwrap();

        let report = reconcile(&paths, &conn, &mut |_, _| {}).unwrap();
        assert_eq!(report.queued, 1, "Thumbs.db is not library content");
        assert_eq!(db::jobs::counts(&conn).unwrap().pending, 1);
    }

    #[test]
    fn reconcile_does_not_double_queue_an_already_pending_arrival() {
        let root = scratch("walk-reconcile-no-dup");
        let (paths, conn) = open_db(&root);
        std::fs::write(paths.inbox_dir().join("photo.jpg"), b"bytes").unwrap();

        reconcile(&paths, &conn, &mut |_, _| {}).unwrap();
        let second = reconcile(&paths, &conn, &mut |_, _| {}).unwrap();
        assert_eq!(second.queued, 0);
        assert_eq!(db::jobs::counts(&conn).unwrap().pending, 1);
    }

    /// PLAN.md decision 20: verified at scale before this ever runs against
    /// the real library. `cargo test --release scale_check_reconcile --
    /// --ignored --nocapture`.
    #[test]
    #[ignore]
    fn scale_check_reconcile() {
        const N: i64 = 100_000;
        let root = scratch("walk-reconcile-scale");
        let (paths, conn) = open_db(&root);

        let setup_start = std::time::Instant::now();
        db::begin_batch(&conn).unwrap();
        for i in 0..N {
            if i % 5000 == 0 && i > 0 {
                db::commit_batch(&conn).unwrap();
                db::begin_batch(&conn).unwrap();
            }
            let uuid = uuid::Uuid::new_v4().to_string();
            let id = seed_item(&conn, &uuid, "bin");
            let _ = id;
            let dest = paths.item_path(&uuid, "bin");
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::write(&dest, b"x").unwrap();
        }
        db::commit_batch(&conn).unwrap();
        println!("setup: {:?} for {N} items", setup_start.elapsed());

        let start = std::time::Instant::now();
        let report = reconcile(&paths, &conn, &mut |_, _| {}).unwrap();
        let elapsed = start.elapsed();
        println!("reconcile: {elapsed:?} for {N} items");
        assert_eq!(report.items_checked, N as u64);
        assert_eq!(report.vanished, 0);
        assert!(elapsed < std::time::Duration::from_secs(30), "reconcile too slow: {elapsed:?}");
    }
}
