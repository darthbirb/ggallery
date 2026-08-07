//! The effective-tag cache — DATA-MODEL's "Tag resolution" — and its
//! invalidation.
//!
//! Folder-level tag edits (`folder_tag`/`tag`) are applied synchronously,
//! inline, by the caller — that table is small. Only the downstream fan-out
//! into `item_effective_tag` across a subtree is meant to run off the UI
//! thread; `rebuild_subtree`/`rebuild_item` here are the work, `jobs::kinds`
//! is what queues them. See docs/SCHEMA.md#tag-resolution.

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::db::now;
use crate::error::{AppError, Result};

/// Get or create the shared `tag` row for `(key, value)`. `key` is `None`
/// for a flag. `IS` rather than `=` so `NULL` compares correctly.
///
/// Case-folded on the way in (decision 31) — every tag/label/flag
/// creation funnels through here, which is what makes this the one place
/// that needs to fold rather than each of its callers.
pub fn get_or_create_tag(conn: &Connection, key: Option<&str>, value: &str) -> Result<i64> {
    let key = key.map(crate::db::fold);
    let value = crate::db::fold(value);
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
/// own manual tags. `None` for an unfiled item (the Sorting Box) — nothing to
/// inherit from. Used for a brand-new item — called inline from
/// `jobs::worker::run_hash`, not queued, since the cost is one bounded
/// ancestor walk rather than a scan — and for item-level manual tag changes.
pub fn rebuild_item(conn: &Connection, item_id: i64) -> Result<()> {
    let folder_id: Option<i64> = conn.query_row(
        "SELECT folder_id FROM item WHERE id = ?1",
        params![item_id],
        |r| r.get(0),
    )?;
    let inherited = match folder_id {
        Some(folder_id) => resolve_ancestor_tags(conn, folder_id)?,
        None => Vec::new(),
    };

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
/// descendant, per DATA-MODEL's invalidation table. `None` means the whole
/// library (a root-level folder's own tags changed, or none of the above —
/// see `db::items::Scope::Everything` for the same "no folder" shape).
///
/// Walks the subtree parent-before-child (`depth ASC` in the recursive CTE
/// below), accumulating each folder's tag set from its parent's
/// already-computed set plus its own `folder_tag` rows. That is one pass
/// over folders (thousands at most, per `db::folders::tree`'s own
/// reasoning) rather than one recursive ancestry query per item, which is
/// exactly the shape decision 20 warns is catastrophic at scale.
pub fn rebuild_subtree(conn: &Connection, folder_id: Option<i64>) -> Result<()> {
    let Some(folder_id) = folder_id else {
        return rebuild_whole_library(conn);
    };
    if !folder_exists(conn, folder_id)? {
        return Ok(());
    }
    let parent_id: Option<i64> = conn.query_row(
        "SELECT parent_id FROM folder WHERE id = ?1",
        params![folder_id],
        |r| r.get(0),
    )?;

    // Seed: whatever lies above `folder_id` contributes tags that have not
    // changed, but the walk below still needs their value to start from.
    let seed: HashMap<i64, i64> = match parent_id {
        Some(pid) => resolve_ancestor_tags(conn, pid)?.into_iter().collect(),
        None => HashMap::new(),
    };

    let subtree = subtree_folders(conn, folder_id)?;
    rebuild_from_seed(conn, parent_id.unwrap_or(-1), seed, &subtree)
}

fn rebuild_whole_library(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare(
        "WITH RECURSIVE t(id, parent_id, depth) AS (
             SELECT id, parent_id, 0 FROM folder WHERE parent_id IS NULL AND deleted_at IS NULL
           UNION ALL
             SELECT f.id, f.parent_id, t.depth + 1
               FROM folder f JOIN t ON f.parent_id = t.id
              WHERE f.deleted_at IS NULL
         )
         SELECT id, parent_id FROM t ORDER BY depth",
    )?;
    let subtree: Vec<(i64, Option<i64>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    drop(stmt);
    rebuild_from_seed(conn, -1, HashMap::new(), &subtree)
}

fn folder_exists(conn: &Connection, folder_id: i64) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM folder WHERE id = ?1 AND deleted_at IS NULL)",
        params![folder_id],
        |r| r.get(0),
    )?)
}

fn rebuild_from_seed(
    conn: &Connection,
    seed_key: i64,
    seed: HashMap<i64, i64>,
    subtree: &[(i64, Option<i64>)],
) -> Result<()> {
    crate::db::begin_batch(conn)?;
    let result = (|| -> Result<()> {
        let mut tag_sets: HashMap<i64, HashMap<i64, i64>> = HashMap::new();
        tag_sets.insert(seed_key, seed);

        for (id, parent) in subtree {
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

/// `(id, parent_id)` for `folder_id` and everything beneath it, parent
/// before child.
fn subtree_folders(conn: &Connection, folder_id: i64) -> Result<Vec<(i64, Option<i64>)>> {
    let mut stmt = conn.prepare(
        "WITH RECURSIVE t(id, parent_id, depth) AS (
             SELECT id, parent_id, 0 FROM folder WHERE id = ?1
           UNION ALL
             SELECT f.id, f.parent_id, t.depth + 1
               FROM folder f JOIN t ON f.parent_id = t.id
              WHERE f.deleted_at IS NULL
         )
         SELECT id, parent_id FROM t ORDER BY depth",
    )?;
    let rows = stmt
        .query_map(params![folder_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
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
// ROADMAP.md §M2. These exist so the data model is complete and testable now,
// and so `commands/tags.rs` has something real to expose when M2.5 needs it.

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveTag {
    pub tag_id: i64,
    pub key: Option<String>,
    pub value: String,
    /// `None` for a manual tag; the contributing ancestor folder otherwise.
    pub origin_id: Option<i64>,
    /// Whether the *contribution named by `origin_id`* is that folder's
    /// title tag (`folder_tag.source = 'title'`) rather than a manual flag
    /// or label — the same tag id can be one folder's title and another
    /// folder's manual tag, so this is a fact about the contribution, not
    /// about the tag. Always `false` for a manual tag. What a folder-name
    /// tag is suppressed on, structurally, instead of comparing display
    /// text against a breadcrumb.
    pub origin_is_title: bool,
}

pub fn item_effective_tags(conn: &Connection, item_id: i64) -> Result<Vec<EffectiveTag>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.key, t.value, e.origin_id,
                EXISTS(
                  SELECT 1 FROM folder_tag ft
                   WHERE ft.folder_id = e.origin_id AND ft.tag_id = e.tag_id AND ft.source = 'title'
                ) AS origin_is_title
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
                origin_is_title: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// A folder's *inherited* labels and flags — every ancestor's `folder_tag`
/// rows, minus this folder's own (already surfaced by `get_detail`'s
/// `fields`/`flags`). SPEC.md §2's "inherited greyed, manual solid" rule
/// applies to a folder's own band the same way it already does to an item's
/// details.
///
/// Live, not cached: folder depth is small and this is read only when a band
/// expands, unlike `item_effective_tag`, which backs every grid query and so
/// has to be materialised.
pub fn folder_inherited_tags(conn: &Connection, folder_id: i64) -> Result<Vec<EffectiveTag>> {
    let ancestry = resolve_ancestor_tags(conn, folder_id)?;
    let mut rows = Vec::with_capacity(ancestry.len());
    for (tag_id, origin_id) in ancestry {
        if origin_id == folder_id {
            continue;
        }
        let (key, value): (Option<String>, String) = conn.query_row(
            "SELECT key, value FROM tag WHERE id = ?1",
            params![tag_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let source: String = conn.query_row(
            "SELECT source FROM folder_tag WHERE folder_id = ?1 AND tag_id = ?2",
            params![origin_id, tag_id],
            |r| r.get(0),
        )?;
        rows.push(EffectiveTag {
            tag_id,
            key,
            value,
            origin_id: Some(origin_id),
            origin_is_title: source == "title",
        });
    }
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

// --- rename / delete a tag (M2.1) -----------------------------------------
//
// The minimum that stops the vocabulary rotting — a typo fix and a way to
// remove one. Merge, aliases and usage counts stay M8's full tag-management
// screen, per SPEC.md "Item operations".

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagSummary {
    pub id: i64,
    pub key: Option<String>,
    pub value: String,
    pub usage_count: i64,
}

pub fn list_tags(conn: &Connection, filter: Option<&str>) -> Result<Vec<TagSummary>> {
    let pattern = filter.map(|f| format!("%{f}%"));
    let mut stmt = conn.prepare(
        "SELECT t.id, t.key, t.value,
                (SELECT COUNT(*) FROM folder_tag WHERE tag_id = t.id)
              + (SELECT COUNT(*) FROM item_tag WHERE tag_id = t.id) AS usage
           FROM tag t
          WHERE ?1 IS NULL OR t.value LIKE ?1 OR t.key LIKE ?1
          ORDER BY t.key IS NOT NULL, t.value COLLATE NOCASE",
    )?;
    let rows = stmt
        .query_map(params![pattern], |r| {
            Ok(TagSummary {
                id: r.get(0)?,
                key: r.get(1)?,
                value: r.get(2)?,
                usage_count: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Renames a tag's display value. Only the value changes — a label's key
/// (what field it represents) is a bigger edit than a typo fix and stays
/// out of scope here. No cache rebuild needed: `item_effective_tag` and
/// `folder_tag`/`item_tag` all reference `tag_id`, not a copy of the text,
/// so every place the tag renders picks the new value up through the join.
pub fn rename_tag(conn: &Connection, tag_id: i64, new_value: &str) -> Result<()> {
    // Folded on the way in, same as `get_or_create_tag` — this is the one
    // other place a tag's text is written (decision 31).
    let new_value = crate::db::fold(new_value);

    let (key, old_value): (Option<String>, String) = conn
        .query_row(
            "SELECT key, value FROM tag WHERE id = ?1",
            params![tag_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::invalid("tag not found"))?;

    let collision: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM tag WHERE key IS ?1 AND value = ?2 AND id != ?3)",
        params![key, new_value, tag_id],
        |r| r.get(0),
    )?;
    if collision {
        return Err(AppError::invalid("a tag with that name already exists"));
    }

    conn.execute(
        "UPDATE tag SET value = ?1 WHERE id = ?2",
        params![new_value, tag_id],
    )?;
    crate::db::journal::record_tag_rename(conn, tag_id, &old_value, &new_value)?;
    Ok(())
}

/// Deletes a tag across the whole library — `folder_tag`, `item_tag`,
/// `item_effective_tag`, `tag_alias`, then the `tag` row itself. Refuses if
/// the tag is a `source = 'title'` folder-title tag, mirroring the
/// per-folder protection `db::folders::remove_tag` already enforces — a
/// folder's title is renamed, not deleted out from under it.
///
/// No rebuild job needed: `item_effective_tag` rows are deleted directly by
/// `tag_id` here, which is cheap and immediate, unlike a subtree recompute.
pub fn delete_tag(conn: &Connection, tag_id: i64) -> Result<()> {
    let is_title: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM folder_tag WHERE tag_id = ?1 AND source = 'title')",
        params![tag_id],
        |r| r.get(0),
    )?;
    if is_title {
        return Err(AppError::invalid(
            "this tag is a folder title and can't be deleted directly — rename the folder instead",
        ));
    }

    let (key, value): (Option<String>, String) = conn
        .query_row(
            "SELECT key, value FROM tag WHERE id = ?1",
            params![tag_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::invalid("tag not found"))?;

    let mut fstmt = conn.prepare("SELECT folder_id FROM folder_tag WHERE tag_id = ?1")?;
    let folder_ids: Vec<i64> = fstmt
        .query_map(params![tag_id], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    drop(fstmt);
    let mut istmt = conn.prepare("SELECT item_id FROM item_tag WHERE tag_id = ?1")?;
    let item_ids: Vec<i64> = istmt
        .query_map(params![tag_id], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    drop(istmt);

    conn.execute("DELETE FROM folder_tag WHERE tag_id = ?1", params![tag_id])?;
    conn.execute("DELETE FROM item_tag WHERE tag_id = ?1", params![tag_id])?;
    conn.execute(
        "DELETE FROM item_effective_tag WHERE tag_id = ?1",
        params![tag_id],
    )?;
    conn.execute("DELETE FROM tag_alias WHERE tag_id = ?1", params![tag_id])?;
    conn.execute("DELETE FROM tag WHERE id = ?1", params![tag_id])?;

    crate::db::journal::record_tag_delete(conn, tag_id, key.as_deref(), &value, &folder_ids, &item_ids)?;
    Ok(())
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

    fn folder(conn: &Connection, path: &str, title: &str) -> i64 {
        db::folders::ensure_path(conn, path, title).unwrap()
    }

    fn item(conn: &Connection, folder_id: i64, name: &str) -> i64 {
        db::items::upsert(
            conn,
            &db::items::NewItem {
                uuid: uuid::Uuid::new_v4().to_string(),
                folder_id: Some(folder_id),
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
        // Folded on the way in — decision 31.
        assert_eq!(tags, vec!["ana".to_string()]);
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
        assert_eq!(tags, vec!["anastasia".to_string()]);
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
        // people, ana (titles), family, beach (flags) — folded on the way in
        // (decision 31), alphabetical once sorted.
        assert_eq!(values, vec!["ana", "beach", "family", "people"]);
    }

    #[test]
    fn a_subfolder_inherits_its_ancestors_labels_and_flags_but_not_its_own() {
        let conn = memory_conn();
        let people = folder(&conn, "people", "People");
        let ana = folder(&conn, "people/ana", "Ana");
        flag(&conn, people, "family", "manual");
        crate::db::folders::set_label(&conn, people, "country", "Portugal").unwrap();
        flag(&conn, ana, "beach", "manual");

        let inherited = folder_inherited_tags(&conn, ana).unwrap();

        // Ana's own "beach" flag is not inherited from itself.
        assert!(!inherited.iter().any(|t| t.value == "beach"));
        // People's title, its manual flag and its label all come through,
        // each carrying People's id as the origin — folded on the way in
        // (decision 31).
        assert!(inherited
            .iter()
            .any(|t| t.key.is_none() && t.value == "people" && t.origin_id == Some(people)));
        assert!(inherited
            .iter()
            .any(|t| t.key.is_none() && t.value == "family" && t.origin_id == Some(people)));
        assert!(inherited
            .iter()
            .any(|t| t.key.as_deref() == Some("country")
                && t.value == "portugal"
                && t.origin_id == Some(people)));
    }

    #[test]
    fn an_items_inherited_title_tag_is_marked_as_such() {
        let conn = memory_conn();
        let _people = folder(&conn, "people", "People");
        let ana = folder(&conn, "people/ana", "Ana");
        let item_id = item(&conn, ana, "photo.jpg");
        rebuild_item(&conn, item_id).unwrap();

        let tags = item_effective_tags(&conn, item_id).unwrap();
        let ana_title = tags.iter().find(|t| t.value == "ana").unwrap();
        assert!(ana_title.origin_is_title, "Ana's own title tag, inherited by its item");
        assert!(ana_title.origin_id.is_some());
    }

    #[test]
    fn a_closer_manual_duplicate_of_a_title_wins_and_is_not_marked_as_one() {
        let conn = memory_conn();
        let _people = folder(&conn, "people", "People");
        let ana = folder(&conn, "people/ana", "Ana");
        // A manual flag on Ana that happens to share People's own title text
        // — a deliberate choice on Ana, not the ancestor's name leaking in.
        // Item_effective_tag's `PRIMARY KEY (item_id, tag_id)` means only one
        // contribution for this shared tag id can survive per item; the
        // closer one (Ana's) does, same as `own_tag_wins_over_an_inherited_
        // duplicate_of_the_same_value` above.
        flag(&conn, ana, "people", "manual");

        let item_id = item(&conn, ana, "photo.jpg");
        rebuild_item(&conn, item_id).unwrap();

        let tags = item_effective_tags(&conn, item_id).unwrap();
        let surviving = tags.iter().find(|t| t.value == "people").unwrap();
        assert_eq!(surviving.origin_id, Some(ana));
        assert!(!surviving.origin_is_title, "the surviving contribution is Ana's manual flag, not People's title");
    }

    #[test]
    fn a_folders_inherited_view_marks_a_title_contribution_but_not_a_same_text_manual_one() {
        let conn = memory_conn();
        let family = folder(&conn, "family", "Family");
        let people = folder(&conn, "family/people", "People");
        let ana = folder(&conn, "family/people/ana", "Ana");
        // Unlike the item case, `folder_inherited_tags` does not collapse
        // same-tag-id contributions from different ancestors — both stay
        // visible, each correctly marked by its own origin.
        flag(&conn, family, "people", "manual");

        let inherited = folder_inherited_tags(&conn, ana).unwrap();
        let title_contribution = inherited
            .iter()
            .find(|t| t.value == "people" && t.origin_id == Some(people))
            .unwrap();
        assert!(title_contribution.origin_is_title);

        let manual_contribution = inherited
            .iter()
            .find(|t| t.value == "people" && t.origin_id == Some(family))
            .unwrap();
        assert!(!manual_contribution.origin_is_title);
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
        rebuild_subtree(&conn, Some(ana)).unwrap();

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
        rebuild_subtree(&conn, Some(ana)).unwrap();

        let values: Vec<String> = item_effective_tags(&conn, item_id)
            .unwrap()
            .into_iter()
            .map(|t| t.value)
            .collect();
        assert!(values.contains(&"favourite-shot".to_string()));
        assert!(values.contains(&"beach".to_string()));
    }

    #[test]
    fn rebuild_subtree_with_no_folder_rebuilds_the_whole_library() {
        // `None` is the whole-library case (decision 30 — there is
        // no root folder any more for a "root-level edit" to mean anything
        // narrower than that).
        let conn = memory_conn();
        let top = folder(&conn, "top", "Top");
        let item_id = item(&conn, top, "photo.jpg");
        rebuild_item(&conn, item_id).unwrap();

        flag(&conn, top, "all", "manual");
        rebuild_subtree(&conn, None).unwrap();

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

        rebuild_subtree(&conn, Some(people)).unwrap();

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
