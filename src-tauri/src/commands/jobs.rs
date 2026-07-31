use tauri::State;

use crate::commands::blocking;
use crate::db;
use crate::db::jobs::Failure;
use crate::error::Result;
use crate::jobs::{self, Progress};
use crate::AppState;

/// Queue a walk of the library. Idempotent: a second call while one is already
/// queued or running does nothing.
#[tauri::command]
pub async fn start_index(state: State<'_, AppState>) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        jobs::enqueue_index(&conn)
    })
    .await
}

/// Pull rather than push. The queue emits `job-progress` on a tick while it is
/// working; this is for the first paint and for reconnecting after the queue
/// has gone quiet.
#[tauri::command]
pub async fn index_progress(state: State<'_, AppState>) -> Result<Progress> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        library.queue().inner().progress(&conn)
    })
    .await
}

/// What failed, per file, with the error the decoder or the tool actually
/// produced. A count on its own is not something anyone can act on.
#[tauri::command]
pub async fn index_failures(state: State<'_, AppState>) -> Result<Vec<Failure>> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::jobs::failures(&conn)
    })
    .await
}

#[tauri::command]
pub async fn retry_failed_jobs(state: State<'_, AppState>) -> Result<usize> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::jobs::retry_failed(&conn)
    })
    .await
}
