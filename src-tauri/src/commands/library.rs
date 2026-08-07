use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::commands::blocking;
use crate::config::Config;
use crate::db;
use crate::db::folders::FolderNode;
use crate::error::{AppError, Result};
use crate::fs::lowercase_migration::LowercaseMergeReport;
use crate::fs::paths::same_dir;
use crate::{AppState, Library};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryInfo {
    /// Absolute, and deliberately so — this is the one path the app knows in
    /// absolute terms, and it never reaches the database.
    pub root: String,
    pub name: String,
    /// Absolute cache roots. Item rows carry `ab/cd/<uuid>.webp` relative to
    /// these, so the frontend joins rather than derives.
    pub thumbs_dir: String,
    pub sprites_dir: String,
    pub item_count: i64,
    pub folder_count: i64,
    /// Which ffmpeg is in use, if any. Without one, videos are indexed but get
    /// no poster frame and no scrub strip.
    pub ffmpeg: Option<String>,
    /// Set only on the `open` that actually ran the lowercase fold-and-merge
    /// (decision 31) and only if it merged something — surfaced
    /// once, silently absent otherwise, same as `verifyIssue`.
    pub lowercase_merge_report: Option<LowercaseMergeReport>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStatus {
    pub info: Option<LibraryInfo>,
    /// The root remembered in `gallery.config.json`, so the app can offer to
    /// reopen it without a picker.
    pub remembered: Option<String>,
}

#[tauri::command]
pub async fn open_library(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<LibraryInfo> {
    let root = PathBuf::from(path);

    // Checked before anything is closed: picking a bad folder must not cost
    // the user the library they already had open.
    if !root.is_dir() {
        return Err(AppError::invalid(format!(
            "{} is not a folder",
            root.display()
        )));
    }

    // Switching to the library that is already open would fail on its own
    // single-instance lock: the new handle is acquired before the old one is
    // released. Nothing to do, so say so cheaply.
    if let Some(current) = state.current() {
        if same_dir(current.paths.root(), &root) {
            return blocking(move || describe(&current)).await;
        }
        // Release the lock, stop the workers and checkpoint before the new
        // library starts competing for the same threads.
        if let Some(previous) = state.take() {
            blocking(move || {
                previous.close();
                Ok(())
            })
            .await?;
        }
    }

    let library = Arc::new(blocking(move || Library::open(app, root)).await?);

    let info = {
        let library = Arc::clone(&library);
        blocking(move || describe(&library)).await?
    };

    Config::set_library_root(library.paths.root())?;
    state.set(library)?;
    Ok(info)
}

#[tauri::command]
pub async fn current_library(state: State<'_, AppState>) -> Result<LibraryStatus> {
    let remembered = Config::load().library_root;
    let Some(library) = state.current() else {
        return Ok(LibraryStatus {
            info: None,
            remembered,
        });
    };

    let info = blocking(move || describe(&library)).await?;
    Ok(LibraryStatus {
        info: Some(info),
        remembered,
    })
}

#[tauri::command]
pub async fn close_library(state: State<'_, AppState>) -> Result<()> {
    let Some(library) = state.take() else {
        return Ok(());
    };
    blocking(move || {
        library.close();
        Ok(())
    })
    .await
}

/// The sidebar tree. Read-only in M1 — folder records gain titles, archetypes
/// and tags in M2.
#[tauri::command]
pub async fn folder_tree(state: State<'_, AppState>) -> Result<Vec<FolderNode>> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::tree(&conn)
    })
    .await
}

// --- interface preferences (M2.5a) ---------------------------------------
//
// Panel widths, folded and expanded states, the accent. They live in
// `gallery.config.json` next to the exe, alongside window geometry and for
// the same reasons: they are about this installation, not about the library,
// and a library copied to another machine should not drag them along.

#[tauri::command]
pub async fn ui_prefs() -> Result<Option<serde_json::Value>> {
    blocking(move || Ok(Config::load().ui)).await
}

#[tauri::command]
pub async fn set_ui_prefs(prefs: serde_json::Value) -> Result<()> {
    blocking(move || Config::set_ui(prefs)).await
}

fn describe(library: &Library) -> Result<LibraryInfo> {
    let conn = library.conn()?;
    let paths = &library.paths;

    let lowercase_merge_report = library.take_lowercase_report().filter(|report| {
        !report.tags_merged.is_empty() || !report.folders_merged.is_empty()
    });

    Ok(LibraryInfo {
        root: paths.root().to_string_lossy().to_string(),
        name: paths
            .root()
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "Library".to_string()),
        thumbs_dir: paths.thumbs_dir().to_string_lossy().to_string(),
        sprites_dir: paths.sprites_dir().to_string_lossy().to_string(),
        item_count: db::items::count(&conn)?,
        folder_count: db::folders::count(&conn)?,
        ffmpeg: library.tools.describe(),
        lowercase_merge_report,
    })
}
