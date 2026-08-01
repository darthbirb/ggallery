//! Manual per-item tags and the effective-tag cache they feed. No frontend
//! caller in M2 — item-level tag UI is M2.5's preview panel, per PLAN.md
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
