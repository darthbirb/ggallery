//! Import commands: the M1.7 startup flow (`prepare_import` /
//! `execute_prepared_import` / `cancel_prepared_import`), filesystem-only and
//! run before a library is ever opened, plus the original database-backed
//! scan/dry-run/execute/verify for the repair case — Settings → Normalise
//! filenames, run against an already-open library. See `fs::import` for the
//! actual logic; these are the thin IPC shell around it.

use std::path::PathBuf;

use tauri::{AppHandle, Emitter, State};

use crate::commands::blocking;
use crate::db;
use crate::error::{AppError, Result};
use crate::fs::import::{
    self, DryRunReport, ExecuteReport, FsExecuteReport, ImportProgress, ReviewReport, ScanReport,
    VerifyReport,
};
use crate::AppState;

const PROGRESS_EVENT: &str = "import-progress";

#[tauri::command]
pub async fn scan_import(state: State<'_, AppState>) -> Result<ScanReport> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        import::scan(&conn)
    })
    .await
}

#[tauri::command]
pub async fn dry_run_import(state: State<'_, AppState>, sample_size: i64) -> Result<DryRunReport> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        import::dry_run(&conn, sample_size)
    })
    .await
}

/// Rename everything not yet renamed. Refuses to run without an explicit
/// backup acknowledgement, and while the index queue is still busy — renaming
/// out from under a hash or thumbnail job in flight is not a scenario worth
/// supporting when simply waiting for the queue to go idle costs nothing.
#[tauri::command]
pub async fn execute_import(
    app: AppHandle,
    state: State<'_, AppState>,
    confirmed_backup: bool,
) -> Result<ExecuteReport> {
    if !confirmed_backup {
        return Err(AppError::invalid(
            "cannot rename without confirming a backup exists",
        ));
    }

    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        let busy = library.queue().inner().progress(&conn)?;
        if busy.phase != "idle" {
            return Err(AppError::invalid(
                "the index is still running — wait for it to finish before renaming",
            ));
        }

        let result = import::execute(&library.paths, &conn, &mut |progress: &ImportProgress| {
            let _ = app.emit(PROGRESS_EVENT, progress);
        });
        if result.is_err() {
            db::rollback_batch(&conn);
        }
        result
    })
    .await
}

#[tauri::command]
pub async fn verify_import(state: State<'_, AppState>, sample_size: i64) -> Result<VerifyReport> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        import::verify(&library.paths, &conn, sample_size)
    })
    .await
}

/// Mark a library imported without running the wizard — for the case where
/// `scan_import` reports nothing to rename at all (an empty library, or one
/// that was already all UUID-named). Nothing destructive happens, so this
/// skips the backup gate entirely.
#[tauri::command]
pub async fn mark_imported(state: State<'_, AppState>) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::settings::mark_imported(&conn)
    })
    .await
}

// --- M1.7 startup flow ------------------------------------------------
//
// Filesystem-only: no `Library` exists yet when these run, so they work
// directly against `AppState::pending_import` rather than `state.library()`.

/// The Choose-folder step's follow-through. Scans the picked folder directly
/// — no job queue, no thumbnail, nothing written into `.gallery/` beyond the
/// two trivial cases `fs::import::prepare` resolves on the spot (already
/// imported, or nothing to rename).
#[tauri::command]
pub async fn prepare_import(state: State<'_, AppState>, path: String) -> Result<ReviewReport> {
    let root = PathBuf::from(path);
    let (report, pending) = blocking(move || import::prepare(&root)).await?;
    match pending {
        Some(pending) => state.set_pending_import(pending)?,
        None => state.clear_pending_import()?,
    }
    Ok(report)
}

/// Rename everything `prepare_import` staged, then mark the library imported.
/// Refuses to run without an explicit backup acknowledgement — the one
/// interruption that stays, now that there is no reversal tooling to fall
/// back on. See docs/DESIGN.md#first-import.
#[tauri::command]
pub async fn execute_prepared_import(
    app: AppHandle,
    state: State<'_, AppState>,
    confirmed_backup: bool,
) -> Result<FsExecuteReport> {
    if !confirmed_backup {
        return Err(AppError::invalid(
            "cannot rename without confirming a backup exists",
        ));
    }

    let pending = state
        .take_pending_import()?
        .ok_or_else(|| AppError::invalid("nothing staged to import — choose a folder first"))?;

    blocking(move || {
        import::execute_prepared(&pending, &mut |progress: &ImportProgress| {
            let _ = app.emit(PROGRESS_EVENT, progress);
        })
    })
    .await
}

/// Review → Cancel returns to the folder picker. Nothing destructive has
/// happened yet, so this only needs to discard the staged plan.
#[tauri::command]
pub async fn cancel_prepared_import(state: State<'_, AppState>) -> Result<()> {
    state.clear_pending_import()
}
