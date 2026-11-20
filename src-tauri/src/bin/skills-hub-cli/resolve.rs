//! 把用户输入的名字解析成具体的技能与副本位置。
//!
//! 名字匹配故意宽松：允许省略大小写、允许只给片段——技能名里有 `腾讯文档` 这种
//! 中文名，也有 `shengcai-shared-mcp` 这种长英文名，让用户打全名很折磨。
//! 但**只有唯一命中时**才放行；命中多个就列出来让用户打得更准，绝不猜。

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use app_lib::core::skill_store::{SkillRecord, SkillStore};

/// 按名字筛技能。`names` 为空表示全部。
pub fn select_skills(store: &SkillStore, names: &[String]) -> Result<Vec<SkillRecord>> {
    let all = store.list_skills()?;
    if names.is_empty() {
        return Ok(all);
    }

    let mut picked: Vec<SkillRecord> = Vec::new();
    for want in names {
        let needle = want.to_lowercase();
        let hits: Vec<&SkillRecord> = all
            .iter()
            .filter(|s| {
                let name = s.name.to_lowercase();
                name == needle || name.contains(&needle)
            })
            .collect();
        match hits.len() {
            0 => {
                return Err(anyhow!(
                    "找不到技能「{want}」。用 `status` 可以看到现有技能。"
                ))
            }
            1 => {
                let hit = hits[0].clone();
                if !picked.iter().any(|p| p.id == hit.id) {
                    picked.push(hit);
                }
            }
            n => {
                let list: Vec<&str> = hits.iter().map(|s| s.name.as_str()).take(8).collect();
                return Err(anyhow!(
                    "「{want}」匹配到 {n} 个技能：{}。请打得更具体。",
                    list.join("、")
                ));
            }
        }
    }
    Ok(picked)
}

/// 精确取一个技能（`diff` / `adopt` 只需单个）。
pub fn find_skill_by_name(store: &SkillStore, name: &str) -> Result<SkillRecord> {
    let mut skills = select_skills(store, &[name.to_string()])?;
    skills.pop().ok_or_else(|| anyhow!("找不到技能「{name}」"))
}

/// 找出某工具（可带实例）那份副本的路径。
pub fn target_path_of(
    store: &SkillStore,
    skill: &SkillRecord,
    tool: &str,
    instance: &str,
) -> Result<PathBuf> {
    let targets = store.list_skill_targets(&skill.id)?;
    let matched: Vec<_> = targets
        .iter()
        .filter(|t| t.tool.eq_ignore_ascii_case(tool))
        .filter(|t| instance.is_empty() || t.instance == instance)
        .collect();

    match matched.len() {
        0 => {
            let available: Vec<String> = targets
                .iter()
                .map(|t| {
                    if t.instance.is_empty() {
                        t.tool.clone()
                    } else {
                        format!("{}#{}", t.tool, t.instance)
                    }
                })
                .collect();
            Err(anyhow!(
                "技能「{}」没有 {tool}{} 这份副本。现有：{}",
                skill.name,
                if instance.is_empty() {
                    String::new()
                } else {
                    format!("#{instance}")
                },
                if available.is_empty() {
                    "（无任何副本）".to_string()
                } else {
                    available.join("、")
                }
            ))
        }
        1 => Ok(PathBuf::from(&matched[0].target_path)),
        _ => {
            let list: Vec<String> = matched
                .iter()
                .map(|t| format!("{}#{}", t.tool, t.instance))
                .collect();
            Err(anyhow!(
                "「{}」的 {tool} 有多份实例：{}。请用 --instance 指定。",
                skill.name,
                list.join("、")
            ))
        }
    }
}
