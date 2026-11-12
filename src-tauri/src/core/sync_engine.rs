use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::safe_fs::{
    move_skill_to_trash, path_entry_exists, restore_skill_from_trash, validate_direct_skill_path,
    TrashReceipt,
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
    pub replaced: bool,
}

pub fn sync_dir_copy_with_overwrite(
    source: &Path,
    target: &Path,
    overwrite: bool,
) -> Result<SyncOutcome> {
    let mut trashed_previous = None;
    if path_entry_exists(target)? {
        if overwrite {
            trashed_previous = move_sync_target_to_trash(target)
                .with_context(|| format!("move existing target to Trash {:?}", target))?;
        } else {
            anyhow::bail!("target already exists: {:?}", target);
        }
    }

    ensure_parent_dir(target)?;
    if let Err(err) = copy_dir_recursive(source, target) {
        return rollback_sync_failure(target, trashed_previous.as_ref(), err);
    }

    Ok(SyncOutcome {
        mode_used: SyncMode::Copy,
        target_path: target.to_path_buf(),
        replaced: trashed_previous.is_some(),
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

#[cfg(windows)]
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
