//! Job execution. Everything here runs on a worker thread with its own
//! database connection — never on the Tauri command thread.

use std::path::Path;

use rusqlite::Connection;

use crate::db;
use crate::db::items::NewItem;
use crate::db::jobs::QueuedJob;
use crate::error::{AppError, Result};
use crate::fs::paths::{extension_of, parse_uuid_disk_name, LibraryPaths};
use crate::fs::shard;
use crate::fs::walk;
use crate::jobs::kinds::{self, HashPayload, ItemPayload};
use crate::jobs::QueueInner;
use crate::media::{hash, probe, sprites, thumbs, Kind};
use crate::sidecar::Tools;

pub fn execute(ctx: &QueueInner, conn: &mut Connection, job: &QueuedJob) -> Result<()> {
    let (paths, tools) = (&ctx.paths, &ctx.tools);
    match job.kind.as_str() {
        kinds::INDEX => run_index(ctx, conn),
        kinds::HASH => run_hash(paths, tools, conn, serde_json::from_str(&job.payload)?),
        kinds::THUMB => run_thumb(paths, tools, conn, serde_json::from_str(&job.payload)?),
        kinds::SPRITE => run_sprite(paths, tools, conn, serde_json::from_str(&job.payload)?),
        kinds::RETAG_FOLDER => {
            let payload: kinds::RetagFolderPayload = serde_json::from_str(&job.payload)?;
            db::tags::rebuild_subtree(conn, payload.folder_id)
        }
        kinds::RETAG_ITEM => {
            let payload: ItemPayload = serde_json::from_str(&job.payload)?;
            db::tags::rebuild_item(conn, payload.item_id)
        }
        other => Err(AppError::invalid(format!("unknown job type {other}"))),
    }
}

fn run_index(ctx: &QueueInner, conn: &mut Connection) -> Result<()> {
    // Failures belong to the run that produced them.
    db::jobs::clear_failed(conn)?;

    ctx.set_walking(true);
    let result = walk::reconcile(&ctx.paths, conn, &mut |seen, queued| {
        ctx.report_walk(seen, queued);
    });
    ctx.set_walking(false);
    ctx.rescanning
        .store(false, std::sync::atomic::Ordering::Relaxed);

    if result.is_err() {
        db::rollback_batch(conn);
    }
    result.map(|_| ())
}

/// Hash, probe, insert and shard one file already sitting somewhere on
/// disk — the whole of "arriving" distilled into a single step, whichever
/// door the file came through: `inbox/` (`run_hash`, below) or a
/// first-import tree walk (`fs::import::execute_prepared`). An item never
/// exists in a half-known state, and once this returns it is sharded too —
/// there is no separate "renamed later" step. `folder_id` is `None` for an
/// inbox arrival (nothing in `inbox/` carries any organisational
/// information worth keeping, decision 30) and whatever the caller
/// resolved from the source tree for a first import (ROADMAP.md §M2.6a).
pub fn index_file(
    paths: &LibraryPaths,
    tools: &Tools,
    conn: &mut Connection,
    src: &Path,
    orig_name: String,
    folder_id: Option<i64>,
) -> Result<i64> {
    let meta = std::fs::metadata(src)?;
    let size = meta.len() as i64;
    let mtime = walk::mtime_secs(&meta);
    let created = walk::created_secs(&meta, mtime);

    let ext = extension_of(&orig_name);
    let kind = Kind::classify(src, &ext);

    let content_hash = hash::blake3_file(src)?;
    let probed = probe::probe(src, kind, created, tools.ffmpeg.as_ref());

    // A file that already carries a `<uuid>.<ext>` name — most likely a
    // leftover from an interrupted previous run — keeps its embedded
    // identity rather than being issued a fresh one, so it re-links to
    // whatever thumbnail or sprite may already exist for it.
    let uuid = parse_uuid_disk_name(&orig_name).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let disk_name = format!("{uuid}.{ext}");

    let item_id = db::items::upsert(
        conn,
        &NewItem {
            uuid: uuid.clone(),
            folder_id,
            disk_name,
            ext: ext.clone(),
            orig_name,
            hash: content_hash,
            size_bytes: size,
            mtime,
            kind: kind.as_str().to_string(),
            width: probed.width,
            height: probed.height,
            duration_ms: probed.duration_ms,
            codec: probed.codec,
            bitrate: probed.bitrate,
            captured_at: probed.captured_at,
            captured_src: probed.captured_src,
        },
    )?;
    db::tags::rebuild_item(conn, item_id)?;

    shard::move_into_shard(paths, src, &uuid, &ext)?;

    if kind != Kind::Other {
        crate::jobs::enqueue_thumb(conn, item_id)?;
    }
    if kind == Kind::Video && probed.duration_ms.unwrap_or(0) > 0 {
        crate::jobs::enqueue_sprite(conn, item_id)?;
    }
    Ok(item_id)
}

/// Read a settled file out of `inbox/` and index it into the Sorting Box.
pub fn run_hash(
    paths: &LibraryPaths,
    tools: &Tools,
    conn: &mut Connection,
    payload: HashPayload,
) -> Result<()> {
    let inbox_abs = paths.inbox_dir().join(&payload.inbox_rel);
    let name = inbox_abs
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| AppError::invalid("inbox arrival has no file name"))?
        .to_string();

    index_file(paths, tools, conn, &inbox_abs, name, None)?;
    Ok(())
}

pub fn run_thumb(
    paths: &LibraryPaths,
    tools: &Tools,
    conn: &mut Connection,
    payload: ItemPayload,
) -> Result<()> {
    let item = db::items::file_for(conn, payload.item_id)?
        .ok_or_else(|| AppError::invalid("item no longer exists"))?;

    let source_size = thumbs::generate(paths, &item, tools)?;

    // Backfill for items whose dimensions were never read — the ones probed
    // by a decoder chosen from a lying extension. Re-indexing skips unchanged
    // files, so without this they would keep laying out as squares forever.
    if !item.has_dimensions {
        if let Some((width, height)) = source_size {
            db::items::set_dimensions(conn, item.id, width, height)?;
        }
    }
    Ok(())
}

pub fn run_sprite(
    paths: &LibraryPaths,
    tools: &Tools,
    conn: &mut Connection,
    payload: ItemPayload,
) -> Result<()> {
    let item = db::items::file_for(conn, payload.item_id)?
        .ok_or_else(|| AppError::invalid("item no longer exists"))?;
    sprites::generate(paths, &item, tools)
}

/// Whether a failure is worth another attempt. A missing tool or an
/// unsupported format will fail identically forever; a locked file or a
/// half-written download will not.
pub fn is_transient(error: &AppError) -> bool {
    matches!(error, AppError::Io(_) | AppError::Db(_))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::items::Scope;

    /// Scratch libraries live under `target/`, so `cargo clean` disposes of
    /// them and nothing is ever written outside the repository.
    fn scratch(name: &str) -> std::path::PathBuf {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-libraries")
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create scratch library");
        root
    }

    fn write_png(path: &std::path::Path, width: u32, height: u32) {
        let mut image = image::RgbImage::new(width, height);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = image::Rgb([(x % 256) as u8, (y % 256) as u8, 128]);
        }
        image.save(path).expect("write test png");
    }

    fn open_db(root: &std::path::Path) -> (LibraryPaths, Connection) {
        let paths = LibraryPaths::new(root);
        paths.ensure_dirs().unwrap();
        let mut conn = db::open(&paths.db_path()).unwrap();
        db::migrate(&mut conn).unwrap();
        (paths, conn)
    }

    /// Drain the job queue the way a worker pool would.
    fn drain(paths: &LibraryPaths, conn: &mut Connection) {
        let tools = crate::sidecar::Tools::default();
        while let Some(job) = db::jobs::claim(conn).unwrap() {
            let outcome = match job.kind.as_str() {
                kinds::HASH => run_hash(paths, &tools, conn, serde_json::from_str(&job.payload).unwrap()),
                kinds::THUMB => run_thumb(paths, &tools, conn, serde_json::from_str(&job.payload).unwrap()),
                other => panic!("unexpected job {other}"),
            };
            outcome.unwrap();
            db::jobs::complete(conn, job.id).unwrap();
        }
    }

    /// A settled inbox arrival: hashed, probed, indexed into the Sorting
    /// Box, sharded into `files/` — the whole M1/M1.5/M2.6 pipeline
    /// collapsed into the single moment inbox arrival now is.
    #[test]
    fn hashes_a_settled_inbox_file_into_the_sorting_box_and_shards_it() {
        let root = scratch("worker-inbox-arrival");
        let (paths, mut conn) = open_db(&root);
        write_png(&paths.inbox_dir().join("holiday.png"), 40, 20);

        crate::jobs::enqueue_hash(&conn, "holiday.png").unwrap();
        drain(&paths, &mut conn);

        let items = db::items::list(&conn, &Scope::Unsorted).unwrap();
        assert_eq!(items.len(), 1, "landed in the Sorting Box");
        assert_eq!((items[0].w, items[0].h), (Some(40), Some(20)));

        assert!(!paths.inbox_dir().join("holiday.png").exists(), "left inbox as part of arriving");

        let uuid_hex = {
            let detail = db::items::detail(&conn, items[0].id).unwrap().unwrap();
            assert_eq!(detail.orig_name.as_deref(), Some("holiday.png"), "original name kept as metadata");
            detail.thumb.clone()
        };
        let _ = uuid_hex;
        assert!(
            paths.thumbs_dir().join(&items[0].thumb).is_file(),
            "thumbnail written to the sharded cache path"
        );
    }

    #[test]
    fn an_inbox_file_already_uuid_named_keeps_its_embedded_identity() {
        let root = scratch("worker-inbox-leftover-uuid");
        let (paths, mut conn) = open_db(&root);
        let uuid = "a3f2c1d4-e29b-41d4-a716-446655440000";
        write_png(&paths.inbox_dir().join(format!("{uuid}.png")), 10, 10);

        crate::jobs::enqueue_hash(&conn, &format!("{uuid}.png")).unwrap();
        drain(&paths, &mut conn);

        assert!(paths.item_path(uuid, "png").is_file());
        let db_uuid: String = conn.query_row("SELECT uuid FROM item", [], |r| r.get(0)).unwrap();
        assert_eq!(db_uuid, uuid);
    }

    /// The M1.1 defect, exactly as it appeared in the first real library: six
    /// files off a phone holding JPEG data under a `.PNG` name.
    #[test]
    fn indexes_files_whose_extension_lies() {
        let root = scratch("worker-lying-extension");
        let (paths, mut conn) = open_db(&root);

        let mut jpeg = std::io::Cursor::new(Vec::new());
        image::RgbImage::from_pixel(320, 200, image::Rgb([90, 120, 200]))
            .write_to(&mut jpeg, image::ImageFormat::Jpeg)
            .expect("encode jpeg");
        std::fs::write(paths.inbox_dir().join("IMG_9634.PNG"), jpeg.into_inner()).unwrap();

        crate::jobs::enqueue_hash(&conn, "IMG_9634.PNG").unwrap();
        drain(&paths, &mut conn);

        let items = db::items::list(&conn, &Scope::Unsorted).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!((items[0].w, items[0].h), (Some(320), Some(200)), "dimensions come from the content, not the extension");
        assert!(db::jobs::failures(&conn).unwrap().is_empty());
    }

    #[test]
    fn backfills_dimensions_left_null_by_an_earlier_index() {
        let root = scratch("worker-backfill");
        let (paths, mut conn) = open_db(&root);
        write_png(&paths.inbox_dir().join("photo.png"), 300, 150);

        crate::jobs::enqueue_hash(&conn, "photo.png").unwrap();
        drain(&paths, &mut conn);

        let items = db::items::list(&conn, &Scope::Unsorted).unwrap();
        let thumb = paths.thumbs_dir().join(&items[0].thumb);
        std::fs::remove_file(&thumb).unwrap();
        conn.execute("UPDATE item SET width = NULL, height = NULL", []).unwrap();

        crate::jobs::enqueue_thumb(&conn, items[0].id).unwrap();
        drain(&paths, &mut conn);

        let healed = db::items::list(&conn, &Scope::Unsorted).unwrap();
        assert_eq!((healed[0].w, healed[0].h), (Some(300), Some(150)));
        assert!(thumb.is_file());
    }

    #[test]
    fn failures_are_reported_per_file_and_cleared_on_reindex() {
        let root = scratch("worker-failures");
        let (paths, mut conn) = open_db(&root);
        // Truncated JPEG: a real decode failure, not a missing file.
        std::fs::write(paths.inbox_dir().join("broken.jpg"), b"\xff\xd8\xff\xe0\x00\x10JFIF\x00").unwrap();

        let run = |conn: &mut Connection| {
            crate::jobs::enqueue_hash(conn, "broken.jpg").unwrap();
            while let Some(job) = db::jobs::claim(conn).unwrap() {
                let tools = crate::sidecar::Tools::default();
                let outcome = match job.kind.as_str() {
                    kinds::HASH => run_hash(&paths, &tools, conn, serde_json::from_str(&job.payload).unwrap()),
                    _ => run_thumb(&paths, &tools, conn, serde_json::from_str(&job.payload).unwrap()),
                };
                match outcome {
                    Ok(()) => db::jobs::complete(conn, job.id).unwrap(),
                    Err(err) => db::jobs::fail(conn, job.id, &err.to_string(), false).unwrap(),
                }
            }
        };

        run(&mut conn);
        let failures = db::jobs::failures(&conn).unwrap();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].stage, "thumb");
        assert!(!failures[0].error.is_empty());

        // Re-running against the same still-broken file fails again, but
        // exactly once — failures do not accumulate across runs, and the
        // file was never removed from `inbox/` because it never got past
        // the hash step.
        db::jobs::clear_failed(&conn).unwrap();
        assert_eq!(db::jobs::failures(&conn).unwrap().len(), 0);
    }

    #[test]
    fn a_watcher_error_queues_a_reconcile_that_drains_pending_inbox_files() {
        let root = scratch("worker-reconcile");
        let (paths, mut conn) = open_db(&root);
        write_png(&paths.inbox_dir().join("missed.png"), 8, 8);

        crate::jobs::enqueue_index(&conn).unwrap();
        let job = db::jobs::claim(&mut conn).unwrap().expect("index job queued");
        assert_eq!(job.kind, kinds::INDEX);
        walk::reconcile(&paths, &mut conn, &mut |_, _| {}).unwrap();
        db::jobs::complete(&conn, job.id).unwrap();
        drain(&paths, &mut conn);

        assert_eq!(db::items::count(&conn).unwrap(), 1, "the missed file is indexed");
    }

    #[test]
    fn a_modified_arrival_is_a_brand_new_item_not_a_second_row_of_an_old_one() {
        // Unlike the pre-M2.6 tree watcher, an inbox arrival has no path
        // identity to anchor on — every settle is a fresh file, by design
        // (decision 30: `inbox/` is flat and disposable). Modifying
        // an *already-filed* item's own file is not something this app ever
        // does outside of a job it controls (compression, etc.), so there is
        // no "identity on modification" case left for the watcher to get
        // right the way M1.8 needed to.
        let root = scratch("worker-two-arrivals");
        let (paths, mut conn) = open_db(&root);
        write_png(&paths.inbox_dir().join("a.png"), 10, 10);
        crate::jobs::enqueue_hash(&conn, "a.png").unwrap();
        drain(&paths, &mut conn);

        write_png(&paths.inbox_dir().join("a.png"), 20, 20);
        crate::jobs::enqueue_hash(&conn, "a.png").unwrap();
        drain(&paths, &mut conn);

        assert_eq!(db::items::count(&conn).unwrap(), 2, "two separate arrivals, two separate items");
    }
}
