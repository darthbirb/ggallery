use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::db::now;
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderNode {
    pub id: i64,
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

pub fn count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM folder WHERE deleted_at IS NULL",
        [],
        |r| r.get(0),
    )?)
}

/// The live sibling at `(parent_id, title)`, if any — the one lookup
/// `UNIQUE(parent_id, title) WHERE deleted_at IS NULL` makes meaningful.
/// `title` is folded by the caller already in every real call site; this
/// does not fold it again so a caller checking an *already-folded* value
/// against itself never double-folds by accident.
pub fn id_for(conn: &Connection, parent_id: Option<i64>, title: &str) -> Result<Option<i64>> {
    Ok(conn
        .query_row(
            "SELECT id FROM folder WHERE parent_id IS ?1 AND title = ?2 AND deleted_at IS NULL",
            params![parent_id, title],
            |r| r.get(0),
        )
        .optional()?)
}

/// The whole tree, parent-first, with direct and recursive counts. Folder
/// counts are in the thousands at most, so this is one query plus an
/// in-memory roll-up rather than a recursive CTE per node.
pub fn tree(conn: &Connection) -> Result<Vec<FolderNode>> {
    let mut stmt = conn.prepare(
        "SELECT f.id, f.title, f.parent_id,
                (SELECT COUNT(*) FROM item i
                  WHERE i.folder_id = f.id AND i.deleted_at IS NULL) AS direct,
                f.status, f.favorite
           FROM folder f
          WHERE f.deleted_at IS NULL
          ORDER BY f.id",
    )?;

    let mut nodes: Vec<FolderNode> = stmt
        .query_map([], |r| {
            let direct: i64 = r.get(3)?;
            Ok(FolderNode {
                id: r.get(0)?,
                title: r.get(1)?,
                parent_id: r.get(2)?,
                depth: 0, // filled in below, once every node's parent is known
                direct_count: direct,
                total_count: direct,
                status: r.get(4)?,
                favorite: r.get::<_, i64>(5)? != 0,
            })
        })?
        .collect::<std::result::Result<_, _>>()?;

    let index: std::collections::HashMap<i64, usize> =
        nodes.iter().enumerate().map(|(i, n)| (n.id, i)).collect();

    // Depth: a root-level folder is 0, everything else is its parent's depth
    // plus one. `id` order is not guaranteed to be parent-before-child (a
    // folder can be created and then have children created under an
    // unrelated later id), so this resolves each node's depth by walking its
    // own parent chain rather than assuming a single forward pass suffices.
    fn depth_of(nodes: &[FolderNode], index: &std::collections::HashMap<i64, usize>, id: i64) -> u32 {
        let mut depth = 0;
        let mut current = id;
        // Bounded by the tree's real depth; a cycle should be structurally
        // impossible (parent_id only ever set through this module), but an
        // upper bound keeps a corrupted database from looping forever.
        for _ in 0..10_000 {
            let Some(&i) = index.get(&current) else { break };
            let Some(parent) = nodes[i].parent_id else { break };
            depth += 1;
            current = parent;
        }
        depth
    }
    for i in 0..nodes.len() {
        nodes[i].depth = depth_of(&nodes, &index, nodes[i].id);
    }

    // Roll counts up. Deepest first, so every child has already contributed
    // by the time its parent is visited.
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

/// Root-first ancestry, not including `folder_id` itself — what the folder
/// band and `ItemDetail`'s breadcrumb both render. Bounded by tree depth via
/// the primary-key join on `folder.id`, same shape as
/// `db::tags::resolve_ancestor_tags`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BreadcrumbCrumb {
    pub id: i64,
    pub title: String,
}

pub fn breadcrumb(conn: &Connection, folder_id: i64) -> Result<Vec<BreadcrumbCrumb>> {
    let mut stmt = conn.prepare(
        "WITH RECURSIVE ancestry(id, title, parent_id, depth) AS (
             SELECT id, title, parent_id, 0 FROM folder WHERE id = ?1
           UNION ALL
             SELECT f.id, f.title, f.parent_id, a.depth + 1
               FROM folder f JOIN ancestry a ON f.id = a.parent_id
         )
         SELECT id, title FROM ancestry WHERE id != ?1 ORDER BY depth DESC",
    )?;
    let rows = stmt
        .query_map(params![folder_id], |r| Ok(BreadcrumbCrumb { id: r.get(0)?, title: r.get(1)? }))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

// --- M2: folder detail, editing, archetypes -------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchetypeFieldValue {
    pub key: String,
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
    /// Cache-relative thumbnail path of the cover — the item chosen manually,
    /// or the newest item beneath the folder when nothing has been chosen.
    /// `None` for a folder with nothing in it yet.
    pub cover_thumb: Option<String>,
    /// Set only when the cover was chosen rather than picked automatically —
    /// what the band's "clear cover" control is enabled by.
    pub cover_item_id: Option<i64>,
}

/// The folder header's whole content in one call.
pub fn get_detail(conn: &Connection, id: i64) -> Result<Option<FolderDetail>> {
    #[allow(clippy::type_complexity)]
    let base: Option<(
        String,
        Option<i64>,
        String,
        i64,
        Option<String>,
        Option<i64>,
        Option<i64>,
        Option<String>,
        Option<i64>,
    )> = conn
        .query_row(
            "SELECT f.title, f.parent_id, f.status, f.favorite, f.notes,
                    f.last_added_at, f.archetype_id, a.name, f.cover_item_id
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
    let Some((title, parent_id, status, favorite, notes, last_added_at, archetype_id, archetype_name, cover_item_id)) =
        base
    else {
        return Ok(None);
    };

    let (direct_count, total_count, subfolder_count) = subtree_totals(conn, id)?;

    // Archetype fields first, in their defined order — then any other
    // labelled tag on this folder that isn't one of them. That second half
    // used to be missing entirely: a folder with no archetype rendered no
    // fields at all, and a folder *with* one only ever showed the fields
    // the archetype defined, so a one-off field added straight from the
    // band's "＋ add field" control wrote to `folder_tag` correctly but
    // never appeared anywhere — the bug reported after M2.5c shipped it.
    let mut archetype_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut fields: Vec<ArchetypeFieldValue> = Vec::new();
    if let Some(aid) = archetype_id {
        let mut stmt = conn.prepare(
            "SELECT af.key, af.ordinal,
                    COALESCE(
                      (SELECT t.value FROM folder_tag ft JOIN tag t ON t.id = ft.tag_id
                        WHERE ft.folder_id = ?1 AND t.key = af.key LIMIT 1),
                      '')
               FROM archetype_field af
              WHERE af.archetype_id = ?2
              ORDER BY af.ordinal",
        )?;
        fields = stmt
            .query_map(params![id, aid], |r| {
                Ok(ArchetypeFieldValue {
                    key: r.get(0)?,
                    ordinal: r.get(1)?,
                    value: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        archetype_keys.extend(fields.iter().map(|f| f.key.clone()));
    }

    let mut one_off_stmt = conn.prepare(
        "SELECT t.key, t.value FROM folder_tag ft JOIN tag t ON t.id = ft.tag_id
          WHERE ft.folder_id = ?1 AND t.key IS NOT NULL
          ORDER BY t.key COLLATE NOCASE",
    )?;
    let one_off_rows: Vec<(String, String)> = one_off_stmt
        .query_map(params![id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut next_ordinal = fields.len() as i64;
    for (key, value) in one_off_rows {
        if archetype_keys.contains(&key) {
            continue;
        }
        fields.push(ArchetypeFieldValue {
            key,
            ordinal: next_ordinal,
            value,
        });
        next_ordinal += 1;
    }

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

    let cover_thumb = cover_uuid(conn, id, cover_item_id)?.map(|uuid| crate::fs::paths::shard(&uuid));

    Ok(Some(FolderDetail {
        id,
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
        cover_thumb,
        cover_item_id,
    }))
}

/// The uuid the cover thumbnail is cached under: the chosen item if there is
/// one and it still exists, the newest item anywhere beneath the folder
/// otherwise. A folder with no cover set is never coverless in the interface —
/// "picked automatically" is one of the two options locked decision 1 names.
fn cover_uuid(conn: &Connection, folder_id: i64, cover_item_id: Option<i64>) -> Result<Option<String>> {
    if let Some(item_id) = cover_item_id {
        let chosen: Option<String> = conn
            .query_row(
                "SELECT uuid FROM item WHERE id = ?1 AND deleted_at IS NULL",
                params![item_id],
                |r| r.get(0),
            )
            .optional()?;
        if chosen.is_some() {
            return Ok(chosen);
        }
    }

    Ok(conn
        .query_row(
            "WITH RECURSIVE subtree(id) AS (
                 SELECT ?1
               UNION ALL
                 SELECT f.id FROM folder f JOIN subtree s ON f.parent_id = s.id
                 WHERE f.deleted_at IS NULL
             )
             SELECT i.uuid FROM item i
              WHERE i.deleted_at IS NULL AND i.folder_id IN (SELECT id FROM subtree)
              ORDER BY COALESCE(i.captured_at, i.mtime) DESC, i.id DESC LIMIT 1",
            params![folder_id],
            |r| r.get(0),
        )
        .optional()?)
}

/// Choose (or, with `None`, clear) the folder's cover. Clearing falls back to
/// the automatic pick rather than leaving the band blank.
pub fn set_cover(conn: &Connection, folder_id: i64, item_id: Option<i64>) -> Result<()> {
    conn.execute(
        "UPDATE folder SET cover_item_id = ?1 WHERE id = ?2",
        params![item_id, folder_id],
    )?;
    Ok(())
}

/// `(direct items, total items, subfolders)` for a folder and its subtree.
fn subtree_totals(conn: &Connection, folder_id: i64) -> Result<(i64, i64, i64)> {
    let direct: i64 = conn.query_row(
        "SELECT COUNT(*) FROM item WHERE folder_id = ?1 AND deleted_at IS NULL",
        params![folder_id],
        |r| r.get(0),
    )?;
    let (total, subfolders): (i64, i64) = conn.query_row(
        "WITH RECURSIVE subtree(id) AS (
             SELECT ?1
           UNION ALL
             SELECT f.id FROM folder f JOIN subtree s ON f.parent_id = s.id
             WHERE f.deleted_at IS NULL
         )
         SELECT
           (SELECT COUNT(*) FROM item WHERE deleted_at IS NULL AND folder_id IN (SELECT id FROM subtree)),
           (SELECT COUNT(*) - 1 FROM subtree)",
        params![folder_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok((direct, total, subfolders.max(0)))
}

/// The record's half of a retitle — title, its tag, and the journal entry.
/// Pure DB, no filesystem access — since PLAN.md decision 30 that is now the
/// *whole* of a retitle, not just the half `fs::relocate::retitle_folder`
/// used to call before deciding whether a directory needed to follow.
pub fn set_title(conn: &Connection, id: i64, title: &str, batch_id: &str) -> Result<()> {
    // Folded before comparing, not just before storing — otherwise retyping
    // "Ana" over an already-folded "ana" reads as a real change and journals
    // a rename that did nothing (PLAN.md decision 31).
    let title = crate::db::fold(title);
    let previous: String = conn.query_row(
        "SELECT title FROM folder WHERE id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    set_title_unjournalled(conn, id, &title)?;
    if previous != title {
        crate::db::journal::record_folder_rename_title(conn, batch_id, id, &previous, &title)?;
    }
    Ok(())
}

/// The title change alone — no journal entry. `fs::undo` uses this to put a
/// title back; a reversal is not itself an operation to reverse.
pub fn set_title_unjournalled(conn: &Connection, id: i64, title: &str) -> Result<()> {
    let title = crate::db::fold(title);
    conn.execute(
        "UPDATE folder SET title = ?1 WHERE id = ?2",
        params![title, id],
    )?;
    crate::db::tags::sync_title_tag(conn, id, &title)?;
    // A rename never changes `parent_id`, so no item's ancestry changed —
    // only the title tag's text did, which `sync_title_tag` above already
    // handled directly. Nothing to fan out.
    Ok(())
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

/// The other half of `apply_archetype`: un-applies the archetype from one
/// folder and drops the field values it owned (`source = 'archetype'`) —
/// "and its tags go with it too". A one-off field added independently
/// through "＋ add field" is `source = 'manual'` and is untouched.
pub fn clear_archetype(conn: &Connection, folder_id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM folder_tag WHERE folder_id = ?1 AND source = 'archetype'",
        params![folder_id],
    )?;
    conn.execute(
        "UPDATE folder SET archetype_id = NULL WHERE id = ?1",
        params![folder_id],
    )?;
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
    crate::jobs::enqueue_retag_folder(conn, Some(folder_id))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// A field on an archetype: a name and a position, and nothing else. It
/// carried a `type` until M2.5a.1 — see `005_drop_archetype_field_type.sql`.
pub struct ArchetypeFieldDef {
    pub key: String,
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
            "SELECT key, ordinal FROM archetype_field WHERE archetype_id = ?1 ORDER BY ordinal",
        )?;
        let fields = fstmt
            .query_map(params![id], |r| {
                Ok(ArchetypeFieldDef {
                    key: r.get(0)?,
                    ordinal: r.get(1)?,
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
    apply_to_existing: bool,
) -> Result<()> {
    let next_ordinal: i64 = conn.query_row(
        "SELECT COALESCE(MAX(ordinal) + 1, 0) FROM archetype_field WHERE archetype_id = ?1",
        params![archetype_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO archetype_field (archetype_id, key, ordinal) VALUES (?1, ?2, ?3)",
        params![archetype_id, key, next_ordinal],
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
    pub title: String,
    pub value: String,
}

/// Folders on this archetype that have actually filled the field in —
/// what the frontend names in the confirmation before `remove_archetype_field`
/// deletes real data. Empty (unfilled, per the archetype-field convention)
/// values are excluded: there is nothing to lose for those folders.
pub fn archetype_field_usage(conn: &Connection, archetype_id: i64, key: &str) -> Result<Vec<ArchetypeFieldUsage>> {
    let mut stmt = conn.prepare(
        "SELECT f.id, f.title, t.value
           FROM folder f
           JOIN folder_tag ft ON ft.folder_id = f.id
           JOIN tag t ON t.id = ft.tag_id
          WHERE f.archetype_id = ?1 AND f.deleted_at IS NULL AND t.key = ?2 AND t.value != ''
          ORDER BY f.title",
    )?;
    let rows = stmt
        .query_map(params![archetype_id, key], |r| {
            Ok(ArchetypeFieldUsage {
                folder_id: r.get(0)?,
                title: r.get(1)?,
                value: r.get(2)?,
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

// --- create / retitle / move / trash (M2.1, rebuilt for PLAN.md §M2.6 —
// none of this touches a file any more; see fs::relocate and fs::trash for
// the journal-writing orchestration around these) --------------------------

/// Pure record insert — errors if a live sibling already has this
/// `(parent_id, title)` rather than silently reusing it, so a deliberate
/// `create` reports a clean error instead of the raw `UNIQUE constraint
/// failed` the DB index would otherwise surface.
pub fn create_record(conn: &Connection, parent_id: Option<i64>, title: &str) -> Result<i64> {
    let title = crate::db::fold(title);
    if id_for(conn, parent_id, &title)?.is_some() {
        return Err(AppError::invalid(format!("a folder named '{title}' already exists here")));
    }
    conn.execute(
        "INSERT INTO folder (title, parent_id, created_at) VALUES (?1, ?2, ?3)",
        params![title, parent_id, now()],
    )?;
    let id = conn.last_insert_rowid();
    crate::db::tags::sync_title_tag(conn, id, &title)?;
    Ok(id)
}

/// The move operation's DB half — a pure `parent_id` write. Descendants need
/// no rewrite at all (hierarchy is `parent_id`, not a path prefix); only the
/// effective-tag cache does, because inherited tags are recomputed from the
/// new ancestry — see `jobs::enqueue_retag_folder`, which the caller in
/// `fs::relocate` enqueues alongside this.
pub fn set_parent(conn: &Connection, id: i64, new_parent_id: Option<i64>) -> Result<()> {
    conn.execute(
        "UPDATE folder SET parent_id = ?1 WHERE id = ?2",
        params![new_parent_id, id],
    )?;
    Ok(())
}

/// Soft-delete a folder and its whole subtree: every item in it, and every
/// descendant folder. `parent_id`/`title` are left exactly as they are —
/// `UNIQUE(parent_id, title)` is a partial index scoped `WHERE deleted_at IS
/// NULL`, so a trashed folder never blocks a new one at the same spot, and
/// there is nothing left to free by rewriting anything (PLAN.md §M2.6 — this
/// used to rewrite `rel_path` to `.trashed/<id>` for exactly that purpose).
/// Returns the timestamp stamped on every row, which `db::journal` records so
/// undo can recognise exactly this batch's rows later.
pub fn trash_subtree(conn: &Connection, folder_id: i64) -> Result<i64> {
    let ts = now();
    conn.execute(
        "WITH RECURSIVE subtree(id) AS (
             SELECT ?1
           UNION ALL
             SELECT f.id FROM folder f JOIN subtree s ON f.parent_id = s.id
             WHERE f.deleted_at IS NULL
         )
         UPDATE item SET deleted_at = ?2
          WHERE deleted_at IS NULL AND folder_id IN (SELECT id FROM subtree)",
        params![folder_id, ts],
    )?;
    conn.execute(
        "WITH RECURSIVE subtree(id) AS (
             SELECT ?1
           UNION ALL
             SELECT f.id FROM folder f JOIN subtree s ON f.parent_id = s.id
             WHERE f.deleted_at IS NULL
         )
         UPDATE folder SET deleted_at = ?2 WHERE deleted_at IS NULL AND id IN (SELECT id FROM subtree)",
        params![folder_id, ts],
    )?;
    Ok(ts)
}

/// Undo's half of `trash_subtree`. Re-derives the subtree by walking
/// `parent_id` from `folder_id` — safe even while every row in it is
/// currently marked deleted, since trashing never touched the hierarchy
/// itself — and clears `deleted_at` only on rows stamped exactly
/// `trashed_at`, so a folder independently deleted (and not yet restored) in
/// the meantime is left alone.
pub fn restore_subtree(conn: &Connection, folder_id: i64, trashed_at: i64) -> Result<()> {
    conn.execute(
        "WITH RECURSIVE subtree(id) AS (
             SELECT ?1
           UNION ALL
             SELECT f.id FROM folder f JOIN subtree s ON f.parent_id = s.id
         )
         UPDATE folder SET deleted_at = NULL
          WHERE deleted_at = ?2 AND id IN (SELECT id FROM subtree)",
        params![folder_id, trashed_at],
    )?;
    conn.execute(
        "WITH RECURSIVE subtree(id) AS (
             SELECT ?1
           UNION ALL
             SELECT f.id FROM folder f JOIN subtree s ON f.parent_id = s.id
         )
         UPDATE item SET deleted_at = NULL
          WHERE deleted_at = ?2 AND folder_id IN (SELECT id FROM subtree)",
        params![folder_id, trashed_at],
    )?;
    Ok(())
}

#[cfg(test)]
/// Test-only convenience: get-or-create a chain of folders from a
/// `/`-separated path of titles, auto-titling every intermediate ancestor
/// from its own segment and giving the leaf `leaf_title` — replicates the
/// pre-M2.6 walker's `upsert(rel_path, title)` ergonomics for the many tests
/// across this crate that build a folder tree by path, without any
/// production code depending on path-based lookup any more (PLAN.md decision
/// 30 — folders are created explicitly, one at a time, by id).
pub fn ensure_path(conn: &Connection, path: &str, leaf_title: &str) -> Result<i64> {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    assert!(!segments.is_empty(), "ensure_path does not create a root — there is no root folder any more");

    let mut parent_id: Option<i64> = None;
    let mut id = 0i64;
    let last = segments.len() - 1;
    for (i, seg) in segments.iter().enumerate() {
        let title = crate::db::fold(if i == last { leaf_title } else { seg });
        id = match id_for(conn, parent_id, &title)? {
            Some(existing) => existing,
            None => create_record(conn, parent_id, &title)?,
        };
        parent_id = Some(id);
    }
    Ok(id)
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
        add_archetype_field(conn, id, "instagram", false).unwrap();
        add_archetype_field(conn, id, "tiktok", false).unwrap();
        id
    }

    #[test]
    fn apply_archetype_creates_empty_fields_without_clobbering_existing_values() {
        let conn = memory_conn();
        let ana = ensure_path(&conn, "people/ana", "Ana").unwrap();
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
        let ana = ensure_path(&conn, "people/ana", "Ana").unwrap();
        apply_archetype(&conn, ana, person).unwrap();

        add_archetype_field(&conn, person, "youtube", true).unwrap();

        let detail = get_detail(&conn, ana).unwrap().unwrap();
        assert!(detail.fields.iter().any(|f| f.key == "youtube"));
    }

    #[test]
    fn remove_archetype_field_deletes_values_on_folders_using_it() {
        let conn = memory_conn();
        let person = person_archetype(&conn);
        let ana = ensure_path(&conn, "people/ana", "Ana").unwrap();
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
        let ana = ensure_path(&conn, "people/ana", "Ana").unwrap();
        apply_archetype(&conn, ana, person).unwrap();
        set_label(&conn, ana, "instagram", "@ana").unwrap();

        delete_archetype(&conn, person).unwrap();

        let detail = get_detail(&conn, ana).unwrap().unwrap();
        assert!(detail.archetype_id.is_none());
        let instagram = detail.fields.iter().find(|f| f.key == "instagram").unwrap();
        assert_eq!(instagram.value, "@ana");
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
    fn one_off_fields_render_with_or_without_an_archetype() {
        let conn = memory_conn();
        let ana = ensure_path(&conn, "people/ana", "Ana").unwrap();

        set_label(&conn, ana, "city", "Lisbon").unwrap();
        let detail = get_detail(&conn, ana).unwrap().unwrap();
        let city = detail.fields.iter().find(|f| f.key == "city").unwrap();
        assert_eq!(city.value, "lisbon");

        let person = person_archetype(&conn);
        apply_archetype(&conn, ana, person).unwrap();
        let detail = get_detail(&conn, ana).unwrap().unwrap();
        assert!(detail.fields.iter().any(|f| f.key == "instagram"));
        assert!(detail.fields.iter().any(|f| f.key == "tiktok"));
        let city = detail.fields.iter().find(|f| f.key == "city").unwrap();
        assert_eq!(city.value, "lisbon");
    }

    #[test]
    fn clear_archetype_drops_the_folder_and_its_field_values() {
        let conn = memory_conn();
        let person = person_archetype(&conn);
        let ana = ensure_path(&conn, "people/ana", "Ana").unwrap();
        apply_archetype(&conn, ana, person).unwrap();
        set_label(&conn, ana, "instagram", "@ana").unwrap();
        set_label(&conn, ana, "city", "Lisbon").unwrap();

        clear_archetype(&conn, ana).unwrap();

        let detail = get_detail(&conn, ana).unwrap().unwrap();
        assert!(detail.archetype_id.is_none());
        assert!(!detail.fields.iter().any(|f| f.key == "instagram"));
        assert!(!detail.fields.iter().any(|f| f.key == "tiktok"));
        let city = detail.fields.iter().find(|f| f.key == "city").unwrap();
        assert_eq!(city.value, "lisbon");

        let orphaned: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM folder_tag ft JOIN tag t ON t.id = ft.tag_id
                  WHERE ft.folder_id = ?1 AND t.key = 'instagram'",
                params![ana],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(orphaned, 0, "the archetype's own field values are gone, not just hidden");
    }

    #[test]
    fn remove_folder_status_requires_reassignment_when_in_use() {
        let conn = memory_conn();
        let ana = ensure_path(&conn, "people/ana", "Ana").unwrap();
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
        let ana = ensure_path(&conn, "people/ana", "Ana").unwrap();
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
    fn subtree_totals_cover_the_whole_subtree_recursively() {
        let conn = memory_conn();
        let people = ensure_path(&conn, "people", "People").unwrap();
        let ana = ensure_path(&conn, "people/ana", "Ana").unwrap();
        db::items::upsert(
            &conn,
            &db::items::NewItem {
                uuid: uuid::Uuid::new_v4().to_string(),
                folder_id: Some(ana),
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

        let detail = get_detail(&conn, people).unwrap().unwrap();
        assert_eq!(detail.direct_count, 0);
        assert_eq!(detail.total_count, 1);
        assert_eq!(detail.subfolder_count, 1, "ana");
    }

    #[test]
    fn creating_a_folder_at_an_occupied_spot_errors_cleanly() {
        let conn = memory_conn();
        create_record(&conn, None, "Ana").unwrap();
        let err = create_record(&conn, None, "ana").unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn a_new_folder_can_be_created_at_a_still_trashed_folders_old_spot() {
        let conn = memory_conn();
        let people = ensure_path(&conn, "people", "People").unwrap();
        let ana = ensure_path(&conn, "people/ana", "Ana").unwrap();
        let trip = ensure_path(&conn, "people/ana/trip", "Trip").unwrap();

        trash_subtree(&conn, ana).unwrap();
        assert!(get_detail(&conn, ana).unwrap().is_none(), "no longer live");
        assert!(get_detail(&conn, trip).unwrap().is_none(), "descendant trashed too");

        // A new folder can be created at the exact same (parent, title) spot
        // while the old one is still trashed — the partial index is what
        // makes this possible without any path-mangling.
        let replacement = create_record(&conn, Some(people), "ana").unwrap();
        assert_ne!(replacement, ana);
    }

    #[test]
    fn restoring_a_trashed_folder_puts_back_exactly_its_own_parent_and_title() {
        let conn = memory_conn();
        let people = ensure_path(&conn, "people", "People").unwrap();
        let ana = ensure_path(&conn, "people/ana", "Ana").unwrap();
        let trip = ensure_path(&conn, "people/ana/trip", "Trip").unwrap();

        // Its own row is never touched by the trash — parent_id and title
        // are exactly what a restore, with nothing else contending for the
        // spot, puts back.
        let ts = trash_subtree(&conn, ana).unwrap();
        restore_subtree(&conn, ana, ts).unwrap();

        let restored = get_detail(&conn, ana).unwrap().unwrap();
        assert_eq!(restored.parent_id, Some(people));
        assert_eq!(restored.title, "ana");
        assert!(get_detail(&conn, trip).unwrap().is_some(), "descendant restored too");
    }

    /// PLAN.md decision 20 / §M2.1: "verify subtree cases at scale with
    /// synth_library". §M2.6 replaces the rel_path-prefix rewrite this used
    /// to time with the new `parent_id`-recursive queries — the fan-out a
    /// folder-level tag edit still needs (`db::tags::rebuild_subtree`), and
    /// the subtree totals/breadcrumb every folder detail fetch runs. Run
    /// explicitly with `cargo test --release scale_check_folder_subtree --
    /// --ignored --nocapture`.
    #[test]
    #[ignore]
    fn scale_check_folder_subtree() {
        const LEAVES: usize = 2_000;
        let conn = memory_conn();

        db::begin_batch(&conn).unwrap();
        let cat = create_record(&conn, None, "category").unwrap();
        let mut leaf_ids = Vec::with_capacity(LEAVES);
        for i in 0..LEAVES {
            leaf_ids.push(create_record(&conn, Some(cat), &format!("leaf-{i:04}")).unwrap());
        }
        db::commit_batch(&conn).unwrap();

        db::begin_batch(&conn).unwrap();
        for (i, &leaf_id) in leaf_ids.iter().enumerate() {
            db::items::upsert(
                &conn,
                &db::items::NewItem {
                    uuid: uuid::Uuid::new_v4().to_string(),
                    folder_id: Some(leaf_id),
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

        let detail_start = std::time::Instant::now();
        let detail = get_detail(&conn, cat).unwrap().unwrap();
        let detail_elapsed = detail_start.elapsed();
        assert_eq!(detail.total_count, LEAVES as i64);
        println!("get_detail (subtree totals) over {LEAVES} descendants: {detail_elapsed:?}");
        assert!(detail_elapsed < std::time::Duration::from_millis(500), "subtree totals too slow: {detail_elapsed:?}");

        // A folder-level tag edit's fan-out into item_effective_tag.
        add_flag(&conn, cat, "tagged").unwrap();
        let retag_start = std::time::Instant::now();
        crate::db::tags::rebuild_subtree(&conn, Some(cat)).unwrap();
        let retag_elapsed = retag_start.elapsed();
        println!("rebuild_subtree over {LEAVES} descendants: {retag_elapsed:?}");
        assert!(retag_elapsed < std::time::Duration::from_secs(3), "retag fan-out too slow: {retag_elapsed:?}");

        // A move: parent_id write plus the same fan-out — no path rewrite
        // left to time at all, unlike the pre-M2.6 version of this check.
        let other = create_record(&conn, None, "other").unwrap();
        let move_start = std::time::Instant::now();
        set_parent(&conn, cat, Some(other)).unwrap();
        crate::db::tags::rebuild_subtree(&conn, Some(cat)).unwrap();
        let move_elapsed = move_start.elapsed();
        println!("move (parent_id write + retag) over {LEAVES} descendants: {move_elapsed:?}");
        assert!(move_elapsed < std::time::Duration::from_secs(3), "move too slow: {move_elapsed:?}");
    }
}
