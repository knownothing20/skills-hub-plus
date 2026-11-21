use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use walkdir::{DirEntry, WalkDir};

const DEFAULT_IGNORES: &[&str] = &[
    ".git",
    ".DS_Store",
    "Thumbs.db",
    ".gitignore",
    ".skillignore",
    "node_modules",
    "__pycache__",
    "venv",
    ".venv",
    ".pytest_cache",
    ".cache",
    ".tmp",
    ".agentport-tmp",
    ".codex-tmp",
    "tmp",
];

pub(crate) fn load_ignore_patterns(root: &Path) -> Vec<String> {
    let mut patterns: Vec<String> = DEFAULT_IGNORES
        .iter()
        .map(|&s| s.trim_matches('/').to_string())
        .collect();

    let skillignore_path = root.join(".skillignore");
    if let Ok(content) = std::fs::read_to_string(&skillignore_path) {
        for line in content.lines() {
            let trimmed = line.trim().trim_start_matches('\u{feff}').trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let normalized = trimmed.replace('\\', "/");
            let clean = normalized.trim_matches('/').to_string();
            if !clean.is_empty() && !patterns.iter().any(|p| p == &clean) {
                patterns.push(clean);
            }
        }
    }

    patterns
}

fn matches_rule(pattern: &str, relative_norm: &str, file_name: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }

    // 1. 完全匹配文件名或相对路径
    if file_name == pattern || relative_norm == pattern {
        return true;
    }

    // 2. 匹配目录层级（例如 pattern 为 "local"，则 "local/..." 或以 "/local/" 结尾或包含 "/local/"）
    if relative_norm.starts_with(&format!("{}/", pattern))
        || relative_norm.contains(&format!("/{}/", pattern))
        || relative_norm.ends_with(&format!("/{}", pattern))
    {
        return true;
    }

    // 3. 通配符后缀匹配（如 *.key, *.log）
    if let Some(suffix) = pattern.strip_prefix('*') {
        if file_name.ends_with(suffix) || relative_norm.ends_with(suffix) {
            return true;
        }
    }

    // 4. 通配符前缀匹配（如 connections*.json）
    if let Some(prefix) = pattern.strip_suffix('*') {
        if file_name.starts_with(prefix) {
            return true;
        }
    }

    false
}

/// 按「相对路径 + 文件名」判定是否命中忽略清单。
///
/// 和 `is_entry_ignored` 同一套规则，但不需要真实的 `DirEntry`——同步引擎在
/// 删除/保护本地私有内容时手里只有路径。
pub(crate) fn is_rel_ignored(relative_norm: &str, file_name: &str, rules: &[String]) -> bool {
    rules
        .iter()
        .any(|rule| matches_rule(rule, relative_norm, file_name))
}

pub(crate) fn is_entry_ignored(root: &Path, entry: &DirEntry, rules: &[String]) -> bool {
    let relative = match entry.path().strip_prefix(root) {
        Ok(rel) => rel,
        Err(_) => return false,
    };

    let relative_str = relative.to_string_lossy();
    if relative_str.is_empty() {
        return false;
    }

    let relative_norm = relative_str.replace('\\', "/");
    let file_name = entry.file_name().to_string_lossy();

    for rule in rules {
        if matches_rule(rule, &relative_norm, &file_name) {
            return true;
        }
    }

    false
}

/// 整目录的内容指纹。
///
/// **必须排序后再喂给 hasher。** `WalkDir` 默认按底层 `readdir` 顺序返回条目，
/// 而那个顺序只保证「同一次枚举内稳定」，不保证跨目录、跨文件系统、跨机器一致：
/// NTFS 基本按名称返回，ext4/APFS 的哈希序则完全看运气，网络盘更没谱。
/// 一旦两个目录的枚举顺序不同，内容逐字节相同的两份副本也会算出不同指纹——
/// 界面上就会出现「版本字母全是 A（说明内容一致），却同时报副本落后」这种
/// 自相矛盾的状态，而自动刷新又会因为「内容其实没差」而什么都不做，用户
/// 无论点多少次都消不掉那个红色感叹号。
///
/// `peer_sync::snapshot_with_time` 早在算版本字母时就 `sort_by(rel)` 了，
/// 两套口径本该一致；这里补上排序，正是让「落后」与「A/B」看同一件事。
pub fn hash_dir(path: &Path) -> Result<String> {
    let rules = load_ignore_patterns(path);

    // 先收集、排序，再算哈希：顺序必须由内容本身决定，不能由文件系统决定。
    let mut entries: Vec<(String, bool, PathBuf)> = Vec::new();
    for entry in WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !is_entry_ignored(path, entry, &rules))
    {
        let entry = entry?;
        if is_entry_ignored(path, &entry, &rules) {
            continue;
        }

        let relative = entry
            .path()
            .strip_prefix(path)
            .with_context(|| format!("strip prefix {:?}", entry.path()))?;

        entries.push((
            relative.to_string_lossy().replace('\\', "/"),
            entry.file_type().is_file(),
            entry.path().to_path_buf(),
        ));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    for (relative_norm, is_file, full) in entries {
        hasher.update(relative_norm.as_bytes());
        if is_file {
            let bytes = std::fs::read(&full).with_context(|| format!("read file {full:?}"))?;
            hasher.update(bytes);
        }
    }

    let digest = hasher.finalize();
    Ok(hex::encode(digest))
}

#[cfg(test)]
#[path = "tests/content_hash.rs"]
mod tests;
