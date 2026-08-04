//! Folder CRUD, tags, archetypes, status, cover. M2's data surfaced for the
//! folder header — see `features/folder` on the frontend.

use tauri::State;

use crate::commands::blocking;
use crate::db;
use crate::db::folders::{ArchetypeInfo, FolderDetail, FolderStatusDef};
use crate::error::{AppError, Result};
use crate::AppState;

#[tauri::command]
pub async fn get_folder(state: State<'_, AppState>, id: i64) -> Result<FolderDetail> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::get_detail(&conn, id)?.ok_or_else(|| AppError::invalid("folder not found"))
    })
    .await
}

/// A folder has one name (PLAN.md §M2.2) — this both updates the title and,
/// when the sanitised result differs from what's on disk, renames the
/// directory to match. There is no separate rename-directory command any
/// more.
/// Returns the journal batch, so the toast that follows can offer Undo.
#[tauri::command]
pub async fn set_folder_title(
    state: State<'_, AppState>,
    id: i64,
    title: String,
) -> Result<String> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        let suppressor = library.queue().inner().suppressor.clone();
        let batch = db::journal::new_batch();
        crate::fs::relocate::retitle_folder(
            &library.paths,
            &conn,
            &suppressor,
            id,
            &title,
            &batch,
        )?;
        Ok(batch)
    })
    .await
}

/// Choose the folder's cover, or clear it back to the automatic pick.
#[tauri::command]
pub async fn set_folder_cover(
    state: State<'_, AppState>,
    id: i64,
    item_id: Option<i64>,
) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::set_cover(&conn, id, item_id)
    })
    .await
}

#[tauri::command]
pub async fn set_folder_status(state: State<'_, AppState>, id: i64, status: String) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::set_status(&conn, id, &status)
    })
    .await
}

#[tauri::command]
pub async fn set_folder_favorite(
    state: State<'_, AppState>,
    id: i64,
    favorite: bool,
) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::set_favorite(&conn, id, favorite)
    })
    .await
}

#[tauri::command]
pub async fn set_folder_notes(
    state: State<'_, AppState>,
    id: i64,
    notes: Option<String>,
) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::set_notes(&conn, id, notes.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn apply_folder_archetype(
    state: State<'_, AppState>,
    id: i64,
    archetype_id: i64,
) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::apply_archetype(&conn, id, archetype_id)
    })
    .await
}

#[tauri::command]
pub async fn remove_folder_archetype(state: State<'_, AppState>, id: i64) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::clear_archetype(&conn, id)
    })
    .await
}

#[tauri::command]
pub async fn set_folder_label(
    state: State<'_, AppState>,
    id: i64,
    key: String,
    value: String,
) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::set_label(&conn, id, &key, &value)
    })
    .await
}

#[tauri::command]
pub async fn add_folder_flag(state: State<'_, AppState>, id: i64, value: String) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::add_flag(&conn, id, &value)
    })
    .await
}

#[tauri::command]
pub async fn remove_folder_tag(state: State<'_, AppState>, id: i64, tag_id: i64) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::remove_tag(&conn, id, tag_id)
    })
    .await
}

#[tauri::command]
pub async fn list_folder_statuses(state: State<'_, AppState>) -> Result<Vec<FolderStatusDef>> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::list_statuses(&conn)
    })
    .await
}

#[tauri::command]
pub async fn list_archetypes(state: State<'_, AppState>) -> Result<Vec<ArchetypeInfo>> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::list_archetypes(&conn)
    })
    .await
}

// --- folder lifecycle: create, rename directory, move, delete (M2.1) -----

#[tauri::command]
pub async fn create_folder(
    state: State<'_, AppState>,
    parent_id: Option<i64>,
    name: String,
    archetype_id: Option<i64>,
) -> Result<i64> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        crate::fs::relocate::create_folder(
            &library.paths,
            &conn,
            parent_id,
            &name,
            archetype_id,
            &db::journal::new_batch(),
        )
    })
    .await
}

#[tauri::command]
pub async fn move_folder(
    state: State<'_, AppState>,
    id: i64,
    new_parent_id: Option<i64>,
) -> Result<String> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        let batch = db::journal::new_batch();
        crate::fs::relocate::move_folder(&library.paths, &conn, id, new_parent_id, &batch)?;
        Ok(batch)
    })
    .await
}

#[tauri::command]
pub async fn delete_folder(state: State<'_, AppState>, id: i64) -> Result<String> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        let batch = db::journal::new_batch();
        crate::fs::trash::trash_folder(&library.paths, &conn, id, &batch)?;
        Ok(batch)
    })
    .await
}

/// The same escape hatch items get (docs/DESIGN.md §1 "Item operations"),
/// for a folder: open Explorer with the directory selected.
#[tauri::command]
pub async fn reveal_folder(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let rel = {
            let conn = library.conn()?;
            db::folders::rel_for(&conn, id)?
                .ok_or_else(|| AppError::invalid("folder not found"))?
        };
        let path = library.paths.to_abs(&rel)?;
        use tauri_plugin_opener::OpenerExt;
        app.opener()
            .reveal_item_in_dir(path)
            .map_err(|err| AppError::invalid(err.to_string()))
    })
    .await
}

// --- archetype lifecycle (M2.1 — nothing is seeded; see PLAN.md decision 21) --

#[tauri::command]
pub async fn create_archetype(state: State<'_, AppState>, name: String) -> Result<i64> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::create_archetype(&conn, &name)
    })
    .await
}

#[tauri::command]
pub async fn rename_archetype(state: State<'_, AppState>, id: i64, name: String) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::rename_archetype(&conn, id, &name)
    })
    .await
}

#[tauri::command]
pub async fn delete_archetype(state: State<'_, AppState>, id: i64) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::delete_archetype(&conn, id)
    })
    .await
}

#[tauri::command]
pub async fn count_folders_using_archetype(
    state: State<'_, AppState>,
    archetype_id: i64,
) -> Result<i64> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::count_folders_using_archetype(&conn, archetype_id)
    })
    .await
}

#[tauri::command]
pub async fn add_archetype_field(
    state: State<'_, AppState>,
    archetype_id: i64,
    key: String,
    apply_to_existing: bool,
) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::add_archetype_field(&conn, archetype_id, &key, apply_to_existing)
    })
    .await
}

#[tauri::command]
pub async fn reorder_archetype_fields(
    state: State<'_, AppState>,
    archetype_id: i64,
    ordered_keys: Vec<String>,
) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::reorder_archetype_fields(&conn, archetype_id, &ordered_keys)
    })
    .await
}

#[tauri::command]
pub async fn archetype_field_usage(
    state: State<'_, AppState>,
    archetype_id: i64,
    key: String,
) -> Result<Vec<db::folders::ArchetypeFieldUsage>> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::archetype_field_usage(&conn, archetype_id, &key)
    })
    .await
}

#[tauri::command]
pub async fn remove_archetype_field(
    state: State<'_, AppState>,
    archetype_id: i64,
    key: String,
) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::remove_archetype_field(&conn, archetype_id, &key)
    })
    .await
}

// --- folder status lifecycle (M2.1) ---------------------------------------

#[tauri::command]
pub async fn create_folder_status(
    state: State<'_, AppState>,
    label: String,
    colour: String,
) -> Result<String> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::create_folder_status(&conn, &label, &colour)
    })
    .await
}

#[tauri::command]
pub async fn rename_folder_status(
    state: State<'_, AppState>,
    key: String,
    label: String,
) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::rename_folder_status(&conn, &key, &label)
    })
    .await
}

#[tauri::command]
pub async fn recolour_folder_status(
    state: State<'_, AppState>,
    key: String,
    colour: String,
) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::recolour_folder_status(&conn, &key, &colour)
    })
    .await
}

#[tauri::command]
pub async fn reorder_folder_statuses(
    state: State<'_, AppState>,
    ordered_keys: Vec<String>,
) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::reorder_folder_statuses(&conn, &ordered_keys)
    })
    .await
}

#[tauri::command]
pub async fn count_folders_by_status(state: State<'_, AppState>, key: String) -> Result<i64> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::count_folders_by_status(&conn, &key)
    })
    .await
}

#[tauri::command]
pub async fn remove_folder_status(
    state: State<'_, AppState>,
    key: String,
    reassign_to: Option<String>,
) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::remove_folder_status(&conn, &key, reassign_to.as_deref())
    })
    .await
}
