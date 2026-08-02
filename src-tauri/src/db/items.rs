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
    pub folder_id: i64,
    pub folder_rel: String,
    pub folder_title: String,
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
            "SELECT i.id, i.kind, i.uuid, i.disk_name, i.orig_name, i.folder_id,
                    f.rel_path, f.title, i.size_bytes, i.width, i.height, i.duration_ms,
                    i.codec, i.bitrate, i.captured_at, i.captured_src, i.added_at,
                    i.favorite, i.notes, i.hash, d.url
               FROM item i
               JOIN folder f ON f.id = i.folder_id
               LEFT JOIN download d ON d.id = i.download_id
              WHERE i.id = ?1 AND i.deleted_at IS NULL",
            params![id],
            |r| {
                Ok(ItemDetail {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    path: String::new(),
                    thumb: crate::fs::paths::shard(&r.get::<_, String>(2)?),
                    disk_name: r.get(3)?,
                    orig_name: r.get(4)?,
                    folder_id: r.get(5)?,
                    folder_rel: r.get(6)?,
                    folder_title: r.get(7)?,
                    size_bytes: r.get(8)?,
                    width: r.get(9)?,
                    height: r.get(10)?,
                    duration_ms: r.get(11)?,
                    codec: r.get(12)?,
                    bitrate: r.get(13)?,
                    captured_at: r.get(14)?,
                    captured_src: r.get(15)?,
                    added_at: r.get(16)?,
                    favorite: r.get::<_, i64>(17)? != 0,
                    notes: r.get(18)?,
                    hash: r.get(19)?,
                    source_url: r.get(20)?,
                })
            },
        )
        .optional()?)
}

/// Favourite is first-class, not a tag (PLAN.md decision 12), and binary.
/// Takes the whole selection so one keystroke or one menu click covers it.
pub fn set_favorite(conn: &Connection, ids: &[i64], favorite: bool) -> Result<()> {
    let mut stmt = conn.prepare("UPDATE item SET favorite = ?1 WHERE id = ?2")?;
    for &id in ids {
        stmt.execute(params![i64::from(favorite), id])?;
    }
    Ok(())
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
/// and sprites, and re-issuing it would orphan them. `orig_name` is likewise
/// left untouched on refresh: it is the pre-import filename, recorded once
/// and never revised just because the file's content changed under the same
/// name. Only the insert branch below sets it.
pub fn upsert(conn: &Connection, item: &NewItem) -> Result<i64> {
    if let Some(found) = existing(conn, item.folder_id, &item.disk_name)? {
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

/// `(already renamed, still needs renaming)`. A row is "renamed" exactly when
/// `disk_name` already equals `<uuid>.<ext>` — the same equality the schema
/// comment in `001_initial.sql` describes as what M1.5 establishes for good,
/// so there is no separate flag to track it.
pub fn rename_counts(conn: &Connection) -> Result<(i64, i64)> {
    let already: i64 = conn.query_row(
        "SELECT COUNT(*) FROM item
          WHERE deleted_at IS NULL AND disk_name = uuid || '.' || ext",
        [],
        |r| r.get(0),
    )?;
    let total = count(conn)?;
    Ok((already, total - already))
}

/// One file the import wizard still needs to rename.
#[derive(Debug, Clone)]
pub struct RenameCandidate {
    pub id: i64,
    pub uuid: String,
    pub ext: String,
    pub orig_name: Option<String>,
    pub folder_rel: String,
    pub disk_name: String,
}

/// The next batch of not-yet-renamed items after `after_id`, in id order.
///
/// `disk_name != uuid || '.' || ext` is not indexed, so filtering on it alone
/// would rescan the whole table on every batch — quadratic over a 300GB
/// library. Pairing it with `id > ?` lets SQLite walk the primary key forward
/// from where the last batch stopped, so the filter only ever looks at rows
/// this call has not already passed over: one forward pass across the whole
/// import, not one per batch. See PLAN.md decision 19.
pub fn rename_candidates_after(
    conn: &Connection,
    after_id: i64,
    limit: i64,
) -> Result<Vec<RenameCandidate>> {
    let mut stmt = conn.prepare(
        "SELECT i.id, i.uuid, i.ext, i.orig_name, f.rel_path, i.disk_name
           FROM item i JOIN folder f ON f.id = i.folder_id
          WHERE i.deleted_at IS NULL AND i.id > ?1 AND i.disk_name != (i.uuid || '.' || i.ext)
          ORDER BY i.id
          LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![after_id, limit], |r| {
            Ok(RenameCandidate {
                id: r.get(0)?,
                uuid: r.get(1)?,
                ext: r.get(2)?,
                orig_name: r.get(3)?,
                folder_rel: r.get(4)?,
                disk_name: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// One item, by id — what `fs::import::rename_on_arrival` needs to rename a
/// single freshly-indexed file. Same shape as `rename_candidates_after`, just
/// not filtered to "still needs renaming": the caller already knows this item
/// is new.
pub fn rename_target(conn: &Connection, id: i64) -> Result<Option<RenameCandidate>> {
    Ok(conn
        .query_row(
            "SELECT i.id, i.uuid, i.ext, i.orig_name, f.rel_path, i.disk_name
               FROM item i JOIN folder f ON f.id = i.folder_id
              WHERE i.id = ?1",
            params![id],
            |r| {
                Ok(RenameCandidate {
                    id: r.get(0)?,
                    uuid: r.get(1)?,
                    ext: r.get(2)?,
                    orig_name: r.get(3)?,
                    folder_rel: r.get(4)?,
                    disk_name: r.get(5)?,
                })
            },
        )
        .optional()?)
}

/// Record the file's new on-disk name. Nothing else about the row changes —
/// `orig_name` keeps the pre-import filename as searchable metadata forever.
pub fn set_disk_name(conn: &Connection, id: i64, disk_name: &str) -> Result<()> {
    conn.execute(
        "UPDATE item SET disk_name = ?1 WHERE id = ?2",
        params![disk_name, id],
    )?;
    Ok(())
}

pub fn folder_id_of(conn: &Connection, id: i64) -> Result<Option<i64>> {
    Ok(conn
        .query_row(
            "SELECT folder_id FROM item WHERE id = ?1 AND deleted_at IS NULL",
            params![id],
            |r| r.get(0),
        )
        .optional()?)
}

/// The move operation's DB half — `fs::relocate::move_items` has already
/// moved the file on disk by the time this runs.
pub fn set_folder(conn: &Connection, id: i64, folder_id: i64) -> Result<()> {
    conn.execute(
        "UPDATE item SET folder_id = ?1 WHERE id = ?2",
        params![folder_id, id],
    )?;
    Ok(())
}

/// The delete operation's DB half — reuses the same soft-delete column the
/// watcher already uses for a file that vanished from disk. `fs::trash`
/// has already moved the file into `.gallery/trash/` by the time this runs.
pub fn trash_one(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE item SET deleted_at = ?1 WHERE id = ?2",
        params![now(), id],
    )?;
    Ok(())
}

/// Undo's half of `trash_one` — `fs::undo` has already moved the file back
/// out of `.gallery/trash/` by the time this runs.
pub fn restore_one(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("UPDATE item SET deleted_at = NULL WHERE id = ?1", params![id])?;
    Ok(())
}

/// What the import wizard's verify step re-hashes.
#[derive(Debug, Clone)]
pub struct VerifyCandidate {
    pub id: i64,
    pub folder_rel: String,
    pub disk_name: String,
    pub hash: String,
}

/// A random sample of already-renamed items. Called once, after the whole
/// rename has finished, so the `ORDER BY RANDOM()` full-table sort is a
/// one-off cost rather than a repeated query path — unlike the grid or search,
/// this is not a shape PLAN.md decision 19 is warning about.
pub fn random_sample_for_verify(conn: &Connection, n: i64) -> Result<Vec<VerifyCandidate>> {
    let mut stmt = conn.prepare(
        "SELECT i.id, f.rel_path, i.disk_name, i.hash
           FROM item i JOIN folder f ON f.id = i.folder_id
          WHERE i.deleted_at IS NULL AND i.disk_name = (i.uuid || '.' || i.ext)
          ORDER BY RANDOM()
          LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![n], |r| {
            Ok(VerifyCandidate {
                id: r.get(0)?,
                folder_rel: r.get(1)?,
                disk_name: r.get(2)?,
                hash: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn list(conn: &Connection, scope: &Scope) -> Result<Vec<GridItem>> {
    // `None` and `Some("")` are different questions, and both are asked:
    // *Everything* ignores folder structure entirely, while *Loose items* is
    // the root folder and nothing beneath it. See docs/DESIGN.md §2
    // "Navigation roots" — the library root is not a folder in the interface,
    // but items at the top level still have to belong somewhere.
    let whole_library = match scope.folder.as_deref() {
        None => true,
        Some("") => scope.recursive,
        Some(_) => false,
    };
    let folder = if whole_library {
        ""
    } else {
        scope.folder.as_deref().unwrap_or("")
    };
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
    let rows: Vec<GridItem> = if whole_library {
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

// --- watcher-driven single-path retirement --------------------------------
//
// The sweep above is for a whole-library walk. The watcher retires one path
// at a time, as soon as it sees the file or folder disappear, rather than
// waiting for a reconcile pass to notice.

/// Soft-delete one item by its folder and disk name. A no-op if nothing
/// matches — the file may never have been indexed, or this may be the tail
/// half of a rename the app itself suppressed.
pub fn retire_one(conn: &Connection, folder_id: i64, disk_name: &str) -> Result<bool> {
    let n = conn.execute(
        "UPDATE item SET deleted_at = ?1
          WHERE folder_id = ?2 AND disk_name = ?3 COLLATE NOCASE AND deleted_at IS NULL",
        params![now(), folder_id, disk_name],
    )?;
    Ok(n > 0)
}

/// Soft-delete every item in `folder_rel` and everything beneath it — the
/// watcher's response to a whole folder disappearing at once. Folders
/// themselves have no lifecycle yet (that is M2's job), so the row is left
/// behind, empty, rather than removed.
pub fn retire_folder(conn: &Connection, folder_rel: &str) -> Result<usize> {
    Ok(conn.execute(
        "UPDATE item SET deleted_at = ?1
          WHERE deleted_at IS NULL
            AND folder_id IN (SELECT id FROM folder
                               WHERE rel_path = ?2 OR rel_path LIKE ?2 || '/%')",
        params![now(), folder_rel],
    )?)
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
