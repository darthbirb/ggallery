//! PLAN.md decision 20's synthetic-library generator. Dev tool only, not
//! shipped — see docs/STRUCTURE.md's `bin/` entry.
//!
//! Populates a scratch database directly through `db::*` (no real files, no
//! thumbnailing — M2 adds no query that touches either) and times the query
//! paths M2 introduces: the effective-tag cache's full-library and
//! single-folder rebuilds, the per-item rebuild `jobs::worker::run_hash` now
//! does inline for every new item, and the sidebar's folder-tree query.
//!
//! Usage: `cargo run --release --bin synth_library -- --items 100000`
//! (release — see CLAUDE.md: debug numbers are 6-40x slower and meaningless
//! here). `--dir <path>` overrides the scratch database location.
//!
//! Kept, not thrown away like the M0 spike: later milestones extend this
//! with their own new query paths rather than writing a fresh generator.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gallery_lib::db;
use gallery_lib::error::Result;
use gallery_lib::fs::paths::LibraryPaths;
use uuid::Uuid;

const CATEGORIES: usize = 20;
const LEAVES_PER_CATEGORY: usize = 100;
/// Person, seeded with id 1 in `002_folder_metadata.sql`.
const PERSON_ARCHETYPE: i64 = 1;

fn main() {
    if let Err(err) = run() {
        eprintln!("synth_library failed: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let items_total = arg_usize("--items").unwrap_or(100_000);
    let dir = arg_path("--dir").unwrap_or_else(default_scratch_dir);

    println!("synth_library: {items_total} items, folder tree of {} leaves, at {}", CATEGORIES * LEAVES_PER_CATEGORY, dir.display());
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;

    let paths = LibraryPaths::new(&dir);
    paths.ensure_dirs()?;
    let mut conn = db::open(&paths.db_path())?;
    db::migrate(&mut conn)?;

    let t0 = Instant::now();
    db::begin_batch(&conn)?;
    let leaves = build_tree(&conn)?;
    db::commit_batch(&conn)?;
    println!(
        "  folder tree: {} folders in {:?}",
        CATEGORIES * LEAVES_PER_CATEGORY + CATEGORIES + 1,
        t0.elapsed()
    );

    let t1 = Instant::now();
    db::begin_batch(&conn)?;
    let item_ids = insert_items(&conn, &leaves, items_total)?;
    db::commit_batch(&conn)?;
    println!("  items: {} inserted in {:?}", item_ids.len(), t1.elapsed());

    // The core question: rebuilding the whole library's effective-tag cache
    // at scale — decision 20's stated risk.
    let t2 = Instant::now();
    db::tags::rebuild_subtree(&conn, "")?;
    let full_rebuild = t2.elapsed();

    // The common case: one leaf folder's tags change.
    let leaf_rel = db::folders::rel_for(&conn, leaves[0])?.expect("leaf exists");
    db::folders::add_flag(&conn, leaves[0], "edited")?;
    let t3 = Instant::now();
    db::tags::rebuild_subtree(&conn, &leaf_rel)?;
    let leaf_rebuild = t3.elapsed();

    // What `jobs::worker::run_hash` now does inline for every new item.
    let stride = (item_ids.len() / 500).max(1);
    let sample: Vec<i64> = item_ids.iter().step_by(stride).copied().collect();
    let t4 = Instant::now();
    for id in &sample {
        db::tags::rebuild_item(&conn, *id)?;
    }
    let per_item = t4.elapsed() / sample.len() as u32;

    // The sidebar's own query, now carrying two more columns.
    let t5 = Instant::now();
    let tree = db::folders::tree(&conn)?;
    let tree_query = t5.elapsed();

    println!();
    report(
        "root-level rebuild_subtree (whole library)",
        full_rebuild,
        Duration::from_secs(10),
    );
    report("leaf-level rebuild_subtree", leaf_rebuild, Duration::from_millis(500));
    report(
        &format!("rebuild_item (avg over {})", sample.len()),
        per_item,
        Duration::from_millis(2),
    );
    report(
        &format!("folders::tree() ({} rows)", tree.len()),
        tree_query,
        Duration::from_millis(500),
    );

    Ok(())
}

/// Category folders → leaf folders, matching the "folder counts in the
/// thousands" scale `db::folders::tree`'s own doc comment already assumes.
/// Every tenth leaf gets the Person archetype applied, so archetype
/// resolution is exercised at scale too, not just for one folder.
fn build_tree(conn: &rusqlite::Connection) -> Result<Vec<i64>> {
    db::folders::upsert(conn, "", "Library")?;
    let mut leaves = Vec::with_capacity(CATEGORIES * LEAVES_PER_CATEGORY);

    for c in 0..CATEGORIES {
        let cat_rel = format!("category-{c:02}");
        let cat_id = db::folders::upsert(conn, &cat_rel, &format!("Category {c:02}"))?;
        db::folders::add_flag(conn, cat_id, "synthetic")?;

        for l in 0..LEAVES_PER_CATEGORY {
            let rel = format!("{cat_rel}/leaf-{l:03}");
            let id = db::folders::upsert(conn, &rel, &format!("Leaf {c:02}-{l:03}"))?;
            if l % 10 == 0 {
                db::folders::apply_archetype(conn, id, PERSON_ARCHETYPE)?;
                db::folders::set_label(conn, id, "instagram", &format!("@leaf{c}{l}"))?;
            }
            leaves.push(id);
        }
    }
    Ok(leaves)
}

/// Items spread evenly across the leaf folders. Fabricated content —
/// uuid/hash/dimensions — since nothing here is ever decoded or thumbnailed.
fn insert_items(conn: &rusqlite::Connection, leaves: &[i64], total: usize) -> Result<Vec<i64>> {
    let mut ids = Vec::with_capacity(total);
    let now = db::now();
    for i in 0..total {
        let folder_id = leaves[i % leaves.len()];
        let uuid = Uuid::new_v4().to_string();
        let item = db::items::NewItem {
            uuid: uuid.clone(),
            folder_id,
            disk_name: format!("{uuid}.jpg"),
            ext: "jpg".to_string(),
            orig_name: format!("{uuid}.jpg"),
            hash: Uuid::new_v4().simple().to_string(),
            size_bytes: 1_000_000,
            mtime: now,
            kind: "image".to_string(),
            width: Some(1920),
            height: Some(1080),
            duration_ms: None,
            codec: None,
            bitrate: None,
            captured_at: Some(now),
            captured_src: Some("mtime".to_string()),
        };
        ids.push(db::items::upsert(conn, &item)?);
    }
    Ok(ids)
}

fn report(name: &str, actual: Duration, budget: Duration) {
    let verdict = if actual <= budget { "OK  " } else { "SLOW" };
    println!("[{verdict}] {name}: {actual:?} (budget {budget:?})");
}

fn arg_usize(flag: &str) -> Option<usize> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
}

fn arg_path(flag: &str) -> Option<PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
}

fn default_scratch_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("synth-library")
}
