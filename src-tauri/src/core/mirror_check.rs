//! 母库镜像与主 skill 的一致性检测。
//!
//! 主 skill 是原件（在创造者软件目录里），母库 `skills/<name>` 是镜像。
//! 原件被直接修改后，如果没在应用里点「更新」，镜像就会落后——而健康检查、
//! 卡片状态、副本比对**全都不看这个维度**，于是界面永远显示「全部正常」。
//!
//! 判定沿用在 stale_copies 里用过的保守口径：mtime 初筛，内容哈希定论；
//! 母库含原件没有的文件时视为有人直接改过镜像，只标记不自动处理。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;

use super::content_hash::{hash_dir, is_entry_ignored, load_ignore_patterns};
use super::skill_store::SkillStore;
use super::stale_copies::scan_dir_latest_mtime;

const MTIME_TOLERANCE_MS: u64 = 2000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StaleMirror {
    pub skill_id: String,
    pub name: String,
    pub owner_path: String,
    pub central_path: String,
    /// behind = 母库只是旧快照（可安全更新）；hand_edited = 母库有独有文件（不动）
    pub verdict: String,
}

fn normalize(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_lowercase()
}

fn relative_file_set(dir: &Path) -> BTreeSet<String> {
    let rules = load_ignore_patterns(dir);
    let mut out = BTreeSet::new();
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
    out
}

/// 比较单个技能的原件与母库镜像。返回 None 表示一致或无法判定。
fn classify_mirror(owner: &Path, mirror: &Path) -> Option<&'static str> {
    if !owner.is_dir() || !mirror.is_dir() {
        return None;
    }
    if normalize(owner) == normalize(mirror) {
        return None;
    }
    let (owner_mtime, _) = scan_dir_latest_mtime(owner);
    let (mirror_mtime, _) = scan_dir_latest_mtime(mirror);
    // 复制保留 mtime，正常同步后两者几乎相同，直接跳过，避免全库算哈希。
    if owner_mtime.abs_diff(mirror_mtime) <= MTIME_TOLERANCE_MS {
        return None;
    }
    // 内容优先：mtime 不同但内容相同（例如镜像被触碰、复制未保留时间）不算落后。
    match (hash_dir(owner), hash_dir(mirror)) {
        (Ok(a), Ok(b)) if a == b => return None,
        (Ok(_), Ok(_)) => {}
        _ => return None,
    }
    // 内容确实不同：母库比原件还新 → 有人直接改过镜像，不自动处理。
    if mirror_mtime > owner_mtime {
        return Some("hand_edited");
    }
    let owner_set = relative_file_set(owner);
    let mirror_set = relative_file_set(mirror);
    if !mirror_set.is_subset(&owner_set) {
        return Some("hand_edited");
    }
    Some("behind")
}

/// 扫描全库：哪些技能的母库镜像落后于原件。
pub fn scan_stale_mirrors(store: &SkillStore) -> Result<Vec<StaleMirror>> {
    let mut out = Vec::new();
    for skill in store.list_skills()? {
        // 只有记录了创造者目录（local 且 source_ref 非空）才有「原件 vs 镜像」可比
        if skill.source_type != "local" {
            continue;
        }
        let Some(owner_ref) = skill
            .source_ref
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let owner = PathBuf::from(owner_ref);
        let mirror = PathBuf::from(&skill.central_path);
        if let Some(verdict) = classify_mirror(&owner, &mirror) {
            out.push(StaleMirror {
                skill_id: skill.id.clone(),
                name: skill.name.clone(),
                owner_path: owner.to_string_lossy().to_string(),
                central_path: mirror.to_string_lossy().to_string(),
                verdict: verdict.to_string(),
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "tests/mirror_check.rs"]
mod tests;
