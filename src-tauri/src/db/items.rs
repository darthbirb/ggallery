use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::db::now;
use crate::error::Result;

/// What the walker knows about a file before anything has been opened.
#[derive(Debug, Clone)]
pub struct NewItem {
    pub uuid: String,
    pub folder_id: i64,
    pub disk_name: String,
    pub ext: String,
    pub orig_name: String,
    pub hash: String,
    pub size_bytes: i64,
    pub mtime: i64,
    pub kind: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<i64>,
    pub codec: Option<String>,
    pub bitrate: Option<i64>,
    pub captured_at: Option<i64>,
    pub captured_src: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ExistingItem {
    pub id: i64,
    pub uuid: String,
    pub size_bytes: i64,
    pub mtime: i64,
    pub deleted: bool,
}

/// Everything a thumbnail or sprite job needs to find the file and name its
/// output. `folder_rel` + `disk_name` go through `fs::paths` to become an
/// absolute path — never concatenated anywhere else.
#[derive(Debug, Clone)]
pub struct ItemFile {
    pub id: i64,
    pub uuid: String,
    pub kind: String,
    pub folder_rel: String,
    pub disk_name: String,
    pub duration_ms: Option<i64>,
    /// False when probing never managed to read a width and height.
    pub has_dimensions: bool,
}

/// One row of the grid. Kept deliberately narrow: at 100k items this whole
/// list crosses the IPC boundary in one go, so anything the grid does not draw
/// stays out of it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GridItem {
    pub id: i64,
    /// Cache-relative path of this item's thumbnail and sprite —
    /// `ab/cd/<uuid>.webp`. Resolved against the cache directories the library
    /// reports on open, so the frontend never derives a path itself.
    pub thumb: String,
    pub kind: String,
    pub w: Option<i64>,
    pub h: Option<i64>,
    pub duration_ms: Option<i64>,
    pub favorite: bool,
    /// Sort timestamp — captured date where known, file mtime otherwise.
    pub at: i64,
    pub name: String,
}

#[derive(Debug, Clone, Default)]
pub struct Scope {
    /// Normalised folder rel_path. `None` or `""` means the whole library.
    pub folder: Option<String>,
    pub recursive: bool,
}

pub fn existing(
    conn: &Connection,
    folder_id: i64,
    disk_name: &str,
) -> Result<Option<ExistingItem>> {
    Ok(conn
        .query_row(
            "SELECT id, uuid, size_bytes, mtime, deleted_at
               FROM item
              WHERE folder_id = ?1 AND disk_name = ?2 COLLATE NOCASE",
            params![folder_id, disk_name],
            |r| {
                let deleted: Option<i64> = r.get(4)?;
                Ok(ExistingItem {
                    id: r.get(0)?,
                    uuid: r.get(1)?,
                    size_bytes: r.get(2)?,
                    mtime: r.get(3)?,
                    deleted: deleted.is_some(),
                })
            },
        )
        .optional()?)
}

/// Insert a fully-probed item, or refresh the existing row for that file.
///
/// The uuid of an existing row is kept — it is the cache key for thumbnails
/// and sprites, and re-issuing it would orphan them.
pub fn upsert(conn: &Connection, item: &NewItem) -> Result<i64> {
    if let Some(found) = existing(conn, item.folder_id, &item.disk_name)? {
        conn.execute(
            "UPDATE item
                SET ext = ?1, orig_name = ?2, hash = ?3, size_bytes = ?4, mtime = ?5,
                    kind = ?6, width = ?7, height = ?8, duration_ms = ?9, codec = ?10,
                    bitrate = ?11, captured_at = ?12, captured_src = ?13, deleted_at = NULL
              WHERE id = ?14",
            params![
                item.ext,
                item.orig_name,
                item.hash,
                item.size_bytes,
                item.mtime,
                item.kind,
                item.width,
                item.height,
                item.duration_ms,
                item.codec,
                item.bitrate,
                item.captured_at,
                item.captured_src,
                found.id,
            ],
        )?;
        return Ok(found.id);
    }

    conn.execute(
        "INSERT INTO item (uuid, folder_id, disk_name, ext, orig_name, hash, size_bytes,
                           mtime, kind, width, height, duration_ms, codec, bitrate,
                           captured_at, captured_src, added_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        params![
            item.uuid,
            item.folder_id,
            item.disk_name,
            item.ext,
            item.orig_name,
            item.hash,
            item.size_bytes,
            item.mtime,
            item.kind,
            item.width,
            item.height,
            item.duration_ms,
            item.codec,
            item.bitrate,
            item.captured_at,
            item.captured_src,
            now(),
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn file_for(conn: &Connection, id: i64) -> Result<Option<ItemFile>> {
    Ok(conn
        .query_row(
            "SELECT i.id, i.uuid, i.kind, f.rel_path, i.disk_name, i.duration_ms,
                    i.width IS NOT NULL AND i.height IS NOT NULL
               FROM item i JOIN folder f ON f.id = i.folder_id
              WHERE i.id = ?1",
            params![id],
            |r| {
                Ok(ItemFile {
                    id: r.get(0)?,
                    uuid: r.get(1)?,
                    kind: r.get(2)?,
                    folder_rel: r.get(3)?,
                    disk_name: r.get(4)?,
                    duration_ms: r.get(5)?,
                    has_dimensions: r.get::<_, i64>(6)? != 0,
                })
            },
        )
        .optional()?)
}

/// Fill in dimensions learned later, without disturbing anything else on the
/// row. Used by the thumbnail job when the original probe came up empty.
pub fn set_dimensions(conn: &Connection, id: i64, width: i64, height: i64) -> Result<()> {
    conn.execute(
        "UPDATE item SET width = ?1, height = ?2 WHERE id = ?3",
        params![width, height, id],
    )?;
    Ok(())
}

pub fn count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM item WHERE deleted_at IS NULL",
        [],
        |r| r.get(0),
    )?)
}

pub fn list(conn: &Connection, scope: &Scope) -> Result<Vec<GridItem>> {
    let folder = scope.folder.as_deref().unwrap_or("");
    let base = "SELECT id, uuid, kind, width, height, duration_ms, favorite,
                       COALESCE(captured_at, mtime) AS at, COALESCE(orig_name, disk_name)
                  FROM item
                 WHERE deleted_at IS NULL";
    let order = " ORDER BY at DESC, id DESC";

    let map = |r: &rusqlite::Row<'_>| -> rusqlite::Result<GridItem> {
        Ok(GridItem {
            id: r.get(0)?,
            thumb: crate::fs::paths::shard(&r.get::<_, String>(1)?),
            kind: r.get(2)?,
            w: r.get(3)?,
            h: r.get(4)?,
            duration_ms: r.get(5)?,
            favorite: r.get::<_, i64>(6)? != 0,
            at: r.get(7)?,
            name: r.get(8)?,
        })
    };

    // The whole library, or one folder, or one folder and everything beneath
    // it. Folder views are recursive by default — see PLAN.md decision 10.
    let rows: Vec<GridItem> = if folder.is_empty() {
        let sql = format!("{base}{order}");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], map)?.collect::<rusqlite::Result<_>>()?;
        rows
    } else if scope.recursive {
        let sql = format!(
            "{base} AND folder_id IN (SELECT id FROM folder
                                       WHERE rel_path = ?1 OR rel_path LIKE ?1 || '/%'){order}"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params![folder], map)?
            .collect::<rusqlite::Result<_>>()?;
        rows
    } else {
        let sql =
            format!("{base} AND folder_id = (SELECT id FROM folder WHERE rel_path = ?1){order}");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params![folder], map)?
            .collect::<rusqlite::Result<_>>()?;
        rows
    };

    Ok(rows)
}

// --- reconciliation sweep -------------------------------------------------
//
// A re-index records every file it saw, then soft-deletes the rows it did not.
// This is bookkeeping only: nothing on disk is touched, and a file that comes
// back clears its own `deleted_at` through `upsert`.

pub fn begin_sweep(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "DROP TABLE IF EXISTS temp.seen;
         CREATE TEMP TABLE seen (folder_id INTEGER NOT NULL, disk_name TEXT NOT NULL);",
    )?;
    Ok(())
}

pub fn mark_seen(conn: &Connection, folder_id: i64, disk_name: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO temp.seen (folder_id, disk_name) VALUES (?1, ?2)",
        params![folder_id, disk_name],
    )?;
    Ok(())
}

pub fn finish_sweep(conn: &Connection) -> Result<usize> {
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS temp.idx_seen ON seen(folder_id, disk_name COLLATE NOCASE);",
    )?;
    let gone = conn.execute(
        "UPDATE item SET deleted_at = ?1
          WHERE deleted_at IS NULL
            AND NOT EXISTS (SELECT 1 FROM temp.seen s
                             WHERE s.folder_id = item.folder_id
                               AND s.disk_name = item.disk_name COLLATE NOCASE)",
        params![now()],
    )?;
    conn.execute_batch("DROP TABLE IF EXISTS temp.seen;")?;
    Ok(gone)
}
