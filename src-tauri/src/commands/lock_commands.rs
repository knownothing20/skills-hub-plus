use tauri::State;

use crate::core::skill_store::SkillStore;
use crate::core::skills_lock::{
    export_skills_lock as export_skills_lock_core, import_skills_lock as import_skills_lock_core,
    ExportLockResult, LockDiffReport,
};

use super::common::format_anyhow_error;

#[tauri::command]
pub async fn export_skills_lock_cmd(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
    path: String,
) -> Result<ExportLockResult, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || export_skills_lock_core(&app, &store, &path))
        .await
        .map_err(|err| err.to_string())?
        .map_err(format_anyhow_error)
}

#[tauri::command]
pub async fn import_skills_lock_cmd(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
    path: String,
) -> Result<LockDiffReport, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || import_skills_lock_core(&app, &store, &path))
        .await
        .map_err(|err| err.to_string())?
        .map_err(format_anyhow_error)
}
