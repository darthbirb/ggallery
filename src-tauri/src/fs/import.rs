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
//! **Rewritten a third time, for PLAN.md §M2.6a.** The M2.6 rewrite (see git
//! history) swept every top-level entry into `inbox/` and let the ordinary
//! inbox pipeline flatten it all into the Sorting Box — correct for a drop,
//! wrong for an import: it discarded the entire organisation a user built
//! before the app ever existed, irreversibly, at the door. This is the one
//! moment the app ever reads meaning out of a directory structure (not
//! folder-name **parsing**, which stays a non-goal — a directory becomes a
//! folder carrying that title, nothing is inferred from how the title is
//! written), so `execute_prepared` now reads the tree once: every directory
//! becomes a folder record with matching parentage, and every file is filed
//! into the folder it was already found in. Only files genuinely loose at
//! the top level land in the Sorting Box. See docs/DESIGN.md#first-import.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use walkdir::WalkDir;

use crate::db;
use crate::error::Result;
use crate::fs::paths::LibraryPaths;
use crate::fs::walk;
use crate::jobs::worker;
use crate::sidecar::Tools;

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
// so `prepare` works the filesystem directly, the same way a plain `ls -R`
// would, and never opens a job queue or writes anything into `.ggallery/`.

/// Held in `AppState` between `prepare` and `execute_prepared`. Just the root
/// — `execute_prepared` re-walks the tree itself, which is cheap and, unlike
/// a stashed plan, never goes stale if something changed on disk between the
/// two calls.
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
    /// Directories that will become folder records — PLAN.md §M2.6a restored
    /// this once there was something real to count again.
    pub folder_count: i64,
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
    paths.migrate_legacy_dir()?;

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
        // No files means nothing worth a confirmation screen for — there is
        // no backup risk — but the tree may still hold folders worth
        // keeping (an empty directory made ahead of filling it), so this
        // runs the same tree-mirroring import quietly rather than jumping
        // straight to `mark_imported` and discarding them.
        let pending = PendingImport { root: root.to_path_buf() };
        execute_prepared(&pending, &mut |_| {})?;
        return Ok((ReviewReport { already_imported: true, ..Default::default() }, None));
    }

    let report = ReviewReport {
        already_imported: false,
        by_kind: scan.by_kind,
        total_items: scan.total_items,
        total_bytes: scan.total_bytes,
        folder_count: scan.total_folders,
        unreadable: scan.unreadable,
    };
    let pending = PendingImport { root: root.to_path_buf() };
    Ok((report, Some(pending)))
}

struct FsScan {
    by_kind: Vec<db::items::KindTotal>,
    total_bytes: i64,
    total_items: i64,
    total_folders: i64,
    unreadable: i64,
}

/// Mirrors the OS-litter filtering `fs::walk` applies to `inbox/`, without
/// reading a single byte of file content, so it stays fast over a few hundred
/// thousand files. Counts only — `execute_prepared` re-walks the tree itself
/// rather than trusting anything staged here.
fn scan_filesystem(paths: &LibraryPaths) -> Result<FsScan> {
    let mut kind_totals: HashMap<&'static str, (i64, i64)> = HashMap::new();
    let mut unreadable = 0i64;
    let mut total_bytes = 0i64;
    let mut total_items = 0i64;
    let mut total_folders = 0i64;

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
            // The root itself (depth 0) is never a folder — DESIGN.md §2
            // *Navigation roots*: it exists in the database for items with
            // nowhere else to belong, but is never presented, or counted,
            // as one the user made.
            if entry.depth() > 0 {
                total_folders += 1;
            }
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

    Ok(FsScan { by_kind, total_bytes, total_items, total_folders, unreadable })
}

/// A directory that also skips the app's own reserved top-level names, so a
/// second `prepare` after a partially-completed `execute_prepared` does not
/// recount what has already been indexed out of the tree.
fn is_skipped_dir(paths: &LibraryPaths, entry: &walkdir::DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }
    if paths.is_ggallery_dir(entry.path()) {
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
    pub folders: i64,
    pub errors: Vec<FsMoveError>,
}

/// The Progress screen's actual work, and PLAN.md §M2.6a's whole point: read
/// the directory tree once and turn it into folders rather than flattening
/// everything into the Sorting Box.
///
/// **Pass 1** walks every directory and creates its folder record, parent
/// before child — `WalkDir`'s default pre-order guarantees a directory is
/// yielded before anything inside it, so `folder_ids` already holds an
/// entry for a file or subdirectory's parent by the time it is reached.
/// Titles are lowercased like any other (PLAN.md decision 31); a sibling
/// that collides with one already created once folded is **merged onto the
/// existing record** (`db::folders::id_for` finds it) rather than
/// suffixed. Files are only collected here, not yet indexed, so the total
/// is known before the first progress callback.
///
/// **Pass 2** indexes every collected file — hash, probe, insert, shard,
/// via the same `jobs::worker::index_file` an inbox arrival uses — into the
/// folder its own directory resolved to above. A file directly at the root
/// (no directory of its own) gets `folder_id: None`: it was genuinely loose
/// and lands in the Sorting Box, exactly as DESIGN.md#first-import
/// specifies.
///
/// Resumable for free, the same way the old inbox-sweep was: a file already
/// indexed has already been moved out of the tree into `files/`, so a
/// second run over the same root simply does not find it again. An
/// already-created folder is found and reused, not recreated.
pub fn execute_prepared(
    pending: &PendingImport,
    on_progress: &mut dyn FnMut(&ImportProgress),
) -> Result<FsExecuteReport> {
    let paths = LibraryPaths::new(&pending.root);
    paths.ensure_dirs()?;

    let mut conn = db::open(&paths.db_path())?;
    db::migrate(&mut conn)?;
    let tools = Tools::discover();

    let mut folder_ids: HashMap<PathBuf, i64> = HashMap::new();
    let mut pending_files: Vec<PathBuf> = Vec::new();

    let walker = WalkDir::new(paths.root())
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !is_skipped_dir(&paths, entry));

    for entry in walker.flatten() {
        if entry.depth() == 0 {
            continue; // the root itself is never a folder — DESIGN.md §2
        }
        let path = entry.path().to_path_buf();

        if entry.file_type().is_dir() {
            let parent_id = path.parent().and_then(|parent| folder_ids.get(parent)).copied();
            let title = db::fold(&entry.file_name().to_string_lossy());
            let folder_id = match db::folders::id_for(&conn, parent_id, &title)? {
                Some(existing) => existing,
                None => db::folders::create_record(&conn, parent_id, &title)?,
            };
            folder_ids.insert(path, folder_id);
            continue;
        }

        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if walk::IGNORED_FILES.contains(&name.to_lowercase().as_str()) {
            continue;
        }
        pending_files.push(path);
    }

    let total = pending_files.len() as i64;
    let mut report = FsExecuteReport {
        moved: 0,
        folders: folder_ids.len() as i64,
        errors: Vec::new(),
    };

    for path in pending_files {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let folder_id = path.parent().and_then(|parent| folder_ids.get(parent)).copied();

        match worker::index_file(&paths, &tools, &mut conn, &path, name.clone(), folder_id) {
            Ok(_) => report.moved += 1,
            Err(err) => report.errors.push(FsMoveError { name, error: err.to_string() }),
        }
        on_progress(&ImportProgress { done: report.moved, total, errors: report.errors.len() as i64 });
    }

    db::settings::mark_imported(&conn)?;

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};

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
    fn prepare_finds_files_and_folders_needing_import() {
        let root = scratch("m26a-prepare");
        std::fs::create_dir_all(root.join("People/Ana")).unwrap();
        std::fs::write(root.join("People/Ana/holiday.jpg"), b"hello world").unwrap();
        std::fs::write(root.join("cover.png"), b"cover bytes").unwrap();

        let (report, pending) = prepare(&root).unwrap();
        assert!(!report.already_imported);
        assert_eq!(report.total_items, 2);
        assert_eq!(report.folder_count, 2, "People and People/Ana");
        assert!(pending.is_some());
    }

    fn open_migrated(root: &std::path::Path) -> (LibraryPaths, Connection) {
        let paths = LibraryPaths::new(root);
        let mut conn = db::open(&paths.db_path()).unwrap();
        db::migrate(&mut conn).unwrap();
        (paths, conn)
    }

    fn folder_id_at(conn: &Connection, path: &[&str]) -> i64 {
        let mut parent = None;
        let mut id = 0;
        for title in path {
            id = db::folders::id_for(conn, parent, &db::fold(title))
                .unwrap()
                .unwrap_or_else(|| panic!("no folder for {title:?} under {parent:?}"));
            parent = Some(id);
        }
        id
    }

    /// The actual bug PLAN.md §M2.6a exists to fix: several levels deep,
    /// files sitting at an intermediate level (not just the leaves), an
    /// empty directory, and one file genuinely loose at the root. Every
    /// directory becomes a folder with the matching parentage; only the
    /// loose file reaches the Sorting Box.
    #[test]
    fn execute_prepared_mirrors_a_multi_level_tree() {
        let root = scratch("m26a-mirror");
        std::fs::create_dir_all(root.join("People/Ana/Trip")).unwrap();
        std::fs::create_dir_all(root.join("People/Empty")).unwrap();
        std::fs::write(root.join("People/ana.jpg"), b"a file at an intermediate level").unwrap();
        std::fs::write(root.join("People/Ana/holiday.jpg"), b"hello world").unwrap();
        std::fs::write(root.join("People/Ana/Trip/photo1.jpg"), b"one").unwrap();
        std::fs::write(root.join("People/Ana/Trip/photo2.jpg"), b"two").unwrap();
        std::fs::write(root.join("cover.png"), b"loose at the root").unwrap();

        let (_, pending) = prepare(&root).unwrap();
        let report = execute_prepared(&pending.unwrap(), &mut |_| {}).unwrap();

        assert_eq!(report.moved, 5, "every file, including the loose one, is indexed");
        assert_eq!(report.folders, 4, "People, People/Ana, People/Ana/Trip, People/Empty");
        assert!(report.errors.is_empty());

        let (paths, conn) = open_migrated(&root);
        assert_eq!(db::folders::count(&conn).unwrap(), 4);

        let people = folder_id_at(&conn, &["People"]);
        let ana = folder_id_at(&conn, &["People", "Ana"]);
        let trip = folder_id_at(&conn, &["People", "Ana", "Trip"]);
        let _empty = folder_id_at(&conn, &["People", "Empty"]);

        let crumb: Vec<i64> = db::folders::breadcrumb(&conn, trip)
            .unwrap()
            .into_iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(crumb, vec![people, ana], "parentage is preserved several levels deep");

        // Titles fold lowercase like any other (PLAN.md decision 31).
        let title: String = conn
            .query_row("SELECT title FROM folder WHERE id = ?1", params![people], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "people");

        let items = db::items::list(&conn, &db::items::Scope::Unsorted).unwrap();
        assert_eq!(items.len(), 1, "only the file loose at the root is unfiled");

        let ana_direct: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM item WHERE folder_id = ?1",
                params![ana],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ana_direct, 1, "holiday.jpg only — ana.jpg belongs to People, photo*.jpg to Trip");

        // Nothing was left behind in `inbox/` — files were indexed and
        // sharded directly, not swept in and drained by the ordinary
        // pipeline.
        assert_eq!(std::fs::read_dir(paths.inbox_dir()).unwrap().count(), 0);

        assert!(db::settings::imported_at(&conn).unwrap().is_some());
    }

    #[test]
    fn a_second_execute_does_not_recreate_folders_or_reindex_files() {
        let root = scratch("m26a-resume");
        std::fs::create_dir_all(root.join("People/Ana")).unwrap();
        std::fs::write(root.join("People/Ana/holiday.jpg"), b"hello world").unwrap();
        std::fs::write(root.join("cover.png"), b"cover bytes").unwrap();

        let (_, pending) = prepare(&root).unwrap();
        let pending = pending.unwrap();
        let first = execute_prepared(&pending, &mut |_| {}).unwrap();
        assert_eq!(first.moved, 2);

        // Every file the first run indexed has already left the tree (moved
        // into `files/`), so a second run — as if the app had been pointed
        // at the same root again, or a crash mid-run were retried — finds
        // nothing left to do and recreates nothing.
        let second = execute_prepared(&pending, &mut |_| {}).unwrap();
        assert_eq!(second.moved, 0);
        assert_eq!(second.folders, 2, "People and People/Ana are both resolved again, but reused rather than recreated");
        assert!(second.errors.is_empty());

        let (_, conn) = open_migrated(&root);
        assert_eq!(db::folders::count(&conn).unwrap(), 2, "People and People/Ana — no duplicates");
        assert_eq!(db::items::count(&conn).unwrap(), 2);
    }

    /// PLAN.md §M2.6a: "siblings that collide once folded are merged, not
    /// suffixed." Two directories differing only by case can only coexist as
    /// siblings on disk with NTFS per-directory case sensitivity opted in —
    /// an obscure, normally WSL-only flag not worth exercising here just to
    /// prove this. Folding only ever collides on case (`db::fold` is a plain
    /// lowercase), so seeding a folder that already exists under a different
    /// case and then importing a same-titled-once-folded directory exercises
    /// the exact `id_for`-then-`create_record` lookup-or-create path
    /// `execute_prepared` uses, without needing two case-variant directories
    /// to exist at once.
    #[test]
    fn execute_prepared_merges_a_directory_that_collides_once_folded() {
        let root = scratch("m26a-case-merge");
        std::fs::create_dir_all(root.join("Trip")).unwrap();
        std::fs::write(root.join("Trip/a.jpg"), b"a").unwrap();

        let paths = LibraryPaths::new(&root);
        paths.ensure_dirs().unwrap();
        let existing = {
            let mut conn = db::open(&paths.db_path()).unwrap();
            db::migrate(&mut conn).unwrap();
            db::folders::create_record(&conn, None, "trip").unwrap()
        };

        let (_, pending) = prepare(&root).unwrap();
        let report = execute_prepared(&pending.unwrap(), &mut |_| {}).unwrap();

        assert_eq!(report.moved, 1);
        assert!(report.errors.is_empty());

        let (_, conn) = open_migrated(&root);
        assert_eq!(db::folders::count(&conn).unwrap(), 1, "Trip merged onto the existing trip, not duplicated");
        let direct: i64 = conn
            .query_row("SELECT COUNT(*) FROM item WHERE folder_id = ?1", params![existing], |r| r.get(0))
            .unwrap();
        assert_eq!(direct, 1, "the file landed in the pre-existing folder");
    }

    #[test]
    fn prepare_marks_an_empty_library_imported_with_no_ceremony() {
        let root = scratch("m26a-nothing-to-import");
        let (report, pending) = prepare(&root).unwrap();
        assert!(report.already_imported);
        assert!(pending.is_none());

        let paths = LibraryPaths::new(&root);
        let conn = db::open(&paths.db_path()).unwrap();
        assert!(db::settings::imported_at(&conn).unwrap().is_some());
    }

    #[test]
    fn prepare_imports_an_empty_directorys_folder_even_with_no_files_anywhere() {
        // Zero files skips the Review screen (no backup risk worth
        // interrupting for), but the tree can still hold folders worth
        // keeping — an empty directory made ahead of filling it.
        let root = scratch("m26a-empty-dirs-only");
        std::fs::create_dir_all(root.join("People/Empty")).unwrap();

        let (report, pending) = prepare(&root).unwrap();
        assert!(report.already_imported, "no files means no ceremony");
        assert!(pending.is_none());

        let (_, conn) = open_migrated(&root);
        assert_eq!(db::folders::count(&conn).unwrap(), 2, "People and People/Empty");
    }
}
