//! Soft delete — pulled forward from M4 (see PLAN.md §M2.1). Deleting a
//! folder or a selection of items never hard-deletes: the file moves into
//! `.gallery/trash/`, preserving its relative path, and the database row is
//! soft-deleted (`deleted_at`) rather than removed. There is no restore
//! tooling yet — that, and the `Ctrl+Z` replayer that reads the journal
//! entries written here, are M4's job — but nothing here is destroyed in
//! the meantime.

use std::path::PathBuf;

use rusqlite::Connection;
use serde::Serialize;

use crate::db;
use crate::error::{AppError, Result};
use crate::fs::paths::LibraryPaths;
use crate::fs::relocate::ItemOpError;

/// Where `rel_path` lands in trash. Mirrors the library structure 1:1 so a
/// human with a file browser can see exactly where something came from;
/// uniquified with a short suffix only in the rare case the same path has
/// already been trashed once before (something deleted, a new folder or
/// file created at the same location, then deleted again).
fn trash_destination(paths: &LibraryPaths, rel_path: &str) -> PathBuf {
    let base = paths.trash_dir().join(rel_path);
    if !base.exists() {
        return base;
    }
    for n in 1..1000 {
        let candidate = paths.trash_dir().join(format!("{rel_path}-{n}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    paths
        .trash_dir()
        .join(format!("{rel_path}-{}", uuid::Uuid::new_v4()))
}

/// Physically moves whatever is at `rel_path` into trash, creating parent
/// directories as needed. Returns the trash-relative destination actually
/// used (forward slashes, matching the rest of the app's path convention),
/// or an empty string if there was nothing there to move.
///
/// **Deleting has to work even when the source is already gone.** A folder
/// record can be left pointing at a directory that no longer exists (moving
/// a folder can leave one behind); if a broken record's own delete also
/// failed with the same raw "cannot find the path", removing it would be the
/// one way out this app doesn't actually offer (docs/DESIGN.md §M2.5d).
/// There is nothing left on disk to lose, so this is not a silent skip of a
/// real failure — it is the correct outcome.
fn move_to_trash(paths: &LibraryPaths, rel_path: &str) -> Result<String> {
    let src = paths.to_abs(rel_path)?;
    if !src.exists() {
        return Ok(String::new());
    }
    let dest = trash_destination(paths, rel_path);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(&src, &dest)?;
    let trash_rel = dest
        .strip_prefix(paths.trash_dir())
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    Ok(trash_rel)
}

/// Trash a folder and its whole subtree: physically moves the directory in
/// one OS call (Windows moves the contents with it — no per-file cost), then
/// soft-deletes every item in the subtree and the folder plus every
/// descendant folder in two bulk `UPDATE`s. Both are cheap even for a large
/// subtree, which is why this runs synchronously rather than as a job,
/// unlike the tag-cache rebuild.
pub fn trash_folder(
    paths: &LibraryPaths,
    conn: &Connection,
    folder_id: i64,
    batch_id: &str,
) -> Result<()> {
    let (rel_path, title, parent_id): (String, String, Option<i64>) = conn
        .query_row(
            "SELECT rel_path, title, parent_id FROM folder WHERE id = ?1 AND deleted_at IS NULL",
            [folder_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|_| AppError::invalid("folder not found"))?;
    if parent_id.is_none() {
        return Err(AppError::invalid("the library root can't be deleted"));
    }

    // Read before the write: `trash_subtree_rows` overwrites every one of
    // these paths, so this is the only moment they can be captured for undo.
    let subtree = db::folders::subtree_rows(conn, &rel_path)?;

    let trash_rel = move_to_trash(paths, &rel_path)?;
    let trashed_at = db::folders::trash_subtree_rows(conn, &rel_path)?;
    db::journal::record_folder_trash(
        conn, batch_id, folder_id, &rel_path, &title, &trash_rel, &subtree, trashed_at,
    )?;
    Ok(())
}

/// Move something back out of trash to `rel_path`. Undo's half of
/// `move_to_trash`; the caller puts the database rows back.
pub fn restore_from_trash(paths: &LibraryPaths, trash_rel: &str, rel_path: &str) -> Result<()> {
    // `move_to_trash`'s sentinel for "there was nothing on disk to move" —
    // the directory was already missing when it was deleted. The database
    // rows are still worth putting back; there is just nothing to move.
    if trash_rel.is_empty() {
        return Ok(());
    }
    let src = paths.trash_dir().join(trash_rel);
    if !src.exists() {
        return Err(AppError::invalid(format!(
            "{trash_rel} is no longer in the trash"
        )));
    }
    let dest = paths.to_abs(rel_path)?;
    if dest.exists() {
        return Err(AppError::invalid(format!(
            "something already exists at {rel_path}"
        )));
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(&src, &dest)?;
    Ok(())
}

/// What `trash_items` reports back — trashed count plus any per-item
/// failures, so one locked file in a large selection doesn't cost every
/// other item in it.
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
pub fn trash_items(
    paths: &LibraryPaths,
    conn: &Connection,
    item_ids: &[i64],
    batch_id: &str,
) -> Result<TrashItemsReport> {
    let mut report = TrashItemsReport {
        batch_id: batch_id.to_string(),
        ..Default::default()
    };
    for &item_id in item_ids {
        match trash_one_item(paths, conn, item_id, batch_id) {
            Ok(()) => report.trashed += 1,
            Err(err) => report.errors.push(ItemOpError {
                item_id,
                error: err.to_string(),
            }),
        }
    }
    Ok(report)
}

fn trash_one_item(paths: &LibraryPaths, conn: &Connection, item_id: i64, batch_id: &str) -> Result<()> {
    let item = db::items::rename_target(conn, item_id)?
        .ok_or_else(|| AppError::invalid("item no longer exists"))?;
    let rel_path = if item.folder_rel.is_empty() {
        item.disk_name.clone()
    } else {
        format!("{}/{}", item.folder_rel, item.disk_name)
    };

    let trash_rel = move_to_trash(paths, &rel_path)?;
    db::items::trash_one(conn, item_id)?;
    db::journal::record_item_trash(conn, batch_id, item_id, &item.folder_rel, &item.disk_name, &trash_rel)?;
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

    #[test]
    fn trashing_a_folder_moves_it_and_soft_deletes_its_contents() {
        let root = scratch("trash-folder");
        std::fs::create_dir_all(root.join("People/Ana")).unwrap();
        std::fs::write(root.join("People/Ana/photo.jpg"), b"bytes").unwrap();

        let (paths, conn) = open_db(&root);
        db::folders::upsert(&conn, "", "Library").unwrap();
        db::folders::upsert(&conn, "people", "People").unwrap();
        let ana = db::folders::upsert(&conn, "people/ana", "Ana").unwrap();
        let item_id = db::items::upsert(
            &conn,
            &NewItem {
                uuid: uuid::Uuid::new_v4().to_string(),
                folder_id: ana,
                disk_name: "photo.jpg".to_string(),
                ext: "jpg".to_string(),
                orig_name: "photo.jpg".to_string(),
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
        .unwrap();

        trash_folder(&paths, &conn, ana, &db::journal::new_batch()).unwrap();

        assert!(!root.join("People/Ana").exists(), "moved off its original path");
        assert!(
            paths.trash_dir().join("people/ana/photo.jpg").is_file(),
            "landed in trash, path preserved"
        );

        assert!(
            db::folders::id_for_rel(&conn, "people/ana").unwrap().is_none(),
            "no longer resolvable at its old path"
        );
        let item_deleted: Option<i64> = conn
            .query_row(
                "SELECT deleted_at FROM item WHERE id = ?1",
                [item_id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(item_deleted.is_some());
    }

    #[test]
    fn a_second_folder_can_be_created_at_a_trashed_folders_old_path() {
        let root = scratch("trash-folder-reuse-path");
        std::fs::create_dir_all(root.join("People/Ana")).unwrap();

        let (paths, conn) = open_db(&root);
        db::folders::upsert(&conn, "", "Library").unwrap();
        db::folders::upsert(&conn, "people", "People").unwrap();
        let ana = db::folders::upsert(&conn, "people/ana", "Ana").unwrap();
        trash_folder(&paths, &conn, ana, &db::journal::new_batch()).unwrap();

        // Recreate a directory at the freed path and let the walker's own
        // `upsert` (id_for_rel-backed) pick it up as a brand-new folder.
        std::fs::create_dir_all(root.join("People/Ana")).unwrap();
        let new_ana = db::folders::upsert(&conn, "people/ana", "Ana").unwrap();
        assert_ne!(new_ana, ana, "a fresh row, not the trashed one resurrected");
    }

    #[test]
    fn trashing_an_item_preserves_its_relative_path() {
        let root = scratch("trash-item");
        std::fs::write(root.join("a.jpg"), b"bytes").unwrap();

        let (paths, conn) = open_db(&root);
        let root_id = db::folders::upsert(&conn, "", "Library").unwrap();
        let item_id = db::items::upsert(
            &conn,
            &NewItem {
                uuid: uuid::Uuid::new_v4().to_string(),
                folder_id: root_id,
                disk_name: "a.jpg".to_string(),
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
        .unwrap();

        let batch = db::journal::new_batch();
        let report = trash_items(&paths, &conn, &[item_id], &batch).unwrap();
        assert_eq!(report.trashed, 1);
        assert!(report.errors.is_empty());
        assert!(paths.trash_dir().join("a.jpg").is_file());
        assert!(!root.join("a.jpg").exists());
    }

    // docs/DESIGN.md §M2.5d — "a folder whose directory is missing must fail
    // usefully". Delete is the one action that must still *succeed*: there
    // is nothing left on disk to lose, so refusing would be the one way out
    // this app doesn't offer.
    #[test]
    fn deleting_a_folder_whose_directory_is_already_gone_still_removes_the_record() {
        let root = scratch("trash-folder-already-gone");
        std::fs::create_dir_all(root.join("People/Ana")).unwrap();

        let (paths, conn) = open_db(&root);
        db::folders::upsert(&conn, "", "Library").unwrap();
        db::folders::upsert(&conn, "people", "People").unwrap();
        let ana = db::folders::upsert(&conn, "people/ana", "Ana").unwrap();

        // Simulate the record outliving its directory — moved or deleted
        // from outside the app.
        std::fs::remove_dir_all(root.join("People/Ana")).unwrap();

        trash_folder(&paths, &conn, ana, &db::journal::new_batch()).unwrap();

        assert!(
            db::folders::id_for_rel(&conn, "people/ana").unwrap().is_none(),
            "no longer resolvable at its old path"
        );
        assert!(!paths.trash_dir().join("people/ana").exists(), "nothing to move in");
    }

    #[test]
    fn restoring_a_folder_that_had_nothing_to_trash_is_a_no_op() {
        let root = scratch("restore-nothing-to-trash");
        let (paths, _conn) = open_db(&root);
        // The empty-string sentinel `move_to_trash` returns when its source
        // was already missing — restoring it must not try to move anything.
        restore_from_trash(&paths, "", "people/ana").unwrap();
        assert!(!root.join("people/ana").exists());
    }
}
