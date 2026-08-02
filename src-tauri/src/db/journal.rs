//! The undo stack. M2.1 adds the writers for every folder/item/tag operation
//! it introduces (create, rename, move, trash, tag rename/delete) alongside
//! the rename-on-arrival writer M1.6 already had. Replaying a batch into an
//! actual `Ctrl+Z` is still M4's job — these functions only write rows with
//! enough information for that future replayer to act on. See
//! docs/DATA-MODEL.md#queues-and-history.

use rusqlite::{params, Connection};
use serde_json::{json, Value};

use crate::db::now;
use crate::error::Result;

/// One journal row. `batch_id` groups related rows so a future undo can act
/// on all of them at once — a multi-item move or delete shares one batch id
/// across every row it writes; a single-folder operation mints its own via
/// `new_batch`.
pub fn record(conn: &Connection, batch_id: &str, op: &str, forward: Value, inverse: Value) -> Result<()> {
    conn.execute(
        "INSERT INTO journal (op, forward, inverse, batch_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![op, forward.to_string(), inverse.to_string(), batch_id, now()],
    )?;
    Ok(())
}

/// A fresh id for an operation that is its own batch — not part of a
/// caller-supplied multi-item selection.
pub fn new_batch() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// One journal row, as `fs::undo` reads it back.
pub struct Entry {
    pub id: i64,
    pub op: String,
    pub inverse: Value,
}

/// Every row in a batch, newest first — the order an undo has to apply them
/// in, since a batch's later rows may depend on its earlier ones.
pub fn batch(conn: &Connection, batch_id: &str) -> Result<Vec<Entry>> {
    let mut stmt = conn.prepare(
        "SELECT id, op, inverse FROM journal WHERE batch_id = ?1 ORDER BY id DESC",
    )?;
    let rows = stmt
        .query_map(params![batch_id], |r| {
            let raw: String = r.get(2)?;
            Ok(Entry {
                id: r.get(0)?,
                op: r.get(1)?,
                inverse: serde_json::from_str(&raw).unwrap_or(Value::Null),
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Drop a batch once it has been undone. An operation that has been reversed
/// is not history a replayer should ever act on again — and until M4 builds
/// the full stack, "gone" is the honest representation of undone.
pub fn drop_batch(conn: &Connection, batch_id: &str) -> Result<()> {
    conn.execute("DELETE FROM journal WHERE batch_id = ?1", params![batch_id])?;
    Ok(())
}

/// One file renamed on arrival, recorded so a future undo can put it back
/// under its old name. `forward`/`inverse` are deliberately symmetric — both
/// are just `{ itemId, folderRel, from, to }` with the names swapped, so a
/// generic replayer needs no rename-specific logic.
///
/// Not used by the bulk import wizard: that operation's undo path is the
/// `library.jsonl` reversal map, sized for renaming a whole library at once.
/// A single arriving file is exactly what `Ctrl+Z` is for instead.
pub fn record_rename(
    conn: &Connection,
    item_id: i64,
    folder_rel: &str,
    from_name: &str,
    to_name: &str,
) -> Result<()> {
    let forward = json!({ "itemId": item_id, "folderRel": folder_rel, "from": from_name, "to": to_name });
    let inverse = json!({ "itemId": item_id, "folderRel": folder_rel, "from": to_name, "to": from_name });
    // Every arriving file is its own undo step for now — nothing yet groups
    // several arrivals (e.g. one drag-and-drop drop) into a single batch.
    record(conn, &new_batch(), "rename", forward, inverse)
}

// --- folders ---------------------------------------------------------------

pub fn record_folder_create(conn: &Connection, batch_id: &str, folder_id: i64, parent_id: Option<i64>, rel_path: &str) -> Result<()> {
    let forward = json!({ "folderId": folder_id, "parentId": parent_id, "relPath": rel_path });
    let inverse = json!({ "folderId": folder_id });
    record(conn, batch_id, "folder_create", forward, inverse)
}

pub fn record_folder_rename_title(conn: &Connection, batch_id: &str, folder_id: i64, from: &str, to: &str) -> Result<()> {
    let forward = json!({ "folderId": folder_id, "from": from, "to": to });
    let inverse = json!({ "folderId": folder_id, "from": to, "to": from });
    record(conn, batch_id, "folder_rename_title", forward, inverse)
}

pub fn record_folder_rename_dir(conn: &Connection, batch_id: &str, folder_id: i64, old_rel: &str, new_rel: &str) -> Result<()> {
    let forward = json!({ "folderId": folder_id, "from": old_rel, "to": new_rel });
    let inverse = json!({ "folderId": folder_id, "from": new_rel, "to": old_rel });
    record(conn, batch_id, "folder_rename_dir", forward, inverse)
}

pub fn record_folder_move(
    conn: &Connection,
    batch_id: &str,
    folder_id: i64,
    from_parent_id: Option<i64>,
    to_parent_id: Option<i64>,
    old_rel: &str,
    new_rel: &str,
) -> Result<()> {
    let forward = json!({ "folderId": folder_id, "fromParentId": from_parent_id, "toParentId": to_parent_id, "fromRel": old_rel, "toRel": new_rel });
    let inverse = json!({ "folderId": folder_id, "fromParentId": to_parent_id, "toParentId": from_parent_id, "fromRel": new_rel, "toRel": old_rel });
    record(conn, batch_id, "folder_move", forward, inverse)
}

/// `subtree` is every folder row the trash retired, as `[id, relPath]` pairs,
/// and `trashedAt` the timestamp it stamped on them. Both are captured because
/// `db::folders::trash_subtree_rows` rewrites each `rel_path` to `.trashed/<id>`
/// to free the string for reuse — after which nothing else in the database
/// remembers where the subtree lived.
pub fn record_folder_trash(
    conn: &Connection,
    batch_id: &str,
    folder_id: i64,
    rel_path: &str,
    title: &str,
    trash_rel: &str,
    subtree: &[(i64, String)],
    trashed_at: i64,
) -> Result<()> {
    let rows: Vec<Value> = subtree
        .iter()
        .map(|(id, rel)| json!([id, rel]))
        .collect();
    let payload = json!({
        "folderId": folder_id,
        "relPath": rel_path,
        "title": title,
        "trashRel": trash_rel,
        "subtree": rows,
        "trashedAt": trashed_at,
    });
    record(conn, batch_id, "folder_trash", payload.clone(), payload)
}

// --- items -------------------------------------------------------------

pub fn record_item_move(
    conn: &Connection,
    batch_id: &str,
    item_id: i64,
    from_folder_id: i64,
    to_folder_id: i64,
) -> Result<()> {
    let forward = json!({ "itemId": item_id, "fromFolderId": from_folder_id, "toFolderId": to_folder_id });
    let inverse = json!({ "itemId": item_id, "fromFolderId": to_folder_id, "toFolderId": from_folder_id });
    record(conn, batch_id, "item_move", forward, inverse)
}

pub fn record_item_trash(
    conn: &Connection,
    batch_id: &str,
    item_id: i64,
    folder_rel: &str,
    disk_name: &str,
    trash_rel: &str,
) -> Result<()> {
    let forward = json!({ "itemId": item_id, "folderRel": folder_rel, "diskName": disk_name, "trashRel": trash_rel });
    let inverse = forward.clone();
    record(conn, batch_id, "item_trash", forward, inverse)
}

// --- tags --------------------------------------------------------------

pub fn record_tag_rename(conn: &Connection, tag_id: i64, from: &str, to: &str) -> Result<()> {
    let forward = json!({ "tagId": tag_id, "from": from, "to": to });
    let inverse = json!({ "tagId": tag_id, "from": to, "to": from });
    record(conn, &new_batch(), "tag_rename", forward, inverse)
}

/// `folder_ids`/`item_ids` are every association the tag had at the moment
/// of deletion — captured so a future replayer can fully reconstruct them,
/// not just recreate the bare `tag` row.
pub fn record_tag_delete(
    conn: &Connection,
    tag_id: i64,
    key: Option<&str>,
    value: &str,
    folder_ids: &[i64],
    item_ids: &[i64],
) -> Result<()> {
    let forward = json!({ "tagId": tag_id, "key": key, "value": value, "folderIds": folder_ids, "itemIds": item_ids });
    let inverse = forward.clone();
    record(conn, &new_batch(), "tag_delete", forward, inverse)
}
