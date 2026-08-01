//! Folder CRUD, tags, archetypes, status, cover. M2's data surfaced for the
//! folder header — see `features/folder` on the frontend.

use tauri::State;

use crate::commands::blocking;
use crate::db;
use crate::db::folders::{ArchetypeInfo, FolderDetail, FolderStatusDef, NameParseCandidate};
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

#[tauri::command]
pub async fn set_folder_title(state: State<'_, AppState>, id: i64, title: String) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::set_title(&conn, id, &title)
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

#[tauri::command]
pub async fn scan_folder_name_parse(
    state: State<'_, AppState>,
) -> Result<Vec<NameParseCandidate>> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::scan_name_parse(&conn)
    })
    .await
}

#[tauri::command]
pub async fn apply_folder_name_parse(
    state: State<'_, AppState>,
    rows: Vec<NameParseCandidate>,
) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::folders::apply_name_parse(&conn, &rows)
    })
    .await
}
