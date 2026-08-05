//! Reversing one journalled batch — what the toast's **Undo** button calls.
//!
//! **Scope.** This reverses *one batch, by id* — the batch the toast is
//! holding. The full `Ctrl+Z` stack replayer, which walks the journal
//! backwards without being told where to start and handles every op M4
//! adds, is still M4's job. The ops covered here are exactly the ones the
//! interface can perform today: item move, item trash, folder move, folder
//! retitle and folder trash.
//!
//! Same shape as its siblings in this module: nothing here writes a *new*
//! journal entry — a reversal is not an operation you would want to reverse
//! again by pressing undo twice — and the batch's rows are dropped once it
//! has been replayed.

use rusqlite::Connection;
use serde_json::Value;

use crate::db;
use crate::error::{AppError, Result};
use crate::fs::paths::LibraryPaths;

/// What was reversed, so the toast that replaces the first one can say so.
#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoReport {
    /// Ops actually reversed.
    pub reversed: i64,
    /// Rows that could not be reversed, with the reason — a file put back by
    /// hand, a destination that no longer exists. Reported rather than
    /// thrown, so one bad row in a large selection doesn't strand the rest.
    pub errors: Vec<String>,
}

/// Reverse every row in `batch_id`, newest first.
pub fn undo_batch(paths: &LibraryPaths, conn: &Connection, batch_id: &str) -> Result<UndoReport> {
    let entries = db::journal::batch(conn, batch_id)?;
    if entries.is_empty() {
        return Err(AppError::invalid("there is nothing left to undo here"));
    }

    let mut report = UndoReport::default();
    for entry in &entries {
        match reverse(paths, conn, &entry.op, &entry.inverse) {
            Ok(()) => report.reversed += 1,
            Err(err) => report.errors.push(err.to_string()),
        }
    }

    // Only clear the batch if the whole of it came back. A partial undo has
    // to stay in the journal, or the rows that failed become unreachable.
    if report.errors.is_empty() {
        db::journal::drop_batch(conn, batch_id)?;
    }
    Ok(report)
}

fn reverse(paths: &LibraryPaths, conn: &Connection, op: &str, inverse: &Value) -> Result<()> {
    match op {
        "item_move" => item_move(conn, inverse),
        "item_trash" => item_trash(paths, conn, inverse),
        "folder_move" => folder_move(conn, inverse),
        "folder_rename_title" => folder_rename_title(conn, inverse),
        "folder_trash" => folder_trash(paths, conn, inverse),
        other => Err(AppError::invalid(format!("'{other}' can't be undone yet"))),
    }
}

// --- items -----------------------------------------------------------------

fn item_move(conn: &Connection, inverse: &Value) -> Result<()> {
    let item_id = number(inverse, "itemId")?;
    let dest_folder_id = optional_number(inverse, "toFolderId");

    db::items::folder_id_of(conn, item_id)?.ok_or_else(|| AppError::invalid("that item no longer exists"))?;
    db::items::set_folder(conn, item_id, dest_folder_id)?;
    crate::jobs::enqueue_retag_item(conn, item_id)?;
    Ok(())
}

fn item_trash(paths: &LibraryPaths, conn: &Connection, inverse: &Value) -> Result<()> {
    let item_id = number(inverse, "itemId")?;
    let uuid = text(inverse, "uuid")?;
    let ext = text(inverse, "ext")?;
    crate::fs::trash::restore_item(paths, conn, item_id, uuid, ext)
}

// --- folders ---------------------------------------------------------------

fn folder_move(conn: &Connection, inverse: &Value) -> Result<()> {
    let folder_id = number(inverse, "folderId")?;
    let to_parent_id = optional_number(inverse, "toParentId");
    crate::fs::relocate::move_folder_unjournalled(conn, folder_id, to_parent_id)
}

fn folder_rename_title(conn: &Connection, inverse: &Value) -> Result<()> {
    let folder_id = number(inverse, "folderId")?;
    let to = text(inverse, "to")?;
    db::folders::set_title_unjournalled(conn, folder_id, to)
}

fn folder_trash(paths: &LibraryPaths, conn: &Connection, inverse: &Value) -> Result<()> {
    let folder_id = number(inverse, "folderId")?;
    let trashed_at = number(inverse, "trashedAt")?;
    crate::fs::trash::restore_folder(paths, conn, folder_id, trashed_at)
}

// --- payload reading -------------------------------------------------------
//
// The journal is JSON on purpose — it outlives the code that wrote it — so
// every field is checked rather than unwrapped. A malformed row reports one
// failed undo; it never takes the process down. (No `assert!` anywhere near
// a command handler; see CLAUDE.md.)

fn number(value: &Value, key: &str) -> Result<i64> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| AppError::invalid(format!("undo entry is missing '{key}'")))
}

fn optional_number(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_i64)
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::invalid(format!("undo entry is missing '{key}'")))
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

    fn seed_item(conn: &Connection, folder_id: Option<i64>, uuid: &str) -> i64 {
        db::items::upsert(
            conn,
            &NewItem {
                uuid: uuid.to_string(),
                folder_id,
                disk_name: format!("{uuid}.jpg"),
                ext: "jpg".to_string(),
                orig_name: "a.jpg".to_string(),
                hash: "h".to_string(),
                size_bytes: 5,
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
    fn undoing_a_delete_puts_the_file_and_the_row_back() {
        let root = scratch("undo-item-trash");
        let (paths, conn) = open_db(&root);
        let uuid = "aaaaaaaa-0000-0000-0000-000000000000";
        let item_id = seed_item(&conn, None, uuid);
        let src = paths.item_path(uuid, "jpg");
        std::fs::create_dir_all(src.parent().unwrap()).unwrap();
        std::fs::write(&src, b"bytes").unwrap();

        let batch = db::journal::new_batch();
        crate::fs::trash::trash_items(&paths, &conn, &[item_id], &batch).unwrap();
        assert!(!src.exists());

        let report = undo_batch(&paths, &conn, &batch).unwrap();
        assert_eq!(report.reversed, 1);
        assert!(report.errors.is_empty());
        assert!(src.is_file(), "the file came back");

        let deleted: Option<i64> = conn
            .query_row("SELECT deleted_at FROM item WHERE id = ?1", [item_id], |r| r.get(0))
            .unwrap();
        assert!(deleted.is_none());
    }

    #[test]
    fn undoing_a_move_returns_every_item_in_the_batch() {
        let root = scratch("undo-item-move");
        let (paths, conn) = open_db(&root);
        let trips = db::folders::create_record(&conn, None, "trips").unwrap();
        let a = seed_item(&conn, None, "aaaaaaaa-0000-0000-0000-000000000000");
        let b = seed_item(&conn, None, "bbbbbbbb-0000-0000-0000-000000000000");

        let batch = db::journal::new_batch();
        let report = crate::fs::relocate::move_items(&conn, &[a, b], trips, &batch).unwrap();
        assert_eq!(report.moved, 2);

        undo_batch(&paths, &conn, &batch).unwrap();
        assert_eq!(db::items::folder_id_of(&conn, a).unwrap(), Some(None));
        assert_eq!(db::items::folder_id_of(&conn, b).unwrap(), Some(None));
    }

    #[test]
    fn undoing_a_folder_delete_restores_the_subtree_and_its_items() {
        let root = scratch("undo-folder-trash");
        let (paths, conn) = open_db(&root);
        let people = db::folders::create_record(&conn, None, "people").unwrap();
        let ana = db::folders::create_record(&conn, Some(people), "ana").unwrap();
        let uuid = "aaaaaaaa-0000-0000-0000-000000000000";
        let item_id = seed_item(&conn, Some(ana), uuid);
        let src = paths.item_path(uuid, "jpg");
        std::fs::create_dir_all(src.parent().unwrap()).unwrap();
        std::fs::write(&src, b"bytes").unwrap();

        let batch = db::journal::new_batch();
        crate::fs::trash::trash_folder(&paths, &conn, people, &batch).unwrap();
        assert!(db::folders::get_detail(&conn, ana).unwrap().is_none());

        undo_batch(&paths, &conn, &batch).unwrap();

        assert!(db::folders::get_detail(&conn, ana).unwrap().is_some());
        assert!(src.is_file());
        let deleted: Option<i64> = conn
            .query_row("SELECT deleted_at FROM item WHERE id = ?1", [item_id], |r| r.get(0))
            .unwrap();
        assert!(deleted.is_none());
    }

    #[test]
    fn undoing_a_folder_move_puts_it_back_under_its_old_parent() {
        let root = scratch("undo-folder-move");
        let (paths, conn) = open_db(&root);
        let people = db::folders::create_record(&conn, None, "people").unwrap();
        let places = db::folders::create_record(&conn, None, "places").unwrap();
        let ana = db::folders::create_record(&conn, Some(people), "ana").unwrap();

        let batch = db::journal::new_batch();
        crate::fs::relocate::move_folder(&conn, ana, Some(places), &batch).unwrap();
        assert_eq!(db::folders::get_detail(&conn, ana).unwrap().unwrap().parent_id, Some(places));

        undo_batch(&paths, &conn, &batch).unwrap();
        assert_eq!(db::folders::get_detail(&conn, ana).unwrap().unwrap().parent_id, Some(people));
    }

    #[test]
    fn a_reversed_batch_leaves_nothing_behind_to_reverse_twice() {
        let root = scratch("undo-once");
        let (paths, conn) = open_db(&root);
        let uuid = "aaaaaaaa-0000-0000-0000-000000000000";
        let item_id = seed_item(&conn, None, uuid);
        let src = paths.item_path(uuid, "jpg");
        std::fs::create_dir_all(src.parent().unwrap()).unwrap();
        std::fs::write(&src, b"bytes").unwrap();

        let batch = db::journal::new_batch();
        crate::fs::trash::trash_items(&paths, &conn, &[item_id], &batch).unwrap();
        undo_batch(&paths, &conn, &batch).unwrap();

        let err = undo_batch(&paths, &conn, &batch).unwrap_err();
        assert!(err.to_string().contains("nothing left to undo"));
    }
}
