//! The M1.7 startup flow (`prepare_import` / `execute_prepared_import` /
//! `cancel_prepared_import`) — filesystem-only, run before a library is ever
//! opened. See `fs::import` for the actual logic; these are the thin IPC
//! shell around it.
//!
//! **PLAN.md §M2.6 removed this file's other half** — the database-backed
//! scan/dry-run/execute/verify commands behind Settings → *Normalise
//! filenames*. See `fs::import`'s module docs for why that repair action's
//! whole premise (a backlog of items not yet renamed to `<uuid>.<ext>`)
//! stopped being able to exist.

use std::path::PathBuf;

use tauri::{AppHandle, Emitter, State};

use crate::commands::blocking;
use crate::error::{AppError, Result};
use crate::fs::import::{self, FsExecuteReport, ImportProgress, ReviewReport};
use crate::AppState;

const PROGRESS_EVENT: &str = "import-progress";

/// The Choose-folder step's follow-through. Scans the picked folder directly
/// — no job queue, no thumbnail — beyond the two trivial cases
/// `fs::import::prepare` resolves on the spot (already imported, or nothing
/// found to import).
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

/// Mirror the tree `prepare_import` staged into folder records and indexed
/// items, then mark the library imported. Refuses to run without an explicit
/// backup acknowledgement — the one interruption that stays, now that there
/// is no reversal tooling to fall back on. See docs/DESIGN.md#first-import.
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
