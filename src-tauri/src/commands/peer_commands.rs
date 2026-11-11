//! 对等同步的命令层：看各副本相对基线的状态、手动触发一次协调。

use tauri::State;

use crate::core::peer_sync::{reconcile_all, reconcile_skill, PeerStanding, ReconcileOutcome};
use crate::core::skill_store::SkillStore;

/// 列出某个技能各副本相对基线的状态（只读，不写盘）
#[tauri::command]
pub async fn get_peer_standings(
    skill_id: String,
    store: State<'_, SkillStore>,
) -> Result<Vec<PeerStanding>, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<_> {
        let skill = store
            .get_skill_by_id(&skill_id)?
            .ok_or_else(|| anyhow::anyhow!("SKILL_NOT_FOUND|{skill_id}"))?;
        crate::core::peer_sync::scan_peers(&store, &skill)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

/// 对某个技能立刻做一次对等协调（采纳偏离基线的版本并推给其它副本）
#[tauri::command]
pub async fn reconcile_skill_now(
    skill_id: String,
    store: State<'_, SkillStore>,
) -> Result<ReconcileOutcome, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<_> {
        let skill = store
            .get_skill_by_id(&skill_id)?
            .ok_or_else(|| anyhow::anyhow!("SKILL_NOT_FOUND|{skill_id}"))?;
        reconcile_skill(&store, &skill)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

/// 对所有技能跑一次对等协调，返回 (采纳, 冲突, 无变化)
#[tauri::command]
pub async fn reconcile_all_now(
    store: State<'_, SkillStore>,
) -> Result<(usize, usize, usize), String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || reconcile_all(&store))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}
