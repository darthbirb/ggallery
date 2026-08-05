//! `fs::shard`'s storage migration (PLAN.md §M2.6), as a startup-flow wizard —
//! same shape as `commands::import`'s M1.7 flow: filesystem-and-database work
//! against the *pre-migration* (v7) schema, run before `Library::open` ever
//! calls `db::migrate`, driven by a `path` the frontend already has from the
//! folder picker rather than a `Library` that doesn't exist yet.
//!
//! `open_library` surfaces `AppError::NeedsStorageMigration` (`kind ===
//! "needs-storage-migration"`) when this is owed; the frontend switches to
//! these commands, then calls `open_library` again once `execute` has
//! verified clean.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::commands::blocking;
use crate::db;
use crate::error::{AppError, Result};
use crate::fs::lowercase_migration::FolderMerge;
use crate::fs::paths::LibraryPaths;
use crate::fs::shard::{self, DryRunReport, ExecuteReport, MigrationProgress, VerifyReport};

const PROGRESS_EVENT: &str = "storage-migration-progress";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewReport {
    /// Sibling-title collisions left over from before decision 31's
    /// write-time fold shipped, resolved (by physically merging directories)
    /// before the manifest below was ever written — surfaced here rather
    /// than silently, same as `LibraryInfo.lowercaseMergeReport`.
    pub folders_merged: Vec<FolderMerge>,
    pub dry_run: DryRunReport,
}

fn open_pre_migration(path: &str) -> Result<(LibraryPaths, rusqlite::Connection)> {
    let root = PathBuf::from(path);
    if !root.is_dir() {
        return Err(AppError::invalid(format!("{} is not a folder", root.display())));
    }
    let paths = LibraryPaths::new(root);
    let conn = db::open(&paths.db_path())?;
    Ok((paths, conn))
}

#[tauri::command]
pub async fn needs_storage_migration(path: String) -> Result<bool> {
    blocking(move || {
        let (_, conn) = open_pre_migration(&path)?;
        db::needs_storage_migration(&conn)
    })
    .await
}

/// Resolves any leftover pre-decision-31 directory collision first (a real
/// existing library may never have had the chance — see
/// `fs::lowercase_migration`'s module docs), then writes the complete
/// `library.jsonl` manifest, fsynced, then reports the dry run — the Review
/// screen's whole content. Nothing is written to `files/` yet.
#[tauri::command]
pub async fn prepare_storage_migration(path: String) -> Result<ReviewReport> {
    blocking(move || {
        let (paths, conn) = open_pre_migration(&path)?;
        let folders_merged = crate::fs::lowercase_migration::merge_folders(&paths, &conn)?;
        shard::write_manifest(&paths, &conn)?;
        let dry_run = shard::dry_run(&paths, &conn)?;
        Ok(ReviewReport { folders_merged, dry_run })
    })
    .await
}

/// Move every file to its shard destination. Refuses without an explicit
/// backup acknowledgement — the one interruption that carries weight, same
/// reasoning as the M1.7 import flow's own gate.
#[tauri::command]
pub async fn execute_storage_migration(
    app: AppHandle,
    path: String,
    confirmed_backup: bool,
) -> Result<ExecuteReport> {
    if !confirmed_backup {
        return Err(AppError::invalid(
            "cannot migrate without confirming a backup exists",
        ));
    }
    blocking(move || {
        let (paths, conn) = open_pre_migration(&path)?;
        shard::execute(&paths, &conn, &mut |progress: &MigrationProgress| {
            let _ = app.emit(PROGRESS_EVENT, progress);
        })
    })
    .await
}

/// Confirms every item resolves to a file at its shard destination and, only
/// on a clean result, marks the migration verified — the marker
/// `Library::open`'s gate checks before it will let schema migration 008
/// run. A dirty result (anything missing) leaves the marker unset so the
/// wizard can be re-run.
#[tauri::command]
pub async fn verify_storage_migration(path: String, full_hash_sweep: bool) -> Result<VerifyReport> {
    blocking(move || {
        let (paths, conn) = open_pre_migration(&path)?;
        let report = shard::verify(&paths, &conn, full_hash_sweep)?;
        if report.missing.is_empty() && report.hash_mismatches.is_empty() {
            db::settings::mark_storage_migration_verified(&conn)?;
        }
        Ok(report)
    })
    .await
}

/// How many now-empty directories the migration left behind — a report-only
/// figure, offered once verification has passed. Removal is a separate,
/// explicit action; nothing here deletes anything.
#[tauri::command]
pub async fn count_empty_directories(path: String) -> Result<i64> {
    blocking(move || {
        let (paths, _) = open_pre_migration(&path)?;
        shard::count_empty_dirs(&paths)
    })
    .await
}
