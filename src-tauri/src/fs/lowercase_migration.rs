//! The one-time fold-and-merge behind PLAN.md decision 31 — "everything the
//! tag system stores is lowercase". Every write path (`db::tags::
//! get_or_create_tag`, `db::folders::{upsert,create_record,
//! set_title_unjournalled}`) already folds new text on the way in; this is
//! what repairs a library that had "Beach" and "beach" (or two sibling
//! folders "Ana" and "ana") sitting side by side before that shipped.
//!
//! **Not a numbered `.sql` migration**, deliberately — `007_lowercase_
//! vocabulary.sql` exists as the paper-trail entry decision 31 asks for, but
//! is a no-op. Merging a real collision needs application logic a `.sql`
//! file can't safely express: which tag wins, repointing three tables' worth
//! of foreign keys without tripping their composite primary keys, and — for
//! a folder collision — physically moving files between two real
//! directories, something only `fs::relocate`'s Rust functions know how to
//! do. Run once, gated by a `setting` marker rather than `schema_version`,
//! from `Library::open` right after `db::migrate`.
//!
//! **Reactive, not a reconciler.** This runs exactly once per library and
//! only ever merges an *exact* collision decision 31's own fold created —
//! it does not scan for or repair anything else.

use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::db;
use crate::error::Result;
use crate::fs::paths::LibraryPaths;

const MARKER: &str = "lowercase_folded_at";

/// What the fold merged, so the caller can tell the user rather than doing
/// this silently (PLAN.md §M2.5d).
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LowercaseMergeReport {
    pub tags_merged: Vec<TagMerge>,
    pub folders_merged: Vec<FolderMerge>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagMerge {
    /// The distinct spellings that collapsed into one, e.g. `["Beach",
    /// "BEACH"]` — original case, so the report reads like what the user
    /// actually typed rather than a second copy of the folded form.
    pub originals: Vec<String>,
    pub folded: String,
    pub key: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderMerge {
    pub originals: Vec<String>,
    pub folded: String,
    /// `None` for a top-level folder — the library root is never one of
    /// these (docs/DESIGN.md §2 "Navigation roots").
    pub parent_title: Option<String>,
}

/// Idempotent and cheap after the first real run: the marker check is one
/// row lookup.
pub fn run(paths: &LibraryPaths, conn: &Connection) -> Result<LowercaseMergeReport> {
    if db::settings::get(conn, MARKER)?.is_some() {
        return Ok(LowercaseMergeReport::default());
    }

    let mut report = LowercaseMergeReport::default();
    merge_tags(conn, &mut report)?;
    merge_folders(paths, conn, &mut report)?;

    db::settings::set(conn, MARKER, &db::now().to_string())?;
    Ok(report)
}

// --- tags -------------------------------------------------------------

fn merge_tags(conn: &Connection, report: &mut LowercaseMergeReport) -> Result<()> {
    let rows: Vec<(i64, Option<String>, String)> = {
        let mut stmt = conn.prepare("SELECT id, key, value FROM tag")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        rows
    };

    let mut groups: HashMap<(Option<String>, String), Vec<(i64, Option<String>, String)>> =
        HashMap::new();
    for row in rows {
        let folded_key = row.1.as_deref().map(db::fold);
        let folded_value = db::fold(&row.2);
        groups.entry((folded_key, folded_value)).or_default().push(row);
    }

    for ((folded_key, folded_value), mut members) in groups {
        if members.len() == 1 {
            let (id, key, value) = &members[0];
            if key.as_deref() != folded_key.as_deref() || *value != folded_value {
                conn.execute(
                    "UPDATE tag SET key = ?1, value = ?2 WHERE id = ?3",
                    params![folded_key, folded_value, id],
                )?;
            }
            continue;
        }

        // Lowest id wins — deterministic, and matches the "first one sticks"
        // idiom `INSERT OR IGNORE` already uses elsewhere in this module.
        members.sort_by_key(|(id, _, _)| *id);
        let winner_id = members[0].0;
        let originals: Vec<String> = members
            .iter()
            .map(|(_, key, value)| match key {
                Some(k) => format!("{k}: {value}"),
                None => value.clone(),
            })
            .collect();

        conn.execute(
            "UPDATE tag SET key = ?1, value = ?2 WHERE id = ?3",
            params![folded_key, folded_value, winner_id],
        )?;
        for (loser_id, _, _) in members.iter().skip(1) {
            repoint_tag(conn, *loser_id, winner_id)?;
            conn.execute("DELETE FROM tag WHERE id = ?1", params![loser_id])?;
        }

        report.tags_merged.push(TagMerge {
            originals,
            folded: folded_value,
            key: folded_key,
        });
    }

    Ok(())
}

/// `folder_tag`'s `source` in order of specificity — the one column that can
/// genuinely disagree between two rows a merge is about to collapse into
/// one, when the same folder happens to link both the winner and the loser
/// under different sources.
const SOURCE_PRIORITY: [&str; 3] = ["title", "archetype", "manual"];

fn source_rank(source: &str) -> usize {
    SOURCE_PRIORITY.iter().position(|s| *s == source).unwrap_or(SOURCE_PRIORITY.len())
}

/// Every attachment `loser` carries — `folder_tag`, `item_tag`,
/// `item_effective_tag`, `tag_alias` — moved onto `winner`, then `loser`'s
/// own rows dropped. `INSERT OR IGNORE` plus delete rather than a bulk
/// `UPDATE tag_id`, because a folder or item can already carry both tags
/// (someone tagged the same folder "Beach" and "beach" separately), and the
/// composite primary keys on these tables would reject a naive rewrite the
/// instant that happens.
fn repoint_tag(conn: &Connection, loser: i64, winner: i64) -> Result<()> {
    let loser_links: Vec<(i64, String)> = {
        let mut stmt =
            conn.prepare("SELECT folder_id, source FROM folder_tag WHERE tag_id = ?1")?;
        let rows = stmt
            .query_map(params![loser], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        rows
    };
    for (folder_id, loser_source) in loser_links {
        let existing_source: Option<String> = conn
            .query_row(
                "SELECT source FROM folder_tag WHERE folder_id = ?1 AND tag_id = ?2",
                params![folder_id, winner],
                |r| r.get(0),
            )
            .optional()?;
        let source = match &existing_source {
            Some(existing) if source_rank(existing) <= source_rank(&loser_source) => {
                existing.clone()
            }
            _ => loser_source,
        };
        conn.execute(
            "INSERT INTO folder_tag (folder_id, tag_id, source) VALUES (?1, ?2, ?3)
             ON CONFLICT(folder_id, tag_id) DO UPDATE SET source = excluded.source",
            params![folder_id, winner, source],
        )?;
    }
    conn.execute("DELETE FROM folder_tag WHERE tag_id = ?1", params![loser])?;

    conn.execute(
        "INSERT OR IGNORE INTO item_tag (item_id, tag_id, added_at)
         SELECT item_id, ?2, added_at FROM item_tag WHERE tag_id = ?1",
        params![loser, winner],
    )?;
    conn.execute("DELETE FROM item_tag WHERE tag_id = ?1", params![loser])?;

    conn.execute(
        "INSERT OR IGNORE INTO item_effective_tag (item_id, tag_id, origin_id)
         SELECT item_id, ?2, origin_id FROM item_effective_tag WHERE tag_id = ?1",
        params![loser, winner],
    )?;
    conn.execute("DELETE FROM item_effective_tag WHERE tag_id = ?1", params![loser])?;

    conn.execute(
        "INSERT OR IGNORE INTO tag_alias (alias, tag_id)
         SELECT alias, ?2 FROM tag_alias WHERE tag_id = ?1",
        params![loser, winner],
    )?;
    conn.execute("DELETE FROM tag_alias WHERE tag_id = ?1", params![loser])?;

    Ok(())
}

// --- folders ------------------------------------------------------------

fn merge_folders(paths: &LibraryPaths, conn: &Connection, report: &mut LowercaseMergeReport) -> Result<()> {
    // Folders a merge attempt already failed on this run (its directory
    // missing, most likely — see `fs::relocate::require_dir`) are excluded
    // from every later pass rather than retried forever: one broken folder
    // must never be able to block the rest of the library from folding, or
    // from opening at all.
    let mut poisoned: HashSet<i64> = HashSet::new();

    loop {
        let rows: Vec<(i64, Option<i64>, String)> = {
            let mut stmt =
                conn.prepare("SELECT id, parent_id, title FROM folder WHERE deleted_at IS NULL")?;
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<rusqlite::Result<_>>()?;
            rows
        };

        let mut groups: HashMap<(Option<i64>, String), Vec<(i64, String)>> = HashMap::new();
        for (id, parent_id, title) in rows {
            if poisoned.contains(&id) {
                continue;
            }
            groups.entry((parent_id, db::fold(&title))).or_default().push((id, title));
        }

        let Some(((parent_id, folded_title), mut members)) =
            groups.into_iter().find(|(_, members)| members.len() > 1)
        else {
            break;
        };

        members.sort_by_key(|(id, _)| *id);
        let (winner_id, _) = members[0];
        let originals: Vec<String> = members.iter().map(|(_, title)| title.clone()).collect();
        let losers: Vec<i64> = members.into_iter().skip(1).map(|(id, _)| id).collect();

        let merged = losers.iter().try_fold(true, |ok, &loser_id| {
            Result::Ok(ok && merge_one_folder(paths, conn, winner_id, loser_id).is_ok())
        });
        if !matches!(merged, Ok(true)) {
            poisoned.insert(winner_id);
            poisoned.extend(&losers);
            continue;
        }

        db::folders::set_title_unjournalled(conn, winner_id, &folded_title)?;

        let parent_title: Option<String> = match parent_id {
            Some(id) => conn
                .query_row("SELECT title FROM folder WHERE id = ?1", params![id], |r| r.get(0))
                .optional()?,
            None => None,
        };

        report.folders_merged.push(FolderMerge {
            originals,
            folded: folded_title,
            parent_title,
        });
    }

    Ok(())
}

/// Folds `loser` into `winner`: every subfolder and item it directly holds
/// physically relocated into `winner` (reusing `fs::relocate`, the same code
/// a user-driven move runs), its manual and archetype tags carried over, its
/// notes/cover/favourite merged in wherever the winner had a gap, then its
/// now-empty directory removed and its record soft-deleted.
///
/// **Must leave nothing for the filesystem watcher to rediscover** — the
/// whole reason this is a physical merge and not just a database one: an
/// orphaned directory with no folder row pointing at it would be walked back
/// into existence as a "new" folder the moment the watcher (or the next
/// walk) saw it.
fn merge_one_folder(paths: &LibraryPaths, conn: &Connection, winner_id: i64, loser_id: i64) -> Result<()> {
    let loser_rel: String =
        conn.query_row("SELECT rel_path FROM folder WHERE id = ?1", params![loser_id], |r| r.get(0))?;

    let children: Vec<i64> = {
        let mut stmt =
            conn.prepare("SELECT id FROM folder WHERE parent_id = ?1 AND deleted_at IS NULL")?;
        let rows = stmt
            .query_map(params![loser_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        rows
    };
    for child_id in children {
        crate::fs::relocate::move_folder_unjournalled(paths, conn, child_id, Some(winner_id))?;
    }

    let item_ids: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT id FROM item WHERE folder_id = ?1 AND deleted_at IS NULL")?;
        let rows = stmt
            .query_map(params![loser_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        rows
    };
    if !item_ids.is_empty() {
        crate::fs::relocate::move_items(paths, conn, &item_ids, winner_id, &db::journal::new_batch())?;
    }

    // Manual and archetype labels/flags — not the loser's own title-tag,
    // which simply stops existing along with the row it belonged to.
    let tags: Vec<(i64, String)> = {
        let mut stmt = conn.prepare(
            "SELECT tag_id, source FROM folder_tag WHERE folder_id = ?1 AND source != 'title'",
        )?;
        let rows = stmt
            .query_map(params![loser_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        rows
    };
    for (tag_id, source) in tags {
        conn.execute(
            "INSERT OR IGNORE INTO folder_tag (folder_id, tag_id, source) VALUES (?1, ?2, ?3)",
            params![winner_id, tag_id, source],
        )?;
    }

    let (winner_notes, winner_cover, winner_favorite): (Option<String>, Option<i64>, i64) = conn
        .query_row(
            "SELECT notes, cover_item_id, favorite FROM folder WHERE id = ?1",
            params![winner_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
    let (loser_notes, loser_cover, loser_favorite): (Option<String>, Option<i64>, i64) = conn
        .query_row(
            "SELECT notes, cover_item_id, favorite FROM folder WHERE id = ?1",
            params![loser_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
    conn.execute(
        "UPDATE folder SET notes = ?1, cover_item_id = ?2, favorite = ?3 WHERE id = ?4",
        params![
            winner_notes.or(loser_notes),
            winner_cover.or(loser_cover),
            (winner_favorite != 0 || loser_favorite != 0) as i64,
            winner_id,
        ],
    )?;

    // Empty now — everything above already moved out. Tolerant of it already
    // being gone, the same as every other action `fs::relocate::require_dir`
    // and `fs::trash::move_to_trash` guard against (docs/DESIGN.md §M2.5d).
    if let Ok(abs) = paths.to_abs(&loser_rel) {
        let _ = std::fs::remove_dir(&abs);
    }

    // Soft-deleted, like every other removal in the app, with `rel_path`
    // freed the same way `fs::trash::trash_folder` frees it — a UNIQUE
    // column left pointing at a merged-away row would refuse the next
    // folder anyone ever creates at that path.
    conn.execute(
        "UPDATE folder SET deleted_at = ?1, rel_path = '.merged/' || id WHERE id = ?2",
        params![db::now(), loser_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::journal;

    fn scratch(name: &str) -> std::path::PathBuf {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-libraries")
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create scratch dir");
        root
    }

    fn open_db(root: &std::path::Path) -> (LibraryPaths, Connection) {
        let paths = LibraryPaths::new(root);
        paths.ensure_dirs().unwrap();
        let mut conn = db::open(&paths.db_path()).unwrap();
        db::migrate(&mut conn).unwrap();
        (paths, conn)
    }

    /// Bypasses the write-time fold so a pre-decision-31 collision can be
    /// simulated, the same trick every test below uses.
    fn raw_tag(conn: &Connection, key: Option<&str>, value: &str) -> i64 {
        conn.execute("INSERT INTO tag (key, value) VALUES (?1, ?2)", params![key, value])
            .unwrap();
        conn.last_insert_rowid()
    }

    fn raw_folder(conn: &Connection, parent_id: Option<i64>, rel_path: &str, title: &str) -> i64 {
        conn.execute(
            "INSERT INTO folder (rel_path, title, parent_id, created_at) VALUES (?1, ?2, ?3, 0)",
            params![rel_path, title, parent_id],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn merges_two_tags_that_collide_once_folded_carrying_both_attachments() {
        let root = scratch("lowercase-tag-merge");
        let (paths, conn) = open_db(&root);
        let root_id = raw_folder(&conn, None, "", "library");
        let ana = raw_folder(&conn, Some(root_id), "ana", "ana");

        let beach = raw_tag(&conn, None, "Beach");
        let beach_lower = raw_tag(&conn, None, "beach");
        conn.execute(
            "INSERT INTO folder_tag (folder_id, tag_id, source) VALUES (?1, ?2, 'manual')",
            params![ana, beach],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO folder_tag (folder_id, tag_id, source) VALUES (?1, ?2, 'manual')",
            params![ana, beach_lower],
        )
        .unwrap();

        let report = run(&paths, &conn).unwrap();

        assert_eq!(report.tags_merged.len(), 1);
        let merge = &report.tags_merged[0];
        assert_eq!(merge.folded, "beach");
        assert_eq!(merge.originals.len(), 2);
        assert!(merge.originals.contains(&"Beach".to_string()));
        assert!(merge.originals.contains(&"beach".to_string()));

        let remaining: i64 =
            conn.query_row("SELECT COUNT(*) FROM tag WHERE value = 'beach'", [], |r| r.get(0)).unwrap();
        assert_eq!(remaining, 1, "one surviving row, not two");

        let links: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM folder_tag WHERE folder_id = ?1",
                params![ana],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(links, 1, "the folder ends up tagged once, not twice");
    }

    #[test]
    fn a_second_run_does_nothing_the_marker_makes_it_idempotent() {
        let root = scratch("lowercase-idempotent");
        let (paths, conn) = open_db(&root);
        raw_tag(&conn, None, "Beach");
        raw_tag(&conn, None, "beach");

        let first = run(&paths, &conn).unwrap();
        assert_eq!(first.tags_merged.len(), 1);

        let second = run(&paths, &conn).unwrap();
        assert!(second.tags_merged.is_empty());
        assert!(second.folders_merged.is_empty());
    }

    #[test]
    fn merges_two_sibling_folders_that_collide_once_folded() {
        let root = scratch("lowercase-folder-merge");
        std::fs::create_dir_all(root.join("Ana")).unwrap();
        std::fs::create_dir_all(root.join("ana-2/2024 Trip")).unwrap();
        std::fs::write(root.join("Ana/keep.jpg"), b"a").unwrap();
        std::fs::write(root.join("ana-2/2024 Trip/photo.jpg"), b"b").unwrap();
        let (paths, conn) = open_db(&root);

        let root_id = raw_folder(&conn, None, "", "library");
        let winner = raw_folder(&conn, Some(root_id), "ana", "Ana");
        let loser = raw_folder(&conn, Some(root_id), "ana-2", "ana");
        let trip = raw_folder(&conn, Some(loser), "ana-2/2024 trip", "2024 trip");

        let item_id = db::items::upsert(
            &conn,
            &db::items::NewItem {
                uuid: uuid::Uuid::new_v4().to_string(),
                folder_id: winner,
                disk_name: "keep.jpg".to_string(),
                ext: "jpg".to_string(),
                orig_name: "keep.jpg".to_string(),
                hash: "h1".to_string(),
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
        let trip_item_id = db::items::upsert(
            &conn,
            &db::items::NewItem {
                uuid: uuid::Uuid::new_v4().to_string(),
                folder_id: trip,
                disk_name: "photo.jpg".to_string(),
                ext: "jpg".to_string(),
                orig_name: "photo.jpg".to_string(),
                hash: "h2".to_string(),
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
        let _ = item_id;

        let report = run(&paths, &conn).unwrap();

        assert_eq!(report.folders_merged.len(), 1);
        let merge = &report.folders_merged[0];
        assert_eq!(merge.folded, "ana");
        assert_eq!(merge.originals.len(), 2);

        // The loser's directory is gone; the winner's absorbed its subtree.
        assert!(!root.join("ana-2").exists());
        assert!(root.join("Ana/2024 Trip/photo.jpg").is_file());

        let winner_detail = db::folders::get_detail(&conn, winner).unwrap().unwrap();
        assert_eq!(winner_detail.title, "ana");

        let trip_now: i64 = conn
            .query_row("SELECT parent_id FROM folder WHERE id = ?1", params![trip], |r| r.get(0))
            .unwrap();
        assert_eq!(trip_now, winner, "the subfolder followed its parent");

        let trip_item_folder: i64 = conn
            .query_row("SELECT folder_id FROM item WHERE id = ?1", params![trip_item_id], |r| r.get(0))
            .unwrap();
        assert_eq!(trip_item_folder, trip);

        let loser_gone: Option<i64> = conn
            .query_row("SELECT deleted_at FROM folder WHERE id = ?1", params![loser], |r| r.get(0))
            .unwrap();
        assert!(loser_gone.is_some());

        let _ = journal::new_batch();
    }

    #[test]
    fn a_folder_merge_carries_over_the_losers_manual_tags_and_notes() {
        let root = scratch("lowercase-folder-merge-tags");
        std::fs::create_dir_all(root.join("Ana")).unwrap();
        std::fs::create_dir_all(root.join("ana-2")).unwrap();
        let (paths, conn) = open_db(&root);

        let root_id = raw_folder(&conn, None, "", "library");
        let winner = raw_folder(&conn, Some(root_id), "ana", "Ana");
        let loser = raw_folder(&conn, Some(root_id), "ana-2", "ana");
        conn.execute(
            "UPDATE folder SET notes = 'from the loser' WHERE id = ?1",
            params![loser],
        )
        .unwrap();

        let summer = db::tags::get_or_create_tag(&conn, None, "summer").unwrap();
        conn.execute(
            "INSERT INTO folder_tag (folder_id, tag_id, source) VALUES (?1, ?2, 'manual')",
            params![loser, summer],
        )
        .unwrap();

        run(&paths, &conn).unwrap();

        let detail = db::folders::get_detail(&conn, winner).unwrap().unwrap();
        assert_eq!(detail.notes.as_deref(), Some("from the loser"), "filled the winner's gap");
        assert!(detail.flags.iter().any(|f| f.value == "summer"));
    }

    #[test]
    fn a_folder_merge_survives_a_sibling_whose_directory_is_already_gone() {
        // One broken folder (task #6's own problem) must not block the rest
        // of the library from folding, or the library from opening at all.
        // The failure has to come from something the merge actually *does*
        // — an empty loser with nothing to relocate never touches its own
        // directory except a best-effort cleanup at the very end (tolerant
        // of it already being gone, same as `fs::trash`), so this gives the
        // loser a subfolder to move, forcing `require_dir` to trip over the
        // directory that was never there.
        let root = scratch("lowercase-folder-merge-missing-dir");
        std::fs::create_dir_all(root.join("Ana")).unwrap();
        // "ana-2" and everything under it deliberately never created on disk.
        let (paths, conn) = open_db(&root);

        let root_id = raw_folder(&conn, None, "", "library");
        raw_folder(&conn, Some(root_id), "ana", "Ana");
        let loser = raw_folder(&conn, Some(root_id), "ana-2", "ana");
        raw_folder(&conn, Some(loser), "ana-2/2024 trip", "2024 trip");
        raw_tag(&conn, None, "Beach");
        raw_tag(&conn, None, "beach");

        let report = run(&paths, &conn).unwrap();

        // The tag merge, unrelated to the broken folder, still completed.
        assert_eq!(report.tags_merged.len(), 1);
        // The folder merge could not proceed, so it is not reported as done.
        assert!(report.folders_merged.is_empty());
    }
}
