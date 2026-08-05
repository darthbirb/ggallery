//! `files/<xx>/<uuid>.<ext>` — resolving an item's location from its own
//! identity (PLAN.md decision 30), and the one-time migration that flattens
//! an existing directory-shaped library into it.
//!
//! **The migration is the single most dangerous operation this app performs**:
//! it physically touches every file in what will eventually be a real 300GB
//! library. The order below is not negotiable (see PLAN.md §M2.6):
//!
//! 1. [`plan`] reads the *pre-migration* schema (folders still have
//!    `rel_path`) and writes the complete `library.jsonl` manifest — every
//!    item's uuid, old path, new path, orig_name, hash and resolved tags, and
//!    every folder's id, title, parent and own tags — fsynced, before a
//!    single file moves. If everything else fails, this file plus the media
//!    is enough to rebuild the library by hand.
//! 2. [`dry_run`] reports counts, total bytes and anything unreadable or
//!    unexpectedly occupying a destination. Nothing is written.
//! 3. [`execute`] performs the moves — batched, and resumable: "done" is
//!    decided by what's actually on disk (the destination already exists),
//!    never by a flag trusted across a crash, exactly the property
//!    `fs::import::execute` already established for the M1.5 rename. Each
//!    move is a same-volume rename — atomic, no data copied — so an
//!    interrupted run leaves some files moved and the rest untouched, never a
//!    corrupt one.
//! 4. [`verify`] confirms every item resolves to a file that exists at its
//!    shard path.
//! 5. Old directories are left alone. [`count_empty_dirs`] reports how many
//!    are now empty; removing them is a separate, explicit action — never
//!    automatic.

use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::db;
use crate::error::Result;
use crate::fs::paths::LibraryPaths;
use crate::media::hash;

/// Items per transaction during `execute`. Small enough that a crash never
/// loses much progress; large enough that 300k files is thousands of
/// batches, not hundreds of thousands of fsyncs. Matches `fs::import`'s
/// batch size — the same reasoning applies to the same shape of operation.
const BATCH: i64 = 200;

// --- the pure resolver -----------------------------------------------------

/// `<uuid>` → its shard directory name, the first two hex characters. 256-way
/// sharding — a single directory holding 100k entries is slow to enumerate
/// and painful for every backup tool that walks it (PLAN.md's on-disk
/// layout). Deliberately one level, unlike the cache's two-level
/// `fs::paths::shard` — library content and cache entries are sized
/// differently and were sharded independently in the design.
fn shard_dir(uuid: &str) -> &str {
    let clean_len = uuid
        .as_bytes()
        .iter()
        .take_while(|b| b.is_ascii_hexdigit())
        .count();
    if clean_len >= 2 {
        &uuid[0..2]
    } else {
        "00"
    }
}

/// `<uuid>, <ext>` → `<xx>/<uuid>.<ext>`, relative to whichever root
/// (`files/` or `.ggallery/trash/`) the caller joins it against.
pub fn item_rel(uuid: &str, ext: &str) -> String {
    format!("{}/{uuid}.{ext}", shard_dir(uuid))
}

/// Move a freshly-hashed arrival's file from wherever it currently sits
/// (`inbox/`) into its permanent shard location. Unlike `move_one` below
/// (the bulk migration's idempotent move, which can be resumed), an
/// arrival's source is always real and its destination never already
/// exists — there is nothing to resume here, only a plain same-volume
/// rename. Used by `jobs::worker::run_hash` for every file that settles in
/// `inbox/`, whether the watcher found it live or a startup catch-up walk
/// found it waiting.
pub fn move_into_shard(paths: &LibraryPaths, src: &Path, uuid: &str, ext: &str) -> Result<PathBuf> {
    let dest = paths.item_path(uuid, ext);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(src, &dest)?;
    Ok(dest)
}

#[cfg(test)]
mod resolver_tests {
    use super::*;

    #[test]
    fn shards_by_first_two_hex_characters() {
        assert_eq!(
            item_rel("a3f2c1d4-e29b-41d4-a716-446655440000", "jpg"),
            "a3/a3f2c1d4-e29b-41d4-a716-446655440000.jpg"
        );
    }
}

// --- manifest (step 1: write before anything moves) -------------------------

/// One folder's worth of `library.jsonl` — the rebuild path, not a
/// convenience (PLAN.md decision 30). `tags` is every label (`key:value`)
/// and flag this folder itself carries — human-readable in a text editor,
/// which is the whole point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderManifestRecord {
    pub id: i64,
    pub title: String,
    pub parent_id: Option<i64>,
    pub tags: Vec<String>,
}

/// One item's worth of `library.jsonl`. `old_path`/`new_path` are relative to
/// the library root — kept even after the move completes, because this is
/// the disaster-recovery record of what the migration *did*, not just of
/// where things end up.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemManifestRecord {
    pub uuid: String,
    pub old_path: String,
    pub new_path: String,
    pub orig_name: Option<String>,
    pub hash: String,
    pub tags: Vec<String>,
}

/// One line of `library.jsonl`, folders and items interleaved. Tagged so a
/// single `serde_json::from_str::<ManifestLine>` round-trips either shape —
/// what a human reading the file, or `load_manifest` reconstructing from it,
/// both need.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ManifestLine {
    Folder(FolderManifestRecord),
    Item(ItemManifestRecord),
}

/// Everything about one item the migration needs, read from the
/// *pre-migration* schema — `folder.rel_path` still exists at this point,
/// which is the only reason this function can locate the file at all.
struct MigrationItem {
    id: i64,
    uuid: String,
    ext: String,
    old_rel: String,
    orig_name: Option<String>,
    hash: String,
    size_bytes: i64,
}

fn migration_items_after(conn: &Connection, after_id: i64, limit: i64) -> Result<Vec<MigrationItem>> {
    let mut stmt = conn.prepare(
        "SELECT i.id, i.uuid, i.ext, f.rel_path, i.disk_name, i.orig_name, i.hash, i.size_bytes
           FROM item i JOIN folder f ON f.id = i.folder_id
          WHERE i.id > ?1
          ORDER BY i.id
          LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![after_id, limit], |r| {
            let folder_rel: String = r.get(3)?;
            let disk_name: String = r.get(4)?;
            let old_rel = if folder_rel.is_empty() {
                disk_name
            } else {
                format!("{folder_rel}/{disk_name}")
            };
            Ok(MigrationItem {
                id: r.get(0)?,
                uuid: r.get(1)?,
                ext: r.get(2)?,
                old_rel,
                orig_name: r.get(5)?,
                hash: r.get(6)?,
                size_bytes: r.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub(crate) fn item_tags_display(conn: &Connection, item_id: i64) -> Result<Vec<String>> {
    let tags = db::tags::item_effective_tags(conn, item_id)?;
    Ok(tags
        .into_iter()
        .map(|t| match t.key {
            Some(key) => format!("{key}:{}", t.value),
            None => t.value,
        })
        .collect())
}

pub(crate) fn folder_own_tags_display(conn: &Connection, folder_id: i64) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.key, t.value FROM folder_tag ft JOIN tag t ON t.id = ft.tag_id
          WHERE ft.folder_id = ?1",
    )?;
    let rows: Vec<(Option<String>, String)> = stmt
        .query_map(params![folder_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows
        .into_iter()
        .map(|(key, value)| match key {
            Some(key) => format!("{key}:{value}"),
            None => value,
        })
        .collect())
}

/// Write the complete `library.jsonl` manifest — every folder, every item —
/// fsynced, before returning. Nothing else in this module is allowed to move
/// a file until this has completed.
pub fn write_manifest(paths: &LibraryPaths, conn: &Connection) -> Result<()> {
    if let Some(parent) = paths.jsonl_path().parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = File::create(paths.jsonl_path())?;

    let mut fstmt = conn.prepare("SELECT id, title, parent_id FROM folder WHERE deleted_at IS NULL ORDER BY id")?;
    let folders: Vec<(i64, String, Option<i64>)> = fstmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    drop(fstmt);
    for (id, title, parent_id) in folders {
        let tags = folder_own_tags_display(conn, id)?;
        let line = ManifestLine::Folder(FolderManifestRecord { id, title, parent_id, tags });
        writeln!(file, "{}", serde_json::to_string(&line)?)?;
    }

    let mut after_id = 0i64;
    loop {
        let batch = migration_items_after(conn, after_id, BATCH)?;
        if batch.is_empty() {
            break;
        }
        after_id = batch.last().expect("checked non-empty above").id;
        for item in &batch {
            let tags = item_tags_display(conn, item.id)?;
            let new_rel = format!("files/{}", item_rel(&item.uuid, &item.ext));
            let line = ManifestLine::Item(ItemManifestRecord {
                uuid: item.uuid.clone(),
                old_path: item.old_rel.clone(),
                new_path: new_rel,
                orig_name: item.orig_name.clone(),
                hash: item.hash.clone(),
                tags,
            });
            writeln!(file, "{}", serde_json::to_string(&line)?)?;
        }
    }

    file.flush()?;
    file.sync_all()?;
    Ok(())
}

// --- dry run -----------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Collision {
    pub uuid: String,
    pub old_path: String,
    pub new_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DryRunReport {
    pub total_items: i64,
    pub total_bytes: i64,
    /// Source file no longer readable — gone, or permission denied.
    pub unreadable: i64,
    /// Both the old and the new path exist at once — something the migration
    /// itself did not put there, since a uuid's destination is unique to it.
    /// Worth a human look before `execute` runs.
    pub collisions: Vec<Collision>,
    /// Already at its shard destination — nothing left to do for this one,
    /// e.g. a previous run got partway through.
    pub already_done: i64,
    pub to_move: i64,
}

pub fn dry_run(paths: &LibraryPaths, conn: &Connection) -> Result<DryRunReport> {
    let mut report = DryRunReport {
        total_items: 0,
        total_bytes: 0,
        unreadable: 0,
        collisions: Vec::new(),
        already_done: 0,
        to_move: 0,
    };

    let mut after_id = 0i64;
    loop {
        let batch = migration_items_after(conn, after_id, BATCH)?;
        if batch.is_empty() {
            break;
        }
        after_id = batch.last().expect("checked non-empty above").id;

        for item in &batch {
            report.total_items += 1;
            report.total_bytes += item.size_bytes;

            let old_abs = paths.root().join(&item.old_rel);
            let new_abs = paths.item_path(&item.uuid, &item.ext);

            match (old_abs.is_file(), new_abs.is_file()) {
                (true, true) => report.collisions.push(Collision {
                    uuid: item.uuid.clone(),
                    old_path: item.old_rel.clone(),
                    new_path: new_abs.strip_prefix(paths.root()).unwrap_or(&new_abs).to_string_lossy().replace('\\', "/"),
                }),
                (true, false) => report.to_move += 1,
                (false, true) => report.already_done += 1,
                (false, false) => report.unreadable += 1,
            }
        }
    }

    Ok(report)
}

// --- execute -----------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveError {
    pub item_id: i64,
    pub uuid: String,
    pub error: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteReport {
    pub moved: i64,
    pub already_done: i64,
    pub errors: Vec<MoveError>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationProgress {
    pub done: i64,
    pub total: i64,
    pub errors: i64,
}

enum MoveOutcome {
    Moved,
    AlreadyDone,
    Missing,
}

fn move_one(old_abs: &Path, new_abs: &Path) -> Result<MoveOutcome> {
    if new_abs.is_file() {
        return Ok(MoveOutcome::AlreadyDone);
    }
    if !old_abs.is_file() {
        return Ok(MoveOutcome::Missing);
    }
    if let Some(parent) = new_abs.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(old_abs, new_abs)?;
    Ok(MoveOutcome::Moved)
}

/// Move every item's file to its shard destination. Batched and resumable:
/// "done" is decided by what's on disk (`move_one`'s `AlreadyDone`), never a
/// flag trusted across a crash — a second run after an interruption only
/// ever touches what the first one didn't finish. `write_manifest` must have
/// already completed before this is ever called.
pub fn execute(
    paths: &LibraryPaths,
    conn: &Connection,
    on_progress: &mut dyn FnMut(&MigrationProgress),
) -> Result<ExecuteReport> {
    let mut report = ExecuteReport::default();
    let total = db::items::count_all_including_deleted(conn)?;

    let mut after_id = 0i64;
    loop {
        let batch = migration_items_after(conn, after_id, BATCH)?;
        if batch.is_empty() {
            break;
        }
        after_id = batch.last().expect("checked non-empty above").id;

        for item in &batch {
            let old_abs = paths.root().join(&item.old_rel);
            let new_abs = paths.item_path(&item.uuid, &item.ext);
            match move_one(&old_abs, &new_abs) {
                Ok(MoveOutcome::Moved) => report.moved += 1,
                Ok(MoveOutcome::AlreadyDone) => report.already_done += 1,
                Ok(MoveOutcome::Missing) => report.errors.push(MoveError {
                    item_id: item.id,
                    uuid: item.uuid.clone(),
                    error: "source file is no longer on disk".to_string(),
                }),
                Err(err) => report.errors.push(MoveError {
                    item_id: item.id,
                    uuid: item.uuid.clone(),
                    error: err.to_string(),
                }),
            }
        }

        on_progress(&MigrationProgress {
            done: report.moved + report.already_done,
            total,
            errors: report.errors.len() as i64,
        });
    }

    Ok(report)
}

// --- verify ------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyMissing {
    pub item_id: i64,
    pub uuid: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyReport {
    pub count_total: i64,
    pub count_at_destination: i64,
    pub missing: Vec<VerifyMissing>,
    /// Only populated when the caller opts into the full sweep — disk-bound
    /// over the whole library, so it is never run implicitly.
    pub hash_mismatches: Vec<VerifyMissing>,
}

pub fn verify(paths: &LibraryPaths, conn: &Connection, full_hash_sweep: bool) -> Result<VerifyReport> {
    let mut report = VerifyReport {
        count_total: 0,
        count_at_destination: 0,
        missing: Vec::new(),
        hash_mismatches: Vec::new(),
    };

    let mut after_id = 0i64;
    loop {
        let batch = migration_items_after(conn, after_id, BATCH)?;
        if batch.is_empty() {
            break;
        }
        after_id = batch.last().expect("checked non-empty above").id;

        for item in &batch {
            report.count_total += 1;
            let new_abs = paths.item_path(&item.uuid, &item.ext);
            if !new_abs.is_file() {
                report.missing.push(VerifyMissing { item_id: item.id, uuid: item.uuid.clone() });
                continue;
            }
            report.count_at_destination += 1;

            if full_hash_sweep {
                let actual = hash::blake3_file(&new_abs)?;
                if actual != item.hash {
                    report.hash_mismatches.push(VerifyMissing { item_id: item.id, uuid: item.uuid.clone() });
                }
            }
        }
    }

    Ok(report)
}

// --- old directories, left alone -------------------------------------------

/// Every directory under the root that is now empty, except `.ggallery/`,
/// `files/` and `inbox/` themselves. Report-only — nothing here deletes
/// anything. Walked deepest-first so a directory that is empty only because
/// its own now-empty children were already counted is still recognised
/// correctly by a caller that removes bottom-up.
pub fn count_empty_dirs(paths: &LibraryPaths) -> Result<i64> {
    let mut keep: HashSet<PathBuf> = HashSet::new();
    keep.insert(paths.ggallery_dir());
    keep.insert(paths.files_dir());
    keep.insert(paths.inbox_dir());

    // Walked deepest-first, tracking which directories would themselves be
    // removed — a parent whose only remaining entries are directories
    // already counted this way is empty too, once those are actually gone,
    // even though `read_dir` still lists them right now. Without this, a
    // two-level empty tree (`People/Ana`) would only ever count its
    // leaf, since `People` still literally contains one entry.
    let mut removable: HashSet<PathBuf> = HashSet::new();
    let mut count = 0i64;
    // No `filter_entry` here, deliberately: it is a pre-order mechanism
    // (skip descending by intercepting a directory *before* its contents are
    // visited), which does not compose with `contents_first`'s post-order
    // traversal — by the time a filtered directory would be yielded, walkdir
    // has already descended into it. `.ggallery`/`files`/`inbox` are excluded
    // inside the loop instead.
    let walker = walkdir::WalkDir::new(paths.root()).min_depth(1).contents_first(true);

    for entry in walker.into_iter().flatten() {
        if keep.iter().any(|k| entry.path() == k || entry.path().starts_with(k)) {
            continue;
        }
        if !entry.file_type().is_dir() {
            continue;
        }
        // Child paths are built the same way walkdir itself builds them —
        // `entry.path().join(name)` — rather than compared against
        // `std::fs::read_dir`'s own `DirEntry::path()`, which can represent
        // an identical file with a different (if logically equal) `PathBuf`
        // on Windows.
        let all_children_removable = std::fs::read_dir(entry.path())
            .map(|it| {
                it.flatten()
                    .all(|child| removable.contains(&entry.path().join(child.file_name())))
            })
            .unwrap_or(false);
        if all_children_removable {
            count += 1;
            removable.insert(entry.path().to_path_buf());
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    fn scratch(name: &str) -> PathBuf {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-libraries")
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create scratch library");
        root
    }

    /// Opens a raw, unmigrated (v7-shaped) database directly — `fs::shard`'s
    /// migration functions run against exactly this schema, *before*
    /// migration 008 (which they are the precondition for) ever applies.
    /// Mirrors `fs::lowercase_migration`'s own `open_v7_db`.
    fn open_db(root: &Path) -> (LibraryPaths, Connection) {
        let paths = LibraryPaths::new(root);
        paths.ensure_dirs().unwrap();
        let conn = db::open(&paths.db_path()).unwrap();
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

    fn raw_folder(conn: &Connection, parent_id: Option<i64>, rel_path: &str, title: &str) -> i64 {
        conn.execute(
            "INSERT INTO folder (rel_path, title, parent_id, created_at) VALUES (?1, ?2, ?3, 0)",
            params![rel_path, title, parent_id],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        db::tags::sync_title_tag(conn, id, title).unwrap();
        id
    }

    fn seed_item(conn: &Connection, folder_id: i64, uuid: &str, ext: &str, bytes: &[u8]) -> i64 {
        let hash = blake3::hash(bytes).to_hex().to_string();
        conn.execute(
            "INSERT INTO item (uuid, folder_id, disk_name, ext, orig_name, hash, size_bytes, mtime, kind, added_at)
             VALUES (?1, ?2, ?3, ?4, 'holiday.jpg', ?5, ?6, 0, 'image', 0)",
            params![uuid, folder_id, format!("{uuid}.{ext}"), ext, hash, bytes.len() as i64],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn write_manifest_records_every_folder_and_item_before_anything_moves() {
        let root = scratch("shard-manifest");
        std::fs::create_dir_all(root.join("People/Ana")).unwrap();
        std::fs::write(root.join("People/Ana/a3f2c1d4-e29b-41d4-a716-446655440000.jpg"), b"hello").unwrap();
        let (paths, conn) = open_db(&root);
        let people = raw_folder(&conn, None, "people", "people");
        let ana = raw_folder(&conn, Some(people), "people/ana", "ana");
        let item_id = seed_item(&conn, ana, "a3f2c1d4-e29b-41d4-a716-446655440000", "jpg", b"hello");
        // Raw SQL inserts bypass the normal indexing path, which is what
        // actually populates `item_effective_tag` — do it explicitly so the
        // manifest has real tags to record, the same as any indexed item
        // would.
        db::tags::rebuild_item(&conn, item_id).unwrap();

        write_manifest(&paths, &conn).unwrap();

        // Old file untouched — this step only ever writes the manifest.
        assert!(root.join("People/Ana/a3f2c1d4-e29b-41d4-a716-446655440000.jpg").is_file());

        let text = std::fs::read_to_string(paths.jsonl_path()).unwrap();
        let lines: Vec<ManifestLine> = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();

        let folders: Vec<_> = lines
            .iter()
            .filter_map(|l| match l {
                ManifestLine::Folder(f) => Some(f),
                _ => None,
            })
            .collect();
        assert!(folders.iter().any(|f| f.id == people && f.title == "people"));
        assert!(folders.iter().any(|f| f.id == ana && f.title == "ana"));

        let items: Vec<_> = lines
            .iter()
            .filter_map(|l| match l {
                ManifestLine::Item(i) => Some(i),
                _ => None,
            })
            .collect();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].old_path, "people/ana/a3f2c1d4-e29b-41d4-a716-446655440000.jpg");
        assert_eq!(items[0].new_path, "files/a3/a3f2c1d4-e29b-41d4-a716-446655440000.jpg");
        assert!(items[0].tags.contains(&"ana".to_string()));
    }

    #[test]
    fn dry_run_reports_counts_and_bytes_without_writing_anything() {
        let root = scratch("shard-dry-run");
        std::fs::write(root.join("a3f2c1d4-e29b-41d4-a716-446655440000.png"), b"pixels").unwrap();
        let (paths, conn) = open_db(&root);
        let root_id = raw_folder(&conn, None, "", "library");
        seed_item(&conn, root_id, "a3f2c1d4-e29b-41d4-a716-446655440000", "png", b"pixels");

        let report = dry_run(&paths, &conn).unwrap();
        assert_eq!(report.total_items, 1);
        assert_eq!(report.total_bytes, 6);
        assert_eq!(report.to_move, 1);
        assert_eq!(report.already_done, 0);
        assert!(report.collisions.is_empty());

        // Nothing written — `files_dir()` exists (`ensure_dirs` always
        // creates it), but nothing has been moved into it.
        assert!(root.join("a3f2c1d4-e29b-41d4-a716-446655440000.png").is_file());
        assert!(std::fs::read_dir(paths.files_dir()).unwrap().next().is_none());
    }

    #[test]
    fn execute_moves_files_into_their_shard_and_is_idempotent() {
        let root = scratch("shard-execute");
        std::fs::write(root.join("a3f2c1d4-e29b-41d4-a716-446655440000.png"), b"pixels").unwrap();
        let (paths, conn) = open_db(&root);
        let root_id = raw_folder(&conn, None, "", "library");
        seed_item(&conn, root_id, "a3f2c1d4-e29b-41d4-a716-446655440000", "png", b"pixels");
        write_manifest(&paths, &conn).unwrap();

        let report = execute(&paths, &conn, &mut |_| {}).unwrap();
        assert_eq!(report.moved, 1);
        assert!(report.errors.is_empty());
        assert!(paths.item_path("a3f2c1d4-e29b-41d4-a716-446655440000", "png").is_file());
        assert!(!root.join("a3f2c1d4-e29b-41d4-a716-446655440000.png").exists());

        // Running again is a no-op that still reports success.
        let second = execute(&paths, &conn, &mut |_| {}).unwrap();
        assert_eq!(second.moved, 0);
        assert_eq!(second.already_done, 1);
        assert!(second.errors.is_empty());
    }

    #[test]
    fn resumes_after_a_crash_mid_batch() {
        let root = scratch("shard-resume");
        std::fs::write(root.join("aaaaaaaa-0000-0000-0000-000000000000.png"), b"aaa").unwrap();
        std::fs::write(root.join("bbbbbbbb-0000-0000-0000-000000000000.png"), b"bbb").unwrap();
        let (paths, conn) = open_db(&root);
        let root_id = raw_folder(&conn, None, "", "library");
        seed_item(&conn, root_id, "aaaaaaaa-0000-0000-0000-000000000000", "png", b"aaa");
        seed_item(&conn, root_id, "bbbbbbbb-0000-0000-0000-000000000000", "png", b"bbb");
        write_manifest(&paths, &conn).unwrap();

        // Simulate the exact crash window: one file already at its shard
        // destination, the other still at its old path, with no bookkeeping
        // recording either fact anywhere but the filesystem itself.
        std::fs::create_dir_all(paths.item_path("aaaaaaaa-0000-0000-0000-000000000000", "png").parent().unwrap())
            .unwrap();
        std::fs::rename(
            root.join("aaaaaaaa-0000-0000-0000-000000000000.png"),
            paths.item_path("aaaaaaaa-0000-0000-0000-000000000000", "png"),
        )
        .unwrap();

        let report = execute(&paths, &conn, &mut |_| {}).unwrap();
        assert_eq!(report.moved, 1, "only the file not yet moved is actually renamed");
        assert_eq!(report.already_done, 1);
        assert!(report.errors.is_empty());

        assert!(paths.item_path("aaaaaaaa-0000-0000-0000-000000000000", "png").is_file());
        assert!(paths.item_path("bbbbbbbb-0000-0000-0000-000000000000", "png").is_file());
    }

    #[test]
    fn verify_confirms_every_item_resolves_after_execute() {
        let root = scratch("shard-verify");
        std::fs::write(root.join("a3f2c1d4-e29b-41d4-a716-446655440000.png"), b"pixels").unwrap();
        let (paths, conn) = open_db(&root);
        let root_id = raw_folder(&conn, None, "", "library");
        seed_item(&conn, root_id, "a3f2c1d4-e29b-41d4-a716-446655440000", "png", b"pixels");
        write_manifest(&paths, &conn).unwrap();
        execute(&paths, &conn, &mut |_| {}).unwrap();

        let report = verify(&paths, &conn, true).unwrap();
        assert_eq!(report.count_total, 1);
        assert_eq!(report.count_at_destination, 1);
        assert!(report.missing.is_empty());
        assert!(report.hash_mismatches.is_empty());
    }

    #[test]
    fn verify_reports_a_missing_file_the_move_could_not_find() {
        let root = scratch("shard-verify-missing");
        let (paths, conn) = open_db(&root);
        let root_id = raw_folder(&conn, None, "", "library");
        // Seeded in the database but never actually written to disk — the
        // pre-migration equivalent of a file that vanished after indexing.
        seed_item(&conn, root_id, "a3f2c1d4-e29b-41d4-a716-446655440000", "png", b"pixels");

        let report = verify(&paths, &conn, false).unwrap();
        assert_eq!(report.count_total, 1);
        assert_eq!(report.count_at_destination, 0);
        assert_eq!(report.missing.len(), 1);
    }

    #[test]
    fn old_directories_are_left_alone_and_counted_not_deleted() {
        let root = scratch("shard-old-dirs");
        std::fs::create_dir_all(root.join("People/Ana")).unwrap();
        std::fs::write(root.join("People/Ana/a3f2c1d4-e29b-41d4-a716-446655440000.png"), b"pixels").unwrap();
        let (paths, conn) = open_db(&root);
        let people = raw_folder(&conn, None, "people", "people");
        let ana = raw_folder(&conn, Some(people), "people/ana", "ana");
        seed_item(&conn, ana, "a3f2c1d4-e29b-41d4-a716-446655440000", "png", b"pixels");
        write_manifest(&paths, &conn).unwrap();
        execute(&paths, &conn, &mut |_| {}).unwrap();

        assert!(root.join("People/Ana").is_dir(), "nothing here deletes the old directory");
        let empty = count_empty_dirs(&paths).unwrap();
        assert_eq!(empty, 2, "People/Ana and People are both now empty");
    }

    /// PLAN.md decision 20: verified at scale before this ever runs against
    /// the real library. `cargo test --release shard_scale_100k -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn shard_scale_100k() {
        const N: i64 = 100_000;
        let root = scratch("shard-scale");
        let (paths, conn) = open_db(&root);
        let root_id = raw_folder(&conn, None, "", "library");

        let setup_start = std::time::Instant::now();
        db::begin_batch(&conn).unwrap();
        for i in 0..N {
            if i % 5000 == 0 && i > 0 {
                db::commit_batch(&conn).unwrap();
                db::begin_batch(&conn).unwrap();
            }
            let uuid = uuid::Uuid::new_v4().to_string();
            let bytes = format!("payload {i}").into_bytes();
            std::fs::write(root.join(format!("{uuid}.bin")), &bytes).unwrap();
            seed_item(&conn, root_id, &uuid, "bin", &bytes);
        }
        db::commit_batch(&conn).unwrap();
        println!("setup: {:?} for {N} items", setup_start.elapsed());

        let manifest_start = std::time::Instant::now();
        write_manifest(&paths, &conn).unwrap();
        println!("write_manifest: {:?}", manifest_start.elapsed());

        let dry_start = std::time::Instant::now();
        let dry = dry_run(&paths, &conn).unwrap();
        println!("dry_run: {:?}", dry_start.elapsed());
        assert_eq!(dry.to_move, N);

        let exec_start = std::time::Instant::now();
        let report = execute(&paths, &conn, &mut |_| {}).unwrap();
        println!("execute: {:?} for {N} items", exec_start.elapsed());
        assert_eq!(report.moved, N);
        assert!(report.errors.is_empty());

        let verify_start = std::time::Instant::now();
        let verified = verify(&paths, &conn, false).unwrap();
        println!("verify: {:?}", verify_start.elapsed());
        assert_eq!(verified.count_at_destination, N);
        assert!(verified.missing.is_empty());
    }
}
