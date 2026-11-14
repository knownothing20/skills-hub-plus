use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::safe_fs::{
    move_skill_to_trash, path_entry_exists, restore_skill_from_trash, TrashReceipt,
};

/// 2.0.1 起同步只有一种方式：整目录复制。
/// 保留这个枚举是为了让历史序列化数据（settings、targets.mode）继续可读。
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncMode {
    #[default]
    Copy,
}

#[derive(Clone, Debug)]
pub struct SyncOutcome {
    pub mode_used: SyncMode,
    pub target_path: PathBuf,
}

pub fn sync_dir_copy_with_overwrite(
    source: &Path,
    target: &Path,
    overwrite: bool,
) -> Result<SyncOutcome> {
    let mut trashed_previous = None;
    // 目标侧的忽略清单：命中的条目属于「本地私有内容」（local/、node_modules、
    // .skillignore 声明的路径…）。它们不参与同步，因此整目录替换前必须先挪出来，
    // 否则会和旧目录一起进回收站——2026-11 的 AgentPort 事故就是这么丢的配置。
    let mut preserved: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut staging: Option<PathBuf> = None;

    if path_entry_exists(target)? {
        if overwrite {
            let rules = super::content_hash::load_ignore_patterns(target);
            let (staged_dir, moved) = preserve_private_entries(target, &rules)?;
            staging = staged_dir;
            preserved = moved;
            trashed_previous = move_sync_target_to_trash(target)
                .with_context(|| format!("move existing target to Trash {:?}", target))?;
        } else {
            anyhow::bail!("target already exists: {:?}", target);
        }
    }

    ensure_parent_dir(target)?;
    let copied =
        copy_dir_recursive(source, target).and_then(|()| restore_private_entries(&preserved));
    if let Err(err) = copied {
        // 半成品不能留：清掉它，再从回收站把旧目录恢复回来，最后把私有内容放回原位。
        let _ = std::fs::remove_dir_all(target);
        if let Err(restore_err) = restore_private_entries(&preserved) {
            log::error!("[sync] could not restore private entries after failure: {restore_err:#}");
        }
        return rollback_sync_failure(target, trashed_previous.as_ref(), err);
    }
    // 私有内容已经搬回原位，暂存目录只剩空壳。
    if let Some(dir) = &staging {
        let _ = std::fs::remove_dir_all(dir);
    }

    Ok(SyncOutcome {
        mode_used: SyncMode::Copy,
        target_path: target.to_path_buf(),
    })
}

pub fn sync_dir_with_mode_with_overwrite(
    _mode: SyncMode,
    source: &Path,
    target: &Path,
    overwrite: bool,
) -> Result<SyncOutcome> {
    sync_dir_copy_with_overwrite(source, target, overwrite)
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create dir {:?}", parent))?;
    }
    Ok(())
}

fn move_sync_target_to_trash(path: &Path) -> Result<Option<TrashReceipt>> {
    let root = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("UNSAFE_PATH|Sync target has no parent"))?;
    move_skill_to_trash(root, path)
}

fn rollback_sync_failure<T>(
    target: &Path,
    trashed_previous: Option<&TrashReceipt>,
    original: anyhow::Error,
) -> Result<T> {
    if let Some(backup) = trashed_previous {
        let root = target
            .parent()
            .ok_or_else(|| anyhow::anyhow!("UNSAFE_PATH|Sync target has no parent"))?;
        restore_skill_from_trash(root, target, backup).with_context(|| {
            format!("sync failed ({original:#}) and the previous target could not be restored")
        })?;
    }
    Err(original)
}

/// 把目标目录里的「本地私有内容」挪到同级暂存目录，等整目录替换完成后再搬回。
///
/// 只处理顶层条目：忽略规则里 `local` 这样的名字同时命中 `local/**`，
/// 所以顶层命中即可整棵子树一起保护。
fn preserve_private_entries(
    target: &Path,
    rules: &[String],
) -> Result<(Option<PathBuf>, Vec<(PathBuf, PathBuf)>)> {
    let parent = target
        .parent()
        .ok_or_else(|| anyhow::anyhow!("UNSAFE_PATH|Sync target has no parent"))?;
    let entries = match std::fs::read_dir(target) {
        Ok(entries) => entries,
        Err(_) => return Ok((None, Vec::new())),
    };

    let staging = parent.join(format!(
        ".skills-hub-preserve-{}",
        uuid::Uuid::new_v4().simple()
    ));
    let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let rel = name.replace('\\', "/");
        if !super::content_hash::is_rel_ignored(&rel, &name, rules) {
            continue;
        }
        let from = entry.path();
        let to = staging.join(&name);
        if let Some(dir) = to.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::rename(&from, &to).with_context(|| {
            format!("preserve private entry {from:?} while replacing the target")
        })?;
        moved.push((from, to));
    }

    Ok((
        if moved.is_empty() {
            None
        } else {
            Some(staging)
        },
        moved,
    ))
}

/// 把暂存的私有内容搬回原位。原位置若已被同步内容占用，以私有内容为准。
fn restore_private_entries(moved: &[(PathBuf, PathBuf)]) -> Result<()> {
    for (original, staged) in moved {
        if !staged.exists() {
            continue;
        }
        if original.exists() {
            if original.is_dir() {
                std::fs::remove_dir_all(original)?;
            } else {
                std::fs::remove_file(original)?;
            }
        }
        if let Some(parent) = original.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(staged, original).with_context(|| {
            format!("restore private entry {original:?} after the target was replaced")
        })?;
    }
    Ok(())
}

/// 同步后的轻量依赖自检：带 `package.json` 的技能，`node_modules` 里必须能
/// 解析到声明的 dependencies。只做静态检查、不启动进程（避免副作用和延迟）。
///
/// 返回 `Some(原因)` 表示这份副本「同步成功但跑不起来」，由调用方记日志。
pub fn check_runtime_dependencies(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("package.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let deps = json.get("dependencies")?.as_object()?;
    if deps.is_empty() {
        return None;
    }

    let node_modules = dir.join("node_modules");
    if !node_modules.is_dir() {
        return Some(format!("缺少 node_modules（{} 个依赖未安装）", deps.len()));
    }

    let mut missing: Vec<&str> = deps
        .keys()
        .filter(|name| !node_modules.join(name).exists())
        .map(String::as_str)
        .collect();
    missing.sort_unstable();
    if missing.is_empty() {
        None
    } else {
        Some(format!("缺少依赖: {}", missing.join(", ")))
    }
}

fn should_skip_copy(entry: &walkdir::DirEntry) -> bool {
    entry.file_name() == ".git"
}

pub fn copy_dir_recursive(source: &Path, target: &Path) -> Result<()> {
    let profile = std::env::var("SKILLS_HUB_PROFILE_IO")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let started = std::time::Instant::now();
    let mut copied_files: u64 = 0;
    let mut copied_bytes: u64 = 0;

    let ignore_rules = super::content_hash::load_ignore_patterns(source);
    for entry in walkdir::WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            !should_skip_copy(entry)
                && !super::content_hash::is_entry_ignored(source, entry, &ignore_rules)
        })
    {
        let entry = entry?;
        if should_skip_copy(&entry) {
            continue;
        }
        let relative = entry.path().strip_prefix(source)?;
        let target_path = target.join(relative);

        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target_path)
                .with_context(|| format!("create dir {:?}", target_path))?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let bytes = std::fs::copy(entry.path(), &target_path)
                .with_context(|| format!("copy file {:?} -> {:?}", entry.path(), target_path))?;
            if profile {
                copied_files += 1;
                copied_bytes = copied_bytes.saturating_add(bytes);
            }
        }
    }
    if profile {
        log::info!(
            "[sync_engine] copy_dir_recursive {} files, {} bytes in {}s (src={:?} dst={:?})",
            copied_files,
            copied_bytes,
            started.elapsed().as_secs_f32(),
            source,
            target
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/sync_engine.rs"]
mod tests;
