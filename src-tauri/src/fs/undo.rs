//! Reversing one journalled batch — what the toast's **Undo** button calls.
//!
//! Locked decision 23: `Ctrl+Z` alone is not a path to undo, so every
//! destructive action ends in a toast naming what happened with an Undo
//! button. That button needs something to call, and M2.1 shipped journalled
//! moves and deletes with nothing on the other end of them. This is that
//! other end.
//!
//! **Scope.** This reverses *one batch, by id* — the batch the toast is
//! holding. The full `Ctrl+Z` stack replayer, which walks the journal
//! backwards without being told where to start and handles every op M4 adds,
//! is still M4's job (PLAN.md §M4, docs/STRUCTURE.md `db/journal.rs`). The
//! ops covered here are exactly the ones the interface can perform today:
//! item move, item trash, folder move, folder retitle and folder trash.
//!
//! Same shape as its siblings in this module: disk first, then the database,
//! because the filesystem is authoritative. Nothing here writes a *new*
//! journal entry — a reversal is not an operation you would want to reverse
//! again by pressing undo twice — and the batch's rows are dropped once it
//! has been replayed.

use rusqlite::Connection;
use serde_json::Value;

use crate::db;
use crate::error::{AppError, Result};
use crate::fs::paths::LibraryPaths;
use crate::jobs;

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
        "item_move" => item_move(paths, conn, inverse),
        "item_trash" => item_trash(paths, conn, inverse),
        "folder_move" => folder_move(paths, conn, inverse),
        "folder_rename_dir" => folder_rename_dir(paths, conn, inverse),
        "folder_rename_title" => folder_rename_title(conn, inverse),
        "folder_trash" => folder_trash(paths, conn, inverse),
        other => Err(AppError::invalid(format!("'{other}' can't be undone yet"))),
    }
}

// --- items -----------------------------------------------------------------

fn item_move(paths: &LibraryPaths, conn: &Connection, inverse: &Value) -> Result<()> {
    let item_id = number(inverse, "itemId")?;
    let dest_folder_id = number(inverse, "toFolderId")?;
    let dest_rel = db::folders::rel_for(conn, dest_folder_id)?
        .ok_or_else(|| AppError::invalid("the folder it came from is gone"))?;

    let item = db::items::rename_target(conn, item_id)?
        .ok_or_else(|| AppError::invalid("that item no longer exists"))?;
    if item.folder_rel == dest_rel {
        return Ok(()); // already back
    }

    let from = paths.item_path(&item.folder_rel, &item.disk_name)?;
    let to = paths.item_path(&dest_rel, &item.disk_name)?;
    if to.exists() {
        return Err(AppError::invalid(format!(
            "{} already exists back at {dest_rel}",
            item.disk_name
        )));
    }
    std::fs::rename(&from, &to)?;

    db::items::set_folder(conn, item_id, dest_folder_id)?;
    jobs::enqueue_retag_item(conn, item_id)?;
    Ok(())
}

fn item_trash(paths: &LibraryPaths, conn: &Connection, inverse: &Value) -> Result<()> {
    let item_id = number(inverse, "itemId")?;
    let folder_rel = text(inverse, "folderRel")?;
    let disk_name = text(inverse, "diskName")?;
    let trash_rel = text(inverse, "trashRel")?;

    let rel_path = if folder_rel.is_empty() {
        disk_name.to_string()
    } else {
        format!("{folder_rel}/{disk_name}")
    };
    crate::fs::trash::restore_from_trash(paths, trash_rel, &rel_path)?;
    db::items::restore_one(conn, item_id)?;
    Ok(())
}

// --- folders ---------------------------------------------------------------

fn folder_move(paths: &LibraryPaths, conn: &Connection, inverse: &Value) -> Result<()> {
    let folder_id = number(inverse, "folderId")?;
    let to_parent_id = optional_number(inverse, "toParentId");
    crate::fs::relocate::move_folder_unjournalled(paths, conn, folder_id, to_parent_id)
}

fn folder_rename_dir(paths: &LibraryPaths, conn: &Connection, inverse: &Value) -> Result<()> {
    let folder_id = number(inverse, "folderId")?;
    let from_rel = text(inverse, "from")?;
    let to_rel = text(inverse, "to")?;

    let from_abs = paths.to_abs(from_rel)?;
    let to_abs = paths.to_abs(to_rel)?;
    if from_abs.exists() {
        if to_abs.exists() {
            return Err(AppError::invalid(format!(
                "something already exists at {to_rel}"
            )));
        }
        std::fs::rename(&from_abs, &to_abs)?;
    }

    db::folders::set_rel_path(conn, folder_id, to_rel)?;
    jobs::enqueue_rename_folder_subtree(conn, from_rel, to_rel)?;
    Ok(())
}

fn folder_rename_title(conn: &Connection, inverse: &Value) -> Result<()> {
    let folder_id = number(inverse, "folderId")?;
    let to = text(inverse, "to")?;
    // Straight to the row: `retitle_folder` would derive a directory name
    // from the title again, and the `folder_rename_dir` row in this same
    // batch is what puts the directory back.
    db::folders::set_title_unjournalled(conn, folder_id, to)
}

fn folder_trash(paths: &LibraryPaths, conn: &Connection, inverse: &Value) -> Result<()> {
    let rel_path = text(inverse, "relPath")?;
    let trash_rel = text(inverse, "trashRel")?;
    let trashed_at = number(inverse, "trashedAt")?;

    let subtree: Vec<(i64, String)> = inverse
        .get("subtree")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let pair = row.as_array()?;
                    Some((pair.first()?.as_i64()?, pair.get(1)?.as_str()?.to_string()))
                })
                .collect()
        })
        .unwrap_or_default();
    if subtree.is_empty() {
        return Err(AppError::invalid(
            "this folder was deleted by an older version that recorded too little to undo",
        ));
    }

    crate::fs::trash::restore_from_trash(paths, trash_rel, rel_path)?;
    db::folders::restore_subtree_rows(conn, &subtree, trashed_at)?;
    Ok(())
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
        db::folders::upsert(&conn, "", "Library").unwrap();
        (paths, conn)
    }

    fn add_item(conn: &Connection, folder_id: i64, name: &str) -> i64 {
        db::items::upsert(
            conn,
            &NewItem {
                uuid: uuid::Uuid::new_v4().to_string(),
                folder_id,
                disk_name: name.to_string(),
                ext: "jpg".to_string(),
                orig_name: name.to_string(),
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
        std::fs::write(root.join("a.jpg"), b"bytes").unwrap();
        let (paths, conn) = open_db(&root);
        let root_id = db::folders::id_for_rel(&conn, "").unwrap().unwrap();
        let item_id = add_item(&conn, root_id, "a.jpg");

        let batch = db::journal::new_batch();
        crate::fs::trash::trash_items(&paths, &conn, &[item_id], &batch).unwrap();
        assert!(!root.join("a.jpg").exists());

        let report = undo_batch(&paths, &conn, &batch).unwrap();
        assert_eq!(report.reversed, 1);
        assert!(report.errors.is_empty());
        assert!(root.join("a.jpg").is_file(), "the file came back");

        let deleted: Option<i64> = conn
            .query_row("SELECT deleted_at FROM item WHERE id = ?1", [item_id], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(deleted.is_none(), "the row is live again");
    }

    #[test]
    fn undoing_a_move_returns_every_item_in_the_batch() {
        let root = scratch("undo-item-move");
        std::fs::create_dir_all(root.join("Trips")).unwrap();
        std::fs::write(root.join("a.jpg"), b"one").unwrap();
        std::fs::write(root.join("b.jpg"), b"two").unwrap();
        let (paths, conn) = open_db(&root);
        let root_id = db::folders::id_for_rel(&conn, "").unwrap().unwrap();
        let trips = db::folders::upsert(&conn, "trips", "Trips").unwrap();
        let a = add_item(&conn, root_id, "a.jpg");
        let b = add_item(&conn, root_id, "b.jpg");

        let batch = db::journal::new_batch();
        let report =
            crate::fs::relocate::move_items(&paths, &conn, &[a, b], trips, &batch).unwrap();
        assert_eq!(report.moved, 2);
        assert!(root.join("Trips/a.jpg").is_file());

        undo_batch(&paths, &conn, &batch).unwrap();
        assert!(root.join("a.jpg").is_file());
        assert!(root.join("b.jpg").is_file());
        assert_eq!(db::items::folder_id_of(&conn, a).unwrap(), Some(root_id));
        assert_eq!(db::items::folder_id_of(&conn, b).unwrap(), Some(root_id));
    }

    #[test]
    fn undoing_a_folder_delete_restores_the_subtree_and_its_items() {
        let root = scratch("undo-folder-trash");
        std::fs::create_dir_all(root.join("People/Ana")).unwrap();
        std::fs::write(root.join("People/Ana/photo.jpg"), b"bytes").unwrap();
        let (paths, conn) = open_db(&root);
        let people = db::folders::upsert(&conn, "people", "People").unwrap();
        let ana = db::folders::upsert(&conn, "people/ana", "Ana").unwrap();
        let item_id = add_item(&conn, ana, "photo.jpg");

        let batch = db::journal::new_batch();
        crate::fs::trash::trash_folder(&paths, &conn, people, &batch).unwrap();
        assert!(!root.join("People").exists());

        undo_batch(&paths, &conn, &batch).unwrap();

        assert!(root.join("People/Ana/photo.jpg").is_file());
        assert_eq!(
            db::folders::id_for_rel(&conn, "people/ana").unwrap(),
            Some(ana),
            "the same row came back at the same path, not a fresh one"
        );
        let deleted: Option<i64> = conn
            .query_row("SELECT deleted_at FROM item WHERE id = ?1", [item_id], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(deleted.is_none());
    }

    #[test]
    fn a_reversed_batch_leaves_nothing_behind_to_reverse_twice() {
        let root = scratch("undo-once");
        std::fs::write(root.join("a.jpg"), b"bytes").unwrap();
        let (paths, conn) = open_db(&root);
        let root_id = db::folders::id_for_rel(&conn, "").unwrap().unwrap();
        let item_id = add_item(&conn, root_id, "a.jpg");

        let batch = db::journal::new_batch();
        crate::fs::trash::trash_items(&paths, &conn, &[item_id], &batch).unwrap();
        undo_batch(&paths, &conn, &batch).unwrap();

        let err = undo_batch(&paths, &conn, &batch).unwrap_err();
        assert!(err.to_string().contains("nothing left to undo"));
    }
}
