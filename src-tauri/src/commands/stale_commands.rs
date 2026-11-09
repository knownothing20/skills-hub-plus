use serde::Serialize;
use tauri::State;

use crate::core::central_repo::resolve_central_repo_path;
use crate::core::skill_store::SkillStore;
use crate::core::stale_copies::{
    auto_refresh_stale_copies_enabled, refresh_stale_copy, scan_stale_copy_targets,
    set_auto_refresh_stale_copies, startup_stale_refresh, StaleCopyStatus,
};

use super::common::format_anyhow_error;

/// 带展示名的落后状态条目。
#[derive(Debug, Serialize)]
pub struct StaleCopyStatusDto {
    pub skill_id: String,
    pub skill_name: String,
    pub tool: String,
    pub tool_label: String,
    pub target_path: String,
    /// behind | hand_edited | conflict
    pub verdict: String,
}

impl StaleCopyStatusDto {
    fn from_status(status: StaleCopyStatus, tool_label: String) -> Self {
        let verdict = match status.verdict {
            crate::core::stale_copies::StaleVerdict::Behind => "behind",
            crate::core::stale_copies::StaleVerdict::HandEdited => "hand_edited",
            crate::core::stale_copies::StaleVerdict::Conflict => "conflict",
        };
        Self {
            skill_id: status.skill_id,
            skill_name: status.skill_name,
            tool: status.tool,
            tool_label,
            target_path: status.target_path,
            verdict: verdict.to_string(),
        }
    }
}

async fn with_store<T, F>(store: State<'_, SkillStore>, work: F) -> Result<T, String>
where
    F: FnOnce(&SkillStore) -> anyhow::Result<T> + Send + 'static,
    T: Send + 'static,
{
    let owned = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || work(&owned))
        .await
        .map_err(|err| err.to_string())?
        .map_err(format_anyhow_error)
}

#[tauri::command]
pub async fn get_stale_copy_status(
    store: State<'_, SkillStore>,
) -> Result<Vec<StaleCopyStatusDto>, String> {
    with_store(store, |store| {
        let labels = super::common::runtime_tools(store, true)?
            .into_iter()
            .map(|tool| (tool.key, tool.label))
            .collect::<std::collections::HashMap<_, _>>();
        Ok(scan_stale_copy_targets(store)?
            .into_iter()
            .map(|status| {
                let label = labels
                    .get(&status.tool)
                    .cloned()
                    .unwrap_or_else(|| status.tool.clone());
                StaleCopyStatusDto::from_status(status, label)
            })
            .collect())
    })
    .await
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn refresh_stale_copy_cmd(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
    skillId: String,
    tool: String,
) -> Result<(), String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let central_root = resolve_central_repo_path(&app, &store)?;
        refresh_stale_copy(&store, &central_root, &skillId, &tool)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[derive(Debug, Serialize)]
pub struct StartupStaleRefreshDto {
    pub refreshed: usize,
    /// 刷新后仍需人工处理的条目（手改 / 冲突，或自动刷新已关闭时的全部落后条目）。
    pub remaining: usize,
    pub auto_refresh_enabled: bool,
}

#[tauri::command]
pub async fn run_startup_stale_refresh(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
) -> Result<StartupStaleRefreshDto, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let central_root = resolve_central_repo_path(&app, &store)?;
        let auto = auto_refresh_stale_copies_enabled(&store);
        let (refreshed, remaining) = startup_stale_refresh(&store, &central_root)?;
        Ok(StartupStaleRefreshDto {
            refreshed,
            remaining,
            auto_refresh_enabled: auto,
        })
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
pub async fn get_auto_refresh_stale_copies(store: State<'_, SkillStore>) -> Result<bool, String> {
    with_store(store, |store| Ok(auto_refresh_stale_copies_enabled(store))).await
}

#[tauri::command]
pub async fn set_auto_refresh_stale_copies_cmd(
    store: State<'_, SkillStore>,
    enabled: bool,
) -> Result<bool, String> {
    with_store(store, move |store| {
        set_auto_refresh_stale_copies(store, enabled)?;
        Ok(auto_refresh_stale_copies_enabled(store))
    })
    .await
}
