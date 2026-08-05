//! Soft delete. Deleting a folder or a selection of items never hard-deletes:
//! the file moves into `.gallery/trash/`, and the database row is
//! soft-deleted (`deleted_at`) rather than removed.
//!
//! **Shard-based since PLAN.md §M2.6** — keyed by uuid, not by a mirrored
//! relative path. `trash_folder`'s physical side is a real behavioural
//! change from before: with no directory to rename in one O(1) move, trashing
//! a folder with many descendants is now one same-volume rename *per item*
//! in its subtree. Each one is still cheap; there are just more of them than
//! there used to be one of.

use rusqlite::Connection;
use serde::Serialize;

use crate::db;
use crate::error::{AppError, Result};
use crate::fs::paths::LibraryPaths;
use crate::fs::relocate::ItemOpError;

/// Physically move one item's file into trash, if it's actually there.
/// Tolerant of it already being gone — a row can end up describing a file
/// that vanished outside the app, and deleting it is the one way out that
/// must still work.
fn move_item_to_trash(paths: &LibraryPaths, uuid: &str, ext: &str) {
    let src = paths.item_path(uuid, ext);
    if !src.is_file() {
        return;
    }
    let dest = paths.trash_item_path(uuid, ext);
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::rename(&src, &dest);
}

/// Trash a folder and its whole subtree: soft-deletes every item in it and
/// every descendant folder in the database (`db::folders::trash_subtree`,
/// two bulk `UPDATE`s — cheap even for a large subtree, which is why this
/// still runs synchronously rather than as a job), then physically moves
/// each item's file into trash. The item locations are gathered *before*
/// the soft-delete, while the subtree is still live to walk.
pub fn trash_folder(paths: &LibraryPaths, conn: &Connection, folder_id: i64, batch_id: &str) -> Result<()> {
    if db::folders::get_detail(conn, folder_id)?.is_none() {
        return Err(AppError::invalid("folder not found"));
    }

    let items = db::items::locations_in_subtree(conn, folder_id)?;
    let trashed_at = db::folders::trash_subtree(conn, folder_id)?;

    for item in &items {
        move_item_to_trash(paths, &item.uuid, &item.ext);
    }

    db::journal::record_folder_trash(conn, batch_id, folder_id, trashed_at)?;
    Ok(())
}

/// Undo's half of `trash_folder`. `parent_id`/`title` were never touched by
/// the trash (PLAN.md §M2.6 — no path to free by rewriting them), so all
/// this needs to do is put each item's file back and clear `deleted_at`.
pub fn restore_folder(paths: &LibraryPaths, conn: &Connection, folder_id: i64, trashed_at: i64) -> Result<()> {
    db::folders::restore_subtree(conn, folder_id, trashed_at)?;
    for item in db::items::locations_in_subtree(conn, folder_id)? {
        restore_item_file(paths, &item.uuid, &item.ext);
    }
    Ok(())
}

fn restore_item_file(paths: &LibraryPaths, uuid: &str, ext: &str) {
    let src = paths.trash_item_path(uuid, ext);
    if !src.is_file() {
        return;
    }
    let dest = paths.item_path(uuid, ext);
    if dest.exists() {
        return; // already back, or something else is there — leave it alone
    }
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::rename(&src, &dest);
}

/// What `trash_items` reports back — trashed count plus any per-item
/// failures, so one already-gone item in a large selection doesn't cost
/// every other item in it.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashItemsReport {
    pub trashed: i64,
    pub errors: Vec<ItemOpError>,
    /// The journal batch this delete wrote — what the toast's Undo button
    /// hands back to `undo_batch`.
    pub batch_id: String,
}

/// Trash a batch of items, sharing `batch_id` so one future undo covers the
/// whole selection.
pub fn trash_items(paths: &LibraryPaths, conn: &Connection, item_ids: &[i64], batch_id: &str) -> Result<TrashItemsReport> {
    let mut report = TrashItemsReport {
        batch_id: batch_id.to_string(),
        ..Default::default()
    };
    for &item_id in item_ids {
        match trash_one_item(paths, conn, item_id, batch_id) {
            Ok(()) => report.trashed += 1,
            Err(err) => report.errors.push(ItemOpError { item_id, error: err.to_string() }),
        }
    }
    Ok(report)
}

fn trash_one_item(paths: &LibraryPaths, conn: &Connection, item_id: i64, batch_id: &str) -> Result<()> {
    let loc = db::items::location(conn, item_id)?
        .ok_or_else(|| AppError::invalid("item no longer exists"))?;

    move_item_to_trash(paths, &loc.uuid, &loc.ext);
    db::items::trash_one(conn, item_id)?;
    db::journal::record_item_trash(conn, batch_id, item_id, &loc.uuid, &loc.ext)?;
    Ok(())
}

/// Undo's half of `trash_one_item`.
pub fn restore_item(paths: &LibraryPaths, conn: &Connection, item_id: i64, uuid: &str, ext: &str) -> Result<()> {
    restore_item_file(paths, uuid, ext);
    db::items::restore_one(conn, item_id)?;
    Ok(())
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

    fn seed_item(conn: &Connection, folder_id: Option<i64>, uuid: &str, ext: &str) -> i64 {
        db::items::upsert(
            conn,
            &NewItem {
                uuid: uuid.to_string(),
                folder_id,
                disk_name: format!("{uuid}.{ext}"),
                ext: ext.to_string(),
                orig_name: format!("orig.{ext}"),
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
    fn trashing_an_item_moves_its_shard_file_and_soft_deletes_the_row() {
        let root = scratch("trash-item-shard");
        let (paths, conn) = open_db(&root);
        let uuid = "a3f2c1d4-e29b-41d4-a716-446655440000";
        let item_id = seed_item(&conn, None, uuid, "jpg");
        let src = paths.item_path(uuid, "jpg");
        std::fs::create_dir_all(src.parent().unwrap()).unwrap();
        std::fs::write(&src, b"bytes").unwrap();

        let batch = db::journal::new_batch();
        let report = trash_items(&paths, &conn, &[item_id], &batch).unwrap();
        assert_eq!(report.trashed, 1);
        assert!(report.errors.is_empty());

        assert!(!src.exists());
        assert!(paths.trash_item_path(uuid, "jpg").is_file());
        let deleted: Option<i64> = conn
            .query_row("SELECT deleted_at FROM item WHERE id = ?1", [item_id], |r| r.get(0))
            .unwrap();
        assert!(deleted.is_some());
    }

    #[test]
    fn restoring_an_item_puts_the_file_and_the_row_back() {
        let root = scratch("trash-item-restore");
        let (paths, conn) = open_db(&root);
        let uuid = "a3f2c1d4-e29b-41d4-a716-446655440000";
        let item_id = seed_item(&conn, None, uuid, "jpg");
        let src = paths.item_path(uuid, "jpg");
        std::fs::create_dir_all(src.parent().unwrap()).unwrap();
        std::fs::write(&src, b"bytes").unwrap();
        let batch = db::journal::new_batch();
        trash_items(&paths, &conn, &[item_id], &batch).unwrap();

        restore_item(&paths, &conn, item_id, uuid, "jpg").unwrap();

        assert!(src.is_file());
        let deleted: Option<i64> = conn
            .query_row("SELECT deleted_at FROM item WHERE id = ?1", [item_id], |r| r.get(0))
            .unwrap();
        assert!(deleted.is_none());
    }

    #[test]
    fn trashing_a_folder_moves_every_descendant_items_file_and_soft_deletes_the_subtree() {
        let root = scratch("trash-folder-subtree");
        let (paths, conn) = open_db(&root);
        let people = db::folders::create_record(&conn, None, "people").unwrap();
        let ana = db::folders::create_record(&conn, Some(people), "ana").unwrap();

        let uuid_a = "aaaaaaaa-0000-0000-0000-000000000000";
        let uuid_b = "bbbbbbbb-0000-0000-0000-000000000000";
        let item_a = seed_item(&conn, Some(people), uuid_a, "jpg");
        let item_b = seed_item(&conn, Some(ana), uuid_b, "jpg");
        for (uuid, _) in [(uuid_a, item_a), (uuid_b, item_b)] {
            let src = paths.item_path(uuid, "jpg");
            std::fs::create_dir_all(src.parent().unwrap()).unwrap();
            std::fs::write(&src, b"bytes").unwrap();
        }

        let batch = db::journal::new_batch();
        trash_folder(&paths, &conn, people, &batch).unwrap();

        assert!(paths.trash_item_path(uuid_a, "jpg").is_file());
        assert!(paths.trash_item_path(uuid_b, "jpg").is_file());
        assert!(db::folders::get_detail(&conn, people).unwrap().is_none());
        assert!(db::folders::get_detail(&conn, ana).unwrap().is_none());
        let item_a_deleted: Option<i64> = conn
            .query_row("SELECT deleted_at FROM item WHERE id = ?1", [item_a], |r| r.get(0))
            .unwrap();
        assert!(item_a_deleted.is_some());
    }

    #[test]
    fn a_new_folder_can_be_created_at_the_same_spot_while_the_old_one_is_still_trashed() {
        let root = scratch("trash-folder-reuse-spot");
        let (paths, conn) = open_db(&root);
        let people = db::folders::create_record(&conn, None, "people").unwrap();
        let ana = db::folders::create_record(&conn, Some(people), "ana").unwrap();

        trash_folder(&paths, &conn, ana, &db::journal::new_batch()).unwrap();

        let new_ana = db::folders::create_record(&conn, Some(people), "ana").unwrap();
        assert_ne!(new_ana, ana, "a fresh row, not the trashed one resurrected");
    }

    #[test]
    fn deleting_a_folder_whose_items_file_is_already_gone_still_removes_the_record() {
        let root = scratch("trash-folder-missing-file");
        let (paths, conn) = open_db(&root);
        let ana = db::folders::create_record(&conn, None, "ana").unwrap();
        seed_item(&conn, Some(ana), "aaaaaaaa-0000-0000-0000-000000000000", "jpg");
        // Never actually written to `files/`.

        trash_folder(&paths, &conn, ana, &db::journal::new_batch()).unwrap();

        assert!(db::folders::get_detail(&conn, ana).unwrap().is_none());
    }

    #[test]
    fn restoring_a_trashed_folder_and_its_subtree() {
        let root = scratch("trash-folder-restore");
        let (paths, conn) = open_db(&root);
        let people = db::folders::create_record(&conn, None, "people").unwrap();
        let ana = db::folders::create_record(&conn, Some(people), "ana").unwrap();
        let uuid = "aaaaaaaa-0000-0000-0000-000000000000";
        let item_id = seed_item(&conn, Some(ana), uuid, "jpg");
        let src = paths.item_path(uuid, "jpg");
        std::fs::create_dir_all(src.parent().unwrap()).unwrap();
        std::fs::write(&src, b"bytes").unwrap();

        let ts = {
            trash_folder(&paths, &conn, people, &db::journal::new_batch()).unwrap();
            conn.query_row("SELECT deleted_at FROM folder WHERE id = ?1", [people], |r| r.get::<_, i64>(0))
                .unwrap()
        };

        restore_folder(&paths, &conn, people, ts).unwrap();

        assert!(db::folders::get_detail(&conn, people).unwrap().is_some());
        assert!(db::folders::get_detail(&conn, ana).unwrap().is_some());
        assert!(paths.item_path(uuid, "jpg").is_file());
        let deleted: Option<i64> = conn
            .query_row("SELECT deleted_at FROM item WHERE id = ?1", [item_id], |r| r.get(0))
            .unwrap();
        assert!(deleted.is_none());
    }
}
