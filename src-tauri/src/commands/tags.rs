//! Manual per-item tags and the effective-tag cache they feed. No frontend
//! caller in M2 — item-level tag UI is M2.5's preview panel, per DECISIONS.md
//! §M2 — but the commands exist for that milestone to pick up, and are
//! covered by `db::tags`'s own tests.

use tauri::State;

use crate::commands::blocking;
use crate::db;
use crate::db::tags::EffectiveTag;
use crate::error::Result;
use crate::AppState;

#[tauri::command]
pub async fn item_effective_tags(
    state: State<'_, AppState>,
    item_id: i64,
) -> Result<Vec<EffectiveTag>> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::tags::item_effective_tags(&conn, item_id)
    })
    .await
}

#[tauri::command]
pub async fn folder_inherited_tags(
    state: State<'_, AppState>,
    folder_id: i64,
) -> Result<Vec<EffectiveTag>> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::tags::folder_inherited_tags(&conn, folder_id)
    })
    .await
}

#[tauri::command]
pub async fn add_item_tag(
    state: State<'_, AppState>,
    item_id: i64,
    key: Option<String>,
    value: String,
) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::tags::add_item_tag(&conn, item_id, key.as_deref(), &value)
    })
    .await
}

#[tauri::command]
pub async fn remove_item_tag(state: State<'_, AppState>, item_id: i64, tag_id: i64) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::tags::remove_item_tag(&conn, item_id, tag_id)
    })
    .await
}

// --- rename / delete a tag (M2.1) — the minimum that stops the vocabulary
// rotting; see SPEC.md "Item operations" and ROADMAP.md §M2.1.

#[tauri::command]
pub async fn list_tags(
    state: State<'_, AppState>,
    filter: Option<String>,
) -> Result<Vec<db::tags::TagSummary>> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::tags::list_tags(&conn, filter.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn rename_tag(state: State<'_, AppState>, tag_id: i64, value: String) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::tags::rename_tag(&conn, tag_id, &value)
    })
    .await
}

#[tauri::command]
pub async fn delete_tag(state: State<'_, AppState>, tag_id: i64) -> Result<()> {
    let library = state.library()?;
    blocking(move || {
        let conn = library.conn()?;
        db::tags::delete_tag(&conn, tag_id)
    })
    .await
}
