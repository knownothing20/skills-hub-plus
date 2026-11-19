use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use tauri::Manager;

use super::safe_fs::move_internal_to_trash;

const TEMP_PREFIX: &str = "skills-hub-git-";
const TEMP_MARKER: &str = ".skills-hub-git-temp";

#[allow(dead_code)]
pub fn mark_temp_dir(dir: &Path) -> Result<()> {
    let marker = dir.join(TEMP_MARKER);
    if marker.exists() {
        return Ok(());
    }
    std::fs::write(&marker, b"skills-hub-git-temp-v1\n")
        .with_context(|| format!("failed to write marker {:?}", marker))?;
    Ok(())
}

// 这个模块此前从未在 `core/mod.rs` 里声明过，一直没被编译，所以这些旧函数
// 的 dead_code 到最近才暴露。保留实现、显式放行，避免顺手删掉别人将来要接的
// 清理逻辑；模块内新增的 `cleanup_central_residue` 是有真实调用方的。
#[allow(dead_code)]
pub fn cleanup_old_git_temp_dirs<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    max_age: Duration,
) -> Result<usize> {
    let cache_dir = app
        .path()
        .app_cache_dir()
        .context("failed to resolve app cache dir")?;

    cleanup_old_git_temp_dirs_in(&cache_dir, max_age)
}

#[cfg_attr(not(test), allow(dead_code))]
fn cleanup_old_git_temp_dirs_in(cache_dir: &Path, max_age: Duration) -> Result<usize> {
    if !cache_dir.exists() {
        return Ok(0);
    }

    let cutoff = SystemTime::now()
        .checked_sub(max_age)
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let mut removed = 0usize;
    let rd = match std::fs::read_dir(cache_dir) {
        Ok(v) => v,
        Err(err) => {
            // No permission / missing dir is not fatal.
            return Err(anyhow::anyhow!(
                "failed to read cache dir {:?}: {}",
                cache_dir,
                err
            ));
        }
    };

    for entry in rd.flatten() {
        let path: PathBuf = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(TEMP_PREFIX) {
            continue;
        }

        // Safety: only delete directories we have explicitly marked.
        if !path.join(TEMP_MARKER).exists() {
            continue;
        }

        let meta = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        if modified > cutoff {
            continue;
        }

        // Best-effort move to Trash; any failure leaves the original intact.
        if move_internal_to_trash(cache_dir, &path).is_ok() {
            removed += 1;
        }
    }

    Ok(removed)
}

#[cfg(test)]
#[path = "tests/temp_cleanup.rs"]
mod tests;

/// 归档在中心库根下、由本应用创建的隐藏暂存目录前缀。
///
/// `.skills-hub-<operation>-<uuid>` 由 `installer::hidden_staging_path` 生成，
/// 正常路径下成功会 publish 掉、失败会 `move_internal_to_trash` 掉。但进程被杀、
/// 断电、回收站写失败时它会留在原地——实测母库曾堆积 47 个这类目录（5.1MB）。
/// 启动时清扫一次，把它们补送回收站。
const CENTRAL_RESIDUE_PREFIX: &str = ".skills-hub-";

/// 清扫中心库根下的残留暂存目录。`max_age` 之前的才动，避免误删正在安装的。
pub fn cleanup_central_residue(central_root: &Path, max_age: Duration) -> Result<usize> {
    let stale = stale_central_residue(central_root, max_age);
    let mut removed = 0usize;
    for path in stale {
        if move_internal_to_trash(central_root, &path).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

/// 挑出可以清理的残留目录。与删除动作分开，这样筛选规则本身可以单测，
/// 不必依赖系统回收站在测试环境里可用。
pub(crate) fn stale_central_residue(central_root: &Path, max_age: Duration) -> Vec<PathBuf> {
    if !central_root.is_dir() {
        return Vec::new();
    }
    let cutoff = SystemTime::now()
        .checked_sub(max_age)
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(central_root) else {
        return Vec::new();
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        // 只动本应用的隐藏目录；正式技能名不会以点开头（validate_skill_name 拦着）。
        if !name.starts_with(CENTRAL_RESIDUE_PREFIX) {
            continue;
        }
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if meta.modified().unwrap_or(SystemTime::UNIX_EPOCH) > cutoff {
            continue;
        }
        out.push(path);
    }
    out
}
