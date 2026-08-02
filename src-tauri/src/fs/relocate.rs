//! Folder creation, retitling (which is now the only rename — see PLAN.md
//! §M2.2), move, and item move. Each public function here is the whole
//! orchestration for one user-facing operation: disk first, then the
//! database and the journal, because the filesystem is authoritative
//! (docs/DESIGN.md "Folder operations") — a failed disk operation must
//! never leave a database row claiming something that never happened.
//! Mirrors `fs::import::rename_on_arrival`'s shape: an `fs::` function that
//! takes `conn`, because writing the record of what it did is part of the
//! operation, not a separate step.
//!
//! Subtree path rewrites are never done here inline — see
//! `db::folders::rewrite_subtree_paths` and the `RENAME_FOLDER_SUBTREE` /
//! `MOVE_FOLDER_SUBTREE` jobs (`jobs::enqueue_rename_folder_subtree` /
//! `enqueue_move_folder_subtree`). Only the top folder's own row is updated
//! synchronously here — see docs/STRUCTURE.md and PLAN.md §M2.1, "Subtree
//! path rewrites are jobs, not synchronous commands."

use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;

use crate::db;
use crate::error::{AppError, Result};
use crate::fs::paths::{normalise_rel, parent_rel, LibraryPaths};
use crate::fs::watch::Suppressor;
use crate::jobs;

// --- folder naming (M2.2 — docs/DESIGN.md §1 "Folder names") --------------
//
// A folder has one name. The title is what the user types; the directory on
// disk is derived from it, sanitised for Windows. These functions are the
// whole of that derivation — `retitle_folder` and `create_folder` are the
// only callers, and the watcher's `handle_dir_renamed` calls
// `sanitise_folder_name` alone to check whether an externally-renamed
// directory still matches what the current title would produce.

const FORBIDDEN: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

const RESERVED_DEVICE_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Long enough that no real title is ever visibly truncated, short enough
/// that a library nested a dozen folders deep still fits under Windows'
/// `MAX_PATH`-adjacent limits even without long-path support enabled.
const MAX_SEGMENT_LEN: usize = 100;

/// `title`, made safe as a single Windows path segment. Can return an empty
/// string (a title that is nothing but forbidden characters) — callers
/// decide what an empty result means for them; see `retitle_folder`'s
/// "keep the previous name" rule and `create_folder`'s fallback.
pub fn sanitise_folder_name(title: &str) -> String {
    let mut name: String = title
        .chars()
        .map(|c| if FORBIDDEN.contains(&c) { '-' } else { c })
        .collect();
    name = trim_trailing_dots_and_spaces(&name);

    if name.chars().count() > MAX_SEGMENT_LEN {
        name = name.chars().take(MAX_SEGMENT_LEN).collect();
        // Truncation can itself expose a trailing dot or space that was
        // previously mid-string.
        name = trim_trailing_dots_and_spaces(&name);
    }

    if RESERVED_DEVICE_NAMES.iter().any(|r| r.eq_ignore_ascii_case(&name)) {
        name.push('_');
    }

    name
}

fn trim_trailing_dots_and_spaces(name: &str) -> String {
    name.trim_end_matches(['.', ' ']).to_string()
}

/// `candidate`, or `candidate (2)`, `candidate (3)`… if that name is already
/// occupied at `parent_abs` by something other than `exclude_self` (the
/// folder's own current directory, when this is a retitle rather than a
/// create — a case-only rename would otherwise see its own directory as a
/// collision with itself, since Windows paths are case-insensitive).
fn unique_sibling_name(parent_abs: &Path, candidate: &str, exclude_self: Option<&Path>) -> Result<String> {
    let self_canon = exclude_self.and_then(|p| std::fs::canonicalize(p).ok());
    let mut name = candidate.to_string();
    let mut suffix = 2;
    loop {
        let target = parent_abs.join(&name);
        let occupied = match std::fs::canonicalize(&target) {
            Ok(canon) => self_canon.as_ref() != Some(&canon),
            Err(_) => false, // does not exist — free
        };
        if !occupied {
            return Ok(name);
        }
        name = format!("{candidate} ({suffix})");
        suffix += 1;
        if suffix > 1000 {
            return Err(AppError::invalid("could not find a free name for this folder"));
        }
    }
}

/// Windows' case-insensitive filesystem treats a rename that only changes
/// case as a no-op unless done in two steps. Both legs — and the whole
/// operation's start and end paths — are suppressed so the watcher never
/// feeds this back to itself; see `Suppressor`.
fn rename_case_safe(
    suppressor: &Suppressor,
    paths: &LibraryPaths,
    old_abs: &Path,
    new_abs: &Path,
) -> Result<()> {
    if old_abs == new_abs {
        return Ok(());
    }

    suppressor.suppress(paths, old_abs);
    suppressor.suppress(paths, new_abs);

    let case_only_change = old_abs.parent() == new_abs.parent()
        && old_abs
            .file_name()
            .zip(new_abs.file_name())
            .map(|(a, b)| a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase())
            .unwrap_or(false);

    if case_only_change {
        // A dot-prefixed name is already excluded from the walk and the
        // watcher (`walk::is_skipped_dir`, `watch::is_hidden`) — belt and
        // suspenders alongside the explicit suppression above.
        let tmp = old_abs.with_file_name(format!(".ggallery-rename-{}", uuid::Uuid::new_v4().simple()));
        suppressor.suppress(paths, &tmp);
        std::fs::rename(old_abs, &tmp)?;
        std::fs::rename(&tmp, new_abs)?;
        return Ok(());
    }

    std::fs::rename(old_abs, new_abs)?;
    Ok(())
}

// --- folder lifecycle -------------------------------------------------

/// Create a folder: the directory on disk, the record, and — if given — an
/// applied archetype. `parent_id` of `None` means directly under the
/// library root. `title` is stored verbatim — never sanitised itself, only
/// the directory name derived from it is.
pub fn create_folder(
    paths: &LibraryPaths,
    conn: &Connection,
    parent_id: Option<i64>,
    title: &str,
    archetype_id: Option<i64>,
    batch_id: &str,
) -> Result<i64> {
    let parent_path = match parent_id {
        Some(id) => db::folders::rel_for(conn, id)?
            .ok_or_else(|| AppError::invalid("parent folder not found"))?,
        None => String::new(),
    };
    let parent_abs = paths.to_abs(&parent_path)?;

    let sanitised = sanitise_folder_name(title);
    // A freshly created folder has no "previous name" to fall back on the
    // way a retitle does when the new title sanitises to nothing — it needs
    // *some* name to exist on disk at all.
    let base_name = if sanitised.is_empty() { "untitled".to_string() } else { sanitised };
    let dir_name = unique_sibling_name(&parent_abs, &base_name, None)?;

    let abs = parent_abs.join(&dir_name);
    std::fs::create_dir(&abs)?;

    let rel = join_rel(&parent_path, &dir_name);
    let id = match db::folders::create_record(conn, parent_id, &rel, title) {
        Ok(id) => id,
        Err(err) => {
            // Roll the disk operation back — a DB error here (e.g. a race
            // with the walker) must not leave an orphan directory the user
            // never asked for and the app doesn't know about.
            let _ = std::fs::remove_dir(&abs);
            return Err(err);
        }
    };

    if let Some(archetype_id) = archetype_id {
        db::folders::apply_archetype(conn, id, archetype_id)?;
    }
    db::journal::record_folder_create(conn, batch_id, id, parent_id, &rel)?;
    Ok(id)
}

/// The only rename there is (PLAN.md §M2.2). Always updates the title;
/// renames the directory to match whenever the new title sanitises to a
/// real name that differs from what is already on disk. Reuses
/// `RENAME_FOLDER_SUBTREE` — every retitle is now a subtree path rewrite,
/// so it must not run on the command thread any more than a standalone
/// directory rename would have.
pub fn retitle_folder(
    paths: &LibraryPaths,
    conn: &Connection,
    suppressor: &Suppressor,
    folder_id: i64,
    new_title: &str,
    batch_id: &str,
) -> Result<()> {
    let (old_rel, parent_id, old_title) = folder_location(conn, folder_id)?;

    if old_title != new_title {
        db::folders::set_title(conn, folder_id, new_title, batch_id)?;
    }

    if parent_id.is_none() {
        return Ok(()); // the library root has no directory of its own
    }

    let sanitised = sanitise_folder_name(new_title);
    if sanitised.is_empty() {
        // "the directory keeps its previous name and the title still
        // changes" — docs/DESIGN.md §1 "Folder names".
        return Ok(());
    }

    let parent_path = parent_rel(&old_rel).unwrap_or_default();
    let parent_abs = paths.to_abs(&parent_path)?;
    let old_abs = paths.to_abs(&old_rel)?;

    // `rel_path` is normalised (lower-cased), so it can't tell a
    // collision-suffixed or case-preserved name from the plain sanitised
    // one — `canonicalize` recovers what is actually on disk.
    let true_old_abs = std::fs::canonicalize(&old_abs)?;
    let current_name = true_old_abs
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    if current_name == sanitised {
        return Ok(()); // already exactly right — nothing to do on disk
    }

    let target_name = unique_sibling_name(&parent_abs, &sanitised, Some(&true_old_abs))?;
    let new_abs = parent_abs.join(&target_name);

    rename_case_safe(suppressor, paths, &old_abs, &new_abs)?;

    let new_rel = join_rel(&parent_path, &target_name);
    db::folders::set_rel_path(conn, folder_id, &new_rel)?;
    jobs::enqueue_rename_folder_subtree(conn, &old_rel, &new_rel)?;
    db::journal::record_folder_rename_dir(conn, batch_id, folder_id, &old_rel, &new_rel)?;
    Ok(())
}

/// Moves a folder to a new parent — descendant paths and the effective-tag
/// cache both follow, because inherited tags are recomputed from the new
/// ancestry (docs/DESIGN.md "Folder operations"). The directory keeps its
/// own name; only its parent changes.
pub fn move_folder(
    paths: &LibraryPaths,
    conn: &Connection,
    folder_id: i64,
    new_parent_id: Option<i64>,
    batch_id: &str,
) -> Result<()> {
    move_folder_inner(paths, conn, folder_id, new_parent_id, Some(batch_id))
}

/// The same move, without a journal entry — what `fs::undo` calls to put a
/// folder back. A reversal is not itself an operation to reverse.
pub fn move_folder_unjournalled(
    paths: &LibraryPaths,
    conn: &Connection,
    folder_id: i64,
    new_parent_id: Option<i64>,
) -> Result<()> {
    move_folder_inner(paths, conn, folder_id, new_parent_id, None)
}

fn move_folder_inner(
    paths: &LibraryPaths,
    conn: &Connection,
    folder_id: i64,
    new_parent_id: Option<i64>,
    batch_id: Option<&str>,
) -> Result<()> {
    let (old_rel, parent_id, title) = folder_location(conn, folder_id)?;
    if parent_id.is_none() {
        return Err(AppError::invalid("the library root can't be moved"));
    }
    if new_parent_id == Some(folder_id) {
        return Err(AppError::invalid("can't move a folder into itself"));
    }
    let new_parent_path = match new_parent_id {
        Some(id) => db::folders::rel_for(conn, id)?
            .ok_or_else(|| AppError::invalid("destination folder not found"))?,
        None => String::new(),
    };
    if new_parent_path == old_rel || new_parent_path.starts_with(&format!("{old_rel}/")) {
        return Err(AppError::invalid(
            "can't move a folder into itself or a descendant",
        ));
    }
    if parent_id == new_parent_id {
        return Ok(()); // already there
    }

    let old_abs = paths.to_abs(&old_rel)?;
    // The name on disk is derived from the title (M2.2), not read back from
    // the lower-cased `rel_path` — recovering it that way would silently
    // lower-case the directory on every move. Falls back to whatever is
    // actually there, same as a retitle whose title sanitises to nothing.
    let sanitised = sanitise_folder_name(&title);
    let name = if !sanitised.is_empty() {
        sanitised
    } else {
        std::fs::canonicalize(&old_abs)?
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .ok_or_else(|| AppError::invalid("could not resolve the folder's name"))?
    };

    let new_parent_abs = paths.to_abs(&new_parent_path)?;
    let new_abs = new_parent_abs.join(&name);
    if new_abs.exists() {
        return Err(AppError::invalid(format!(
            "{} already exists",
            new_abs.display()
        )));
    }
    std::fs::rename(&old_abs, &new_abs)?;

    let new_rel = join_rel(&new_parent_path, &name);
    db::folders::set_parent_and_rel_path(conn, folder_id, new_parent_id, &new_rel)?;
    jobs::enqueue_move_folder_subtree(conn, &old_rel, &new_rel)?;
    if let Some(batch_id) = batch_id {
        db::journal::record_folder_move(
            conn,
            batch_id,
            folder_id,
            parent_id,
            new_parent_id,
            &old_rel,
            &new_rel,
        )?;
    }
    Ok(())
}

/// One item that couldn't be moved, with the error the filesystem or the
/// database actually gave.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemOpError {
    pub item_id: i64,
    pub error: String,
}

/// What `move_items` reports back — moved count plus any per-item failures,
/// so one locked file in a large selection doesn't cost every other item.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveItemsReport {
    pub moved: i64,
    pub errors: Vec<ItemOpError>,
    /// The journal batch this move wrote — what the toast's Undo button
    /// hands back to `undo_batch`. Empty only when nothing moved.
    pub batch_id: String,
}

/// Move a batch of items into `dest_folder_id`, sharing `batch_id` so one
/// future undo covers the whole selection.
pub fn move_items(
    paths: &LibraryPaths,
    conn: &Connection,
    item_ids: &[i64],
    dest_folder_id: i64,
    batch_id: &str,
) -> Result<MoveItemsReport> {
    let dest_rel = db::folders::rel_for(conn, dest_folder_id)?
        .ok_or_else(|| AppError::invalid("destination folder not found"))?;

    let mut report = MoveItemsReport {
        batch_id: batch_id.to_string(),
        ..Default::default()
    };
    for &item_id in item_ids {
        match move_one_item(paths, conn, item_id, dest_folder_id, &dest_rel, batch_id) {
            Ok(()) => report.moved += 1,
            Err(err) => report.errors.push(ItemOpError {
                item_id,
                error: err.to_string(),
            }),
        }
    }
    Ok(report)
}

fn move_one_item(
    paths: &LibraryPaths,
    conn: &Connection,
    item_id: i64,
    dest_folder_id: i64,
    dest_rel: &str,
    batch_id: &str,
) -> Result<()> {
    let item = db::items::rename_target(conn, item_id)?
        .ok_or_else(|| AppError::invalid("item no longer exists"))?;
    if item.folder_rel == dest_rel {
        return Ok(()); // already there
    }
    let from_folder_id = db::items::folder_id_of(conn, item_id)?
        .ok_or_else(|| AppError::invalid("item no longer exists"))?;

    let old_abs = paths.item_path(&item.folder_rel, &item.disk_name)?;
    let new_abs = paths.item_path(dest_rel, &item.disk_name)?;
    if new_abs.exists() {
        return Err(AppError::invalid(format!(
            "{} already exists at the destination",
            item.disk_name
        )));
    }
    std::fs::rename(&old_abs, &new_abs)?;

    db::items::set_folder(conn, item_id, dest_folder_id)?;
    jobs::enqueue_retag_item(conn, item_id)?;
    db::journal::record_item_move(conn, batch_id, item_id, from_folder_id, dest_folder_id)?;
    Ok(())
}

/// `(rel_path, parent_id, title)` for a live folder.
fn folder_location(conn: &Connection, folder_id: i64) -> Result<(String, Option<i64>, String)> {
    conn.query_row(
        "SELECT rel_path, parent_id, title FROM folder WHERE id = ?1 AND deleted_at IS NULL",
        [folder_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .map_err(|_| AppError::invalid("folder not found"))
}

fn join_rel(parent_rel: &str, name: &str) -> String {
    let joined = if parent_rel.is_empty() {
        name.to_string()
    } else {
        format!("{parent_rel}/{name}")
    };
    normalise_rel(&joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forbidden_characters_become_dashes() {
        assert_eq!(sanitise_folder_name("a/b\\c:d*e?f\"g<h>i|j"), "a-b-c-d-e-f-g-h-i-j");
    }

    #[test]
    fn trailing_dots_and_spaces_are_stripped() {
        assert_eq!(sanitise_folder_name("Ana..  "), "Ana");
        // The whole trailing run goes, not just one occurrence.
        assert_eq!(sanitise_folder_name("Ana. . "), "Ana");
        // A dot or space in the *middle* is untouched — only trailing ones
        // are a Windows quirk.
        assert_eq!(sanitise_folder_name("Ana Maria"), "Ana Maria");
    }

    #[test]
    fn reserved_device_names_get_a_trailing_underscore() {
        for reserved in ["CON", "con", "PRN", "AUX", "NUL", "COM1", "lpt9"] {
            let sanitised = sanitise_folder_name(reserved);
            assert_eq!(sanitised, format!("{reserved}_"));
        }
        // Not a reserved name on its own — must not be touched.
        assert_eq!(sanitise_folder_name("CONcert"), "CONcert");
    }

    #[test]
    fn long_titles_are_capped() {
        let title = "a".repeat(500);
        let sanitised = sanitise_folder_name(&title);
        assert_eq!(sanitised.chars().count(), MAX_SEGMENT_LEN);
    }

    #[test]
    fn capping_that_exposes_a_trailing_space_trims_it_too() {
        let mut title = "a".repeat(MAX_SEGMENT_LEN - 1);
        title.push(' ');
        title.push('b'); // the space now lands exactly at the cap boundary
        let sanitised = sanitise_folder_name(&title);
        assert!(!sanitised.ends_with(' '));
    }

    #[test]
    fn forbidden_characters_are_replaced_not_dropped() {
        // "///" is not empty once sanitised — each `/` becomes a `-`, a
        // perfectly valid directory name. Only trailing dots/spaces are
        // ever *removed* outright.
        assert_eq!(sanitise_folder_name("///"), "---");
    }

    #[test]
    fn a_title_of_only_trailing_dots_and_spaces_sanitises_to_nothing() {
        assert_eq!(sanitise_folder_name("..."), "");
        assert_eq!(sanitise_folder_name("   "), "");
        assert_eq!(sanitise_folder_name(""), "");
    }

    #[test]
    fn unique_sibling_name_appends_a_counter_on_collision() {
        let root = scratch("sanitise-collision");
        std::fs::create_dir_all(root.join("Ana")).unwrap();
        std::fs::create_dir_all(root.join("Ana (2)")).unwrap();

        let name = unique_sibling_name(&root, "Ana", None).unwrap();
        assert_eq!(name, "Ana (3)");
    }

    #[test]
    fn unique_sibling_name_excludes_its_own_directory() {
        let root = scratch("sanitise-self-exclude");
        std::fs::create_dir_all(root.join("Ana")).unwrap();

        // Renaming "Ana" to "Ana" (no real change) must not be treated as a
        // collision against itself.
        let name = unique_sibling_name(&root, "Ana", Some(&root.join("Ana"))).unwrap();
        assert_eq!(name, "Ana");
    }

    // --- retitle_folder: the collapsed title+directory rename -------------

    fn open_db(root: &std::path::Path) -> (LibraryPaths, Connection) {
        let paths = LibraryPaths::new(root);
        paths.ensure_dirs().unwrap();
        let mut conn = db::open(&paths.db_path()).unwrap();
        db::migrate(&mut conn).unwrap();
        // Every folder used in these tests needs a real parent to rename
        // relative to — without a root row, `upsert("ana", ..)` resolves
        // its own `parent_id` to `None` and reads as the library root
        // itself, which `retitle_folder` deliberately never renames.
        db::folders::upsert(&conn, "", "Library").unwrap();
        (paths, conn)
    }

    #[test]
    fn retitling_renames_the_directory_and_rewrites_descendants() {
        let root = scratch("retitle-basic");
        std::fs::create_dir_all(root.join("Ana/2024 Trip")).unwrap();
        let (paths, conn) = open_db(&root);
        let ana = db::folders::upsert(&conn, "ana", "Ana").unwrap();
        let trip = db::folders::upsert(&conn, "ana/2024 trip", "2024 Trip").unwrap();

        retitle_folder(&paths, &conn, &Suppressor::default(), ana, "Anastasia", &db::journal::new_batch()).unwrap();

        assert!(root.join("Anastasia").is_dir());
        assert!(!root.join("Ana").exists());

        let detail = db::folders::get_detail(&conn, ana).unwrap().unwrap();
        assert_eq!(detail.title, "Anastasia");
        assert_eq!(detail.rel_path, "anastasia");

        // The descendant's own row is not rewritten synchronously — that is
        // the RENAME_FOLDER_SUBTREE job's work.
        let stale = db::folders::get_detail(&conn, trip).unwrap().unwrap();
        assert_eq!(stale.rel_path, "ana/2024 trip", "job has not run yet");

        db::folders::rewrite_subtree_paths(&conn, "ana", "anastasia").unwrap();
        let fixed = db::folders::get_detail(&conn, trip).unwrap().unwrap();
        assert_eq!(fixed.rel_path, "anastasia/2024 trip");
    }

    #[test]
    fn retitling_journals_and_enqueues_the_subtree_job() {
        let root = scratch("retitle-journal");
        std::fs::create_dir_all(root.join("Ana")).unwrap();
        let (paths, conn) = open_db(&root);
        let ana = db::folders::upsert(&conn, "ana", "Ana").unwrap();

        retitle_folder(&paths, &conn, &Suppressor::default(), ana, "Anastasia", &db::journal::new_batch()).unwrap();

        let dir_renames: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM journal WHERE op = 'folder_rename_dir'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(dir_renames, 1);
        let title_renames: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM journal WHERE op = 'folder_rename_title'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(title_renames, 1);

        let queued: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM job WHERE type = 'rename_folder_subtree'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(queued, 1);
    }

    #[test]
    fn retitling_to_something_that_sanitises_to_the_same_name_touches_nothing_on_disk() {
        let root = scratch("retitle-no-op");
        std::fs::create_dir_all(root.join("Ana-Trip")).unwrap();
        let (paths, conn) = open_db(&root);
        let ana = db::folders::upsert(&conn, "ana-trip", "Ana/Trip").unwrap();

        // "Ana:Trip" sanitises to "Ana-Trip", identical to what's already
        // on disk (from "Ana/Trip") — only the title should move.
        retitle_folder(&paths, &conn, &Suppressor::default(), ana, "Ana:Trip", &db::journal::new_batch()).unwrap();

        assert!(root.join("Ana-Trip").is_dir());
        let detail = db::folders::get_detail(&conn, ana).unwrap().unwrap();
        assert_eq!(detail.title, "Ana:Trip");
        assert_eq!(detail.rel_path, "ana-trip", "directory was not touched");
    }

    #[test]
    fn retitling_to_a_title_that_sanitises_to_nothing_keeps_the_old_directory_name() {
        let root = scratch("retitle-empty-sanitise");
        std::fs::create_dir_all(root.join("Ana")).unwrap();
        let (paths, conn) = open_db(&root);
        let ana = db::folders::upsert(&conn, "ana", "Ana").unwrap();

        retitle_folder(&paths, &conn, &Suppressor::default(), ana, "...", &db::journal::new_batch()).unwrap();

        assert!(root.join("Ana").is_dir(), "directory keeps its previous name");
        let detail = db::folders::get_detail(&conn, ana).unwrap().unwrap();
        assert_eq!(detail.title, "...", "the title still changes");
        assert_eq!(detail.rel_path, "ana");
    }

    #[test]
    fn retitling_into_a_sibling_collision_appends_a_counter_but_leaves_the_title_alone() {
        let root = scratch("retitle-collision");
        std::fs::create_dir_all(root.join("Ana")).unwrap();
        std::fs::create_dir_all(root.join("Bob")).unwrap();
        let (paths, conn) = open_db(&root);
        db::folders::upsert(&conn, "ana", "Ana").unwrap();
        let bob = db::folders::upsert(&conn, "bob", "Bob").unwrap();

        retitle_folder(&paths, &conn, &Suppressor::default(), bob, "Ana", &db::journal::new_batch()).unwrap();

        assert!(root.join("Ana (2)").is_dir());
        let detail = db::folders::get_detail(&conn, bob).unwrap().unwrap();
        assert_eq!(detail.title, "Ana", "the title is untouched by the suffix");
        assert_eq!(detail.rel_path, "ana (2)");
    }

    #[test]
    fn a_case_only_retitle_actually_changes_the_case_on_disk() {
        let root = scratch("retitle-case-only");
        std::fs::create_dir_all(root.join("Ana")).unwrap();
        let (paths, conn) = open_db(&root);
        let ana = db::folders::upsert(&conn, "ana", "Ana").unwrap();

        retitle_folder(&paths, &conn, &Suppressor::default(), ana, "ANA", &db::journal::new_batch()).unwrap();

        let on_disk = std::fs::canonicalize(root.join("ana")).unwrap();
        assert_eq!(
            on_disk.file_name().and_then(|n| n.to_str()),
            Some("ANA"),
            "a same-case-insensitive rename must not silently no-op on Windows"
        );
        let detail = db::folders::get_detail(&conn, ana).unwrap().unwrap();
        assert_eq!(detail.title, "ANA");
    }

    #[test]
    fn retitling_suppresses_its_own_rename_so_the_watcher_ignores_it() {
        let root = scratch("retitle-suppression");
        std::fs::create_dir_all(root.join("Ana")).unwrap();
        let (paths, conn) = open_db(&root);
        let ana = db::folders::upsert(&conn, "ana", "Ana").unwrap();
        let suppressor = Suppressor::default();

        retitle_folder(&paths, &conn, &suppressor, ana, "Anastasia", &db::journal::new_batch()).unwrap();

        assert!(suppressor.is_suppressed(&paths, &root.join("Ana")));
        assert!(suppressor.is_suppressed(&paths, &root.join("Anastasia")));
    }

    #[test]
    fn moving_a_folder_preserves_its_case_derived_from_the_title() {
        // Before M2.2, the directory name for a move was read back from the
        // lower-cased `rel_path`, which would have silently lower-cased
        // "Ana" to "ana" on every move. The name is derived from the title
        // now, the same as a retitle.
        let root = scratch("move-preserves-case");
        std::fs::create_dir_all(root.join("Ana")).unwrap();
        std::fs::create_dir_all(root.join("People")).unwrap();
        let (paths, conn) = open_db(&root);
        let ana = db::folders::upsert(&conn, "ana", "Ana").unwrap();
        let people = db::folders::upsert(&conn, "people", "People").unwrap();

        move_folder(&paths, &conn, ana, Some(people), &db::journal::new_batch()).unwrap();

        assert!(root.join("People/Ana").is_dir());
        let detail = db::folders::get_detail(&conn, ana).unwrap().unwrap();
        assert_eq!(detail.rel_path, "people/ana");
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-libraries")
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create scratch dir");
        root
    }
}
