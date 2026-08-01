//! The UUID rename: filesystem-only scan and execute for a library that has
//! never been imported (`prepare` / `execute_prepared`, M1.7's startup flow),
//! plus the original database-backed scan/dry-run/execute/verify for the
//! repair case — Settings → Normalise filenames, run against an already-open,
//! already-indexed library. See docs/DESIGN.md#first-import for the flow
//! and why each step exists.
//!
//! **Scoped to the rename alone.** Parsing folder names into archetype fields
//! is a separate M2 step — archetypes do not exist yet.
//!
//! The rename is the most destructive thing the app does, so two properties
//! matter more than anything else here:
//!
//! 1. **Idempotent.** A file is "done" exactly when `disk_name` already equals
//!    `<uuid>.<ext>` — not a flag set once and trusted forever. Running
//!    an execute path again, whether because the flow was re-entered or
//!    because a previous run crashed mid-batch, only ever touches what is
//!    still left.
//! 2. **The disaster-recovery record is written before the risk, not after.**
//!    Each batch's `JsonlRecord`s are appended to `library.jsonl` and fsynced
//!    *before* any file in that batch is renamed. If the process dies between
//!    the fsync and the rename actually landing, the file may already carry
//!    its new name while the record says otherwise — `rename_one` below
//!    treats an already-renamed file as success rather than an error, which
//!    is what makes resuming safe. There is no tooling that reads this record
//!    back to undo anything — see docs/DESIGN.md#first-import, "No reversal"
//!    — but a human with a text editor always has enough to reconstruct what
//!    happened to any given file.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::db;
use crate::error::{AppError, Result};
use crate::fs::paths::LibraryPaths;
use crate::fs::walk;
use crate::fs::watch::Suppressor;
use crate::media::hash;

/// Items per transaction, and per reversal-map flush. Small enough that the
/// jsonl fsync and the database commit are never far behind the renames
/// actually happening on disk; large enough that 300k files is thousands of
/// batches, not hundreds of thousands of fsyncs.
const BATCH: i64 = 200;

// --- scan ------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub by_kind: Vec<db::items::KindTotal>,
    pub total_items: i64,
    pub total_bytes: i64,
    pub folder_count: i64,
    /// Files M1 could not read at all — reported so the wizard does not
    /// imply the whole library was seen when some of it was not.
    pub unreadable: i64,
    pub already_renamed: i64,
    pub to_rename: i64,
    /// `None` until the wizard (or "Normalise filenames") has completed once.
    /// What the frontend uses to decide whether to offer the wizard at all —
    /// see docs/DESIGN.md#first-import.
    pub imported_at: Option<i64>,
}

/// Walk the root, report what was found — except the walk already happened in
/// M1, so this reads the index rather than re-reading the filesystem.
pub fn scan(conn: &Connection) -> Result<ScanReport> {
    let by_kind = db::items::counts_by_kind(conn)?;
    let total_items = by_kind.iter().map(|k| k.count).sum();
    let total_bytes = by_kind.iter().map(|k| k.bytes).sum();
    let folder_count = db::folders::count(conn)?;
    let unreadable = db::jobs::counts(conn)?.failed;
    let (already_renamed, to_rename) = db::items::rename_counts(conn)?;
    let imported_at = db::settings::imported_at(conn)?;

    Ok(ScanReport {
        by_kind,
        total_items,
        total_bytes,
        folder_count,
        unreadable,
        already_renamed,
        to_rename,
        imported_at,
    })
}

// --- dry run -----------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenamePreview {
    pub folder: String,
    pub old_name: String,
    pub new_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DryRunReport {
    pub to_rename: i64,
    pub sample: Vec<RenamePreview>,
}

/// Exactly what will be renamed, with a sample. Nothing is written.
pub fn dry_run(conn: &Connection, sample_size: i64) -> Result<DryRunReport> {
    let (_, to_rename) = db::items::rename_counts(conn)?;
    let candidates = db::items::rename_candidates_after(conn, 0, sample_size)?;
    let sample = candidates
        .into_iter()
        .map(|c| RenamePreview {
            new_name: new_disk_name(&c),
            folder: c.folder_rel,
            old_name: c.disk_name,
        })
        .collect();
    Ok(DryRunReport { to_rename, sample })
}

// --- execute -----------------------------------------------------------------

/// One line of `library.jsonl` — the disaster-recovery record of what a
/// rename did. Self-describing on purpose: a human with a text editor and no
/// working app should be able to see exactly what happened to a given file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonlRecord {
    pub uuid: String,
    pub folder_rel: String,
    pub orig_name: String,
    pub new_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameError {
    pub item_id: i64,
    pub folder: String,
    pub name: String,
    pub error: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteReport {
    pub renamed: i64,
    /// Already done when this run started — either a previous run finished
    /// some of the library, or the wizard is being re-opened on a library
    /// that was fully imported already.
    pub already_done: i64,
    pub errors: Vec<RenameError>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    pub done: i64,
    pub total: i64,
    pub errors: i64,
}

/// Rename every not-yet-renamed item to `<uuid>.<ext>`, in batches, writing
/// the disaster-recovery record continuously. The repair path — Settings →
/// Normalise filenames — on an already-open, already-indexed library.
///
/// Safe to call again after a crash, a close, or on a library that was
/// already fully imported — see the module docs for why.
pub fn execute(
    paths: &LibraryPaths,
    conn: &Connection,
    on_progress: &mut dyn FnMut(&ImportProgress),
) -> Result<ExecuteReport> {
    let (already_done, to_rename) = db::items::rename_counts(conn)?;
    let mut report = ExecuteReport {
        already_done,
        ..Default::default()
    };

    if to_rename == 0 {
        on_progress(&ImportProgress {
            done: 0,
            total: 0,
            errors: 0,
        });
        // Running the wizard (or "Normalise filenames") to completion is what
        // marks a library imported, even when there was nothing left to do —
        // e.g. a re-open, or a repair pass that finds everything already
        // clean.
        db::settings::mark_imported(conn)?;
        return Ok(report);
    }

    if let Some(parent) = paths.jsonl_path().parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut jsonl = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.jsonl_path())?;

    let mut after_id = 0i64;
    loop {
        let batch = db::items::rename_candidates_after(conn, after_id, BATCH)?;
        if batch.is_empty() {
            break;
        }
        after_id = batch.last().expect("checked non-empty above").id;

        // Durable before any file moves — the property the whole crash-safety
        // story rests on.
        let records: Vec<JsonlRecord> = batch
            .iter()
            .map(|c| JsonlRecord {
                uuid: c.uuid.clone(),
                folder_rel: c.folder_rel.clone(),
                orig_name: c
                    .orig_name
                    .clone()
                    .unwrap_or_else(|| c.disk_name.clone()),
                new_name: new_disk_name(c),
            })
            .collect();
        append_jsonl(&mut jsonl, &records)?;

        db::begin_batch(conn)?;
        for candidate in &batch {
            let new_name = new_disk_name(candidate);
            match rename_one(paths, &candidate.folder_rel, &candidate.disk_name, &new_name) {
                Ok(RenameOutcome::Renamed) | Ok(RenameOutcome::AlreadyDone) => {
                    db::items::set_disk_name(conn, candidate.id, &new_name)?;
                    report.renamed += 1;
                }
                Ok(RenameOutcome::Missing) => {
                    report.errors.push(RenameError {
                        item_id: candidate.id,
                        folder: candidate.folder_rel.clone(),
                        name: candidate.disk_name.clone(),
                        error: "file is no longer on disk".to_string(),
                    });
                }
                Err(err) => {
                    report.errors.push(RenameError {
                        item_id: candidate.id,
                        folder: candidate.folder_rel.clone(),
                        name: candidate.disk_name.clone(),
                        error: err.to_string(),
                    });
                }
            }
        }
        db::commit_batch(conn)?;

        on_progress(&ImportProgress {
            done: report.renamed,
            total: to_rename,
            errors: report.errors.len() as i64,
        });
    }

    db::settings::mark_imported(conn)?;
    Ok(report)
}

fn new_disk_name(candidate: &db::items::RenameCandidate) -> String {
    format!("{}.{}", candidate.uuid, candidate.ext)
}

fn append_jsonl(file: &mut File, records: &[JsonlRecord]) -> Result<()> {
    for record in records {
        let line = serde_json::to_string(record)?;
        writeln!(file, "{line}")?;
    }
    file.flush()?;
    file.sync_all()?;
    Ok(())
}

enum RenameOutcome {
    Renamed,
    /// The file already carried its new name — either this item was already
    /// imported, or a previous run renamed it on disk but crashed before the
    /// database commit landed.
    AlreadyDone,
    /// The old file is gone and the new one was never created. Reported, not
    /// silently skipped: M1's read-only guarantee means this file existed at
    /// index time, so something removed it since.
    Missing,
}

fn rename_one(
    paths: &LibraryPaths,
    folder_rel: &str,
    old_name: &str,
    new_name: &str,
) -> Result<RenameOutcome> {
    if old_name.eq_ignore_ascii_case(new_name) {
        return Ok(RenameOutcome::AlreadyDone);
    }

    let old_abs = paths.item_path(folder_rel, old_name)?;
    let new_abs = paths.item_path(folder_rel, new_name)?;

    if !old_abs.exists() {
        return Ok(if new_abs.exists() {
            RenameOutcome::AlreadyDone
        } else {
            RenameOutcome::Missing
        });
    }
    if new_abs.exists() {
        // A uuid collision is astronomically unlikely, but this must never
        // silently overwrite one file with another.
        return Err(AppError::media(format!(
            "{} already exists",
            new_abs.display()
        )));
    }

    std::fs::rename(&old_abs, &new_abs)?;
    Ok(RenameOutcome::Renamed)
}

// --- M1.7 startup flow: filesystem-only scan and execute -------------------
//
// A library that has never been imported has no database worth reading yet —
// under the old M1.5/M1.6 order the walk ran first and populated one, but
// M1.7 inverts that: nothing is indexed, thumbnailed, or written into
// `.gallery/` until the rename has completed. So `prepare` and
// `execute_prepared` below work the filesystem directly, the same way a plain
// `ls -R` would, and never open a job queue or enqueue any work.

/// One file a pre-import filesystem scan found that still needs renaming.
/// Unlike `db::items::RenameCandidate`, this exists before any database row
/// does.
#[derive(Debug, Clone)]
pub struct FsRenameCandidate {
    pub folder_rel: String,
    pub old_name: String,
    pub uuid: String,
    pub ext: String,
}

impl FsRenameCandidate {
    fn new_name(&self) -> String {
        format!("{}.{}", self.uuid, self.ext)
    }
}

/// Held in `AppState` between `prepare` and `execute_prepared` so Review and
/// Progress agree on exactly the same plan, and the execute step never has to
/// re-walk the filesystem to rediscover what Review already showed.
pub struct PendingImport {
    pub root: PathBuf,
    pub candidates: Vec<FsRenameCandidate>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewReport {
    /// True when this library has either already completed import, or needed
    /// no rename at all (an empty folder, or one already entirely
    /// UUID-named) — resolved on the spot by `prepare`. Either way the
    /// frontend skips Review and Progress and opens straight into the
    /// gallery.
    pub already_imported: bool,
    pub by_kind: Vec<db::items::KindTotal>,
    pub total_items: i64,
    pub total_bytes: i64,
    pub folder_count: i64,
    /// Entries the scan could not read at all — a locked file, a permission
    /// error — reported so Review does not imply the whole folder was seen
    /// when some of it was not.
    pub unreadable: i64,
    pub already_renamed: i64,
    pub to_rename: i64,
    /// Five rows, not a full manifest — docs/DESIGN.md#first-import.
    pub sample: Vec<RenamePreview>,
}

/// The Choose-folder step's follow-through. Figures out whether this library
/// needs the import ceremony at all and, if so, scans it — all without
/// opening a job queue or writing a thumbnail. Returns the plan alongside the
/// report so the caller can stash it for `execute_prepared`.
pub fn prepare(root: &Path) -> Result<(ReviewReport, Option<PendingImport>)> {
    let paths = LibraryPaths::new(root);

    if paths.db_path().is_file() {
        let mut conn = db::open(&paths.db_path())?;
        db::migrate(&mut conn)?;
        if db::settings::imported_at(&conn)?.is_some() {
            return Ok((
                ReviewReport {
                    already_imported: true,
                    ..Default::default()
                },
                None,
            ));
        }
    }

    let scan = scan_filesystem(&paths)?;
    if scan.candidates.is_empty() {
        // Nothing to do — mark it imported on the spot rather than asking the
        // user to click through a review screen with nothing on it. Mirrors
        // `execute`'s same shortcut for the repair path.
        paths.ensure_dirs()?;
        let mut conn = db::open(&paths.db_path())?;
        db::migrate(&mut conn)?;
        db::settings::mark_imported(&conn)?;
        return Ok((
            ReviewReport {
                already_imported: true,
                ..Default::default()
            },
            None,
        ));
    }

    let sample = scan
        .candidates
        .iter()
        .take(5)
        .map(|c| RenamePreview {
            folder: c.folder_rel.clone(),
            old_name: c.old_name.clone(),
            new_name: c.new_name(),
        })
        .collect();
    let to_rename = scan.candidates.len() as i64;

    let report = ReviewReport {
        already_imported: false,
        by_kind: scan.by_kind,
        total_items: scan.already_renamed + to_rename,
        total_bytes: scan.total_bytes,
        folder_count: scan.folder_count,
        unreadable: scan.unreadable,
        already_renamed: scan.already_renamed,
        to_rename,
        sample,
    };
    let pending = PendingImport {
        root: root.to_path_buf(),
        candidates: scan.candidates,
    };
    Ok((report, Some(pending)))
}

struct FsScan {
    by_kind: Vec<db::items::KindTotal>,
    total_bytes: i64,
    folder_count: i64,
    unreadable: i64,
    already_renamed: i64,
    candidates: Vec<FsRenameCandidate>,
}

/// Walk the root directly — no database, because there may not be one yet.
/// Mirrors `fs::walk::index`'s filtering (skip `.gallery`, skip OS litter)
/// without reading a single byte of file content, so it stays fast over a
/// few hundred thousand files the same way the M0 spike validated the plain
/// metadata walk was.
fn scan_filesystem(paths: &LibraryPaths) -> Result<FsScan> {
    let mut kind_totals: HashMap<&'static str, (i64, i64)> = HashMap::new();
    // The root itself counts as one folder, matching `fs::walk::index`, which
    // always inserts a folder row for it.
    let mut folder_count = 1i64;
    let mut unreadable = 0i64;
    let mut already_renamed = 0i64;
    let mut total_bytes = 0i64;
    let mut candidates = Vec::new();

    let walker = WalkDir::new(paths.root())
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !walk::is_skipped_dir(paths, entry));

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                unreadable += 1;
                continue;
            }
        };

        if entry.file_type().is_dir() {
            if entry.depth() > 0 {
                folder_count += 1;
            }
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if walk::IGNORED_FILES.contains(&name.to_lowercase().as_str()) {
            continue;
        }

        let Ok(meta) = entry.metadata() else {
            unreadable += 1;
            continue;
        };
        let parent_rel = match entry.path().parent().map(|p| paths.to_rel(p)).transpose() {
            Ok(rel) => rel.unwrap_or_default(),
            Err(_) => {
                unreadable += 1;
                continue;
            }
        };

        let ext = crate::fs::paths::extension_of(&name);
        let kind = crate::media::Kind::from_ext(&ext);
        let totals = kind_totals.entry(kind.as_str()).or_insert((0, 0));
        totals.0 += 1;
        totals.1 += meta.len() as i64;
        total_bytes += meta.len() as i64;

        if crate::fs::paths::parse_uuid_disk_name(&name).is_some() {
            already_renamed += 1;
            continue;
        }

        candidates.push(FsRenameCandidate {
            folder_rel: parent_rel,
            old_name: name,
            uuid: uuid::Uuid::new_v4().to_string(),
            ext,
        });
    }

    let by_kind = kind_totals
        .into_iter()
        .map(|(kind, (count, bytes))| db::items::KindTotal {
            kind: kind.to_string(),
            count,
            bytes,
        })
        .collect();

    Ok(FsScan {
        by_kind,
        total_bytes,
        folder_count,
        unreadable,
        already_renamed,
        candidates,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsRenameError {
    pub folder: String,
    pub name: String,
    pub error: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsExecuteReport {
    pub renamed: i64,
    pub errors: Vec<FsRenameError>,
}

/// Run the plan `prepare` staged: batch-rename every candidate on disk,
/// writing the disaster-recovery record before each batch the same way the
/// repair path's `execute` does, then mark the library imported. Nothing here
/// touches a job queue — indexing starts only once the caller opens the
/// library for real, after this returns.
///
/// A file this misses (permission error, vanished mid-flight) is reported but
/// does not fail the run: it is picked up automatically once indexing starts,
/// because `imported_at` is already set by the time that happens and the
/// indexer renames anything odd-named it finds on the way in — see
/// docs/DESIGN.md#first-import, "After the first import".
pub fn execute_prepared(
    pending: &PendingImport,
    on_progress: &mut dyn FnMut(&ImportProgress),
) -> Result<FsExecuteReport> {
    let paths = LibraryPaths::new(&pending.root);
    paths.ensure_dirs()?;

    let mut report = FsExecuteReport::default();
    let total = pending.candidates.len() as i64;

    let mut jsonl = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.jsonl_path())?;

    for chunk in pending.candidates.chunks(BATCH as usize) {
        let records: Vec<JsonlRecord> = chunk
            .iter()
            .map(|c| JsonlRecord {
                uuid: c.uuid.clone(),
                folder_rel: c.folder_rel.clone(),
                orig_name: c.old_name.clone(),
                new_name: c.new_name(),
            })
            .collect();
        append_jsonl(&mut jsonl, &records)?;

        for candidate in chunk {
            let new_name = candidate.new_name();
            match rename_one(&paths, &candidate.folder_rel, &candidate.old_name, &new_name) {
                Ok(RenameOutcome::Renamed) | Ok(RenameOutcome::AlreadyDone) => {
                    report.renamed += 1;
                }
                Ok(RenameOutcome::Missing) => {
                    report.errors.push(FsRenameError {
                        folder: candidate.folder_rel.clone(),
                        name: candidate.old_name.clone(),
                        error: "file is no longer on disk".to_string(),
                    });
                }
                Err(err) => {
                    report.errors.push(FsRenameError {
                        folder: candidate.folder_rel.clone(),
                        name: candidate.old_name.clone(),
                        error: err.to_string(),
                    });
                }
            }
        }

        on_progress(&ImportProgress {
            done: report.renamed,
            total,
            errors: report.errors.len() as i64,
        });
    }

    let mut conn = db::open(&paths.db_path())?;
    db::migrate(&mut conn)?;
    db::settings::mark_imported(&conn)?;

    Ok(report)
}

/// `uuid -> orig_name`, read from `library.jsonl`. The only consumer is the
/// indexer's hash job, for the one moment a freshly renamed file is walked
/// for the very first time and needs to recover the name the rename itself
/// already recorded — see `jobs::worker::run_hash`. Best-effort: a missing or
/// malformed file yields an empty map rather than failing the library open,
/// since nothing here is required for the rename or the index to be correct,
/// only for `orig_name` to be filled in rather than falling back to the
/// UUID name.
pub fn load_rename_lookup(paths: &LibraryPaths) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Ok(file) = std::fs::File::open(paths.jsonl_path()) else {
        return map;
    };
    for line in BufReader::new(file).lines() {
        let Ok(line) = line else { continue };
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(record) = serde_json::from_str::<JsonlRecord>(&line) {
            map.insert(record.uuid, record.orig_name);
        }
    }
    map
}

// --- rename on arrival ---------------------------------------------------

/// Give one newly-indexed item its UUID name, immediately and silently.
///
/// Called by the indexer for every file that arrives after the library has
/// been marked imported — and, once M4 builds it, the filesystem watcher
/// too; both go through this same function rather than each growing their
/// own copy. See docs/DESIGN.md#first-import, "After the first import".
///
/// Unlike the bulk wizard's `execute`, there is no backup gate and no
/// reversal-map entry: a single file is exactly what `Ctrl+Z` is for, so this
/// writes to the journal instead of `library.jsonl`.
///
/// Never called for files the app writes itself — downloads, compression
/// output, converted GIFs. Those are born `<uuid>.<ext>` and never reach this
/// path at all, because there was never a wrong name to correct.
///
/// `suppressor` is registered with both the old and new absolute paths
/// *before* the rename touches disk — the filesystem watcher (M1.8) checks it
/// on every event it sees, so the delete-then-create pair this rename itself
/// generates is never fed back in as an arrival and a deletion of its own
/// making. See `fs::watch::Suppressor`.
pub fn rename_on_arrival(
    paths: &LibraryPaths,
    conn: &Connection,
    item_id: i64,
    suppressor: &Suppressor,
) -> Result<()> {
    let item = db::items::rename_target(conn, item_id)?
        .ok_or_else(|| AppError::invalid("item disappeared before it could be renamed"))?;

    let new_name = format!("{}.{}", item.uuid, item.ext);
    if item.disk_name == new_name {
        return Ok(()); // already correctly named — nothing to do, nothing to journal
    }

    let old_abs = paths.item_path(&item.folder_rel, &item.disk_name)?;
    let new_abs = paths.item_path(&item.folder_rel, &new_name)?;
    suppressor.suppress(paths, &old_abs);
    suppressor.suppress(paths, &new_abs);

    match rename_one(paths, &item.folder_rel, &item.disk_name, &new_name)? {
        RenameOutcome::Missing => {
            return Err(AppError::media(format!(
                "{} is no longer on disk",
                item.disk_name
            )));
        }
        RenameOutcome::Renamed | RenameOutcome::AlreadyDone => {}
    }

    db::items::set_disk_name(conn, item.id, &new_name)?;
    db::journal::record_rename(conn, item.id, &item.folder_rel, &item.disk_name, &new_name)?;
    Ok(())
}

// --- verify ------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyItem {
    pub item_id: i64,
    pub folder: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyReport {
    pub sample_checked: i64,
    pub mismatches: Vec<VerifyItem>,
    pub missing: Vec<VerifyItem>,
    pub count_total: i64,
    /// Equals `count_total` exactly when the whole library has been renamed —
    /// the "confirm counts match" step in DESIGN.md.
    pub count_renamed: i64,
}

/// Re-hash a random sample of already-renamed items and confirm counts.
pub fn verify(paths: &LibraryPaths, conn: &Connection, sample_size: i64) -> Result<VerifyReport> {
    let count_total = db::items::count(conn)?;
    let (count_renamed, _) = db::items::rename_counts(conn)?;
    let sample = db::items::random_sample_for_verify(conn, sample_size)?;

    let mut mismatches = Vec::new();
    let mut missing = Vec::new();
    let mut sample_checked = 0i64;

    for item in sample {
        let abs = paths.item_path(&item.folder_rel, &item.disk_name)?;
        if !is_file(&abs) {
            missing.push(VerifyItem {
                item_id: item.id,
                folder: item.folder_rel,
                name: item.disk_name,
            });
            continue;
        }
        sample_checked += 1;
        let actual = hash::blake3_file(&abs)?;
        if actual != item.hash {
            mismatches.push(VerifyItem {
                item_id: item.id,
                folder: item.folder_rel,
                name: item.disk_name,
            });
        }
    }

    Ok(VerifyReport {
        sample_checked,
        mismatches,
        missing,
        count_total,
        count_renamed,
    })
}

fn is_file(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::items::{NewItem, Scope};

    fn scratch(name: &str) -> std::path::PathBuf {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-libraries")
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create scratch library");
        root
    }

    /// Index one real file the way M1's hash job would, without spinning up
    /// the whole job queue — these tests are about the rename, not the walk.
    fn index_one(
        conn: &Connection,
        folder_id: i64,
        rel_path: &str,
        bytes: &[u8],
    ) -> (i64, String) {
        let uuid = uuid::Uuid::new_v4().to_string();
        let hash = blake3::hash(bytes).to_hex().to_string();
        let id = db::items::upsert(
            conn,
            &NewItem {
                uuid: uuid.clone(),
                folder_id,
                disk_name: rel_path.to_string(),
                ext: crate::fs::paths::extension_of(rel_path),
                orig_name: rel_path.to_string(),
                hash,
                size_bytes: bytes.len() as i64,
                mtime: 0,
                kind: "image".to_string(),
                width: Some(10),
                height: Some(10),
                duration_ms: None,
                codec: None,
                bitrate: None,
                captured_at: Some(0),
                captured_src: Some("mtime".to_string()),
            },
        )
        .unwrap();
        (id, uuid)
    }

    fn open_db(root: &Path) -> (LibraryPaths, Connection) {
        let paths = LibraryPaths::new(root);
        paths.ensure_dirs().unwrap();
        let mut conn = db::open(&paths.db_path()).unwrap();
        db::migrate(&mut conn).unwrap();
        (paths, conn)
    }

    #[test]
    fn renames_files_to_uuid_names_and_updates_disk_name() {
        let root = scratch("import-basic");
        std::fs::create_dir_all(root.join("People/Ana")).unwrap();
        std::fs::write(root.join("People/Ana/holiday.jpg"), b"hello world").unwrap();
        std::fs::write(root.join("cover.png"), b"cover bytes").unwrap();

        let (paths, conn) = open_db(&root);
        let root_id = db::folders::upsert(&conn, "", "Library").unwrap();
        let ana_id = db::folders::upsert(&conn, "people/ana", "Ana").unwrap();

        let (holiday_id, holiday_uuid) =
            index_one(&conn, ana_id, "holiday.jpg", b"hello world");
        let (cover_id, cover_uuid) = index_one(&conn, root_id, "cover.png", b"cover bytes");

        let scanned = scan(&conn).unwrap();
        assert_eq!(scanned.total_items, 2);
        assert_eq!(scanned.already_renamed, 0);
        assert_eq!(scanned.to_rename, 2);

        let preview = dry_run(&conn, 10).unwrap();
        assert_eq!(preview.to_rename, 2);
        assert_eq!(preview.sample.len(), 2);
        // Nothing on disk yet.
        assert!(root.join("People/Ana/holiday.jpg").is_file());

        let report = execute(&paths, &conn, &mut |_| {}).unwrap();
        assert_eq!(report.renamed, 2);
        assert!(report.errors.is_empty());

        assert!(
            root.join(format!("People/Ana/{holiday_uuid}.jpg")).is_file(),
            "renamed on disk"
        );
        assert!(!root.join("People/Ana/holiday.jpg").exists());
        assert!(root.join(format!("{cover_uuid}.png")).is_file());

        let items = db::items::list(&conn, &Scope::default()).unwrap();
        assert_eq!(items.len(), 2);

        let after = scan(&conn).unwrap();
        assert_eq!(after.already_renamed, 2);
        assert_eq!(after.to_rename, 0);

        // orig_name survives, untouched, as searchable metadata.
        let orig: String = conn
            .query_row(
                "SELECT orig_name FROM item WHERE id = ?1",
                rusqlite::params![holiday_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(orig, "holiday.jpg");

        let disk: String = conn
            .query_row(
                "SELECT disk_name FROM item WHERE id = ?1",
                rusqlite::params![cover_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(disk, format!("{cover_uuid}.png"));
    }

    #[test]
    fn thumbnails_survive_the_rename() {
        let root = scratch("import-thumbs");
        std::fs::write(root.join("photo.png"), b"pixels").unwrap();

        let (paths, conn) = open_db(&root);
        let root_id = db::folders::upsert(&conn, "", "Library").unwrap();
        let (_, uuid) = index_one(&conn, root_id, "photo.png", b"pixels");

        // A thumbnail is keyed by uuid, issued at index time — write one the
        // way the thumb job would, before the rename runs.
        let thumb_path = paths.thumb_path(&uuid);
        std::fs::create_dir_all(thumb_path.parent().unwrap()).unwrap();
        std::fs::write(&thumb_path, b"fake webp bytes").unwrap();

        execute(&paths, &conn, &mut |_| {}).unwrap();

        assert!(
            thumb_path.is_file(),
            "the cache path is derived from uuid alone, so the rename must not disturb it"
        );
        assert_eq!(
            std::fs::read(&thumb_path).unwrap(),
            b"fake webp bytes",
            "and its contents are untouched"
        );
    }

    #[test]
    fn writes_a_disaster_recovery_record_that_can_reconstruct_original_names() {
        let root = scratch("import-jsonl");
        std::fs::write(root.join("holiday.jpg"), b"data one").unwrap();
        std::fs::write(root.join("cover.png"), b"data two").unwrap();

        let (paths, conn) = open_db(&root);
        let root_id = db::folders::upsert(&conn, "", "Library").unwrap();
        index_one(&conn, root_id, "holiday.jpg", b"data one");
        index_one(&conn, root_id, "cover.png", b"data two");

        execute(&paths, &conn, &mut |_| {}).unwrap();

        let jsonl = std::fs::read_to_string(paths.jsonl_path()).unwrap();
        let records: Vec<JsonlRecord> = jsonl
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(records.len(), 2);

        // No tooling ships to reverse this — per docs/DESIGN.md#first-import,
        // "No reversal" — but a human with a text editor always has enough
        // information to put a file back by hand if they ever need to.
        for record in &records {
            let folder_abs = paths.to_abs(&record.folder_rel).unwrap();
            std::fs::rename(
                folder_abs.join(&record.new_name),
                folder_abs.join(&record.orig_name),
            )
            .unwrap();
        }

        assert!(root.join("holiday.jpg").is_file());
        assert!(root.join("cover.png").is_file());
        assert_eq!(std::fs::read(root.join("holiday.jpg")).unwrap(), b"data one");
        assert_eq!(std::fs::read(root.join("cover.png")).unwrap(), b"data two");
    }

    #[test]
    fn resumes_after_a_crash_between_the_disk_rename_and_the_db_commit() {
        let root = scratch("import-resume");
        std::fs::write(root.join("a.png"), b"aaa").unwrap();
        std::fs::write(root.join("b.png"), b"bbb").unwrap();

        let (paths, conn) = open_db(&root);
        let root_id = db::folders::upsert(&conn, "", "Library").unwrap();
        let (_, uuid_a) = index_one(&conn, root_id, "a.png", b"aaa");
        index_one(&conn, root_id, "b.png", b"bbb");

        // Simulate the exact crash window: the file already renamed on disk,
        // but the database still holds the old name because the transaction
        // never committed.
        std::fs::rename(root.join("a.png"), root.join(format!("{uuid_a}.png"))).unwrap();

        let before = scan(&conn).unwrap();
        assert_eq!(before.already_renamed, 0, "db does not know about a.png yet");

        let report = execute(&paths, &conn, &mut |_| {}).unwrap();
        assert_eq!(report.renamed, 2, "both items end up recorded as renamed");
        assert!(report.errors.is_empty());

        let after = scan(&conn).unwrap();
        assert_eq!(after.already_renamed, 2);
        assert_eq!(after.to_rename, 0);
        assert!(root.join(format!("{uuid_a}.png")).is_file());
    }

    #[test]
    fn verify_confirms_hashes_and_counts() {
        let root = scratch("import-verify");
        std::fs::write(root.join("a.png"), b"aaa").unwrap();

        let (paths, conn) = open_db(&root);
        let root_id = db::folders::upsert(&conn, "", "Library").unwrap();
        index_one(&conn, root_id, "a.png", b"aaa");
        execute(&paths, &conn, &mut |_| {}).unwrap();

        let report = verify(&paths, &conn, 10).unwrap();
        assert_eq!(report.sample_checked, 1);
        assert!(report.mismatches.is_empty());
        assert!(report.missing.is_empty());
        assert_eq!(report.count_total, report.count_renamed);
    }

    #[test]
    fn a_second_run_on_a_fully_imported_library_is_a_no_op() {
        let root = scratch("import-idempotent");
        std::fs::write(root.join("a.png"), b"aaa").unwrap();

        let (paths, conn) = open_db(&root);
        let root_id = db::folders::upsert(&conn, "", "Library").unwrap();
        index_one(&conn, root_id, "a.png", b"aaa");
        execute(&paths, &conn, &mut |_| {}).unwrap();

        let second = execute(&paths, &conn, &mut |_| {}).unwrap();
        assert_eq!(second.renamed, 0);
        assert_eq!(second.already_done, 1);
        assert!(second.errors.is_empty());
    }

    #[test]
    fn execute_marks_the_library_imported() {
        let root = scratch("import-marks-imported");
        std::fs::write(root.join("a.png"), b"aaa").unwrap();

        let (paths, conn) = open_db(&root);
        let root_id = db::folders::upsert(&conn, "", "Library").unwrap();
        index_one(&conn, root_id, "a.png", b"aaa");

        assert!(db::settings::imported_at(&conn).unwrap().is_none());
        execute(&paths, &conn, &mut |_| {}).unwrap();
        assert!(db::settings::imported_at(&conn).unwrap().is_some());
    }

    #[test]
    fn execute_marks_imported_even_with_nothing_to_rename() {
        let root = scratch("import-marks-imported-empty");
        let (paths, conn) = open_db(&root);

        assert!(db::settings::imported_at(&conn).unwrap().is_none());
        let report = execute(&paths, &conn, &mut |_| {}).unwrap();
        assert_eq!(report.renamed, 0);
        assert!(db::settings::imported_at(&conn).unwrap().is_some());
    }

    #[test]
    fn rename_on_arrival_renames_journals_and_preserves_orig_name() {
        let root = scratch("import-arrival");
        std::fs::write(root.join("newphoto.jpg"), b"fresh bytes").unwrap();

        let (paths, conn) = open_db(&root);
        let root_id = db::folders::upsert(&conn, "", "Library").unwrap();
        let (item_id, uuid) = index_one(&conn, root_id, "newphoto.jpg", b"fresh bytes");

        rename_on_arrival(&paths, &conn, item_id, &Suppressor::default()).unwrap();

        assert!(root.join(format!("{uuid}.jpg")).is_file());
        assert!(!root.join("newphoto.jpg").exists());

        let disk_name: String = conn
            .query_row(
                "SELECT disk_name FROM item WHERE id = ?1",
                rusqlite::params![item_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(disk_name, format!("{uuid}.jpg"));

        let orig_name: String = conn
            .query_row(
                "SELECT orig_name FROM item WHERE id = ?1",
                rusqlite::params![item_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(orig_name, "newphoto.jpg", "searchable forever, per DATA-MODEL.md");

        let journal_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM journal WHERE op = 'rename'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(journal_count, 1, "unlike the bulk wizard, arrivals are journalled");
    }

    #[test]
    fn rename_on_arrival_does_not_rejournal_an_already_named_item() {
        let root = scratch("import-arrival-idempotent");
        std::fs::write(root.join("photo.jpg"), b"bytes").unwrap();

        let (paths, conn) = open_db(&root);
        let root_id = db::folders::upsert(&conn, "", "Library").unwrap();
        let (item_id, _uuid) = index_one(&conn, root_id, "photo.jpg", b"bytes");

        rename_on_arrival(&paths, &conn, item_id, &Suppressor::default()).unwrap();
        rename_on_arrival(&paths, &conn, item_id, &Suppressor::default()).unwrap();

        let journal_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM journal", [], |r| r.get(0))
            .unwrap();
        assert_eq!(journal_count, 1);
    }

    /// PLAN.md decision 19: any new query path must be checked against a
    /// synthetic library at scale, not just a handful of files. Ignored by
    /// default because it writes 100k real files — run explicitly with
    /// `cargo test --release scale_check_100k_items -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn scale_check_100k_items() {
        const N: i64 = 100_000;
        let root = scratch("import-scale");

        let (paths, conn) = open_db(&root);
        let root_id = db::folders::upsert(&conn, "", "Library").unwrap();

        let setup_start = std::time::Instant::now();
        db::begin_batch(&conn).unwrap();
        for i in 0..N {
            if i % 5000 == 0 && i > 0 {
                db::commit_batch(&conn).unwrap();
                db::begin_batch(&conn).unwrap();
            }
            let name = format!("item_{i}.bin");
            let bytes = format!("payload {i}").into_bytes();
            std::fs::write(root.join(&name), &bytes).unwrap();
            index_one(&conn, root_id, &name, &bytes);
        }
        db::commit_batch(&conn).unwrap();
        println!("setup: {:?} for {N} items", setup_start.elapsed());

        let scan_start = std::time::Instant::now();
        let scanned = scan(&conn).unwrap();
        println!("scan: {:?}", scan_start.elapsed());
        assert_eq!(scanned.total_items, N);
        assert_eq!(scanned.to_rename, N);

        let dry_start = std::time::Instant::now();
        let preview = dry_run(&conn, 20).unwrap();
        println!("dry_run: {:?}", dry_start.elapsed());
        assert_eq!(preview.to_rename, N);
        assert_eq!(preview.sample.len(), 20);

        let exec_start = std::time::Instant::now();
        let report = execute(&paths, &conn, &mut |_| {}).unwrap();
        println!("execute: {:?} for {N} items", exec_start.elapsed());
        assert_eq!(report.renamed, N);
        assert!(report.errors.is_empty());

        let verify_start = std::time::Instant::now();
        let verified = verify(&paths, &conn, 200).unwrap();
        println!("verify: {:?}", verify_start.elapsed());
        assert_eq!(verified.sample_checked, 200);
        assert!(verified.mismatches.is_empty());
        assert_eq!(verified.count_total, verified.count_renamed);
    }
}
