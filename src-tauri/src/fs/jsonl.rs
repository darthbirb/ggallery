//! The standing `library.jsonl` writer. decision 30 makes this "the
//! only other complete copy of the organisation" — `fs::shard::write_manifest`
//! writes the one-shot version that precedes the physical migration; this is
//! what keeps the same file current for as long as the library stays open
//! afterward, not just accurate at migration time.
//!
//! **Debounced by piggybacking on the job queue's own idle state**, rather
//! than a dirty flag threaded through every mutating command — folder edits,
//! item filing, tag changes are dozens of call sites across `db::folders`,
//! `db::items` and `db::tags`, and none of them share a call path a flag
//! could be set from without touching every one. A rewrite only runs while
//! the queue is idle, on a fixed tick, which keeps it from contending with
//! active indexing and bounds staleness to roughly one tick — acceptable for
//! a disaster-recovery file that only ever needs to be *close*, not a
//! change-log.

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db;
use crate::error::Result;
use crate::fs::paths::LibraryPaths;
use crate::fs::shard::{folder_own_tags_display, item_tags_display};
use crate::jobs::QueueInner;

/// How often the writer checks whether it's safe to rewrite. Long enough
/// that a 100k-item rewrite (a few queries plus one sequential file write)
/// is a rounding error next to it; short enough that the file is never far
/// behind reality once things settle.
const TICK: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderRecord {
    pub id: i64,
    pub title: String,
    pub parent_id: Option<i64>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemRecord {
    pub uuid: String,
    pub ext: String,
    /// `None` — the Sorting Box — same as the database.
    pub folder_id: Option<i64>,
    pub orig_name: Option<String>,
    pub hash: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Line {
    Folder(FolderRecord),
    Item(ItemRecord),
}

/// Write the complete, current `library.jsonl` in one shot — every live
/// folder, every live item, fsynced — then atomically swap it into place.
/// The same record shapes `fs::shard::write_manifest` writes for the
/// migration, minus the migration-only path fields, which stop meaning
/// anything once nothing is mid-move.
pub fn write_full(paths: &LibraryPaths, conn: &Connection) -> Result<()> {
    let tmp = paths.jsonl_path().with_extension("jsonl.tmp");
    {
        let mut file = std::fs::File::create(&tmp)?;
        write_folders(conn, &mut file)?;
        write_items(conn, &mut file)?;
        file.flush()?;
        file.sync_all()?;
    }
    // Atomic swap — a reader (or a crash) never sees a half-written file.
    std::fs::rename(&tmp, paths.jsonl_path())?;
    Ok(())
}

fn write_folders(conn: &Connection, file: &mut std::fs::File) -> Result<()> {
    let mut stmt = conn.prepare("SELECT id, title, parent_id FROM folder WHERE deleted_at IS NULL ORDER BY id")?;
    let folders: Vec<(i64, String, Option<i64>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    drop(stmt);
    for (id, title, parent_id) in folders {
        let tags = folder_own_tags_display(conn, id)?;
        let line = Line::Folder(FolderRecord { id, title, parent_id, tags });
        writeln!(file, "{}", serde_json::to_string(&line)?)?;
    }
    Ok(())
}

fn write_items(conn: &Connection, file: &mut std::fs::File) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT id, uuid, ext, folder_id, orig_name, hash FROM item WHERE deleted_at IS NULL ORDER BY id",
    )?;
    let items: Vec<(i64, String, String, Option<i64>, Option<String>, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
        .collect::<rusqlite::Result<_>>()?;
    drop(stmt);
    for (id, uuid, ext, folder_id, orig_name, hash) in items {
        let tags = item_tags_display(conn, id)?;
        let line = Line::Item(ItemRecord { uuid, ext, folder_id, orig_name, hash, tags });
        writeln!(file, "{}", serde_json::to_string(&line)?)?;
    }
    Ok(())
}

/// Owns the background thread. Stops the moment this is dropped or `stop` is
/// called explicitly, same shape as `fs::watch::Watch`.
pub struct StandingWriter {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl StandingWriter {
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for StandingWriter {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Start the standing writer. Opens its own database connection — nothing
/// here runs on the Tauri command thread, consistent with every other
/// long-lived worker in the app.
pub fn start(paths: LibraryPaths, db_path: PathBuf, queue: Arc<QueueInner>) -> StandingWriter {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_loop = Arc::clone(&stop);

    let thread = std::thread::spawn(move || {
        let conn = match db::open(&db_path) {
            Ok(conn) => conn,
            Err(err) => {
                eprintln!("standing jsonl writer could not open the database: {err}");
                return;
            }
        };

        while !stop_loop.load(Ordering::Relaxed) {
            std::thread::sleep(TICK);
            if stop_loop.load(Ordering::Relaxed) {
                break;
            }
            let Ok(progress) = queue.progress(&conn) else { continue };
            if progress.phase != "idle" {
                continue; // never contend with active indexing
            }
            if let Err(err) = write_full(&paths, &conn) {
                eprintln!("standing jsonl writer failed: {err}");
            }
        }
    });

    StandingWriter { stop, thread: Some(thread) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-libraries")
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create scratch library");
        root
    }

    fn open_db(root: &std::path::Path) -> (LibraryPaths, Connection) {
        let paths = LibraryPaths::new(root);
        paths.ensure_dirs().unwrap();
        let mut conn = db::open(&paths.db_path()).unwrap();
        db::migrate(&mut conn).unwrap();
        (paths, conn)
    }

    #[test]
    fn write_full_records_every_live_folder_and_item() {
        let root = scratch("jsonl-write-full");
        let (paths, conn) = open_db(&root);
        let people = db::folders::create_record(&conn, None, "people").unwrap();
        let ana = db::folders::create_record(&conn, Some(people), "ana").unwrap();
        let item_id = db::items::upsert(
            &conn,
            &db::items::NewItem {
                uuid: "a3f2c1d4-e29b-41d4-a716-446655440000".to_string(),
                folder_id: Some(ana),
                disk_name: "a3f2c1d4-e29b-41d4-a716-446655440000.jpg".to_string(),
                ext: "jpg".to_string(),
                orig_name: "holiday.jpg".to_string(),
                hash: "deadbeef".to_string(),
                size_bytes: 5,
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
        db::tags::rebuild_item(&conn, item_id).unwrap();

        write_full(&paths, &conn).unwrap();

        let text = std::fs::read_to_string(paths.jsonl_path()).unwrap();
        let lines: Vec<Line> = text.lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str(l).unwrap()).collect();

        let folders: Vec<_> = lines.iter().filter_map(|l| match l { Line::Folder(f) => Some(f), _ => None }).collect();
        assert!(folders.iter().any(|f| f.id == people && f.title == "people"));
        assert!(folders.iter().any(|f| f.id == ana && f.parent_id == Some(people)));

        let items: Vec<_> = lines.iter().filter_map(|l| match l { Line::Item(i) => Some(i), _ => None }).collect();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].orig_name.as_deref(), Some("holiday.jpg"));
        assert!(items[0].tags.contains(&"ana".to_string()));
    }

    #[test]
    fn write_full_excludes_trashed_folders_and_items() {
        let root = scratch("jsonl-write-full-trashed");
        let (paths, conn) = open_db(&root);
        let ana = db::folders::create_record(&conn, None, "ana").unwrap();
        db::folders::trash_subtree(&conn, ana).unwrap();

        write_full(&paths, &conn).unwrap();

        let text = std::fs::read_to_string(paths.jsonl_path()).unwrap();
        assert!(text.trim().is_empty(), "the trashed folder is not written");
    }

    #[test]
    fn a_second_write_atomically_replaces_the_first() {
        let root = scratch("jsonl-write-full-replace");
        let (paths, conn) = open_db(&root);
        db::folders::create_record(&conn, None, "first").unwrap();
        write_full(&paths, &conn).unwrap();
        let first_len = std::fs::metadata(paths.jsonl_path()).unwrap().len();

        db::folders::create_record(&conn, None, "second").unwrap();
        write_full(&paths, &conn).unwrap();
        let second_len = std::fs::metadata(paths.jsonl_path()).unwrap().len();

        assert!(second_len > first_len);
        assert!(!paths.jsonl_path().with_extension("jsonl.tmp").exists(), "temp file cleaned up by the rename");
    }
}
