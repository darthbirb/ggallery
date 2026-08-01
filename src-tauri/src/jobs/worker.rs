//! Job execution. Everything here runs on a worker thread with its own
//! database connection — never on the Tauri command thread.

use std::collections::HashMap;

use rusqlite::Connection;

use crate::db;
use crate::db::items::NewItem;
use crate::db::jobs::QueuedJob;
use crate::error::{AppError, Result};
use crate::fs::import;
use crate::fs::paths::{extension_of, parse_uuid_disk_name, LibraryPaths};
use crate::fs::walk;
use crate::fs::watch::Suppressor;
use crate::jobs::kinds::{self, HashPayload, ItemPayload};
use crate::jobs::QueueInner;
use crate::media::{hash, probe, sprites, thumbs, Kind};
use crate::sidecar::Tools;

pub fn execute(ctx: &QueueInner, conn: &mut Connection, job: &QueuedJob) -> Result<()> {
    let (paths, tools) = (&ctx.paths, &ctx.tools);
    match job.kind.as_str() {
        kinds::INDEX => run_index(ctx, conn),
        kinds::HASH => run_hash(
            paths,
            tools,
            &ctx.rename_lookup,
            &ctx.suppressor,
            conn,
            serde_json::from_str(&job.payload)?,
        ),
        kinds::THUMB => run_thumb(paths, tools, conn, serde_json::from_str(&job.payload)?),
        kinds::SPRITE => run_sprite(paths, tools, conn, serde_json::from_str(&job.payload)?),
        other => Err(AppError::invalid(format!("unknown job type {other}"))),
    }
}

fn run_index(ctx: &QueueInner, conn: &mut Connection) -> Result<()> {
    // Failures belong to the run that produced them. The walk is about to
    // re-attempt every file that has no thumbnail, so last run's failures are
    // stale the moment it starts — and keeping them is what let a clean
    // re-index still show a count and a retry button.
    db::jobs::clear_failed(conn)?;

    ctx.set_walking(true);
    let result = walk::index(&ctx.paths, conn, &mut |folders, files| {
        ctx.report_walk(folders, files);
    });
    ctx.set_walking(false);
    // Whether this walk ran because of the ordinary startup index or because
    // the watcher lost sync and asked for a reconcile, it is the same walk —
    // once it is done, there is nothing left to distinguish it by.
    ctx.rescanning
        .store(false, std::sync::atomic::Ordering::Relaxed);

    if result.is_err() {
        // The walk commits in batches; a failure part-way leaves one open.
        db::rollback_batch(conn);
    }
    result.map(|_| ())
}

/// Read the file once: hash it, probe it, and only then create the row. An
/// item never exists in a half-known state.
pub fn run_hash(
    paths: &LibraryPaths,
    tools: &Tools,
    rename_lookup: &HashMap<String, String>,
    suppressor: &Suppressor,
    conn: &mut Connection,
    payload: HashPayload,
) -> Result<()> {
    let folder_rel = db::folders::rel_for(conn, payload.folder_id)?
        .ok_or_else(|| AppError::invalid("folder disappeared from the index"))?;
    let path = paths.item_path(&folder_rel, &payload.disk_name)?;

    let meta = std::fs::metadata(&path)?;
    let size = meta.len() as i64;
    let mtime = walk::mtime_secs(&meta);

    let ext = extension_of(&payload.disk_name);
    let kind = Kind::from_ext(&ext);

    let hash = hash::blake3_file(&path)?;
    let probed = probe::probe(&path, kind, mtime, tools.ffmpeg.as_ref());

    let existing = db::items::existing(conn, payload.folder_id, &payload.disk_name)?;
    // A brand-new row whose name already parses as `<uuid>.<ext>` arrived
    // that way because the M1.7 import rename ran before this walk did —
    // reuse the embedded uuid as identity (minting a fresh one here would
    // desync `disk_name` from `uuid` forever) and recover the real original
    // name from the rename's own record, since all the walker itself can see
    // is the name already on disk.
    let (uuid, orig_name) = match &existing {
        Some(item) => (item.uuid.clone(), payload.disk_name.clone()),
        None => match parse_uuid_disk_name(&payload.disk_name) {
            Some(embedded_uuid) => {
                let orig_name = rename_lookup
                    .get(&embedded_uuid)
                    .cloned()
                    .unwrap_or_else(|| payload.disk_name.clone());
                (embedded_uuid, orig_name)
            }
            None => (uuid::Uuid::new_v4().to_string(), payload.disk_name.clone()),
        },
    };

    let item_id = db::items::upsert(
        conn,
        &NewItem {
            uuid,
            folder_id: payload.folder_id,
            disk_name: payload.disk_name.clone(),
            ext,
            orig_name,
            hash,
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

    // M1's read-only stance ends once the library has been imported: from
    // then on, anything the walker finds that the app did not itself write
    // gets its UUID name right here, silently, as part of being indexed. See
    // docs/DESIGN.md#first-import, "After the first import".
    if db::settings::imported_at(conn)?.is_some() {
        import::rename_on_arrival(paths, conn, item_id, suppressor)?;
    }

    if kind != Kind::Other {
        crate::jobs::enqueue_thumb(conn, item_id)?;
    }
    if kind == Kind::Video && probed.duration_ms.unwrap_or(0) > 0 {
        crate::jobs::enqueue_sprite(conn, item_id)?;
    }
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

    /// Walk a real folder tree, then run the jobs it queued, and check what
    /// ends up in the database and the cache. This is the whole M1 read path.
    #[test]
    fn indexes_a_folder_tree_end_to_end() {
        let root = scratch("pipeline");
        std::fs::create_dir_all(root.join("People/Ana")).unwrap();
        write_png(&root.join("People/Ana/holiday.png"), 400, 200);
        write_png(&root.join("cover.png"), 64, 64);
        std::fs::write(root.join("notes.txt"), b"not media, still an item").unwrap();
        std::fs::write(root.join("Thumbs.db"), b"windows litter").unwrap();

        let paths = LibraryPaths::new(&root);
        paths.ensure_dirs().unwrap();
        let mut conn = db::open(&paths.db_path()).unwrap();
        db::migrate(&mut conn).unwrap();
        let tools = Tools::default();

        let report = crate::fs::walk::index(&paths, &mut conn, &mut |_folders, _files| {}).unwrap();
        assert_eq!(report.files, 3, "Thumbs.db is not library content");
        assert_eq!(report.folders, 3, "root, People, People/Ana");

        // Drain the queue the way a worker would.
        let mut ran = 0;
        while let Some(job) = db::jobs::claim(&mut conn).unwrap() {
            let outcome = match job.kind.as_str() {
                kinds::HASH => run_hash(
                    &paths,
                    &tools,
                    &HashMap::new(),
                    &Suppressor::default(),
                    &mut conn,
                    serde_json::from_str(&job.payload).unwrap(),
                ),
                kinds::THUMB => run_thumb(
                    &paths,
                    &tools,
                    &mut conn,
                    serde_json::from_str(&job.payload).unwrap(),
                ),
                other => panic!("unexpected job {other}"),
            };
            outcome.expect("job should succeed");
            db::jobs::complete(&conn, job.id).unwrap();
            ran += 1;
        }
        assert_eq!(ran, 5, "three hashes plus two thumbnails");

        let items = db::items::list(&conn, &Scope::default()).unwrap();
        assert_eq!(items.len(), 3);

        let holiday = items
            .iter()
            .find(|item| item.name == "holiday.png")
            .expect("indexed the nested image");
        assert_eq!((holiday.w, holiday.h), (Some(400), Some(200)));
        assert_eq!(holiday.kind, "image");
        assert!(
            paths.thumbs_dir().join(&holiday.thumb).is_file(),
            "thumbnail written to the sharded cache path"
        );

        let notes = items.iter().find(|item| item.name == "notes.txt").unwrap();
        assert_eq!(
            notes.kind, "other",
            "unknown types are indexed, not skipped"
        );

        // Nothing in the library was renamed, moved or deleted.
        assert!(root.join("People/Ana/holiday.png").is_file());
        assert!(root.join("cover.png").is_file());
        assert!(root.join("notes.txt").is_file());
    }

    /// The M1.1 defect, exactly as it appeared in the first real library: six
    /// files off a phone holding JPEG data under a `.PNG` name. `image::open`
    /// picks its decoder from the extension, so every one of them failed with
    /// "Invalid PNG signature" and indexed with no dimensions.
    #[test]
    fn indexes_files_whose_extension_lies() {
        let root = scratch("lying-extension");

        // JPEG bytes, PNG name — written the way a phone writes them.
        let mut jpeg = std::io::Cursor::new(Vec::new());
        image::RgbImage::from_pixel(320, 200, image::Rgb([90, 120, 200]))
            .write_to(&mut jpeg, image::ImageFormat::Jpeg)
            .expect("encode jpeg");
        std::fs::write(root.join("IMG_9634.PNG"), jpeg.into_inner()).unwrap();

        let paths = LibraryPaths::new(&root);
        paths.ensure_dirs().unwrap();
        let mut conn = db::open(&paths.db_path()).unwrap();
        db::migrate(&mut conn).unwrap();
        let tools = Tools::default();

        crate::fs::walk::index(&paths, &mut conn, &mut |_, _| {}).unwrap();
        while let Some(job) = db::jobs::claim(&mut conn).unwrap() {
            let outcome = match job.kind.as_str() {
                kinds::HASH => run_hash(
                    &paths,
                    &tools,
                    &HashMap::new(),
                    &Suppressor::default(),
                    &mut conn,
                    serde_json::from_str(&job.payload).unwrap(),
                ),
                kinds::THUMB => run_thumb(
                    &paths,
                    &tools,
                    &mut conn,
                    serde_json::from_str(&job.payload).unwrap(),
                ),
                other => panic!("unexpected job {other}"),
            };
            outcome.expect("a JPEG named .PNG must still index");
            db::jobs::complete(&conn, job.id).unwrap();
        }

        let items = db::items::list(&conn, &Scope::default()).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(
            (items[0].w, items[0].h),
            (Some(320), Some(200)),
            "dimensions come from the content, not the extension"
        );
        assert!(paths.thumbs_dir().join(&items[0].thumb).is_file());
        assert!(db::jobs::failures(&conn).unwrap().is_empty());
    }

    /// The upgrade path for a library indexed before the sniffing fix: those
    /// items already exist with no dimensions, and a re-index skips unchanged
    /// files, so the thumbnail job has to heal them as it goes.
    #[test]
    fn backfills_dimensions_left_null_by_an_earlier_index() {
        let root = scratch("backfill");
        write_png(&root.join("photo.png"), 300, 150);

        let paths = LibraryPaths::new(&root);
        paths.ensure_dirs().unwrap();
        let mut conn = db::open(&paths.db_path()).unwrap();
        db::migrate(&mut conn).unwrap();
        let tools = Tools::default();

        crate::fs::walk::index(&paths, &mut conn, &mut |_, _| {}).unwrap();
        while let Some(job) = db::jobs::claim(&mut conn).unwrap() {
            match job.kind.as_str() {
                kinds::HASH => run_hash(
                    &paths,
                    &tools,
                    &HashMap::new(),
                    &Suppressor::default(),
                    &mut conn,
                    serde_json::from_str(&job.payload).unwrap(),
                )
                .unwrap(),
                _ => run_thumb(
                    &paths,
                    &tools,
                    &mut conn,
                    serde_json::from_str(&job.payload).unwrap(),
                )
                .unwrap(),
            }
            db::jobs::complete(&conn, job.id).unwrap();
        }

        // Reproduce what the old probe left behind: a row with no dimensions
        // and no thumbnail on disk.
        let items = db::items::list(&conn, &Scope::default()).unwrap();
        let thumb = paths.thumbs_dir().join(&items[0].thumb);
        std::fs::remove_file(&thumb).unwrap();
        conn.execute("UPDATE item SET width = NULL, height = NULL", [])
            .unwrap();

        crate::fs::walk::index(&paths, &mut conn, &mut |_, _| {}).unwrap();
        while let Some(job) = db::jobs::claim(&mut conn).unwrap() {
            run_thumb(
                &paths,
                &tools,
                &mut conn,
                serde_json::from_str(&job.payload).unwrap(),
            )
            .unwrap();
            db::jobs::complete(&conn, job.id).unwrap();
        }

        let healed = db::items::list(&conn, &Scope::default()).unwrap();
        assert_eq!((healed[0].w, healed[0].h), (Some(300), Some(150)));
        assert!(thumb.is_file());
    }

    /// Failures name the file and carry the real error, and a fresh index run
    /// does not inherit the previous run's.
    #[test]
    fn failures_are_reported_per_file_and_cleared_on_reindex() {
        let root = scratch("failures");
        std::fs::create_dir_all(root.join("People")).unwrap();
        // Truncated JPEG: a real decode failure, not a missing file.
        std::fs::write(root.join("People/broken.jpg"), b"\xff\xd8\xff\xe0\x00\x10JFIF\x00").unwrap();

        let paths = LibraryPaths::new(&root);
        paths.ensure_dirs().unwrap();
        let mut conn = db::open(&paths.db_path()).unwrap();
        db::migrate(&mut conn).unwrap();
        let tools = Tools::default();

        let drain = |conn: &mut rusqlite::Connection| {
            while let Some(job) = db::jobs::claim(conn).unwrap() {
                let outcome = match job.kind.as_str() {
                    kinds::HASH => run_hash(
                        &paths,
                        &tools,
                        &HashMap::new(),
                        &Suppressor::default(),
                        conn,
                        serde_json::from_str(&job.payload).unwrap(),
                    ),
                    _ => run_thumb(
                        &paths,
                        &tools,
                        conn,
                        serde_json::from_str(&job.payload).unwrap(),
                    ),
                };
                match outcome {
                    Ok(()) => db::jobs::complete(conn, job.id).unwrap(),
                    Err(err) => db::jobs::fail(conn, job.id, &err.to_string(), false).unwrap(),
                }
            }
        };

        crate::fs::walk::index(&paths, &mut conn, &mut |_, _| {}).unwrap();
        drain(&mut conn);

        let failures = db::jobs::failures(&conn).unwrap();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].name, "broken.jpg");
        assert_eq!(failures[0].folder, "people");
        assert_eq!(failures[0].stage, "thumb");
        assert!(
            !failures[0].error.is_empty(),
            "the decoder's own message, not a count"
        );

        // Re-index: the same file fails again, but exactly once — failures do
        // not accumulate across runs.
        db::jobs::clear_failed(&conn).unwrap();
        crate::fs::walk::index(&paths, &mut conn, &mut |_, _| {}).unwrap();
        drain(&mut conn);
        assert_eq!(db::jobs::failures(&conn).unwrap().len(), 1);

        // And a run with nothing wrong reports nothing.
        std::fs::remove_file(root.join("People/broken.jpg")).unwrap();
        db::jobs::clear_failed(&conn).unwrap();
        crate::fs::walk::index(&paths, &mut conn, &mut |_, _| {}).unwrap();
        drain(&mut conn);
        assert!(db::jobs::failures(&conn).unwrap().is_empty());
    }

    /// A second walk must be cheap and must notice what changed on disk.
    #[test]
    fn reindex_skips_unchanged_and_retires_missing() {
        let root = scratch("reindex");
        write_png(&root.join("a.png"), 32, 32);
        write_png(&root.join("b.png"), 32, 32);

        let paths = LibraryPaths::new(&root);
        paths.ensure_dirs().unwrap();
        let mut conn = db::open(&paths.db_path()).unwrap();
        db::migrate(&mut conn).unwrap();
        let tools = Tools::default();

        let drain = |conn: &mut rusqlite::Connection| {
            while let Some(job) = db::jobs::claim(conn).unwrap() {
                let outcome = match job.kind.as_str() {
                    kinds::HASH => run_hash(
                        &paths,
                        &tools,
                        &HashMap::new(),
                        &Suppressor::default(),
                        conn,
                        serde_json::from_str(&job.payload).unwrap(),
                    ),
                    kinds::THUMB => run_thumb(
                        &paths,
                        &tools,
                        conn,
                        serde_json::from_str(&job.payload).unwrap(),
                    ),
                    other => panic!("unexpected job {other}"),
                };
                outcome.unwrap();
                db::jobs::complete(conn, job.id).unwrap();
            }
        };

        crate::fs::walk::index(&paths, &mut conn, &mut |_, _| {}).unwrap();
        drain(&mut conn);
        assert_eq!(db::items::count(&conn).unwrap(), 2);

        // Nothing changed: no work should be queued at all.
        let second = crate::fs::walk::index(&paths, &mut conn, &mut |_, _| {}).unwrap();
        assert_eq!(second.queued, 0, "unchanged files are not re-read");
        assert_eq!(second.vanished, 0);

        std::fs::remove_file(root.join("b.png")).unwrap();
        let third = crate::fs::walk::index(&paths, &mut conn, &mut |_, _| {}).unwrap();
        assert_eq!(third.vanished, 1, "a deleted file leaves the grid");
        assert_eq!(db::items::count(&conn).unwrap(), 1);
    }

    /// M1.6: before a library is marked imported, arriving files keep their
    /// real names (M1's read-only stance) — after, the indexer renames them
    /// on the way in, silently, without anyone running the wizard again.
    #[test]
    fn arriving_files_are_left_alone_before_import_and_renamed_after() {
        let root = scratch("arrival-gating");
        write_png(&root.join("before.png"), 10, 10);

        let paths = LibraryPaths::new(&root);
        paths.ensure_dirs().unwrap();
        let mut conn = db::open(&paths.db_path()).unwrap();
        db::migrate(&mut conn).unwrap();
        let tools = Tools::default();

        let drain = |conn: &mut rusqlite::Connection| {
            while let Some(job) = db::jobs::claim(conn).unwrap() {
                let outcome = match job.kind.as_str() {
                    kinds::HASH => run_hash(
                        &paths,
                        &tools,
                        &HashMap::new(),
                        &Suppressor::default(),
                        conn,
                        serde_json::from_str(&job.payload).unwrap(),
                    ),
                    kinds::THUMB => run_thumb(
                        &paths,
                        &tools,
                        conn,
                        serde_json::from_str(&job.payload).unwrap(),
                    ),
                    other => panic!("unexpected job {other}"),
                };
                outcome.unwrap();
                db::jobs::complete(conn, job.id).unwrap();
            }
        };

        crate::fs::walk::index(&paths, &mut conn, &mut |_, _| {}).unwrap();
        drain(&mut conn);
        assert!(
            root.join("before.png").is_file(),
            "not yet imported — M1's read-only stance holds"
        );

        db::settings::mark_imported(&conn).unwrap();

        write_png(&root.join("after.png"), 10, 10);
        crate::fs::walk::index(&paths, &mut conn, &mut |_, _| {}).unwrap();
        drain(&mut conn);

        assert!(
            !root.join("after.png").exists(),
            "arriving after import, this file should have been renamed on the way in"
        );
        assert!(
            root.join("before.png").is_file(),
            "already-indexed files are not retroactively renamed"
        );

        let disk_name: String = conn
            .query_row(
                "SELECT disk_name FROM item WHERE orig_name = 'after.png'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_ne!(disk_name, "after.png");
        assert!(root.join(&disk_name).is_file());

        let journal_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM journal WHERE op = 'rename'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(journal_count, 1, "one arrival, one journal row");
    }

    /// M1.7's whole point: the rename must complete before a single row is
    /// indexed, and the walker must still recover the true original name for
    /// a file it only ever sees already renamed. Drives the exact sequence
    /// the startup flow does — `fs::import::prepare`, then
    /// `fs::import::execute_prepared`, and only afterward the normal
    /// walk+hash pipeline — and checks both halves of that promise.
    #[test]
    fn m1_7_rename_before_index_preserves_orig_name_and_uuid_identity() {
        let root = scratch("m1-7-rename-then-index");
        std::fs::create_dir_all(root.join("People/Ana")).unwrap();
        write_png(&root.join("People/Ana/holiday.png"), 40, 20);
        write_png(&root.join("cover.png"), 8, 8);

        let paths = LibraryPaths::new(&root);

        // Nothing indexed yet — this is the pre-database scan the startup
        // flow's Review screen shows.
        let (report, pending) = import::prepare(&root).unwrap();
        assert!(!report.already_imported);
        assert_eq!(report.to_rename, 2);
        let pending = pending.expect("two files need renaming");

        let executed = import::execute_prepared(&pending, &mut |_| {}).unwrap();
        assert_eq!(executed.renamed, 2);
        assert!(executed.errors.is_empty());

        // Renamed on disk before anything was indexed — exactly the order
        // M1.7 requires.
        assert!(!root.join("People/Ana/holiday.png").exists());
        assert!(!root.join("cover.png").exists());

        let mut conn = db::open(&paths.db_path()).unwrap();
        db::migrate(&mut conn).unwrap();
        assert!(
            db::settings::imported_at(&conn).unwrap().is_some(),
            "execute_prepared marks the library imported on its own"
        );
        assert_eq!(
            db::items::count(&conn).unwrap(),
            0,
            "no item row exists yet — the rename never touched the database"
        );

        let rename_lookup = import::load_rename_lookup(&paths);
        let tools = Tools::default();

        crate::fs::walk::index(&paths, &mut conn, &mut |_, _| {}).unwrap();
        while let Some(job) = db::jobs::claim(&mut conn).unwrap() {
            let outcome = match job.kind.as_str() {
                kinds::HASH => run_hash(
                    &paths,
                    &tools,
                    &rename_lookup,
                    &Suppressor::default(),
                    &mut conn,
                    serde_json::from_str(&job.payload).unwrap(),
                ),
                kinds::THUMB => run_thumb(
                    &paths,
                    &tools,
                    &mut conn,
                    serde_json::from_str(&job.payload).unwrap(),
                ),
                other => panic!("unexpected job {other}"),
            };
            outcome.unwrap();
            db::jobs::complete(&conn, job.id).unwrap();
        }

        let items = db::items::list(&conn, &Scope::default()).unwrap();
        assert_eq!(items.len(), 2);

        let orig_name: String = conn
            .query_row(
                "SELECT orig_name FROM item WHERE orig_name = 'holiday.png'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            orig_name, "holiday.png",
            "the real original name, recovered via the rename lookup — not the uuid the walker actually saw"
        );

        let (disk_name, uuid): (String, String) = conn
            .query_row(
                "SELECT disk_name, uuid FROM item WHERE orig_name = 'holiday.png'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            disk_name,
            format!("{uuid}.png"),
            "the uuid embedded in the filename must become the item's own identity, not a \
             freshly minted one, or disk_name and uuid desync forever"
        );

        let (already_renamed, to_rename) = db::items::rename_counts(&conn).unwrap();
        assert_eq!(already_renamed, 2);
        assert_eq!(to_rename, 0);
    }
}
