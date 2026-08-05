//! Folder creation, retitling, move, and item move.
//!
//! **PLAN.md §M2.6 removed the whole directory half of this module.** Before
//! decision 30, every function here was disk-first-then-database, because
//! the filesystem was authoritative — a folder had a real directory, moving
//! or renaming it meant moving or renaming that directory, and a failed disk
//! operation had to happen before any row could claim it succeeded. None of
//! that exists any more: a folder's identity is its `id`, its hierarchy is
//! `parent_id`, and an item's file location is a pure function of its own
//! uuid, entirely independent of which folder — if any — it is filed in. So
//! every operation below is now a plain database write. `create_folder`
//! creates a record. `retitle_folder` sets a column. `move_folder` sets a
//! column and enqueues the effective-tag rebuild the new ancestry needs.
//! `move_items` sets a column, once per item — no rename, no collision check
//! against a destination directory, because there is no destination
//! directory.
//!
//! What used to need sanitising, reserved-device-name checks, `MAX_PATH`
//! caps and sibling-collision suffixes needs none of it: a title is free
//! text, and the one real constraint — no two live siblings sharing a title —
//! is enforced by `UNIQUE(parent_id, title)` itself.

use rusqlite::Connection;
use serde::Serialize;

use crate::db;
use crate::error::{AppError, Result};

// --- folder lifecycle -------------------------------------------------

/// Create a folder: the record, and — if given — an applied archetype.
/// `parent_id` of `None` means a top-level folder.
pub fn create_folder(
    conn: &Connection,
    parent_id: Option<i64>,
    title: &str,
    archetype_id: Option<i64>,
    batch_id: &str,
) -> Result<i64> {
    let id = db::folders::create_record(conn, parent_id, title)?;
    if let Some(archetype_id) = archetype_id {
        db::folders::apply_archetype(conn, id, archetype_id)?;
    }
    db::journal::record_folder_create(conn, batch_id, id, parent_id)?;
    Ok(id)
}

/// The only rename there is: one column. See `db::folders::set_title`.
pub fn retitle_folder(conn: &Connection, folder_id: i64, new_title: &str, batch_id: &str) -> Result<()> {
    db::folders::set_title(conn, folder_id, new_title, batch_id)
}

/// Moves a folder to a new parent — the effective-tag cache rebuilds for the
/// subtree, because inherited tags are recomputed from the new ancestry
/// (docs/DESIGN.md "Folder operations").
pub fn move_folder(conn: &Connection, folder_id: i64, new_parent_id: Option<i64>, batch_id: &str) -> Result<()> {
    move_folder_inner(conn, folder_id, new_parent_id, Some(batch_id))
}

/// The same move, without a journal entry — what `fs::undo` calls to put a
/// folder back. A reversal is not itself an operation to reverse.
pub fn move_folder_unjournalled(conn: &Connection, folder_id: i64, new_parent_id: Option<i64>) -> Result<()> {
    move_folder_inner(conn, folder_id, new_parent_id, None)
}

fn move_folder_inner(
    conn: &Connection,
    folder_id: i64,
    new_parent_id: Option<i64>,
    batch_id: Option<&str>,
) -> Result<()> {
    let (old_parent_id, title) = folder_location(conn, folder_id)?;
    if new_parent_id == Some(folder_id) {
        return Err(AppError::invalid("can't move a folder into itself"));
    }
    if let Some(new_parent_id) = new_parent_id {
        if is_or_descends_from(conn, new_parent_id, folder_id)? {
            return Err(AppError::invalid("can't move a folder into itself or a descendant"));
        }
    }
    if old_parent_id == new_parent_id {
        return Ok(()); // already there
    }
    if db::folders::id_for(conn, new_parent_id, &title)?.is_some() {
        return Err(AppError::invalid(format!("a folder named '{title}' already exists there")));
    }

    db::folders::set_parent(conn, folder_id, new_parent_id)?;
    crate::jobs::enqueue_retag_folder(conn, Some(folder_id))?;
    if let Some(batch_id) = batch_id {
        db::journal::record_folder_move(conn, batch_id, folder_id, old_parent_id, new_parent_id)?;
    }
    Ok(())
}

/// True if `candidate` is `target` itself or lies anywhere beneath it —
/// what stops a folder from being moved into its own subtree.
fn is_or_descends_from(conn: &Connection, candidate: i64, target: i64) -> Result<bool> {
    Ok(conn.query_row(
        "WITH RECURSIVE ancestry(id) AS (
             SELECT ?1
           UNION ALL
             SELECT f.parent_id FROM folder f JOIN ancestry a ON f.id = a.id
             WHERE f.parent_id IS NOT NULL
         )
         SELECT EXISTS(SELECT 1 FROM ancestry WHERE id = ?2)",
        rusqlite::params![candidate, target],
        |r| r.get(0),
    )?)
}

/// One item that couldn't be moved, with the error the database actually
/// gave.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemOpError {
    pub item_id: i64,
    pub error: String,
}

/// What `move_items` reports back — moved count plus any per-item failures,
/// so one already-gone item in a large selection doesn't cost every other
/// item.
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
/// future undo covers the whole selection. A pure `folder_id` write — a
/// file's location never depended on its folder to begin with.
pub fn move_items(conn: &Connection, item_ids: &[i64], dest_folder_id: i64, batch_id: &str) -> Result<MoveItemsReport> {
    let mut report = MoveItemsReport {
        batch_id: batch_id.to_string(),
        ..Default::default()
    };
    for &item_id in item_ids {
        match move_one_item(conn, item_id, dest_folder_id, batch_id) {
            Ok(()) => report.moved += 1,
            Err(err) => report.errors.push(ItemOpError { item_id, error: err.to_string() }),
        }
    }
    Ok(report)
}

fn move_one_item(conn: &Connection, item_id: i64, dest_folder_id: i64, batch_id: &str) -> Result<()> {
    let from_folder_id = db::items::folder_id_of(conn, item_id)?
        .ok_or_else(|| AppError::invalid("item no longer exists"))?;
    if from_folder_id == Some(dest_folder_id) {
        return Ok(()); // already there
    }

    db::items::set_folder(conn, item_id, Some(dest_folder_id))?;
    crate::jobs::enqueue_retag_item(conn, item_id)?;
    db::journal::record_item_move(conn, batch_id, item_id, from_folder_id, Some(dest_folder_id))?;
    Ok(())
}

/// Move a batch of items back to the Sorting Box (`folder_id = NULL`) —
/// unfiling, the item-level mirror of a folder move to the top level.
pub fn unfile_items(conn: &Connection, item_ids: &[i64], batch_id: &str) -> Result<MoveItemsReport> {
    let mut report = MoveItemsReport {
        batch_id: batch_id.to_string(),
        ..Default::default()
    };
    for &item_id in item_ids {
        let from_folder_id = match db::items::folder_id_of(conn, item_id)? {
            Some(folder_id) => folder_id,
            None => {
                report.errors.push(ItemOpError { item_id, error: "item no longer exists".to_string() });
                continue;
            }
        };
        if from_folder_id.is_none() {
            report.moved += 1; // already unfiled
            continue;
        }
        db::items::set_folder(conn, item_id, None)?;
        let _ = crate::jobs::enqueue_retag_item(conn, item_id);
        db::journal::record_item_move(conn, batch_id, item_id, from_folder_id, None)?;
        report.moved += 1;
    }
    Ok(report)
}

/// `(parent_id, title)` for a live folder.
fn folder_location(conn: &Connection, folder_id: i64) -> Result<(Option<i64>, String)> {
    conn.query_row(
        "SELECT parent_id, title FROM folder WHERE id = ?1 AND deleted_at IS NULL",
        [folder_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .map_err(|_| AppError::invalid("folder not found"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_conn() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        db::migrate(&mut conn).unwrap();
        conn
    }

    #[test]
    fn creating_a_folder_journals_and_applies_an_archetype() {
        let conn = memory_conn();
        let archetype = db::folders::create_archetype(&conn, "Person").unwrap();
        db::folders::add_archetype_field(&conn, archetype, "instagram", false).unwrap();

        let id = create_folder(&conn, None, "Ana", Some(archetype), &db::journal::new_batch()).unwrap();

        let detail = db::folders::get_detail(&conn, id).unwrap().unwrap();
        assert_eq!(detail.title, "ana");
        assert_eq!(detail.archetype_name.as_deref(), Some("Person"));

        let creates: i64 = conn
            .query_row("SELECT COUNT(*) FROM journal WHERE op = 'folder_create'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(creates, 1);
    }

    #[test]
    fn retitling_is_one_column_and_journals() {
        let conn = memory_conn();
        let id = create_folder(&conn, None, "Ana", None, &db::journal::new_batch()).unwrap();

        retitle_folder(&conn, id, "Anastasia", &db::journal::new_batch()).unwrap();

        let detail = db::folders::get_detail(&conn, id).unwrap().unwrap();
        assert_eq!(detail.title, "anastasia");
    }

    #[test]
    fn moving_a_folder_rebuilds_tags_for_its_subtree() {
        let conn = memory_conn();
        let people = create_folder(&conn, None, "People", None, &db::journal::new_batch()).unwrap();
        let places = create_folder(&conn, None, "Places", None, &db::journal::new_batch()).unwrap();
        let ana = create_folder(&conn, Some(people), "Ana", None, &db::journal::new_batch()).unwrap();

        move_folder(&conn, ana, Some(places), &db::journal::new_batch()).unwrap();

        let detail = db::folders::get_detail(&conn, ana).unwrap().unwrap();
        assert_eq!(detail.parent_id, Some(places));

        let queued: i64 = conn
            .query_row("SELECT COUNT(*) FROM job WHERE type = 'retag_folder'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(queued, 1);
    }

    #[test]
    fn moving_a_folder_into_its_own_descendant_is_refused() {
        let conn = memory_conn();
        let a = create_folder(&conn, None, "A", None, &db::journal::new_batch()).unwrap();
        let b = create_folder(&conn, Some(a), "B", None, &db::journal::new_batch()).unwrap();

        let err = move_folder(&conn, a, Some(b), &db::journal::new_batch()).unwrap_err();
        assert!(err.to_string().contains("descendant"));
    }

    #[test]
    fn moving_a_folder_onto_a_colliding_sibling_title_is_refused() {
        let conn = memory_conn();
        let people = create_folder(&conn, None, "People", None, &db::journal::new_batch()).unwrap();
        create_folder(&conn, Some(people), "Ana", None, &db::journal::new_batch()).unwrap();
        let loose_ana = create_folder(&conn, None, "Ana", None, &db::journal::new_batch()).unwrap();

        let err = move_folder(&conn, loose_ana, Some(people), &db::journal::new_batch()).unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn moving_items_is_a_pure_folder_id_write() {
        let conn = memory_conn();
        let a = create_folder(&conn, None, "A", None, &db::journal::new_batch()).unwrap();
        let b = create_folder(&conn, None, "B", None, &db::journal::new_batch()).unwrap();
        let item_id = db::items::upsert(
            &conn,
            &db::items::NewItem {
                uuid: uuid::Uuid::new_v4().to_string(),
                folder_id: Some(a),
                disk_name: "x.jpg".to_string(),
                ext: "jpg".to_string(),
                orig_name: "x.jpg".to_string(),
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
        .unwrap();

        let batch = db::journal::new_batch();
        let report = move_items(&conn, &[item_id], b, &batch).unwrap();
        assert_eq!(report.moved, 1);
        assert!(report.errors.is_empty());
        assert_eq!(db::items::folder_id_of(&conn, item_id).unwrap(), Some(Some(b)));
    }

    #[test]
    fn unfiling_items_clears_folder_id() {
        let conn = memory_conn();
        let a = create_folder(&conn, None, "A", None, &db::journal::new_batch()).unwrap();
        let item_id = db::items::upsert(
            &conn,
            &db::items::NewItem {
                uuid: uuid::Uuid::new_v4().to_string(),
                folder_id: Some(a),
                disk_name: "x.jpg".to_string(),
                ext: "jpg".to_string(),
                orig_name: "x.jpg".to_string(),
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
        .unwrap();

        let batch = db::journal::new_batch();
        let report = unfile_items(&conn, &[item_id], &batch).unwrap();
        assert_eq!(report.moved, 1);
        assert_eq!(db::items::folder_id_of(&conn, item_id).unwrap(), Some(None));
    }
}
