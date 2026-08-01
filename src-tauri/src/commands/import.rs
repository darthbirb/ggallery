//! The M1.5 first-import wizard's commands. See `fs::import` for the actual
//! scan/dry-run/execute/verify logic — these are the thin IPC shell around it.

use tauri::{AppHandle, Emitter, State};

use crate::commands::blocking;
use crate::db;
use crate::error::{AppError, Result};
use crate::fs::import::{self, DryRunReport, ExecuteReport, ImportProgress, ScanReport, VerifyReport};
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
