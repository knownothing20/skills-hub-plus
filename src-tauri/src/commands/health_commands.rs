use tauri::State;

use crate::core::library_health::{library_health_report, LibraryHealthReport};
use crate::core::skill_store::SkillStore;

use super::common::format_anyhow_error;

#[tauri::command]
pub async fn get_library_health(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
) -> Result<LibraryHealthReport, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || library_health_report(&app, &store))
        .await
        .map_err(|err| err.to_string())?
        .map_err(format_anyhow_error)
}
