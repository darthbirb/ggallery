use tauri::State;

use crate::commands::blocking;
use crate::db;
use crate::db::items::{GridItem, Scope};
use crate::error::Result;
use crate::fs::paths::normalise_rel;
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
