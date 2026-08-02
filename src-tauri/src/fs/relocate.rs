//! Folder and item relocation — create, rename-directory, move. Each public
//! function here is the whole orchestration for one user-facing operation:
//! disk first, then the database and the journal, because the filesystem is
//! authoritative (docs/DESIGN.md "Folder operations") — a failed disk
//! operation must never leave a database row claiming something that never
//! happened. Mirrors `fs::import::rename_on_arrival`'s shape: an `fs::`
//! function that takes `conn`, because writing the record of what it did is
//! part of the operation, not a separate step.
//!
//! Subtree path rewrites are never done here inline — see
//! `db::folders::rewrite_subtree_paths` and the `RENAME_FOLDER_SUBTREE` /
//! `MOVE_FOLDER_SUBTREE` jobs (`jobs::enqueue_rename_folder_subtree` /
//! `enqueue_move_folder_subtree`). Only the top folder's own row is updated
//! synchronously here — see docs/STRUCTURE.md and PLAN.md §M2.1, "Subtree
//! path rewrites are jobs, not synchronous commands."

use rusqlite::Connection;
use serde::Serialize;

use crate::db;
use crate::error::{AppError, Result};
use crate::fs::paths::{normalise_rel, parent_rel, LibraryPaths};
use crate::jobs;

/// Create a folder: the directory on disk, the record, and — if given — an
/// applied archetype. `parent_id` of `None` means directly under the
/// library root.
pub fn create_folder(
    paths: &LibraryPaths,
    conn: &Connection,
    parent_id: Option<i64>,
    name: &str,
    archetype_id: Option<i64>,
) -> Result<i64> {
    let parent_path = match parent_id {
        Some(id) => db::folders::rel_for(conn, id)?
            .ok_or_else(|| AppError::invalid("parent folder not found"))?,
        None => String::new(),
    };

    let parent_abs = paths.to_abs(&parent_path)?;
    let abs = parent_abs.join(name);
    if abs.exists() {
        return Err(AppError::invalid(format!(
            "{} already exists",
            abs.display()
        )));
    }
    std::fs::create_dir(&abs)?;

    let rel = join_rel(&parent_path, name);
    let id = match db::folders::create_record(conn, parent_id, &rel, name) {
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
    db::journal::record_folder_create(conn, id, parent_id, &rel)?;
    Ok(id)
}

/// Retitling (`db::folders::set_title`) never reaches here — it touches the
/// record only. This is the directory move: physically renames the folder
/// on disk and rewrites every descendant's `rel_path`.
pub fn rename_folder_dir(
    paths: &LibraryPaths,
    conn: &Connection,
    folder_id: i64,
    new_name: &str,
) -> Result<()> {
    let (old_rel, parent_id) = folder_location(conn, folder_id)?;
    if parent_id.is_none() {
        return Err(AppError::invalid("the library root can't be renamed"));
    }
    let parent_path = parent_rel(&old_rel).unwrap_or_default();

    let parent_abs = paths.to_abs(&parent_path)?;
    let new_abs = parent_abs.join(new_name);
    if new_abs.exists() {
        return Err(AppError::invalid(format!(
            "{} already exists",
            new_abs.display()
        )));
    }
    let old_abs = paths.to_abs(&old_rel)?;
    std::fs::rename(&old_abs, &new_abs)?;

    let new_rel = join_rel(&parent_path, new_name);
    db::folders::set_rel_path(conn, folder_id, &new_rel)?;
    jobs::enqueue_rename_folder_subtree(conn, &old_rel, &new_rel)?;
    db::journal::record_folder_rename_dir(conn, folder_id, &old_rel, &new_rel)?;
    Ok(())
}

/// Moves a folder to a new parent — descendant paths and the effective-tag
/// cache both follow, because inherited tags are recomputed from the new
/// ancestry (docs/DESIGN.md "Folder operations").
pub fn move_folder(
    paths: &LibraryPaths,
    conn: &Connection,
    folder_id: i64,
    new_parent_id: Option<i64>,
) -> Result<()> {
    let (old_rel, parent_id) = folder_location(conn, folder_id)?;
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

    let name = old_rel.rsplit('/').next().unwrap_or(&old_rel).to_string();
    let new_parent_abs = paths.to_abs(&new_parent_path)?;
    let new_abs = new_parent_abs.join(&name);
    if new_abs.exists() {
        return Err(AppError::invalid(format!(
            "{} already exists",
            new_abs.display()
        )));
    }
    let old_abs = paths.to_abs(&old_rel)?;
    std::fs::rename(&old_abs, &new_abs)?;

    let new_rel = join_rel(&new_parent_path, &name);
    db::folders::set_parent_and_rel_path(conn, folder_id, new_parent_id, &new_rel)?;
    jobs::enqueue_move_folder_subtree(conn, &old_rel, &new_rel)?;
    db::journal::record_folder_move(conn, folder_id, parent_id, new_parent_id, &old_rel, &new_rel)?;
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

    let mut report = MoveItemsReport::default();
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

fn folder_location(conn: &Connection, folder_id: i64) -> Result<(String, Option<i64>)> {
    conn.query_row(
        "SELECT rel_path, parent_id FROM folder WHERE id = ?1 AND deleted_at IS NULL",
        [folder_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
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
