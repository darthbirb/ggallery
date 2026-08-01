//! The undo stack. Schema only has one writer so far — replaying a batch into
//! an actual `Ctrl+Z` is M4's job, once move, trash and tag operations exist
//! to undo alongside renames. See docs/DATA-MODEL.md#queues-and-history.

use rusqlite::{params, Connection};
use serde_json::json;

use crate::db::now;
use crate::error::Result;

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
    let forward = json!({
        "itemId": item_id,
        "folderRel": folder_rel,
        "from": from_name,
        "to": to_name,
    });
    let inverse = json!({
        "itemId": item_id,
        "folderRel": folder_rel,
        "from": to_name,
        "to": from_name,
    });
    // Every arriving file is its own undo step for now — nothing yet groups
    // several arrivals (e.g. one drag-and-drop drop) into a single batch.
    let batch_id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO journal (op, forward, inverse, batch_id, created_at)
         VALUES ('rename', ?1, ?2, ?3, ?4)",
        params![forward.to_string(), inverse.to_string(), batch_id, now()],
    )?;
    Ok(())
}
