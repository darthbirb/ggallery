use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

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

pub fn id_for_rel(conn: &Connection, rel_path: &str) -> Result<Option<i64>> {
    Ok(conn
        .query_row(
            "SELECT id FROM folder WHERE rel_path = ?1",
            params![rel_path],
            |r| r.get(0),
        )
        .optional()?)
}

pub fn count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM folder", [], |r| r.get(0))?)
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
              WHERE f.id = ?1",
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
        "SELECT id FROM folder WHERE rel_path = ?1",
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
        let subfolders: i64 =
            conn.query_row("SELECT COUNT(*) - 1 FROM folder", [], |r| r.get(0))?;
        (total, subfolders)
    } else {
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM item
              WHERE deleted_at IS NULL
                AND folder_id IN (SELECT id FROM folder
                                   WHERE rel_path = ?1 OR rel_path LIKE ?1 || '/%')",
            params![rel_path],
            |r| r.get(0),
        )?;
        let subfolders: i64 = conn.query_row(
            "SELECT COUNT(*) FROM folder WHERE rel_path LIKE ?1 || '/%'",
            params![rel_path],
            |r| r.get(0),
        )?;
        (total, subfolders)
    };
    Ok((direct, total, subfolders.max(0)))
}

pub fn set_title(conn: &Connection, id: i64, title: &str) -> Result<()> {
    conn.execute(
        "UPDATE folder SET title = ?1 WHERE id = ?2",
        params![title, id],
    )?;
    crate::db::tags::sync_title_tag(conn, id, title)?;
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

/// Sets `folder.archetype_id` and creates an empty label for every field the
/// archetype defines that this folder doesn't already carry a value for —
/// re-applying, or applying after `apply_name_parse` already set one field,
/// never clobbers an existing value.
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
        let already: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM folder_tag ft JOIN tag t ON t.id = ft.tag_id
              WHERE ft.folder_id = ?1 AND t.key = ?2)",
            params![folder_id, key],
            |r| r.get(0),
        )?;
        if already {
            continue;
        }
        let tag_id = crate::db::tags::get_or_create_tag(conn, Some(&key), "")?;
        conn.execute(
            "INSERT OR IGNORE INTO folder_tag (folder_id, tag_id, source) VALUES (?1, ?2, 'archetype')",
            params![folder_id, tag_id],
        )?;
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

// --- folder-name parsing (deferred from M1.5) -----------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NameParseCandidate {
    pub folder_id: i64,
    pub rel_path: String,
    pub current_title: String,
    pub proposed_title: String,
    pub handle: String,
}

/// `"Ana (@ana)"` → `("Ana", "@ana")`. Only a title that still carries this
/// exact shape is offered, which is what makes the scan safe to run
/// repeatedly: once a folder has been through `apply_name_parse` its title is
/// just `"Ana"`, no longer matches, and won't be offered again.
fn parse_name_handle(title: &str) -> Option<(String, String)> {
    let trimmed = title.trim();
    if !trimmed.ends_with(')') {
        return None;
    }
    let (name, rest) = trimmed.rsplit_once('(')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let inner = rest.trim_end_matches(')').trim();
    let handle = inner.strip_prefix('@')?;
    if handle.is_empty()
        || !handle
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
    {
        return None;
    }
    Some((name.to_string(), format!("@{handle}")))
}

pub fn scan_name_parse(conn: &Connection) -> Result<Vec<NameParseCandidate>> {
    let mut stmt = conn.prepare("SELECT id, rel_path, title FROM folder ORDER BY rel_path")?;
    let rows: Vec<(i64, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    drop(stmt);

    let mut out = Vec::new();
    for (folder_id, rel_path, title) in rows {
        if let Some((proposed_title, handle)) = parse_name_handle(&title) {
            out.push(NameParseCandidate {
                folder_id,
                rel_path,
                current_title: title,
                proposed_title,
                handle,
            });
        }
    }
    Ok(out)
}

/// Applies a (possibly user-edited) subset of `scan_name_parse`'s rows:
/// title becomes the parsed name, the Person archetype is applied if the
/// folder doesn't already have one, and `instagram` is set to the parsed
/// handle.
pub fn apply_name_parse(conn: &Connection, rows: &[NameParseCandidate]) -> Result<()> {
    let person_id: i64 = conn.query_row(
        "SELECT id FROM archetype WHERE name = 'Person'",
        [],
        |r| r.get(0),
    )?;
    for row in rows {
        set_title(conn, row.folder_id, &row.proposed_title)?;
        let has_archetype: Option<i64> = conn.query_row(
            "SELECT archetype_id FROM folder WHERE id = ?1",
            params![row.folder_id],
            |r| r.get(0),
        )?;
        if has_archetype.is_none() {
            apply_archetype(conn, row.folder_id, person_id)?;
        }
        set_label(conn, row.folder_id, "instagram", &row.handle)?;
    }
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

    #[test]
    fn apply_archetype_creates_empty_fields_without_clobbering_existing_values() {
        let conn = memory_conn();
        let ana = upsert(&conn, "people/ana", "Ana").unwrap();
        set_label(&conn, ana, "instagram", "@ana").unwrap();

        apply_archetype(&conn, ana, 1).unwrap(); // Person, seeded id 1

        let detail = get_detail(&conn, ana).unwrap().unwrap();
        assert_eq!(detail.archetype_name.as_deref(), Some("Person"));
        let instagram = detail.fields.iter().find(|f| f.key == "instagram").unwrap();
        assert_eq!(instagram.value, "@ana", "existing value was not overwritten");
        let tiktok = detail.fields.iter().find(|f| f.key == "tiktok").unwrap();
        assert_eq!(tiktok.value, "", "unfilled fields still render");
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
    fn scan_name_parse_matches_only_the_documented_shape() {
        let conn = memory_conn();
        upsert(&conn, "people/ana", "Ana (@ana)").unwrap();
        upsert(&conn, "people/sara", "Sara").unwrap();
        upsert(&conn, "people/bo", "Bo (weird)").unwrap();

        let candidates = scan_name_parse(&conn).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].proposed_title, "Ana");
        assert_eq!(candidates[0].handle, "@ana");
    }

    #[test]
    fn apply_name_parse_sets_title_archetype_and_instagram() {
        let conn = memory_conn();
        let ana = upsert(&conn, "people/ana", "Ana (@ana)").unwrap();
        let candidates = scan_name_parse(&conn).unwrap();

        apply_name_parse(&conn, &candidates).unwrap();

        let detail = get_detail(&conn, ana).unwrap().unwrap();
        assert_eq!(detail.title, "Ana");
        assert_eq!(detail.archetype_name.as_deref(), Some("Person"));
        assert_eq!(
            detail.fields.iter().find(|f| f.key == "instagram").unwrap().value,
            "@ana"
        );

        // Applied once — a second scan finds nothing left to offer.
        assert!(scan_name_parse(&conn).unwrap().is_empty());
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
}
