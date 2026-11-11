//! 复制副本（Copy 目标）的「落后」检测与安全刷新。
//!
//! 链接目标天然与主 skill 同步，不存在落后；本模块只处理物理副本。
//! 判定故意保守：mtime 只做初筛，内容哈希定论，副本出现主 skill 没有
//! 的文件一律视为用户手改。所有拿不准的情况都不自动刷新，宁可漏刷。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use super::backup;
use super::content_hash::{hash_dir, is_entry_ignored, load_ignore_patterns};
use super::skill_store::{SkillRecord, SkillStore, SkillTargetRecord};
use super::sync_engine;

/// mtime 容差（毫秒），与目标比较页的口径一致。
const MTIME_TOLERANCE_MS: u64 = 2000;

/// 自动刷新开关的设置键。存 "true"/"false"，缺省视为开启。
pub const AUTO_REFRESH_STALE_COPIES_SETTING: &str = "auto_refresh_stale_copies_v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StaleVerdict {
    /// 主 skill 明确较新，且副本不含独有文件——可以安全自动刷新。
    Behind,
    /// 副本较新，或副本含有主 skill 没有的文件——用户手改过，绝不自动覆盖。
    HandEdited,
    /// 新旧分不清但内容确实不同——保守起见不自动动。
    Conflict,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaleCopyStatus {
    pub skill_id: String,
    pub skill_name: String,
    pub tool: String,
    pub target_path: String,
    pub verdict: StaleVerdict,
}

/// 与比较页一致的口径：目录树最新 mtime + 文件数，应用 `.skillignore`。
pub fn scan_dir_latest_mtime(dir: &Path) -> (u64, usize) {
    if !dir.is_dir() {
        return (0, 0);
    }
    let mut max_mtime = 0u64;
    let mut file_count = 0usize;
    let rules = load_ignore_patterns(dir);
    let walker = walkdir::WalkDir::new(dir).follow_links(false).into_iter();
    for entry in walker
        .filter_entry(|e| !is_entry_ignored(dir, e, &rules))
        .flatten()
    {
        if entry.file_type().is_file() {
            file_count += 1;
            if let Ok(meta) = entry.metadata() {
                if let Ok(modified) = meta.modified() {
                    let ms = modified
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    if ms > max_mtime {
                        max_mtime = ms;
                    }
                }
            }
        }
    }
    (max_mtime, file_count)
}

/// 主 skill（创造者软件目录）优先，回退母库镜像。
/// 与 `commands::sync_commands::resolve_skill_source` 语义一致；
/// 这里在 core 层复刻，避免 core 反向依赖 commands。
fn skill_source_dir(skill: &SkillRecord) -> PathBuf {
    if skill.source_type == "local" {
        if let Some(source_ref) = skill
            .source_ref
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return PathBuf::from(source_ref);
        }
    }
    PathBuf::from(&skill.central_path)
}

fn same_existing_path(a: &Path, b: &Path) -> bool {
    let resolve = |p: &Path| std::fs::canonicalize(p).ok();
    match (resolve(a), resolve(b)) {
        (Some(ra), Some(rb)) => ra == rb,
        _ => a == b,
    }
}

/// 受管文件的相对路径集合（应用 `.skillignore`），用于检测副本独有文件。
fn relative_file_set(dir: &Path) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    let rules = load_ignore_patterns(dir);
    for entry in walkdir::WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_entry_ignored(dir, e, &rules))
        .flatten()
    {
        if entry.file_type().is_file() {
            if let Ok(relative) = entry.path().strip_prefix(dir) {
                out.insert(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    Ok(out)
}

/// 对一个副本目标做分类。`None` 表示没有落后（或无法判定来源）。
fn classify_copy_target(source: &Path, target: &Path) -> Option<StaleVerdict> {
    if !source.is_dir() || !target.is_dir() {
        return None;
    }
    // 内容为王：只要内容一致，mtime 谁新谁旧都不算问题。
    // 拷贝文件、还原备份、编辑器保存都会刷新 mtime，按时间判断会大量误报。
    let hashes = (hash_dir(source).ok(), hash_dir(target).ok());
    match hashes {
        (Some(a), Some(b)) if a == b => return None,
        (None, None) => return None,
        (None, _) | (_, None) => return Some(StaleVerdict::Conflict),
        _ => {}
    }

    let (source_mtime, _) = scan_dir_latest_mtime(source);
    let (target_mtime, _) = scan_dir_latest_mtime(target);

    // 内容不同、时间也分不清先后：保守起见按冲突处理，不自动动。
    if source_mtime.abs_diff(target_mtime) <= MTIME_TOLERANCE_MS {
        return Some(StaleVerdict::Conflict);
    }

    // 内容不同且副本更新 → 手改，绝不自动覆盖。
    if target_mtime > source_mtime + MTIME_TOLERANCE_MS {
        return Some(StaleVerdict::HandEdited);
    }

    // 母版较新：副本含独有文件，同样是手改铁证。
    let source_set = match relative_file_set(source) {
        Ok(set) => set,
        Err(_) => return Some(StaleVerdict::Conflict),
    };
    let target_set = match relative_file_set(target) {
        Ok(set) => set,
        Err(_) => return Some(StaleVerdict::Conflict),
    };
    if !target_set.is_subset(&source_set) {
        return Some(StaleVerdict::HandEdited);
    }

    Some(StaleVerdict::Behind)
}

/// 扫描全库的物理副本目标，返回所有「落后」条目。
/// 只扫 `mode == "copy"` 的目标；链接目标不存在落后。
pub fn scan_stale_copy_targets(store: &SkillStore) -> Result<Vec<StaleCopyStatus>> {
    let mut out = Vec::new();
    for skill in store.list_skills()? {
        let source = skill_source_dir(&skill);
        if !source.is_dir() {
            continue;
        }
        for target in store.list_skill_targets(&skill.id)? {
            if let Some(status) = classify_record(&source, &skill, &target) {
                out.push(status);
            }
        }
    }
    Ok(out)
}

fn classify_record(
    source: &Path,
    skill: &SkillRecord,
    target: &SkillTargetRecord,
) -> Option<StaleCopyStatus> {
    if target.status == "disabled" || target.mode != "copy" {
        return None;
    }
    let target_path = Path::new(&target.target_path);
    if !target_path.is_dir() || same_existing_path(target_path, source) {
        return None;
    }
    classify_copy_target(source, target_path).map(|verdict| StaleCopyStatus {
        skill_id: skill.id.clone(),
        skill_name: skill.name.clone(),
        tool: target.tool.clone(),
        target_path: target.target_path.clone(),
        verdict,
    })
}

/// 自动刷新是否开启。设置缺失或损坏时默认开启（与预览稿一致）。
pub fn auto_refresh_stale_copies_enabled(store: &SkillStore) -> bool {
    store
        .get_setting(AUTO_REFRESH_STALE_COPIES_SETTING)
        .ok()
        .flatten()
        .map(|raw| raw.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(true)
}

pub fn set_auto_refresh_stale_copies(store: &SkillStore, enabled: bool) -> Result<()> {
    store.set_setting(
        AUTO_REFRESH_STALE_COPIES_SETTING,
        if enabled { "true" } else { "false" },
    )
}

/// 刷新一个落后副本：先重扫确认仍是 `Behind`（安全闸），再备份后替换。
/// 备份写入 `<central_root>/../backup/stale-copies/<skill>/`，每个技能只保留
/// 最近一份，覆盖式，不进系统回收站，避免堆积。
pub fn refresh_stale_copy(
    store: &SkillStore,
    central_root: &Path,
    skill_id: &str,
    tool: &str,
) -> Result<()> {
    let skill = store
        .get_skill_by_id(skill_id)?
        .ok_or_else(|| anyhow::anyhow!("skill not found"))?;
    let target = store
        .get_skill_target(skill_id, tool, "global", None)?
        .ok_or_else(|| anyhow::anyhow!("target tool not found for skill"))?;

    let source = skill_source_dir(&skill);
    let target_path = Path::new(&target.target_path);

    // 安全闸：执行前重新分类，防止界面数据过期后覆盖了新状态。
    match classify_copy_target(&source, target_path) {
        Some(StaleVerdict::Behind) => {}
        Some(other) => bail!(
            "STALE_STATE_CHANGED|target is no longer safe to refresh ({:?})",
            other
        ),
        None => bail!("STALE_STATE_CHANGED|target is already in sync"),
    }

    let backup_root = stale_backup_dir(central_root, &skill.name);
    if backup_root.exists() {
        std::fs::remove_dir_all(&backup_root)
            .with_context(|| format!("clear stale backup {:?}", backup_root))?;
    }
    if let Some(parent) = backup_root.parent() {
        std::fs::create_dir_all(parent)?;
    }
    backup::copy_dir_all(target_path, &backup_root);

    std::fs::remove_dir_all(target_path)
        .with_context(|| format!("replace stale copy {:?}", target_path))?;
    sync_engine::copy_dir_recursive(&source, target_path)
        .with_context(|| format!("refresh stale copy {:?}", target_path))?;

    let mut record = target;
    record.status = "ok".to_string();
    record.last_error = None;
    record.synced_at = Some(now_ms());
    store.upsert_skill_target(&record)?;
    Ok(())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// 启动流程：扫描 + （开关开启时）自动刷新所有 `Behind` 条目。
/// 返回 (刷新数, 剩余需人工处理数)。资源占用受控：链接目标不扫，
/// mtime 一致的目标不计算哈希。
pub fn startup_stale_refresh(store: &SkillStore, central_root: &Path) -> Result<(usize, usize)> {
    let statuses = scan_stale_copy_targets(store)?;
    let behind: Vec<StaleCopyStatus> = statuses
        .iter()
        .filter(|status| status.verdict == StaleVerdict::Behind)
        .cloned()
        .collect();

    let auto = auto_refresh_stale_copies_enabled(store);
    let mut refreshed = 0usize;
    if auto {
        for status in &behind {
            match refresh_stale_copy(store, central_root, &status.skill_id, &status.tool) {
                Ok(()) => refreshed += 1,
                Err(err) => {
                    log::warn!(
                        "[stale_copies] refresh {} for {} failed: {:#}",
                        status.tool,
                        status.skill_name,
                        err
                    );
                }
            }
        }
    }

    let remaining = statuses.len() - refreshed;
    Ok((refreshed, remaining))
}

fn stale_backup_dir(central_root: &Path, skill_name: &str) -> PathBuf {
    let base = central_root
        .parent()
        .unwrap_or(central_root)
        .join("backup")
        .join("stale-copies");
    base.join(sanitize_component(skill_name))
}

fn sanitize_component(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/stale_copies.rs"]
mod tests;
