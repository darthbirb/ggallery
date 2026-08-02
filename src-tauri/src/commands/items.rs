use tauri::State;

use crate::commands::blocking;
use crate::db;
use crate::db::items::{GridItem, Scope};
use crate::error::{AppError, Result};
use crate::fs::paths::normalise_rel;
use crate::fs::relocate::MoveItemsReport;
use crate::fs::trash::TrashItemsReport;
use crate::AppState;

/// Every row the grid will draw, in one call.
///
/// The M0 spike established that a 100k-item manifest parses in ~440ms and
/// lays out in under 20ms, so the grid does not page: it takes the whole list,
/// hands it to the layout worker, and virtualises from there. Paging would buy
/// nothing and cost the scrubber its knowledge of the full date range.
#[tauri::command]
pub async fn list_items(
    state: State<'_, AppState>,
    folder: Option<String>,
    recursive: bool,
) -> Result<Vec<GridItem>> {
    let library = state.library()?;
    let scope = Scope {
        folder: folder.map(|rel| normalise_rel(&rel)),
        recursive,
    };

    blocking(move || {
        let conn = library.conn()?;
        db::items::list(&conn, &scope)
    })
    .await
}

/// One item, in full — the pane's Preview mode. `path` is filled in here
/// rather than in `db/`, because resolving library-relative to absolute is
/// `fs::paths`'s job and nothing else's.
#[tauri::command]
pub async fn get_item(state: State<'_, AppState>, item_id: i64) -> Result<db::items::ItemDetail> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        let mut detail = db::items::detail(&conn, item_id)?
            .ok_or_else(|| AppError::invalid("item no longer exists"))?;
        detail.path = library
            .paths
            .item_path(&detail.folder_rel, &detail.disk_name)?
            .to_string_lossy()
            .to_string();
        Ok(detail)
    })
    .await
}

/// Favourite is binary and first-class (PLAN.md decision 12), and acts on the
/// whole selection.
#[tauri::command]
pub async fn set_items_favorite(
    state: State<'_, AppState>,
    item_ids: Vec<i64>,
    favorite: bool,
) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::items::set_favorite(&conn, &item_ids, favorite)
    })
    .await
}

// --- M2.1: move, delete, and the OS-integration escape hatches an app that
// renames everything to a UUID owes the user — see docs/DESIGN.md "Item
// operations".

#[tauri::command]
pub async fn move_items(
    state: State<'_, AppState>,
    item_ids: Vec<i64>,
    dest_folder_id: i64,
) -> Result<MoveItemsReport> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        let batch = db::journal::new_batch();
        crate::fs::relocate::move_items(&library.paths, &conn, &item_ids, dest_folder_id, &batch)
    })
    .await
}

#[tauri::command]
pub async fn delete_items(
    state: State<'_, AppState>,
    item_ids: Vec<i64>,
) -> Result<TrashItemsReport> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        let batch = db::journal::new_batch();
        crate::fs::trash::trash_items(&library.paths, &conn, &item_ids, &batch)
    })
    .await
}

fn item_abs_path(library: &crate::Library, item_id: i64) -> Result<std::path::PathBuf> {
    let conn = library.conn()?;
    let item = db::items::rename_target(&conn, item_id)?
        .ok_or_else(|| AppError::invalid("item no longer exists"))?;
    library.paths.item_path(&item.folder_rel, &item.disk_name)
}

#[tauri::command]
pub async fn reveal_item(app: tauri::AppHandle, state: State<'_, AppState>, item_id: i64) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let path = item_abs_path(&library, item_id)?;
        use tauri_plugin_opener::OpenerExt;
        app.opener()
            .reveal_item_in_dir(path)
            .map_err(|err| AppError::invalid(err.to_string()))
    })
    .await
}

#[tauri::command]
pub async fn open_item(app: tauri::AppHandle, state: State<'_, AppState>, item_id: i64) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let path = item_abs_path(&library, item_id)?;
        use tauri_plugin_opener::OpenerExt;
        app.opener()
            .open_path(path.to_string_lossy().to_string(), None::<String>)
            .map_err(|err| AppError::invalid(err.to_string()))
    })
    .await
}

/// Real Windows `CF_HDROP` file copy — pasting into Explorer or elsewhere
/// produces the actual file. Known limitation: the pasted file carries its
/// on-disk UUID name; see `fs::clipboard`'s module doc.
#[tauri::command]
pub async fn copy_item_file(state: State<'_, AppState>, item_id: i64) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let path = item_abs_path(&library, item_id)?;
        crate::fs::clipboard::copy_file(&path)
    })
    .await
}

#[tauri::command]
pub async fn copy_item_path(state: State<'_, AppState>, item_id: i64) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let path = item_abs_path(&library, item_id)?;
        crate::fs::clipboard::copy_text(&path.to_string_lossy())
    })
    .await
}
