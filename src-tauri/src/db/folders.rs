use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::db::now;
use crate::error::{AppError, Result};
use crate::fs::paths::parent_rel;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderNode {
    pub id: i64,
    pub rel_path: String,
    pub title: String,
    pub parent_id: Option<i64>,
    pub depth: u32,
    /// Items directly inside this folder.
    pub direct_count: i64,
    /// Items in this folder and everything beneath it.
    pub total_count: i64,
    pub status: String,
    pub favorite: bool,
}

/// Excludes a trashed folder — a walk or a fresh `create` must never resolve
/// to (and thereby resurrect) a row `trash` has already retired. `trash`
/// rewrites `rel_path` to `.trashed/<id>` on the way out, which frees the
/// original string for reuse; this filter is what makes that safe even in
/// the instant before that rewrite is visible to a caller relying on it.
pub fn id_for_rel(conn: &Connection, rel_path: &str) -> Result<Option<i64>> {
    Ok(conn
        .query_row(
            "SELECT id FROM folder WHERE rel_path = ?1 AND deleted_at IS NULL",
            params![rel_path],
            |r| r.get(0),
        )
        .optional()?)
}

pub fn count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM folder WHERE deleted_at IS NULL",
        [],
        |r| r.get(0),
    )?)
}

pub fn rel_for(conn: &Connection, id: i64) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT rel_path FROM folder WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .optional()?)
}

/// Insert the folder if it is new, returning its id either way. `title` is the
/// on-disk directory name with its original casing — `rel_path` is normalised,
/// so this is the only place the display name survives.
pub fn upsert(conn: &Connection, rel_path: &str, title: &str) -> Result<i64> {
    if let Some(id) = id_for_rel(conn, rel_path)? {
        return Ok(id);
    }

    let parent_id = match parent_rel(rel_path) {
        Some(parent) => id_for_rel(conn, &parent)?,
        None => None,
    };

    conn.execute(
        "INSERT INTO folder (rel_path, title, parent_id, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![rel_path, title, parent_id, now()],
    )?;
    let id = conn.last_insert_rowid();
    // Every folder's title is a tag from the moment it exists — see
    // `db::tags::sync_title_tag`. No retag job is enqueued here: a folder
    // the walker just created has zero items under it at this instant
    // (directories are visited before their contents), so there is nothing
    // yet for a rebuild to do.
    crate::db::tags::sync_title_tag(conn, id, title)?;
    Ok(id)
}

/// The whole tree, in `rel_path` order, with direct and recursive counts.
/// Folder counts are in the thousands at most, so this is one query plus an
/// in-memory roll-up rather than a recursive CTE per node.
pub fn tree(conn: &Connection) -> Result<Vec<FolderNode>> {
    let mut stmt = conn.prepare(
        "SELECT f.id, f.rel_path, f.title, f.parent_id,
                (SELECT COUNT(*) FROM item i
                  WHERE i.folder_id = f.id AND i.deleted_at IS NULL) AS direct,
                f.status, f.favorite
           FROM folder f
          WHERE f.deleted_at IS NULL
          ORDER BY f.rel_path",
    )?;

    let mut nodes: Vec<FolderNode> = stmt
        .query_map([], |r| {
            let rel_path: String = r.get(1)?;
            let depth = if rel_path.is_empty() {
                0
            } else {
                rel_path.matches('/').count() as u32 + 1
            };
            let direct: i64 = r.get(4)?;
            Ok(FolderNode {
                id: r.get(0)?,
                rel_path,
                title: r.get(2)?,
                parent_id: r.get(3)?,
                depth,
                direct_count: direct,
                total_count: direct,
                status: r.get(5)?,
                favorite: r.get::<_, i64>(6)? != 0,
            })
        })?
        .collect::<std::result::Result<_, _>>()?;

    // Roll counts up. Deepest first, so every child has already contributed
    // by the time its parent is visited.
    let index: std::collections::HashMap<i64, usize> =
        nodes.iter().enumerate().map(|(i, n)| (n.id, i)).collect();
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(nodes[i].depth));
    for i in order {
        let (parent_id, total) = (nodes[i].parent_id, nodes[i].total_count);
        if let Some(&p) = parent_id.and_then(|pid| index.get(&pid)) {
            nodes[p].total_count += total;
        }
    }

    Ok(nodes)
}

// --- M2: folder detail, editing, archetypes -------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchetypeFieldValue {
    pub key: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub ordinal: i64,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderFlag {
    pub tag_id: i64,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderDetail {
    pub id: i64,
    pub rel_path: String,
    pub title: String,
    pub parent_id: Option<i64>,
    pub status: String,
    pub favorite: bool,
    pub notes: Option<String>,
    pub last_added_at: Option<i64>,
    pub direct_count: i64,
    pub total_count: i64,
    pub subfolder_count: i64,
    pub archetype_id: Option<i64>,
    pub archetype_name: Option<String>,
    pub fields: Vec<ArchetypeFieldValue>,
    pub flags: Vec<FolderFlag>,
}

/// The folder header's whole content in one call. `rel_path` empty means the
/// library root — `subtree_totals` handles that the same way
/// `db::items::list` and `db::tags::rebuild_subtree` already do.
pub fn get_detail(conn: &Connection, id: i64) -> Result<Option<FolderDetail>> {
    #[allow(clippy::type_complexity)]
    let base: Option<(
        String,
        String,
        Option<i64>,
        String,
        i64,
        Option<String>,
        Option<i64>,
        Option<i64>,
        Option<String>,
    )> = conn
        .query_row(
            "SELECT f.rel_path, f.title, f.parent_id, f.status, f.favorite, f.notes,
                    f.last_added_at, f.archetype_id, a.name
               FROM folder f LEFT JOIN archetype a ON a.id = f.archetype_id
              WHERE f.id = ?1 AND f.deleted_at IS NULL",
            params![id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                ))
            },
        )
        .optional()?;
    let Some((
        rel_path,
        title,
        parent_id,
        status,
        favorite,
        notes,
        last_added_at,
        archetype_id,
        archetype_name,
    )) = base
    else {
        return Ok(None);
    };

    let (direct_count, total_count, subfolder_count) = subtree_totals(conn, &rel_path)?;

    let fields = if let Some(aid) = archetype_id {
        let mut stmt = conn.prepare(
            "SELECT af.key, af.type, af.ordinal,
                    COALESCE(
                      (SELECT t.value FROM folder_tag ft JOIN tag t ON t.id = ft.tag_id
                        WHERE ft.folder_id = ?1 AND t.key = af.key LIMIT 1),
                      '')
               FROM archetype_field af
              WHERE af.archetype_id = ?2
              ORDER BY af.ordinal",
        )?;
        let rows = stmt
            .query_map(params![id, aid], |r| {
                Ok(ArchetypeFieldValue {
                    key: r.get(0)?,
                    field_type: r.get(1)?,
                    ordinal: r.get(2)?,
                    value: r.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        rows
    } else {
        Vec::new()
    };

    let mut flag_stmt = conn.prepare(
        "SELECT t.id, t.value FROM folder_tag ft JOIN tag t ON t.id = ft.tag_id
          WHERE ft.folder_id = ?1 AND ft.source != 'title' AND t.key IS NULL
          ORDER BY t.value COLLATE NOCASE",
    )?;
    let flags = flag_stmt
        .query_map(params![id], |r| {
            Ok(FolderFlag {
                tag_id: r.get(0)?,
                value: r.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    Ok(Some(FolderDetail {
        id,
        rel_path,
        title,
        parent_id,
        status,
        favorite: favorite != 0,
        notes,
        last_added_at,
        direct_count,
        total_count,
        subfolder_count,
        archetype_id,
        archetype_name,
        fields,
        flags,
    }))
}

/// `(direct items, total items, subfolders)` for a folder and its subtree.
/// Root (`rel_path == ""`) needs no `WHERE` at all — `'' || '/%'` is `'/%'`,
/// which matches nothing, since no `rel_path` ever carries a leading slash.
fn subtree_totals(conn: &Connection, rel_path: &str) -> Result<(i64, i64, i64)> {
    let id: i64 = conn.query_row(
        "SELECT id FROM folder WHERE rel_path = ?1 AND deleted_at IS NULL",
        params![rel_path],
        |r| r.get(0),
    )?;
    let direct: i64 = conn.query_row(
        "SELECT COUNT(*) FROM item WHERE folder_id = ?1 AND deleted_at IS NULL",
        params![id],
        |r| r.get(0),
    )?;
    let (total, subfolders) = if rel_path.is_empty() {
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM item WHERE deleted_at IS NULL",
            [],
            |r| r.get(0),
        )?;
        let subfolders: i64 = conn.query_row(
            "SELECT COUNT(*) - 1 FROM folder WHERE deleted_at IS NULL",
            [],
            |r| r.get(0),
        )?;
        (total, subfolders)
    } else {
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM item
              WHERE deleted_at IS NULL
                AND folder_id IN (SELECT id FROM folder
                                   WHERE deleted_at IS NULL
                                     AND (rel_path = ?1 OR rel_path LIKE ?1 || '/%'))",
            params![rel_path],
            |r| r.get(0),
        )?;
        let subfolders: i64 = conn.query_row(
            "SELECT COUNT(*) FROM folder
              WHERE deleted_at IS NULL AND rel_path LIKE ?1 || '/%'",
            params![rel_path],
            |r| r.get(0),
        )?;
        (total, subfolders)
    };
    Ok((direct, total, subfolders.max(0)))
}

/// Retitling touches the record only — never the filesystem. The directory
/// name lives in `rel_path`, independent of `title`; see `rename_dir` for
/// the operation that moves the directory.
pub fn set_title(conn: &Connection, id: i64, title: &str) -> Result<()> {
    let previous: String = conn.query_row(
        "SELECT title FROM folder WHERE id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    conn.execute(
        "UPDATE folder SET title = ?1 WHERE id = ?2",
        params![title, id],
    )?;
    crate::db::tags::sync_title_tag(conn, id, title)?;
    if previous != title {
        crate::db::journal::record_folder_rename_title(conn, id, &previous, title)?;
    }
    enqueue_retag(conn, id)
}

pub fn set_status(conn: &Connection, id: i64, status: &str) -> Result<()> {
    let known: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM folder_status WHERE key = ?1)",
        params![status],
        |r| r.get(0),
    )?;
    if !known {
        return Err(AppError::invalid(format!("unknown folder status '{status}'")));
    }
    conn.execute(
        "UPDATE folder SET status = ?1 WHERE id = ?2",
        params![status, id],
    )?;
    Ok(())
}

pub fn set_favorite(conn: &Connection, id: i64, favorite: bool) -> Result<()> {
    conn.execute(
        "UPDATE folder SET favorite = ?1 WHERE id = ?2",
        params![favorite as i64, id],
    )?;
    Ok(())
}

pub fn set_notes(conn: &Connection, id: i64, notes: Option<&str>) -> Result<()> {
    conn.execute(
        "UPDATE folder SET notes = ?1 WHERE id = ?2",
        params![notes, id],
    )?;
    Ok(())
}

/// Creates an empty `source = 'archetype'` label for `key` on `folder_id`
/// unless it already carries a value for that key — the never-clobber rule
/// shared by `apply_archetype` (every field, one folder) and
/// `add_archetype_field`'s "apply to existing folders" path (one field,
/// every folder already on the archetype).
fn ensure_empty_label(conn: &Connection, folder_id: i64, key: &str) -> Result<()> {
    let already: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM folder_tag ft JOIN tag t ON t.id = ft.tag_id
          WHERE ft.folder_id = ?1 AND t.key = ?2)",
        params![folder_id, key],
        |r| r.get(0),
    )?;
    if already {
        return Ok(());
    }
    let tag_id = crate::db::tags::get_or_create_tag(conn, Some(key), "")?;
    conn.execute(
        "INSERT OR IGNORE INTO folder_tag (folder_id, tag_id, source) VALUES (?1, ?2, 'archetype')",
        params![folder_id, tag_id],
    )?;
    Ok(())
}

/// Sets `folder.archetype_id` and creates an empty label for every field the
/// archetype defines that this folder doesn't already carry a value for —
/// re-applying never clobbers an existing value.
pub fn apply_archetype(conn: &Connection, folder_id: i64, archetype_id: i64) -> Result<()> {
    conn.execute(
        "UPDATE folder SET archetype_id = ?1 WHERE id = ?2",
        params![archetype_id, folder_id],
    )?;

    let mut stmt = conn.prepare("SELECT key FROM archetype_field WHERE archetype_id = ?1")?;
    let keys: Vec<String> = stmt
        .query_map(params![archetype_id], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    drop(stmt);

    for key in keys {
        ensure_empty_label(conn, folder_id, &key)?;
    }

    enqueue_retag(conn, folder_id)
}

/// Edits an existing label field's value. `(key, value)` is unique per `tag`
/// row, so "editing" means unlinking the old row for this key and linking
/// (get-or-create) the new one — the `source` it already had (`archetype` in
/// practice, since M2's UI only ever edits archetype-provided fields) is
/// preserved rather than reset to `manual`.
pub fn set_label(conn: &Connection, folder_id: i64, key: &str, value: &str) -> Result<()> {
    let existing: Option<(i64, String)> = conn
        .query_row(
            "SELECT ft.tag_id, ft.source FROM folder_tag ft JOIN tag t ON t.id = ft.tag_id
              WHERE ft.folder_id = ?1 AND t.key = ?2",
            params![folder_id, key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let source = existing
        .as_ref()
        .map(|(_, s)| s.clone())
        .unwrap_or_else(|| "manual".to_string());
    if let Some((old_tag_id, _)) = existing {
        conn.execute(
            "DELETE FROM folder_tag WHERE folder_id = ?1 AND tag_id = ?2",
            params![folder_id, old_tag_id],
        )?;
    }
    let tag_id = crate::db::tags::get_or_create_tag(conn, Some(key), value)?;
    conn.execute(
        "INSERT OR IGNORE INTO folder_tag (folder_id, tag_id, source) VALUES (?1, ?2, ?3)",
        params![folder_id, tag_id, source],
    )?;
    enqueue_retag(conn, folder_id)
}

pub fn add_flag(conn: &Connection, folder_id: i64, value: &str) -> Result<()> {
    let tag_id = crate::db::tags::get_or_create_tag(conn, None, value)?;
    conn.execute(
        "INSERT OR IGNORE INTO folder_tag (folder_id, tag_id, source) VALUES (?1, ?2, 'manual')",
        params![folder_id, tag_id],
    )?;
    enqueue_retag(conn, folder_id)
}

/// Rejects removing the title tag rather than silently no-op-ing — it isn't
/// user-removable, per DATA-MODEL's comment on `folder_tag.source`.
pub fn remove_tag(conn: &Connection, folder_id: i64, tag_id: i64) -> Result<()> {
    let source: Option<String> = conn
        .query_row(
            "SELECT source FROM folder_tag WHERE folder_id = ?1 AND tag_id = ?2",
            params![folder_id, tag_id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(source) = source else { return Ok(()) };
    if source == "title" {
        return Err(AppError::invalid("the title tag can't be removed"));
    }
    conn.execute(
        "DELETE FROM folder_tag WHERE folder_id = ?1 AND tag_id = ?2",
        params![folder_id, tag_id],
    )?;
    enqueue_retag(conn, folder_id)
}

fn enqueue_retag(conn: &Connection, folder_id: i64) -> Result<()> {
    let rel = rel_for(conn, folder_id)?.unwrap_or_default();
    crate::jobs::enqueue_retag_folder(conn, &rel)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchetypeFieldDef {
    pub key: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub ordinal: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchetypeInfo {
    pub id: i64,
    pub name: String,
    pub fields: Vec<ArchetypeFieldDef>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderStatusDef {
    pub key: String,
    pub label: String,
    pub colour: String,
    pub ordinal: i64,
}

pub fn list_statuses(conn: &Connection) -> Result<Vec<FolderStatusDef>> {
    let mut stmt =
        conn.prepare("SELECT key, label, colour, ordinal FROM folder_status ORDER BY ordinal")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(FolderStatusDef {
                key: r.get(0)?,
                label: r.get(1)?,
                colour: r.get(2)?,
                ordinal: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn list_archetypes(conn: &Connection) -> Result<Vec<ArchetypeInfo>> {
    let mut stmt = conn.prepare("SELECT id, name FROM archetype ORDER BY name")?;
    let archetypes: Vec<(i64, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    drop(stmt);

    let mut out = Vec::with_capacity(archetypes.len());
    for (id, name) in archetypes {
        let mut fstmt = conn.prepare(
            "SELECT key, type, ordinal FROM archetype_field WHERE archetype_id = ?1 ORDER BY ordinal",
        )?;
        let fields = fstmt
            .query_map(params![id], |r| {
                Ok(ArchetypeFieldDef {
                    key: r.get(0)?,
                    field_type: r.get(1)?,
                    ordinal: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        out.push(ArchetypeInfo { id, name, fields });
    }
    Ok(out)
}

// --- archetype lifecycle (M2.1 — nothing is seeded, so an editor is
// mandatory; see PLAN.md locked decision 21) ------------------------------

pub fn create_archetype(conn: &Connection, name: &str) -> Result<i64> {
    conn.execute("INSERT INTO archetype (name) VALUES (?1)", params![name])?;
    Ok(conn.last_insert_rowid())
}

pub fn rename_archetype(conn: &Connection, id: i64, name: &str) -> Result<()> {
    conn.execute(
        "UPDATE archetype SET name = ?1 WHERE id = ?2",
        params![name, id],
    )?;
    Ok(())
}

/// Clears `archetype_id` on every folder that used it — their labels stay
/// exactly where they are, per "folders carry labels independently" — then
/// deletes the archetype. `archetype_field` cascades via its own FK.
pub fn delete_archetype(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE folder SET archetype_id = NULL WHERE archetype_id = ?1",
        params![id],
    )?;
    conn.execute("DELETE FROM archetype WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn count_folders_using_archetype(conn: &Connection, archetype_id: i64) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM folder WHERE archetype_id = ?1 AND deleted_at IS NULL",
        params![archetype_id],
        |r| r.get(0),
    )?)
}

/// The empty-label-creation loop `apply_archetype` already used, factored
/// out so `add_archetype_field`'s "add this field to the N folders already
/// using it" path can reuse the same never-clobber-an-existing-value rule
/// for one field instead of the whole archetype.
fn apply_field_to_folders_using_archetype(conn: &Connection, archetype_id: i64, key: &str) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT id FROM folder WHERE archetype_id = ?1 AND deleted_at IS NULL",
    )?;
    let folder_ids: Vec<i64> = stmt
        .query_map(params![archetype_id], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    drop(stmt);

    for folder_id in folder_ids {
        ensure_empty_label(conn, folder_id, key)?;
    }
    Ok(())
}

pub fn add_archetype_field(
    conn: &Connection,
    archetype_id: i64,
    key: &str,
    field_type: &str,
    apply_to_existing: bool,
) -> Result<()> {
    let next_ordinal: i64 = conn.query_row(
        "SELECT COALESCE(MAX(ordinal) + 1, 0) FROM archetype_field WHERE archetype_id = ?1",
        params![archetype_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO archetype_field (archetype_id, key, type, ordinal) VALUES (?1, ?2, ?3, ?4)",
        params![archetype_id, key, field_type, next_ordinal],
    )?;
    if apply_to_existing {
        apply_field_to_folders_using_archetype(conn, archetype_id, key)?;
    }
    Ok(())
}

pub fn reorder_archetype_fields(conn: &Connection, archetype_id: i64, ordered_keys: &[String]) -> Result<()> {
    for (ordinal, key) in ordered_keys.iter().enumerate() {
        conn.execute(
            "UPDATE archetype_field SET ordinal = ?1 WHERE archetype_id = ?2 AND key = ?3",
            params![ordinal as i64, archetype_id, key],
        )?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchetypeFieldUsage {
    pub folder_id: i64,
    pub rel_path: String,
    pub title: String,
    pub value: String,
}

/// Folders on this archetype that have actually filled the field in —
/// what the frontend names in the confirmation before `remove_archetype_field`
/// deletes real data. Empty (unfilled, per the archetype-field convention)
/// values are excluded: there is nothing to lose for those folders.
pub fn archetype_field_usage(conn: &Connection, archetype_id: i64, key: &str) -> Result<Vec<ArchetypeFieldUsage>> {
    let mut stmt = conn.prepare(
        "SELECT f.id, f.rel_path, f.title, t.value
           FROM folder f
           JOIN folder_tag ft ON ft.folder_id = f.id
           JOIN tag t ON t.id = ft.tag_id
          WHERE f.archetype_id = ?1 AND f.deleted_at IS NULL AND t.key = ?2 AND t.value != ''
          ORDER BY f.rel_path",
    )?;
    let rows = stmt
        .query_map(params![archetype_id, key], |r| {
            Ok(ArchetypeFieldUsage {
                folder_id: r.get(0)?,
                rel_path: r.get(1)?,
                title: r.get(2)?,
                value: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Removes the field definition and, for every folder on this archetype,
/// the matching label — real deletion, meant to run only after the caller
/// has shown `archetype_field_usage` in a named confirmation (or found it
/// empty and skipped the prompt). See docs/DESIGN.md "Archetypes".
pub fn remove_archetype_field(conn: &Connection, archetype_id: i64, key: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM archetype_field WHERE archetype_id = ?1 AND key = ?2",
        params![archetype_id, key],
    )?;
    conn.execute(
        "DELETE FROM folder_tag
          WHERE tag_id IN (SELECT id FROM tag WHERE key = ?2)
            AND folder_id IN (SELECT id FROM folder WHERE archetype_id = ?1)",
        params![archetype_id, key],
    )?;
    Ok(())
}

// --- folder status lifecycle (M2.1) ---------------------------------------

/// `label` → a unique lower-case, hyphenated key. The seeded defaults
/// (active/wip/done/archived) are themselves exactly this shape, so a
/// user-created status sits alongside them without looking out of place.
fn slugify_unique(conn: &Connection, label: &str) -> Result<String> {
    let base: String = label
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>();
    let base = base.trim_matches('-').to_string();
    let base = if base.is_empty() { "status".to_string() } else { base };

    let mut candidate = base.clone();
    let mut n = 2;
    loop {
        let taken: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM folder_status WHERE key = ?1)",
            params![candidate],
            |r| r.get(0),
        )?;
        if !taken {
            return Ok(candidate);
        }
        candidate = format!("{base}-{n}");
        n += 1;
    }
}

pub fn create_folder_status(conn: &Connection, label: &str, colour: &str) -> Result<String> {
    let key = slugify_unique(conn, label)?;
    let next_ordinal: i64 = conn.query_row(
        "SELECT COALESCE(MAX(ordinal) + 1, 0) FROM folder_status",
        [],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO folder_status (key, label, colour, ordinal) VALUES (?1, ?2, ?3, ?4)",
        params![key, label, colour, next_ordinal],
    )?;
    Ok(key)
}

pub fn rename_folder_status(conn: &Connection, key: &str, label: &str) -> Result<()> {
    conn.execute(
        "UPDATE folder_status SET label = ?1 WHERE key = ?2",
        params![label, key],
    )?;
    Ok(())
}

pub fn recolour_folder_status(conn: &Connection, key: &str, colour: &str) -> Result<()> {
    conn.execute(
        "UPDATE folder_status SET colour = ?1 WHERE key = ?2",
        params![colour, key],
    )?;
    Ok(())
}

pub fn reorder_folder_statuses(conn: &Connection, ordered_keys: &[String]) -> Result<()> {
    for (ordinal, key) in ordered_keys.iter().enumerate() {
        conn.execute(
            "UPDATE folder_status SET ordinal = ?1 WHERE key = ?2",
            params![ordinal as i64, key],
        )?;
    }
    Ok(())
}

pub fn count_folders_by_status(conn: &Connection, key: &str) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM folder WHERE status = ?1 AND deleted_at IS NULL",
        params![key],
        |r| r.get(0),
    )?)
}

/// Errors rather than silently orphaning folders: if any still carry `key`,
/// the caller must supply `reassign_to` (a different, existing status).
/// Also refuses to remove the last remaining status — there must always be
/// one to fall back on.
pub fn remove_folder_status(conn: &Connection, key: &str, reassign_to: Option<&str>) -> Result<()> {
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM folder_status", [], |r| r.get(0))?;
    if total <= 1 {
        return Err(AppError::invalid("at least one folder status must remain"));
    }

    let in_use = count_folders_by_status(conn, key)?;
    if in_use > 0 {
        let Some(reassign_to) = reassign_to else {
            return Err(AppError::invalid(format!(
                "{in_use} folder(s) still use this status — choose a replacement"
            )));
        };
        if reassign_to == key {
            return Err(AppError::invalid("the replacement status must be different"));
        }
        let known: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM folder_status WHERE key = ?1)",
            params![reassign_to],
            |r| r.get(0),
        )?;
        if !known {
            return Err(AppError::invalid(format!("unknown folder status '{reassign_to}'")));
        }
        conn.execute(
            "UPDATE folder SET status = ?1 WHERE status = ?2",
            params![reassign_to, key],
        )?;
    }

    conn.execute("DELETE FROM folder_status WHERE key = ?1", params![key])?;
    Ok(())
}

// --- create / rename-dir / move / trash (M2.1 — see fs::relocate and
// fs::trash, which orchestrate these with the physical filesystem
// operation and the journal write) -----------------------------------------

/// Pure record insert — like `upsert` but errors if the path is already
/// occupied by a live folder rather than silently reusing it. What a
/// deliberate `create` needs that the walker's idempotent `upsert` must not
/// have.
pub fn create_record(
    conn: &Connection,
    parent_id: Option<i64>,
    rel_path: &str,
    title: &str,
) -> Result<i64> {
    if id_for_rel(conn, rel_path)?.is_some() {
        return Err(AppError::invalid(format!(
            "a folder already exists at '{rel_path}'"
        )));
    }
    conn.execute(
        "INSERT INTO folder (rel_path, title, parent_id, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![rel_path, title, parent_id, now()],
    )?;
    let id = conn.last_insert_rowid();
    crate::db::tags::sync_title_tag(conn, id, title)?;
    Ok(id)
}

/// Sets only `rel_path` — the directory-rename case, where the parent does
/// not change.
pub fn set_rel_path(conn: &Connection, id: i64, new_rel: &str) -> Result<()> {
    conn.execute(
        "UPDATE folder SET rel_path = ?1 WHERE id = ?2",
        params![new_rel, id],
    )?;
    Ok(())
}

/// Sets both — the move case.
pub fn set_parent_and_rel_path(
    conn: &Connection,
    id: i64,
    new_parent_id: Option<i64>,
    new_rel: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE folder SET parent_id = ?1, rel_path = ?2 WHERE id = ?3",
        params![new_parent_id, new_rel, id],
    )?;
    Ok(())
}

/// The `RENAME_FOLDER_SUBTREE`/`MOVE_FOLDER_SUBTREE` job body: rewrites
/// every *descendant*'s `rel_path` prefix after the top folder has already
/// moved on disk and been updated synchronously by the caller (`set_rel_path`
/// / `set_parent_and_rel_path`) — this only ever touches rows still carrying
/// the old prefix, so it is safe to run even if called twice.
///
/// `substr` counts Unicode characters, not bytes, when SQLite's text
/// encoding is UTF-8 (the default) — `chars().count()`, not `len()`, is
/// required here or a folder name with any multi-byte character would
/// corrupt every rel_path rewritten beneath it.
pub fn rewrite_subtree_paths(conn: &Connection, old_rel: &str, new_rel: &str) -> Result<()> {
    let old_len = old_rel.chars().count() as i64;
    conn.execute(
        "UPDATE folder
            SET rel_path = ?1 || substr(rel_path, ?3 + 1)
          WHERE deleted_at IS NULL AND rel_path LIKE ?2 || '/%'",
        params![new_rel, old_rel, old_len],
    )?;
    Ok(())
}

/// The trash operation's DB half: soft-delete every item in the subtree and
/// the folder plus every descendant folder, rewriting each trashed folder's
/// `rel_path` to `.trashed/<id>` so the original path space is immediately
/// reusable. Both statements are single bulk `UPDATE`s — cheap even for a
/// large subtree — which is why `trash` runs synchronously rather than as a
/// job, unlike the tag-cache rebuild.
pub fn trash_subtree_rows(conn: &Connection, rel_path: &str) -> Result<()> {
    let ts = now();
    conn.execute(
        "UPDATE item SET deleted_at = ?1
          WHERE deleted_at IS NULL
            AND folder_id IN (SELECT id FROM folder
                               WHERE rel_path = ?2 OR rel_path LIKE ?2 || '/%')",
        params![ts, rel_path],
    )?;
    conn.execute(
        "UPDATE folder SET deleted_at = ?1, rel_path = '.trashed/' || id
          WHERE deleted_at IS NULL AND (rel_path = ?2 OR rel_path LIKE ?2 || '/%')",
        params![ts, rel_path],
    )?;
    Ok(())
}

#[cfg(test)]
mod folder_metadata_tests {
    use super::*;
    use crate::db;

    fn memory_conn() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        db::migrate(&mut conn).unwrap();
        conn
    }

    fn person_archetype(conn: &Connection) -> i64 {
        let id = create_archetype(conn, "Person").unwrap();
        add_archetype_field(conn, id, "instagram", "handle", false).unwrap();
        add_archetype_field(conn, id, "tiktok", "handle", false).unwrap();
        id
    }

    #[test]
    fn apply_archetype_creates_empty_fields_without_clobbering_existing_values() {
        let conn = memory_conn();
        let ana = upsert(&conn, "people/ana", "Ana").unwrap();
        set_label(&conn, ana, "instagram", "@ana").unwrap();
        let person = person_archetype(&conn);

        apply_archetype(&conn, ana, person).unwrap();

        let detail = get_detail(&conn, ana).unwrap().unwrap();
        assert_eq!(detail.archetype_name.as_deref(), Some("Person"));
        let instagram = detail.fields.iter().find(|f| f.key == "instagram").unwrap();
        assert_eq!(instagram.value, "@ana", "existing value was not overwritten");
        let tiktok = detail.fields.iter().find(|f| f.key == "tiktok").unwrap();
        assert_eq!(tiktok.value, "", "unfilled fields still render");
    }

    #[test]
    fn create_archetype_ships_with_no_fields_until_added() {
        let conn = memory_conn();
        let id = create_archetype(&conn, "Place").unwrap();
        let archetypes = list_archetypes(&conn).unwrap();
        let place = archetypes.iter().find(|a| a.id == id).unwrap();
        assert!(place.fields.is_empty());
    }

    #[test]
    fn add_archetype_field_can_backfill_folders_already_on_the_archetype() {
        let conn = memory_conn();
        let person = person_archetype(&conn);
        let ana = upsert(&conn, "people/ana", "Ana").unwrap();
        apply_archetype(&conn, ana, person).unwrap();

        add_archetype_field(&conn, person, "youtube", "handle", true).unwrap();

        let detail = get_detail(&conn, ana).unwrap().unwrap();
        assert!(detail.fields.iter().any(|f| f.key == "youtube"));
    }

    #[test]
    fn remove_archetype_field_deletes_values_on_folders_using_it() {
        let conn = memory_conn();
        let person = person_archetype(&conn);
        let ana = upsert(&conn, "people/ana", "Ana").unwrap();
        apply_archetype(&conn, ana, person).unwrap();
        set_label(&conn, ana, "instagram", "@ana").unwrap();

        let usage = archetype_field_usage(&conn, person, "instagram").unwrap();
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].value, "@ana");

        remove_archetype_field(&conn, person, "instagram").unwrap();

        let detail = get_detail(&conn, ana).unwrap().unwrap();
        assert!(!detail.fields.iter().any(|f| f.key == "instagram"));
    }

    #[test]
    fn delete_archetype_leaves_folder_labels_in_place() {
        let conn = memory_conn();
        let person = person_archetype(&conn);
        let ana = upsert(&conn, "people/ana", "Ana").unwrap();
        apply_archetype(&conn, ana, person).unwrap();
        set_label(&conn, ana, "instagram", "@ana").unwrap();

        delete_archetype(&conn, person).unwrap();

        let detail = get_detail(&conn, ana).unwrap().unwrap();
        assert!(detail.archetype_id.is_none());
        assert!(
            detail.fields.is_empty(),
            "no archetype means no fields to render, even though the value is still there"
        );
        let still_there: String = conn
            .query_row(
                "SELECT t.value FROM folder_tag ft JOIN tag t ON t.id = ft.tag_id
                  WHERE ft.folder_id = ?1 AND t.key = 'instagram'",
                params![ana],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(still_there, "@ana");
    }

    #[test]
    fn remove_folder_status_requires_reassignment_when_in_use() {
        let conn = memory_conn();
        let ana = upsert(&conn, "people/ana", "Ana").unwrap();
        set_status(&conn, ana, "wip").unwrap();

        assert!(remove_folder_status(&conn, "wip", None).is_err());
        remove_folder_status(&conn, "wip", Some("active")).unwrap();

        let detail = get_detail(&conn, ana).unwrap().unwrap();
        assert_eq!(detail.status, "active");
    }

    #[test]
    fn remove_folder_status_refuses_to_remove_the_last_one() {
        let conn = memory_conn();
        for status in ["wip", "done", "archived"] {
            remove_folder_status(&conn, status, Some("active")).unwrap();
        }
        assert!(remove_folder_status(&conn, "active", None).is_err());
    }

    #[test]
    fn the_title_tag_cannot_be_removed() {
        let conn = memory_conn();
        let ana = upsert(&conn, "people/ana", "Ana").unwrap();
        let title_tag_id: i64 = conn
            .query_row(
                "SELECT tag_id FROM folder_tag WHERE folder_id = ?1 AND source = 'title'",
                params![ana],
                |r| r.get(0),
            )
            .unwrap();
        assert!(remove_tag(&conn, ana, title_tag_id).is_err());
    }

    #[test]
    fn subtree_totals_at_the_root_cover_the_whole_library() {
        let conn = memory_conn();
        let root = upsert(&conn, "", "Library").unwrap();
        upsert(&conn, "people", "People").unwrap();
        let ana = upsert(&conn, "people/ana", "Ana").unwrap();
        db::items::upsert(
            &conn,
            &db::items::NewItem {
                uuid: uuid::Uuid::new_v4().to_string(),
                folder_id: ana,
                disk_name: "a.jpg".to_string(),
                ext: "jpg".to_string(),
                orig_name: "a.jpg".to_string(),
                hash: "h".to_string(),
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

        let detail = get_detail(&conn, root).unwrap().unwrap();
        assert_eq!(detail.direct_count, 0);
        assert_eq!(detail.total_count, 1);
        assert_eq!(detail.subfolder_count, 2, "people, people/ana");
    }

    /// PLAN.md decision 20 / §M2.1: "verify subtree cases at scale with
    /// synth_library, and add the companion #[ignore]d test" — this is that
    /// test, sized like `fs::import::scale_check_100k_items`. DB-only, no
    /// files on disk: the physical directory rename is a single O(1) OS call
    /// regardless of subtree size, so it isn't what either job body spends
    /// its time on. Run explicitly with `cargo test --release
    /// scale_check_folder_relocate -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn scale_check_folder_relocate() {
        const LEAVES: usize = 2_000;
        let conn = memory_conn();

        db::begin_batch(&conn).unwrap();
        upsert(&conn, "", "Library").unwrap();
        let cat = upsert(&conn, "category", "Category").unwrap();
        let mut leaf_ids = Vec::with_capacity(LEAVES);
        for i in 0..LEAVES {
            let rel = format!("category/leaf-{i:04}");
            leaf_ids.push(upsert(&conn, &rel, &format!("Leaf {i:04}")).unwrap());
        }
        db::commit_batch(&conn).unwrap();

        db::begin_batch(&conn).unwrap();
        for (i, &leaf_id) in leaf_ids.iter().enumerate() {
            db::items::upsert(
                &conn,
                &db::items::NewItem {
                    uuid: uuid::Uuid::new_v4().to_string(),
                    folder_id: leaf_id,
                    disk_name: format!("{i}.jpg"),
                    ext: "jpg".to_string(),
                    orig_name: format!("{i}.jpg"),
                    hash: format!("h{i}"),
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
        }
        db::commit_batch(&conn).unwrap();

        // RENAME_FOLDER_SUBTREE: the top folder's own row is already updated
        // synchronously by the caller in production — mirrored here — then
        // only the descendant rewrite is timed.
        set_rel_path(&conn, cat, "category-renamed").unwrap();
        let rename_start = std::time::Instant::now();
        rewrite_subtree_paths(&conn, "category", "category-renamed").unwrap();
        let rename_elapsed = rename_start.elapsed();
        println!("rewrite_subtree_paths over {LEAVES} descendants: {rename_elapsed:?}");
        assert!(
            rename_elapsed < std::time::Duration::from_millis(500),
            "rename subtree rewrite too slow: {rename_elapsed:?}"
        );

        // MOVE_FOLDER_SUBTREE: rewrite, then rebuild the effective-tag cache
        // for the subtree, in the same job execution — see jobs::worker.
        set_rel_path(&conn, cat, "category-moved").unwrap();
        let move_start = std::time::Instant::now();
        rewrite_subtree_paths(&conn, "category-renamed", "category-moved").unwrap();
        crate::db::tags::rebuild_subtree(&conn, "category-moved").unwrap();
        let move_elapsed = move_start.elapsed();
        println!("move_folder_subtree (rewrite+retag) over {LEAVES} descendants: {move_elapsed:?}");
        assert!(
            move_elapsed < std::time::Duration::from_secs(3),
            "move subtree rewrite+retag too slow: {move_elapsed:?}"
        );
    }
}
