use serde::Serialize;
use tauri::State;

use crate::core::shared_dirs::{shared_dir_containing, shared_skill_dirs};
use crate::core::skill_store::SkillStore;

use super::common::format_anyhow_error;

#[derive(Debug, Serialize)]
pub struct SharedSkillDirDto {
    pub path: String,
    pub label: String,
    /// 会读取该目录的工具 key（与工具适配器 id 一致）
    pub readers: Vec<String>,
    pub exists: bool,
    /// 通过该目录暴露的技能名（来自已登记的同步目标）
    pub exposed_skills: Vec<String>,
}

/**
 * 共享目录清单：路径、读取者、里面暴露了哪些技能。
 *
 * 用于界面标注「这个技能不只属于某一个工具」——共享目录的内容不受
 * Skills Hub 的同步控制，但会被多个工具（含豆包）读取。
 */
#[tauri::command]
pub async fn get_shared_skill_dirs(
    store: State<'_, SkillStore>,
) -> Result<Vec<SharedSkillDirDto>, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<Vec<SharedSkillDirDto>> {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("home dir not found"))?;
        let skills = store.list_skills()?;
        let mut out = Vec::new();

        for dir in shared_skill_dirs(&home) {
            let mut exposed = Vec::new();
            for skill in &skills {
                for target in store.list_skill_targets(&skill.id)? {
                    if target.status == "disabled" {
                        continue;
                    }
                    if shared_dir_containing(&home, std::path::Path::new(&target.target_path))
                        .map(|shared| shared.path == dir.path)
                        .unwrap_or(false)
                    {
                        exposed.push(skill.name.clone());
                        break;
                    }
                }
            }
            exposed.sort();
            exposed.dedup();
            out.push(SharedSkillDirDto {
                path: dir.path.to_string_lossy().to_string(),
                label: dir.label.to_string(),
                readers: dir.readers.iter().map(|key| key.to_string()).collect(),
                exists: dir.path.is_dir(),
                exposed_skills: exposed,
            });
        }
        Ok(out)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}
