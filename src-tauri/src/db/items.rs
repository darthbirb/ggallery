use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::db::now;
use crate::error::Result;

/// What the indexer knows about a file before anything has been opened.
/// `folder_id` is `None` for everything — every item is indexed into the
/// Sorting Box first; filing it is a separate, later operation (DECISIONS.md
/// decision 30, "`NULL` is the Sorting Box").
#[derive(Debug, Clone)]
pub struct NewItem {
    pub uuid: String,
    pub folder_id: Option<i64>,
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

/// `id`, `uuid` and `ext` are all a caller needs to resolve an item's file —
/// its location is a pure function of those two (decision 30), so
/// nothing here ever needs to know which folder the item is filed in.
#[derive(Debug, Clone)]
pub struct ItemLocation {
    pub id: i64,
    pub uuid: String,
    pub ext: String,
}

/// Every live item's location directly inside `folder_id` and everywhere
/// beneath it — what `fs::trash::trash_folder` needs to physically move each
/// one's file, gathered *before* the folder's own soft-delete so there is
/// still a live subtree to walk.
pub fn locations_in_subtree(conn: &Connection, folder_id: i64) -> Result<Vec<ItemLocation>> {
    let mut stmt = conn.prepare(
        "WITH RECURSIVE subtree(id) AS (
             SELECT ?1
           UNION ALL
             SELECT f.id FROM folder f JOIN subtree s ON f.parent_id = s.id
             WHERE f.deleted_at IS NULL
         )
         SELECT id, uuid, ext FROM item
          WHERE deleted_at IS NULL AND folder_id IN (SELECT id FROM subtree)",
    )?;
    let rows = stmt
        .query_map(params![folder_id], |r| Ok(ItemLocation { id: r.get(0)?, uuid: r.get(1)?, ext: r.get(2)? }))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn location(conn: &Connection, id: i64) -> Result<Option<ItemLocation>> {
    Ok(conn
        .query_row(
            "SELECT id, uuid, ext FROM item WHERE id = ?1",
            params![id],
            |r| Ok(ItemLocation { id: r.get(0)?, uuid: r.get(1)?, ext: r.get(2)? }),
        )
        .optional()?)
}

/// Everything a thumbnail or sprite job needs to find the file and name its
/// output.
#[derive(Debug, Clone)]
pub struct ItemFile {
    pub id: i64,
    pub uuid: String,
    pub ext: String,
    pub kind: String,
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

/// Everything the pane's Preview mode shows for one item: the collapsed line
/// (name, dimensions, size) and everything the expanded details add. Wider
/// than `GridItem` on purpose — this is fetched one row at a time, not 100k.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemDetail {
    pub id: i64,
    pub kind: String,
    /// Absolute path to the file itself, for the webview to load. Filled in by
    /// the command from `fs::paths` — the database never stores one, and this
    /// struct leaves it empty until then.
    pub path: String,
    /// Cache-relative thumbnail path, so the preview can show something
    /// immediately while the original decodes.
    pub thumb: String,
    pub disk_name: String,
    pub orig_name: Option<String>,
    /// `None` for an item in the Sorting Box.
    pub folder_id: Option<i64>,
    /// Root-first ancestry, empty for the Sorting Box. Left empty by
    /// `db::items::detail` and filled in by the command layer via
    /// `db::folders::breadcrumb` — a separate query, not worth folding into
    /// this one for something fetched a row at a time.
    pub folder_breadcrumb: Vec<crate::db::folders::BreadcrumbCrumb>,
    pub size_bytes: i64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<i64>,
    pub codec: Option<String>,
    pub bitrate: Option<i64>,
    pub captured_at: Option<i64>,
    /// Where `captured_at` came from — EXIF, container metadata, or file
    /// mtime. Shown so a guess is never mistaken for metadata.
    pub captured_src: Option<String>,
    pub added_at: i64,
    pub favorite: bool,
    pub notes: Option<String>,
    pub hash: String,
    /// M5 fills this in; always `None` until downloads exist.
    pub source_url: Option<String>,
}

pub fn detail(conn: &Connection, id: i64) -> Result<Option<ItemDetail>> {
    Ok(conn
        .query_row(
            "SELECT i.id, i.kind, i.uuid, i.ext, i.disk_name, i.orig_name, i.folder_id,
                    i.size_bytes, i.width, i.height, i.duration_ms,
                    i.codec, i.bitrate, i.captured_at, i.captured_src, i.added_at,
                    i.favorite, i.notes, i.hash, d.url
               FROM item i
               LEFT JOIN download d ON d.id = i.download_id
              WHERE i.id = ?1 AND i.deleted_at IS NULL",
            params![id],
            |r| {
                Ok(ItemDetail {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    path: String::new(),
                    thumb: crate::fs::paths::shard(&r.get::<_, String>(2)?),
                    disk_name: r.get(4)?,
                    orig_name: r.get(5)?,
                    folder_id: r.get(6)?,
                    folder_breadcrumb: Vec::new(),
                    size_bytes: r.get(7)?,
                    width: r.get(8)?,
                    height: r.get(9)?,
                    duration_ms: r.get(10)?,
                    codec: r.get(11)?,
                    bitrate: r.get(12)?,
                    captured_at: r.get(13)?,
                    captured_src: r.get(14)?,
                    added_at: r.get(15)?,
                    favorite: r.get::<_, i64>(16)? != 0,
                    notes: r.get(17)?,
                    hash: r.get(18)?,
                    source_url: r.get(19)?,
                })
            },
        )
        .optional()?)
}

/// Favourite is first-class, not a tag (decision 12), and binary.
/// Takes the whole selection so one keystroke or one menu click covers it.
pub fn set_favorite(conn: &Connection, ids: &[i64], favorite: bool) -> Result<()> {
    let mut stmt = conn.prepare("UPDATE item SET favorite = ?1 WHERE id = ?2")?;
    for &id in ids {
        stmt.execute(params![i64::from(favorite), id])?;
    }
    Ok(())
}

/// What the grid asks for. "Root is a folder" is gone (decision 30) —
/// there are three real states, not a folder id plus a magic empty string.
#[derive(Debug, Clone, Default)]
pub enum Scope {
    /// Every item, ignoring folder structure entirely.
    #[default]
    Everything,
    /// The Sorting Box: `folder_id IS NULL`. Flat by definition — "not
    /// everything recursively; just what has not been filed yet"
    /// (SPEC.md §2 "Navigation roots").
    Unsorted,
    Folder { id: i64, recursive: bool },
}

/// Globally unique now (`idx_item_disk`), not per-folder — a file's name is
/// its shard location, which no folder can any longer disambiguate.
pub fn existing_by_disk_name(conn: &Connection, disk_name: &str) -> Result<Option<ExistingItem>> {
    Ok(conn
        .query_row(
            "SELECT id, uuid, size_bytes, mtime, deleted_at
               FROM item
              WHERE disk_name = ?1 COLLATE NOCASE",
            params![disk_name],
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
/// and sprites, and re-issuing it would orphan them. `orig_name` is likewise
/// left untouched on refresh: it is the pre-import filename, recorded once
/// and never revised just because the file's content changed under the same
/// name. `folder_id` is deliberately not touched on refresh either — a
/// modified file is the same item wherever it was already filed. Only the
/// insert branch below sets any of these.
pub fn upsert(conn: &Connection, item: &NewItem) -> Result<i64> {
    if let Some(found) = existing_by_disk_name(conn, &item.disk_name)? {
        conn.execute(
            "UPDATE item
                SET ext = ?1, hash = ?2, size_bytes = ?3, mtime = ?4,
                    kind = ?5, width = ?6, height = ?7, duration_ms = ?8, codec = ?9,
                    bitrate = ?10, captured_at = ?11, captured_src = ?12, deleted_at = NULL
              WHERE id = ?13",
            params![
                item.ext,
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
            "SELECT id, uuid, ext, kind, duration_ms, width IS NOT NULL AND height IS NOT NULL
               FROM item WHERE id = ?1",
            params![id],
            |r| {
                Ok(ItemFile {
                    id: r.get(0)?,
                    uuid: r.get(1)?,
                    ext: r.get(2)?,
                    kind: r.get(3)?,
                    duration_ms: r.get(4)?,
                    has_dimensions: r.get::<_, i64>(5)? != 0,
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

/// Items in the Sorting Box — `folder_id IS NULL` (decision 30).
/// There is no folder row to read a count off any more, so the sidebar badge
/// needs its own small query rather than a folder's `direct_count`.
pub fn unsorted_count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM item WHERE deleted_at IS NULL AND folder_id IS NULL",
        [],
        |r| r.get(0),
    )?)
}

/// Every item, trashed or not — what `fs::shard`'s migration progress
/// denominator counts against, since a trashed item's file still needs
/// moving to its shard location too.
pub fn count_all_including_deleted(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM item", [], |r| r.get(0))?)
}

/// Item counts and total size, grouped by kind. What the import wizard's scan
/// step leads with.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KindTotal {
    pub kind: String,
    pub count: i64,
    pub bytes: i64,
}

pub fn counts_by_kind(conn: &Connection) -> Result<Vec<KindTotal>> {
    let mut stmt = conn.prepare(
        "SELECT kind, COUNT(*), COALESCE(SUM(size_bytes), 0)
           FROM item
          WHERE deleted_at IS NULL
          GROUP BY kind",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(KindTotal {
                kind: r.get(0)?,
                count: r.get(1)?,
                bytes: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// The item's current folder — `None` (outer) if no such live item exists,
/// `Some(None)` if it exists but is unfiled (the Sorting Box). Two levels of
/// `Option` because `folder_id` itself is nullable now (decision 30):
/// this is the one place that distinction actually matters to a caller, so
/// it is spelled out rather than collapsed.
pub fn folder_id_of(conn: &Connection, id: i64) -> Result<Option<Option<i64>>> {
    Ok(conn
        .query_row(
            "SELECT folder_id FROM item WHERE id = ?1 AND deleted_at IS NULL",
            params![id],
            |r| r.get(0),
        )
        .optional()?)
}

/// The move operation's DB half — a pure column write. A file's location
/// never depended on its folder to begin with (decision 30), so
/// there is nothing else for a move to do.
pub fn set_folder(conn: &Connection, id: i64, folder_id: Option<i64>) -> Result<()> {
    conn.execute(
        "UPDATE item SET folder_id = ?1 WHERE id = ?2",
        params![folder_id, id],
    )?;
    Ok(())
}

/// The delete operation's DB half — reuses the same soft-delete column the
/// reconcile sweep already uses for a file that vanished from disk.
/// `fs::trash` has already moved the file into `.ggallery/trash/` by the time
/// this runs.
pub fn trash_one(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE item SET deleted_at = ?1 WHERE id = ?2",
        params![now(), id],
    )?;
    Ok(())
}

/// Undo's half of `trash_one` — `fs::undo` has already moved the file back
/// out of `.ggallery/trash/` by the time this runs.
pub fn restore_one(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("UPDATE item SET deleted_at = NULL WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn list(conn: &Connection, scope: &Scope) -> Result<Vec<GridItem>> {
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

    let rows: Vec<GridItem> = match scope {
        Scope::Everything => {
            let sql = format!("{base}{order}");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([], map)?.collect::<rusqlite::Result<_>>()?;
            rows
        }
        Scope::Unsorted => {
            let sql = format!("{base} AND folder_id IS NULL{order}");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([], map)?.collect::<rusqlite::Result<_>>()?;
            rows
        }
        Scope::Folder { id, recursive: false } => {
            let sql = format!("{base} AND folder_id = ?1{order}");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params![id], map)?.collect::<rusqlite::Result<_>>()?;
            rows
        }
        Scope::Folder { id, recursive: true } => {
            // Folder counts stay in the thousands at most (see
            // `db::folders::tree`'s own reasoning), so a recursive CTE over
            // `folder` alone — joined against, never iterated per item — is
            // not the shape decision 20 warns about. Verified
            // against `synth_library` at scale regardless.
            let sql = format!(
                "WITH RECURSIVE subtree(id) AS (
                     SELECT ?1
                   UNION ALL
                     SELECT f.id FROM folder f JOIN subtree s ON f.parent_id = s.id
                     WHERE f.deleted_at IS NULL
                 )
                 {base} AND folder_id IN (SELECT id FROM subtree){order}"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params![id], map)?.collect::<rusqlite::Result<_>>()?;
            rows
        }
    };

    Ok(rows)
}

// --- reconciliation sweep -------------------------------------------------
//
// `fs::walk`'s startup reconcile records the uuid of every item whose shard
// file it actually found, then soft-deletes the rows it did not — bookkeeping
// only, nothing on disk is touched, and a file that comes back clears its own
// `deleted_at` through `upsert`. Keyed by uuid (decision 30), not by
// a folder and a filename — there is no directory tree left to walk, only
// `files/` itself, sharded by uuid.

pub fn begin_sweep(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "DROP TABLE IF EXISTS temp.seen;
         CREATE TEMP TABLE seen (uuid TEXT NOT NULL);",
    )?;
    Ok(())
}

pub fn mark_seen(conn: &Connection, uuid: &str) -> Result<()> {
    conn.execute("INSERT INTO temp.seen (uuid) VALUES (?1)", params![uuid])?;
    Ok(())
}

pub fn finish_sweep(conn: &Connection) -> Result<usize> {
    conn.execute_batch("CREATE INDEX IF NOT EXISTS temp.idx_seen ON seen(uuid);")?;
    let gone = conn.execute(
        "UPDATE item SET deleted_at = ?1
          WHERE deleted_at IS NULL
            AND NOT EXISTS (SELECT 1 FROM temp.seen s WHERE s.uuid = item.uuid)",
        params![now()],
    )?;
    conn.execute_batch("DROP TABLE IF EXISTS temp.seen;")?;
    Ok(gone)
}
