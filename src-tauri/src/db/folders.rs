use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::db::now;
use crate::error::Result;
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
    Ok(conn.last_insert_rowid())
}

/// The whole tree, in `rel_path` order, with direct and recursive counts.
/// Folder counts are in the thousands at most, so this is one query plus an
/// in-memory roll-up rather than a recursive CTE per node.
pub fn tree(conn: &Connection) -> Result<Vec<FolderNode>> {
    let mut stmt = conn.prepare(
        "SELECT f.id, f.rel_path, f.title, f.parent_id,
                (SELECT COUNT(*) FROM item i
                  WHERE i.folder_id = f.id AND i.deleted_at IS NULL) AS direct
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
