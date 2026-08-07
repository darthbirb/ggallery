//! The one-time fold-and-merge behind decision 31 -- "everything the
//! tag system stores is lowercase". Every write path (`db::tags::
//! get_or_create_tag`, `db::folders::{upsert,create_record,
//! set_title_unjournalled}`) already folds new text on the way in; this is
//! what repairs a library that had "Beach" and "beach" (or two sibling
//! folders "Ana" and "ana") sitting side by side before that shipped.
//!
//! **Not a numbered `.sql` migration**, deliberately -- `007_lowercase_
//! vocabulary.sql` exists as the paper-trail entry decision 31 asks for, but
//! is a no-op. Merging a real collision needs application logic a `.sql`
//! file can't safely express: which tag wins, repointing three tables' worth
//! of foreign keys without tripping their composite primary keys, and -- for
//! a folder collision -- physically moving files between two real
//! directories, something only `fs::relocate`'s Rust functions used to know
//! how to do.
//!
//! **Split in two since ROADMAP.md §M2.6.** [`merge_tags`] is schema-agnostic
//! and keeps running from `Library::open` right after `db::migrate`, gated
//! by its own `setting` marker, same as always. [`merge_folders`] is not: it
//! resolves an on-disk *directory* collision -- `Ana` and `ana` as two real
//! directories -- which is only possible against the pre-M2.6 schema, where
//! `folder.rel_path` still names one. Once schema migration 008 applies,
//! `UNIQUE(parent_id, title)` makes that collision structurally impossible
//! to create in the first place, so there is nothing left for it to repair.
//! It runs exactly once, as a precondition `commands::storage_migration`
//! calls before `fs::shard::write_manifest` -- a real pre-M2.5d library may
//! still carry a collision this never got the chance to resolve, and
//! migration 008's own `UNIQUE` index creation would otherwise fail loudly
//! on exactly that instead (see that migration's comments). Not
//! marker-gated -- naturally idempotent, since a second call simply finds no
//! collision groups left.
//!
//! **Reactive, not a reconciler.** Neither function scans for or repairs
//! anything beyond an *exact* collision decision 31's own fold created.

use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::db;
use crate::error::Result;
use crate::fs::paths::LibraryPaths;

const MARKER: &str = "lowercase_folded_at";

/// What the fold merged, so the caller can tell the user rather than doing
/// this silently (ROADMAP.md §M2.5d).
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
    /// "BEACH"]` -- original case, so the report reads like what the user
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
    /// `None` for a top-level folder -- the library root is never one of
    /// these (SPEC.md §2 "Navigation roots").
    pub parent_title: Option<String>,
}

/// Idempotent and cheap after the first real run: the marker check is one
/// row lookup. Schema-agnostic -- tags never had a directory to collide on,
/// so this is safe to call at any schema version, old or new.
pub fn merge_tags(conn: &Connection) -> Result<Vec<TagMerge>> {
    if db::settings::get(conn, MARKER)?.is_some() {
        return Ok(Vec::new());
    }

    let mut merged = Vec::new();
    merge_tags_inner(conn, &mut merged)?;

    db::settings::set(conn, MARKER, &db::now().to_string())?;
    Ok(merged)
}

fn merge_tags_inner(conn: &Connection, merged: &mut Vec<TagMerge>) -> Result<()> {
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

        // Lowest id wins -- deterministic, and matches the "first one sticks"
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

        merged.push(TagMerge {
            originals,
            folded: folded_value,
            key: folded_key,
        });
    }

    Ok(())
}

/// `folder_tag`'s `source` in order of specificity -- the one column that can
/// genuinely disagree between two rows a merge is about to collapse into
/// one, when the same folder happens to link both the winner and the loser
/// under different sources.
const SOURCE_PRIORITY: [&str; 3] = ["title", "archetype", "manual"];

fn source_rank(source: &str) -> usize {
    SOURCE_PRIORITY.iter().position(|s| *s == source).unwrap_or(SOURCE_PRIORITY.len())
}

/// Every attachment `loser` carries -- `folder_tag`, `item_tag`,
/// `item_effective_tag`, `tag_alias` -- moved onto `winner`, then `loser`'s
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

// --- folders (pre-M2.6 schema only -- see module docs) ---------------------

/// Resolves every sibling-title collision left over from before decision 31's
/// write-time fold shipped, by physically merging the loser directory into
/// the winner's. Called once by `commands::storage_migration` before
/// `fs::shard::write_manifest`, against the still-`rel_path`-shaped schema.
pub fn merge_folders(paths: &LibraryPaths, conn: &Connection) -> Result<Vec<FolderMerge>> {
    let mut merged = Vec::new();
    // Folders a merge attempt already failed on this run (its directory
    // missing, most likely) are excluded from every later pass rather than
    // retried forever: one broken folder must never be able to block the
    // rest of the library from folding, or the migration from running.
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

        let merge_ok = losers.iter().try_fold(true, |ok, &loser_id| {
            Result::Ok(ok && merge_one_folder(paths, conn, winner_id, loser_id).is_ok())
        });
        if !matches!(merge_ok, Ok(true)) {
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

        merged.push(FolderMerge {
            originals,
            folded: folded_title,
            parent_title,
        });
    }

    Ok(merged)
}

/// Folds `loser` into `winner`: every subfolder and item it directly holds
/// physically relocated into `winner` (reusing `fs::relocate`, the same code
/// a user-driven move runs), its manual and archetype tags carried over, its
/// notes/cover/favourite merged in wherever the winner had a gap, then its
/// now-empty directory removed and its record soft-deleted.
///
/// **Must leave nothing for a later walk to rediscover** -- the whole reason
/// this is a physical merge and not just a database one: an orphaned
/// directory with no folder row pointing at it would otherwise still be
/// sitting there when `fs::shard`'s migration walks the pre-migration schema
/// looking for every item's current path.
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
        move_folder_dir_only(paths, conn, child_id, winner_id)?;
    }

    let item_ids: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT id FROM item WHERE folder_id = ?1 AND deleted_at IS NULL")?;
        let rows = stmt
            .query_map(params![loser_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        rows
    };
    for item_id in item_ids {
        conn.execute(
            "UPDATE item SET folder_id = ?1 WHERE id = ?2",
            params![winner_id, item_id],
        )?;
    }

    // Manual and archetype labels/flags -- not the loser's own title-tag,
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

    // Empty now -- everything above already moved out. Tolerant of it already
    // being gone.
    if let Ok(abs) = paths.to_abs(&loser_rel) {
        let _ = std::fs::remove_dir(&abs);
    }

    // Soft-deleted, with `rel_path` freed the same way `fs::trash::
    // trash_folder` used to -- a UNIQUE column left pointing at a
    // merged-away row would refuse the next folder created at that path
    // for the remainder of this (pre-migration) schema's life.
    conn.execute(
        "UPDATE folder SET deleted_at = ?1, rel_path = '.merged/' || id WHERE id = ?2",
        params![db::now(), loser_id],
    )?;
    Ok(())
}

/// Physically relocate one folder's own directory into a new parent,
/// rewriting its `rel_path` and every descendant's — the pre-migration
/// (v7-schema) directory move this repair still needs, now that
/// `fs::relocate::move_folder` no longer touches disk at all. Deliberately
/// separate from that function rather than a call into it: this module is
/// the last piece of code in the application that still assumes a folder
/// has a directory, and keeping that assumption contained here is the point.
fn move_folder_dir_only(paths: &LibraryPaths, conn: &Connection, folder_id: i64, new_parent_id: i64) -> Result<()> {
    let (old_rel, title): (String, String) = conn.query_row(
        "SELECT rel_path, title FROM folder WHERE id = ?1",
        params![folder_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let new_parent_rel: String = conn.query_row(
        "SELECT rel_path FROM folder WHERE id = ?1",
        params![new_parent_id],
        |r| r.get(0),
    )?;
    let name = old_rel.rsplit_once('/').map(|(_, n)| n.to_string()).unwrap_or(old_rel.clone());
    let new_rel = if new_parent_rel.is_empty() {
        name
    } else {
        format!("{new_parent_rel}/{name}")
    };

    let old_abs = paths.to_abs(&old_rel)?;
    let new_abs = paths.to_abs(&new_rel)?;
    if !old_abs.is_dir() {
        return Err(crate::error::AppError::invalid(format!(
            "{} is missing from disk — it may have been moved or deleted outside the app",
            old_abs.display()
        )));
    }
    std::fs::rename(&old_abs, &new_abs)?;

    let old_len = old_rel.chars().count() as i64;
    conn.execute(
        "UPDATE folder SET parent_id = ?1, rel_path = ?2 WHERE id = ?3",
        params![new_parent_id, new_rel, folder_id],
    )?;
    conn.execute(
        "UPDATE folder SET rel_path = ?1 || substr(rel_path, ?3 + 1) WHERE rel_path LIKE ?2 || '/%'",
        params![new_rel, old_rel, old_len],
    )?;
    let _ = title;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-libraries")
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create scratch dir");
        root
    }

    /// Opens a raw, unmigrated (v7-shaped) database directly -- these tests
    /// exercise the pre-M2.6 schema this module still targets, so they build
    /// it by hand rather than going through today's `db::migrate`, which now
    /// also applies migration 008.
    fn open_v7_db(root: &std::path::Path) -> (LibraryPaths, Connection) {
        let paths = LibraryPaths::new(root);
        paths.ensure_dirs().unwrap();
        let conn = Connection::open(paths.db_path()).unwrap();
        conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);").unwrap();
        for (version, sql) in [
            (1, include_str!("../db/migrations/001_initial.sql")),
            (2, include_str!("../db/migrations/002_folder_metadata.sql")),
            (3, include_str!("../db/migrations/003_drop_seeded_archetypes.sql")),
            (4, include_str!("../db/migrations/004_folder_soft_delete.sql")),
            (5, include_str!("../db/migrations/005_drop_archetype_field_type.sql")),
            (6, include_str!("../db/migrations/006_drop_root_title_tag.sql")),
            (7, include_str!("../db/migrations/007_lowercase_vocabulary.sql")),
        ] {
            conn.execute_batch(sql).unwrap();
            conn.execute("INSERT INTO schema_version (version) VALUES (?1)", [version]).unwrap();
        }
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
        let (_paths, conn) = open_v7_db(&root);
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

        let merged = merge_tags(&conn).unwrap();

        assert_eq!(merged.len(), 1);
        let merge = &merged[0];
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
        let (_paths, conn) = open_v7_db(&root);
        raw_tag(&conn, None, "Beach");
        raw_tag(&conn, None, "beach");

        let first = merge_tags(&conn).unwrap();
        assert_eq!(first.len(), 1);

        let second = merge_tags(&conn).unwrap();
        assert!(second.is_empty());
    }

    #[test]
    fn merges_two_sibling_folders_that_collide_once_folded() {
        let root = scratch("lowercase-folder-merge");
        std::fs::create_dir_all(root.join("Ana")).unwrap();
        std::fs::create_dir_all(root.join("ana-2/2024 Trip")).unwrap();
        std::fs::write(root.join("Ana/keep.jpg"), b"a").unwrap();
        std::fs::write(root.join("ana-2/2024 Trip/photo.jpg"), b"b").unwrap();
        let (paths, conn) = open_v7_db(&root);

        let root_id = raw_folder(&conn, None, "", "library");
        let winner = raw_folder(&conn, Some(root_id), "ana", "Ana");
        let loser = raw_folder(&conn, Some(root_id), "ana-2", "ana");
        let trip = raw_folder(&conn, Some(loser), "ana-2/2024 trip", "2024 trip");

        let trip_item_id = db::items::upsert(
            &conn,
            &db::items::NewItem {
                uuid: uuid::Uuid::new_v4().to_string(),
                folder_id: Some(trip),
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

        let merged = merge_folders(&paths, &conn).unwrap();

        assert_eq!(merged.len(), 1);
        let merge = &merged[0];
        assert_eq!(merge.folded, "ana");
        assert_eq!(merge.originals.len(), 2);

        // The loser's directory is gone; the winner's absorbed its subtree.
        assert!(!root.join("ana-2").exists());
        assert!(root.join("Ana/2024 Trip/photo.jpg").is_file());

        let winner_title: String = conn
            .query_row("SELECT title FROM folder WHERE id = ?1", params![winner], |r| r.get(0))
            .unwrap();
        assert_eq!(winner_title, "ana");

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
    }

    #[test]
    fn a_folder_merge_carries_over_the_losers_manual_tags_and_notes() {
        let root = scratch("lowercase-folder-merge-tags");
        std::fs::create_dir_all(root.join("Ana")).unwrap();
        std::fs::create_dir_all(root.join("ana-2")).unwrap();
        let (paths, conn) = open_v7_db(&root);

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

        merge_folders(&paths, &conn).unwrap();

        let (notes,): (Option<String>,) = conn
            .query_row("SELECT notes FROM folder WHERE id = ?1", params![winner], |r| Ok((r.get(0)?,)))
            .unwrap();
        assert_eq!(notes.as_deref(), Some("from the loser"), "filled the winner's gap");

        let has_summer: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM folder_tag ft JOIN tag t ON t.id = ft.tag_id
                  WHERE ft.folder_id = ?1 AND t.value = 'summer')",
                params![winner],
                |r| r.get(0),
            )
            .unwrap();
        assert!(has_summer);
    }

    #[test]
    fn a_folder_merge_survives_a_sibling_whose_directory_is_already_gone() {
        // One broken folder must not block the rest of the library from
        // folding. The failure has to come from something the merge
        // actually *does* -- an empty loser with nothing to relocate never
        // touches its own directory except a best-effort cleanup at the very
        // end -- so this gives the loser a subfolder to move, forcing the
        // physical rename to trip over the directory that was never there.
        let root = scratch("lowercase-folder-merge-missing-dir");
        std::fs::create_dir_all(root.join("Ana")).unwrap();
        // "ana-2" and everything under it deliberately never created on disk.
        let (paths, conn) = open_v7_db(&root);

        let root_id = raw_folder(&conn, None, "", "library");
        raw_folder(&conn, Some(root_id), "ana", "Ana");
        let loser = raw_folder(&conn, Some(root_id), "ana-2", "ana");
        raw_folder(&conn, Some(loser), "ana-2/2024 trip", "2024 trip");
        raw_tag(&conn, None, "Beach");
        raw_tag(&conn, None, "beach");

        let tags_merged = merge_tags(&conn).unwrap();
        let folders_merged = merge_folders(&paths, &conn).unwrap();

        // The tag merge, unrelated to the broken folder, still completed.
        assert_eq!(tags_merged.len(), 1);
        // The folder merge could not proceed, so it is not reported as done.
        assert!(folders_merged.is_empty());
    }
}
