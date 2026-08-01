//! The effective-tag cache — DATA-MODEL's "Tag resolution" — and its
//! invalidation.
//!
//! Folder-level tag edits (`folder_tag`/`tag`) are applied synchronously,
//! inline, by the caller — that table is small. Only the downstream fan-out
//! into `item_effective_tag` across a subtree is meant to run off the UI
//! thread; `rebuild_subtree`/`rebuild_item` here are the work, `jobs::kinds`
//! is what queues them. See docs/DATA-MODEL.md#tag-resolution.

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::db::now;
use crate::error::Result;

/// Get or create the shared `tag` row for `(key, value)`. `key` is `None`
/// for a flag. `IS` rather than `=` so `NULL` compares correctly.
pub fn get_or_create_tag(conn: &Connection, key: Option<&str>, value: &str) -> Result<i64> {
    if let Some(id) = conn
        .query_row(
            "SELECT id FROM tag WHERE key IS ?1 AND value = ?2",
            params![key, value],
            |r| r.get(0),
        )
        .optional()?
    {
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO tag (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Every folder's title is a `source = 'title'` flag — automatic, not
/// user-removable (see `001_initial.sql`'s comment on `folder_tag.source`).
/// Called on folder creation and on every title edit: drops this folder's
/// previous title-tag link, if any, and links the tag for the new value,
/// creating it if this exact title has never been used before.
pub fn sync_title_tag(conn: &Connection, folder_id: i64, title: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM folder_tag WHERE folder_id = ?1 AND source = 'title'",
        params![folder_id],
    )?;
    let tag_id = get_or_create_tag(conn, None, title)?;
    conn.execute(
        "INSERT OR IGNORE INTO folder_tag (folder_id, tag_id, source) VALUES (?1, ?2, 'title')",
        params![folder_id, tag_id],
    )?;
    Ok(())
}

/// The DATA-MODEL ancestry walk, minus the item itself — the seed for a
/// subtree rebuild, and the whole of a single item's rebuild. Walks
/// `parent_id` up to the root; bounded by tree depth via the primary-key
/// join on `folder.id` and the existing `idx_folder_parent` index, not by
/// folder count, so this stays cheap however large the library gets.
///
/// Ordered closest-ancestor-first (`depth` ascending) so that a tag value
/// duplicated at two levels — the same flag text applied both here and
/// higher up — resolves to the closer origin when a caller uses `INSERT OR
/// IGNORE` to dedupe by tag id, consistent with `rebuild_subtree`'s own
/// "closer origin wins" rule.
pub fn resolve_ancestor_tags(conn: &Connection, folder_id: i64) -> Result<Vec<(i64, i64)>> {
    let mut stmt = conn.prepare(
        "WITH RECURSIVE ancestry(id, depth) AS (
             SELECT ?1, 0
           UNION ALL
             SELECT f.parent_id, a.depth + 1 FROM folder f JOIN ancestry a ON f.id = a.id
             WHERE f.parent_id IS NOT NULL
         )
         SELECT ft.tag_id, ft.folder_id
           FROM folder_tag ft JOIN ancestry a ON a.id = ft.folder_id
          ORDER BY a.depth",
    )?;
    let rows = stmt
        .query_map(params![folder_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Full recompute for one item: its current folder's inherited tags plus its
/// own manual tags. Used for a brand-new item — called inline from
/// `jobs::worker::run_hash`, not queued, since the cost is one bounded
/// ancestor walk rather than a scan — and for item-level manual tag changes.
pub fn rebuild_item(conn: &Connection, item_id: i64) -> Result<()> {
    let folder_id: i64 = conn.query_row(
        "SELECT folder_id FROM item WHERE id = ?1",
        params![item_id],
        |r| r.get(0),
    )?;
    let inherited = resolve_ancestor_tags(conn, folder_id)?;

    conn.execute(
        "DELETE FROM item_effective_tag WHERE item_id = ?1",
        params![item_id],
    )?;
    for (tag_id, origin_id) in &inherited {
        conn.execute(
            "INSERT OR IGNORE INTO item_effective_tag (item_id, tag_id, origin_id)
             VALUES (?1, ?2, ?3)",
            params![item_id, tag_id, origin_id],
        )?;
    }
    conn.execute(
        "INSERT OR IGNORE INTO item_effective_tag (item_id, tag_id, origin_id)
           SELECT ?1, tag_id, NULL FROM item_tag WHERE item_id = ?1",
        params![item_id],
    )?;
    Ok(())
}

/// Bulk rebuild for a folder-level change: the folder's own tags edited, an
/// archetype applied, or its title changed. Scope is the folder and every
/// descendant, per DATA-MODEL's invalidation table.
///
/// Walks the subtree in `rel_path` order — a prefix always sorts before
/// anything nested under it, so a folder's row is visited after its
/// parent's — accumulating each folder's tag set from its parent's
/// already-computed set plus its own `folder_tag` rows. That is one pass
/// over folders (thousands at most, per `db::folders::tree`'s own
/// reasoning) rather than one recursive ancestry query per item, which is
/// exactly the shape PLAN.md decision 20 warns is catastrophic at scale.
pub fn rebuild_subtree(conn: &Connection, folder_rel: &str) -> Result<()> {
    let Some(folder_id) = folder_id_for(conn, folder_rel)? else {
        return Ok(());
    };
    let parent_id: Option<i64> = conn.query_row(
        "SELECT parent_id FROM folder WHERE id = ?1",
        params![folder_id],
        |r| r.get(0),
    )?;

    // Seed: whatever lies above `folder_rel` contributes tags that have not
    // changed, but the walk below still needs their value to start from.
    let seed: HashMap<i64, i64> = match parent_id {
        Some(pid) => resolve_ancestor_tags(conn, pid)?.into_iter().collect(),
        None => HashMap::new(),
    };

    let subtree = subtree_folders(conn, folder_rel)?;

    crate::db::begin_batch(conn)?;
    let result = (|| -> Result<()> {
        let mut tag_sets: HashMap<i64, HashMap<i64, i64>> = HashMap::new();
        tag_sets.insert(parent_id.unwrap_or(-1), seed);

        for (id, parent) in &subtree {
            let mut set = tag_sets.get(&parent.unwrap_or(-1)).cloned().unwrap_or_default();

            let mut own_stmt = conn.prepare("SELECT tag_id FROM folder_tag WHERE folder_id = ?1")?;
            let own: Vec<i64> = own_stmt
                .query_map(params![id], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            drop(own_stmt);
            // Own tags take priority over an inherited entry for the same
            // tag id (e.g. the same flag text applied at two levels) — the
            // closer origin is the more useful one to show.
            for tag_id in own {
                set.insert(tag_id, *id);
            }

            apply_folder_tags_to_items(conn, *id, &set)?;
            tag_sets.insert(*id, set);
        }
        Ok(())
    })();

    match result {
        Ok(()) => crate::db::commit_batch(conn),
        Err(err) => {
            crate::db::rollback_batch(conn);
            Err(err)
        }
    }
}

fn folder_id_for(conn: &Connection, folder_rel: &str) -> Result<Option<i64>> {
    Ok(conn
        .query_row(
            "SELECT id FROM folder WHERE rel_path = ?1",
            params![folder_rel],
            |r| r.get(0),
        )
        .optional()?)
}

/// `(id, parent_id)` for `folder_rel` and everything beneath it, parent
/// before child. Mirrors `db::items::list`'s own root-vs-prefix branch:
/// `'' || '/%'` is `'/ %'`, which matches nothing, since no `rel_path` ever
/// carries a leading slash — the whole-library case needs no `WHERE` at all.
fn subtree_folders(conn: &Connection, folder_rel: &str) -> Result<Vec<(i64, Option<i64>)>> {
    let sql = if folder_rel.is_empty() {
        "SELECT id, parent_id FROM folder ORDER BY rel_path".to_string()
    } else {
        "SELECT id, parent_id FROM folder
          WHERE rel_path = ?1 OR rel_path LIKE ?1 || '/%'
          ORDER BY rel_path"
            .to_string()
    };
    let mut stmt = conn.prepare(&sql)?;
    let rows = if folder_rel.is_empty() {
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?
    } else {
        stmt.query_map(params![folder_rel], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?
    };
    Ok(rows)
}

/// Replace every inherited (`origin_id IS NOT NULL`) row for the items
/// directly in `folder_id` with `tags`. Manual rows — from `item_tag`,
/// `origin_id IS NULL` — are untouched.
fn apply_folder_tags_to_items(
    conn: &Connection,
    folder_id: i64,
    tags: &HashMap<i64, i64>,
) -> Result<()> {
    conn.execute(
        "DELETE FROM item_effective_tag
          WHERE origin_id IS NOT NULL
            AND item_id IN (SELECT id FROM item WHERE folder_id = ?1 AND deleted_at IS NULL)",
        params![folder_id],
    )?;
    if tags.is_empty() {
        return Ok(());
    }

    // A `VALUES (...) AS v(col, col)`-style derived table is not accepted by
    // the SQLite version bundled here — a `UNION ALL` of literal `SELECT`s
    // is the portable equivalent and just as cheap for a handful of rows.
    let values = vec!["SELECT ? AS tag_id, ? AS origin_id"; tags.len()].join(" UNION ALL ");
    let sql = format!(
        "INSERT INTO item_effective_tag (item_id, tag_id, origin_id)
         SELECT i.id, v.tag_id, v.origin_id
           FROM item i, ({values}) AS v
          WHERE i.folder_id = ? AND i.deleted_at IS NULL"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut bound: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(tags.len() * 2 + 1);
    for (tag_id, origin_id) in tags {
        bound.push(tag_id);
        bound.push(origin_id);
    }
    bound.push(&folder_id);
    stmt.execute(bound.as_slice())?;
    Ok(())
}

// --- manual per-item tags ---------------------------------------------------
//
// No frontend caller in M2 — item-level tag UI is M2.5's preview panel, per
// PLAN.md §M2. These exist so the data model is complete and testable now,
// and so `commands/tags.rs` has something real to expose when M2.5 needs it.

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveTag {
    pub tag_id: i64,
    pub key: Option<String>,
    pub value: String,
    /// `None` for a manual tag; the contributing ancestor folder otherwise.
    pub origin_id: Option<i64>,
}

pub fn item_effective_tags(conn: &Connection, item_id: i64) -> Result<Vec<EffectiveTag>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.key, t.value, e.origin_id
           FROM item_effective_tag e JOIN tag t ON t.id = e.tag_id
          WHERE e.item_id = ?1
          ORDER BY t.key IS NOT NULL, t.value COLLATE NOCASE",
    )?;
    let rows = stmt
        .query_map(params![item_id], |r| {
            Ok(EffectiveTag {
                tag_id: r.get(0)?,
                key: r.get(1)?,
                value: r.get(2)?,
                origin_id: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn add_item_tag(conn: &Connection, item_id: i64, key: Option<&str>, value: &str) -> Result<()> {
    let tag_id = get_or_create_tag(conn, key, value)?;
    conn.execute(
        "INSERT OR IGNORE INTO item_tag (item_id, tag_id, added_at) VALUES (?1, ?2, ?3)",
        params![item_id, tag_id, now()],
    )?;
    rebuild_item(conn, item_id)
}

pub fn remove_item_tag(conn: &Connection, item_id: i64, tag_id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM item_tag WHERE item_id = ?1 AND tag_id = ?2",
        params![item_id, tag_id],
    )?;
    rebuild_item(conn, item_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn memory_conn() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        db::migrate(&mut conn).unwrap();
        conn
    }

    fn folder(conn: &Connection, rel: &str, title: &str) -> i64 {
        let id = db::folders::upsert(conn, rel, title).unwrap();
        id
    }

    fn item(conn: &Connection, folder_id: i64, name: &str) -> i64 {
        db::items::upsert(
            conn,
            &db::items::NewItem {
                uuid: uuid::Uuid::new_v4().to_string(),
                folder_id,
                disk_name: name.to_string(),
                ext: "jpg".to_string(),
                orig_name: name.to_string(),
                hash: "deadbeef".to_string(),
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
        .unwrap()
    }

    fn flag(conn: &Connection, folder_id: i64, value: &str, source: &str) {
        let tag_id = get_or_create_tag(conn, None, value).unwrap();
        conn.execute(
            "INSERT INTO folder_tag (folder_id, tag_id, source) VALUES (?1, ?2, ?3)",
            params![folder_id, tag_id, source],
        )
        .unwrap();
    }

    #[test]
    fn folder_creation_gets_a_title_tag() {
        let conn = memory_conn();
        let ana = folder(&conn, "people/ana", "Ana");
        let tags: Vec<String> = conn
            .prepare("SELECT t.value FROM folder_tag ft JOIN tag t ON t.id = ft.tag_id WHERE ft.folder_id = ?1")
            .unwrap()
            .query_map(params![ana], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(tags, vec!["Ana".to_string()]);
    }

    #[test]
    fn renaming_a_folder_swaps_its_title_tag() {
        let conn = memory_conn();
        let ana = folder(&conn, "people/ana", "Ana");
        sync_title_tag(&conn, ana, "Anastasia").unwrap();

        let tags: Vec<String> = conn
            .prepare("SELECT t.value FROM folder_tag ft JOIN tag t ON t.id = ft.tag_id WHERE ft.folder_id = ?1 AND ft.source = 'title'")
            .unwrap()
            .query_map(params![ana], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(tags, vec!["Anastasia".to_string()]);
    }

    #[test]
    fn a_new_item_inherits_every_ancestors_tags() {
        let conn = memory_conn();
        let people = folder(&conn, "people", "People");
        let ana = folder(&conn, "people/ana", "Ana");
        flag(&conn, people, "family", "manual");
        flag(&conn, ana, "beach", "manual");

        let item_id = item(&conn, ana, "photo.jpg");
        rebuild_item(&conn, item_id).unwrap();

        let mut values: Vec<String> = item_effective_tags(&conn, item_id)
            .unwrap()
            .into_iter()
            .map(|t| t.value)
            .collect();
        values.sort();
        // People, Ana (titles), family, beach (flags).
        assert_eq!(values, vec!["Ana", "People", "beach", "family"]);
    }

    #[test]
    fn moving_a_tag_off_a_folder_drops_it_from_items_beneath_after_rebuild() {
        let conn = memory_conn();
        let ana = folder(&conn, "people/ana", "Ana");
        flag(&conn, ana, "beach", "manual");
        let item_id = item(&conn, ana, "photo.jpg");
        rebuild_item(&conn, item_id).unwrap();
        assert!(item_effective_tags(&conn, item_id)
            .unwrap()
            .iter()
            .any(|t| t.value == "beach"));

        conn.execute(
            "DELETE FROM folder_tag WHERE folder_id = ?1 AND source = 'manual'",
            params![ana],
        )
        .unwrap();
        rebuild_subtree(&conn, "people/ana").unwrap();

        assert!(!item_effective_tags(&conn, item_id)
            .unwrap()
            .iter()
            .any(|t| t.value == "beach"));
    }

    #[test]
    fn subtree_rebuild_leaves_manual_item_tags_alone() {
        let conn = memory_conn();
        let ana = folder(&conn, "people/ana", "Ana");
        let item_id = item(&conn, ana, "photo.jpg");
        rebuild_item(&conn, item_id).unwrap();
        add_item_tag(&conn, item_id, None, "favourite-shot").unwrap();

        flag(&conn, ana, "beach", "manual");
        rebuild_subtree(&conn, "people/ana").unwrap();

        let values: Vec<String> = item_effective_tags(&conn, item_id)
            .unwrap()
            .into_iter()
            .map(|t| t.value)
            .collect();
        assert!(values.contains(&"favourite-shot".to_string()));
        assert!(values.contains(&"beach".to_string()));
    }

    #[test]
    fn a_root_level_edit_rebuilds_the_whole_library() {
        let conn = memory_conn();
        let root = folder(&conn, "", "Library");
        let ana = folder(&conn, "ana", "Ana");
        let item_id = item(&conn, ana, "photo.jpg");
        rebuild_item(&conn, item_id).unwrap();

        flag(&conn, root, "all", "manual");
        rebuild_subtree(&conn, "").unwrap();

        assert!(item_effective_tags(&conn, item_id)
            .unwrap()
            .iter()
            .any(|t| t.value == "all"));
    }

    #[test]
    fn own_tag_wins_over_an_inherited_duplicate_of_the_same_value() {
        let conn = memory_conn();
        let people = folder(&conn, "people", "People");
        let ana = folder(&conn, "people/ana", "Ana");
        flag(&conn, people, "shared", "manual");
        flag(&conn, ana, "shared", "manual");

        rebuild_subtree(&conn, "people").unwrap();

        let item_id = item(&conn, ana, "photo.jpg");
        rebuild_item(&conn, item_id).unwrap();
        let shared: Vec<_> = item_effective_tags(&conn, item_id)
            .unwrap()
            .into_iter()
            .filter(|t| t.value == "shared")
            .collect();
        assert_eq!(shared.len(), 1, "one row, not a PK collision");
        assert_eq!(shared[0].origin_id, Some(ana));
    }
}
