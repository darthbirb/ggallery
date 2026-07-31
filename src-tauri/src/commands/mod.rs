//! Every `#[tauri::command]` in the application lives under this module and
//! nowhere else.
//!
//! Commands are a thin shell: validate, hand off, map errors. They contain no
//! business logic, and every one of them is `async fn` with the real work
//! inside `spawn_blocking` — a synchronous command blocks the native window
//! message pump and Windows marks the app "Not Responding".

pub mod items;
pub mod jobs;
pub mod library;

use crate::error::{AppError, Result};

/// Run work on the blocking pool and unwrap the join. The one place commands
/// are allowed to touch threads.
pub async fn blocking<T, F>(work: F) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|err| AppError::invalid(format!("background task failed: {err}")))?
}
