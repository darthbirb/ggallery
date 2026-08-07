//! Move, trash, restore, undo — the commands docs/NOTES.md assigns to
//! this file. M4 owns most of it; M2.5a needs one piece of it early.
//!
//! Locked decision 23 requires every destructive action to end in a toast
//! with an Undo button, and M2.1 shipped journalled moves and deletes with
//! nothing behind them. `undo_batch` is that button's command: it reverses
//! one named batch. The `Ctrl+Z` stack replayer — walking the journal
//! backwards with no batch id to start from — is still M4's.

use tauri::State;

use crate::commands::blocking;
use crate::error::Result;
use crate::fs::undo::UndoReport;
use crate::AppState;

#[tauri::command]
pub async fn undo_batch(state: State<'_, AppState>, batch_id: String) -> Result<UndoReport> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        crate::fs::undo::undo_batch(&library.paths, &conn, &batch_id)
    })
    .await
}
