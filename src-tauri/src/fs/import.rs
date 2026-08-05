//! The startup flow for a library that has never been imported — `prepare` /
//! `execute_prepared`, M1.7's Choose → Review → Progress shape. Filesystem-only
//! and runs before any database exists, so it works directly against a raw
//! directory tree. See docs/DESIGN.md#first-import for the flow.
//!
//! **PLAN.md §M2.6 removed this module's other half.** Before decision 30,
//! this also carried the database-backed scan/dry-run/execute/verify used by
//! Settings → *Normalise filenames* — repairing a library where some items
//! had fallen behind `disk_name == <uuid>.<ext>`. Once every item's location
//! *is* `<uuid>.<ext>` by construction (`files/<xx>/<uuid>.<ext>`, resolved
//! by `fs::shard`), "not yet renamed" stops being a backlog that can exist:
//! an item's file is sharded the moment it's indexed, full stop. What that
//! repair action's job turns into — a stray file in `files/` with no
//! matching row, or a row whose shard file has gone missing — is
//! `fs::shard`'s own reconcile pass, not a rename.
//!
//! **Rewritten again for M2.6's own consequence.** This used to rename every
//! file *in place*, preserving whatever subdirectory structure it was already
//! in — the M1-era model, from before folders were entities at all. That
//! stopped working the moment `fs::walk` lost the tree-walker that used to
//! turn directories into folders: nothing was left to ever discover a file
//! sitting in a directory the app no longer looks at, so a first import
//! quietly produced an empty grid. The fix follows decision 30 all the way
//! through: a first import is no different from a bulk `inbox/` drop. Every
//! top-level entry moves into `inbox/` — one `rename` each, not one per file,
//! since a directory move carries everything inside it — and the existing
//! inbox-drain-and-index pipeline (`fs::walk::reconcile`, `jobs::worker`)
//! takes over from there, indexing every file into the Sorting Box exactly as
//! it would for a single file dropped in by hand. Nothing here renames a file
//! to its uuid any more; that happens once, during indexing, the same place
//! it happens for every arrival after the first.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use walkdir::WalkDir;

use crate::db;
use crate::error::Result;
use crate::fs::paths::LibraryPaths;
use crate::fs::walk;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    pub done: i64,
    pub total: i64,
    pub errors: i64,
}

// --- M1.7 startup flow: filesystem-only scan and execute -------------------
//
// A library that has never been imported has no database worth reading yet,
// so `prepare` and `execute_prepared` work the filesystem directly, the same
// way a plain `ls -R` would, and never open a job queue or write anything
// into `.gallery/`.

/// Held in `AppState` between `prepare` and `execute_prepared`. Just the root
/// — unlike the old per-file rename plan, there is nothing left to stage:
/// `execute_prepared` re-reads the root's own top level, which is cheap and,
/// unlike a stashed file list, never goes stale if something changed on disk
/// between the two calls.
pub struct PendingImport {
    pub root: PathBuf,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewReport {
    /// True when this library has either already completed import, or has
    /// nothing in it at all — resolved on the spot by `prepare`. Either way
    /// the frontend skips Review and Progress and opens straight into the
    /// gallery.
    pub already_imported: bool,
    pub by_kind: Vec<db::items::KindTotal>,
    pub total_items: i64,
    pub total_bytes: i64,
    /// Entries the scan could not read at all — a locked file, a permission
    /// error — reported so Review does not imply the whole folder was seen
    /// when some of it was not.
    pub unreadable: i64,
}

/// The Choose-folder step's follow-through. Figures out whether this library
/// needs the import ceremony at all and, if so, scans it — all without
/// opening a job queue or writing a thumbnail. Returns the plan alongside the
/// report so the caller can stash it for `execute_prepared`.
pub fn prepare(root: &Path) -> Result<(ReviewReport, Option<PendingImport>)> {
    let paths = LibraryPaths::new(root);

    if paths.db_path().is_file() {
        let mut conn = db::open(&paths.db_path())?;
        if db::needs_storage_migration(&conn)? {
            // A pre-M2.6 library at this root belongs to
            // `commands::storage_migration`, not the first-import wizard —
            // it has already been imported once, just not yet migrated.
            return Ok((ReviewReport { already_imported: true, ..Default::default() }, None));
        }
        db::migrate(&mut conn)?;
        if db::settings::imported_at(&conn)?.is_some() {
            return Ok((ReviewReport { already_imported: true, ..Default::default() }, None));
        }
    }

    let scan = scan_filesystem(&paths)?;
    if scan.total_items == 0 {
        // Nothing to do — mark it imported on the spot rather than asking the
        // user to click through a review screen with nothing on it.
        paths.ensure_dirs()?;
        let mut conn = db::open(&paths.db_path())?;
        db::migrate(&mut conn)?;
        db::settings::mark_imported(&conn)?;
        return Ok((ReviewReport { already_imported: true, ..Default::default() }, None));
    }

    let report = ReviewReport {
        already_imported: false,
        by_kind: scan.by_kind,
        total_items: scan.total_items,
        total_bytes: scan.total_bytes,
        unreadable: scan.unreadable,
    };
    let pending = PendingImport { root: root.to_path_buf() };
    Ok((report, Some(pending)))
}

struct FsScan {
    by_kind: Vec<db::items::KindTotal>,
    total_bytes: i64,
    total_items: i64,
    unreadable: i64,
}

/// Mirrors the OS-litter filtering `fs::walk` applies to `inbox/`, without
/// reading a single byte of file content, so it stays fast over a few hundred
/// thousand files. Counts only — nothing here plans a rename any more, since
/// `execute_prepared` moves whole top-level directories rather than
/// individual files.
fn scan_filesystem(paths: &LibraryPaths) -> Result<FsScan> {
    let mut kind_totals: HashMap<&'static str, (i64, i64)> = HashMap::new();
    let mut unreadable = 0i64;
    let mut total_bytes = 0i64;
    let mut total_items = 0i64;

    let walker = WalkDir::new(paths.root())
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !is_skipped_dir(paths, entry));

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                unreadable += 1;
                continue;
            }
        };

        if entry.file_type().is_dir() {
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if walk::IGNORED_FILES.contains(&name.to_lowercase().as_str()) {
            continue;
        }

        let Ok(meta) = entry.metadata() else {
            unreadable += 1;
            continue;
        };

        let ext = crate::fs::paths::extension_of(&name);
        let kind = crate::media::Kind::from_ext(&ext);
        let totals = kind_totals.entry(kind.as_str()).or_insert((0, 0));
        totals.0 += 1;
        totals.1 += meta.len() as i64;
        total_bytes += meta.len() as i64;
        total_items += 1;
    }

    let by_kind = kind_totals
        .into_iter()
        .map(|(kind, (count, bytes))| db::items::KindTotal { kind: kind.to_string(), count, bytes })
        .collect();

    Ok(FsScan { by_kind, total_bytes, total_items, unreadable })
}

/// A directory that also skips the app's own reserved top-level names, so a
/// second `prepare` after a partially-completed `execute_prepared` does not
/// recount what has already moved into `inbox/`.
fn is_skipped_dir(paths: &LibraryPaths, entry: &walkdir::DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }
    if paths.is_gallery_dir(entry.path()) {
        return true;
    }
    if entry.depth() == 1 && crate::fs::paths::is_reserved_top_level(&entry.file_name().to_string_lossy()) {
        return true;
    }
    entry.depth() > 0 && entry.file_name().to_string_lossy().starts_with('.')
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsMoveError {
    pub name: String,
    pub error: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsExecuteReport {
    pub moved: i64,
    pub errors: Vec<FsMoveError>,
}

/// Move everything `prepare` found into `inbox/` — delegates to
/// `fs::walk::sweep_root_into_inbox`, the same sweep the startup reconcile
/// and the live root watch both use, so a first import is exactly "sweep the
/// root, then let the ordinary inbox pipeline take over" rather than a
/// separate code path that happens to do the same thing. One `rename` per
/// top-level entry — a directory move carries everything beneath it in one
/// atomic step, so this is cheap regardless of how many files are nested
/// inside. Resumable for free: an entry no longer at the root was already
/// moved by an earlier, interrupted run, so it is counted done rather than
/// retried. Nothing here touches the database beyond marking the library
/// imported — indexing (and the uuid rename that comes with it) starts once
/// the caller opens the library for real, after this returns.
pub fn execute_prepared(
    pending: &PendingImport,
    on_progress: &mut dyn FnMut(&ImportProgress),
) -> Result<FsExecuteReport> {
    let paths = LibraryPaths::new(&pending.root);

    let swept = walk::sweep_root_into_inbox(&paths, &mut |done, total| {
        on_progress(&ImportProgress { done, total, errors: 0 });
    })?;
    let report = FsExecuteReport {
        moved: swept.moved,
        errors: swept
            .errors
            .into_iter()
            .map(|e| FsMoveError { name: e.name, error: e.error })
            .collect(),
    };

    let mut conn = db::open(&paths.db_path())?;
    db::migrate(&mut conn)?;
    db::settings::mark_imported(&conn)?;

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-libraries")
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create scratch library");
        root
    }

    #[test]
    fn prepare_finds_files_needing_import() {
        let root = scratch("m17-prepare");
        std::fs::create_dir_all(root.join("People/Ana")).unwrap();
        std::fs::write(root.join("People/Ana/holiday.jpg"), b"hello world").unwrap();
        std::fs::write(root.join("cover.png"), b"cover bytes").unwrap();

        let (report, pending) = prepare(&root).unwrap();
        assert!(!report.already_imported);
        assert_eq!(report.total_items, 2);
        assert!(pending.is_some());
    }

    #[test]
    fn execute_prepared_moves_everything_into_inbox_and_marks_imported() {
        let root = scratch("m17-execute");
        std::fs::create_dir_all(root.join("People/Ana")).unwrap();
        std::fs::write(root.join("People/Ana/holiday.jpg"), b"hello world").unwrap();
        std::fs::write(root.join("cover.png"), b"cover bytes").unwrap();

        let (_, pending) = prepare(&root).unwrap();
        let pending = pending.unwrap();

        let report = execute_prepared(&pending, &mut |_| {}).unwrap();
        // Two top-level entries: `People/` (carrying `Ana/holiday.jpg` with
        // it) and `cover.png`.
        assert_eq!(report.moved, 2);
        assert!(report.errors.is_empty());

        assert!(root.join("inbox/People/Ana/holiday.jpg").is_file());
        assert!(root.join("inbox/cover.png").is_file());
        assert!(!root.join("People").exists());
        assert!(!root.join("cover.png").exists());

        let paths = LibraryPaths::new(&root);
        let conn = db::open(&paths.db_path()).unwrap();
        assert!(db::settings::imported_at(&conn).unwrap().is_some());
    }

    #[test]
    fn a_second_execute_only_touches_what_is_still_at_the_root() {
        let root = scratch("m17-resume");
        std::fs::write(root.join("cover.png"), b"cover bytes").unwrap();
        std::fs::write(root.join("beach.jpg"), b"beach bytes").unwrap();

        let (_, pending) = prepare(&root).unwrap();
        let pending = pending.unwrap();

        // Simulate an interrupted run: move only one entry by hand, as if the
        // first `execute_prepared` crashed partway through its loop. Because
        // `execute_prepared` re-reads the root's own top level rather than
        // trusting a stashed plan, the already-moved entry is simply not
        // there to find on the next run — nothing re-touches it, and nothing
        // errors on it either.
        let paths = LibraryPaths::new(&root);
        paths.ensure_dirs().unwrap();
        std::fs::rename(root.join("cover.png"), root.join("inbox/cover.png")).unwrap();

        let report = execute_prepared(&pending, &mut |_| {}).unwrap();
        assert_eq!(report.moved, 1, "only the file still at the root needed moving");
        assert!(report.errors.is_empty());
        assert!(root.join("inbox/cover.png").is_file());
        assert!(root.join("inbox/beach.jpg").is_file());
    }

    #[test]
    fn imported_files_are_discoverable_by_the_same_reconcile_a_dropped_in_file_uses() {
        // The actual bug this module exists to guard against: importing used
        // to rename files in place and leave them in whatever directory they
        // already were, but `fs::walk` lost its tree-walker in M2.6, so
        // nothing was left to ever look there again — an empty grid with no
        // error. Proving `reconcile` queues everything `execute_prepared`
        // moved is what closes the loop end to end, not just "the files
        // moved somewhere."
        let root = scratch("m17-then-reconcile");
        std::fs::create_dir_all(root.join("People/Ana")).unwrap();
        std::fs::write(root.join("People/Ana/holiday.jpg"), b"hello world").unwrap();
        std::fs::write(root.join("cover.png"), b"cover bytes").unwrap();

        let (_, pending) = prepare(&root).unwrap();
        execute_prepared(&pending.unwrap(), &mut |_| {}).unwrap();

        let paths = LibraryPaths::new(&root);
        let mut conn = db::open(&paths.db_path()).unwrap();
        db::migrate(&mut conn).unwrap();

        let report = crate::fs::walk::reconcile(&paths, &conn, &mut |_, _| {}).unwrap();
        assert_eq!(report.queued, 2, "both the nested and the top-level file are discovered");
        assert_eq!(db::jobs::counts(&conn).unwrap().pending, 2);
    }

    #[test]
    fn prepare_marks_an_empty_library_imported_with_no_ceremony() {
        let root = scratch("m17-nothing-to-import");
        let (report, pending) = prepare(&root).unwrap();
        assert!(report.already_imported);
        assert!(pending.is_none());

        let paths = LibraryPaths::new(&root);
        let conn = db::open(&paths.db_path()).unwrap();
        assert!(db::settings::imported_at(&conn).unwrap().is_some());
    }
}
