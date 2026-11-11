//! 母库只读浏览：列技能、列文件、读文件内容。
//!
//! 母库是各工具副本的镜像汇总，这里只读；
//! 所有路径都限制在母库目录内，防止通过 `..` 或绝对路径越界。

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::core::central_repo;

/// 单个技能在母库里的概况
#[derive(Debug, Serialize)]
pub struct CentralSkillSummaryDto {
    pub name: String,
    pub path: String,
    pub file_count: usize,
    pub size_bytes: u64,
    /// 目录内最新文件时间（毫秒）
    pub updated_at: u64,
    /// 该技能的原件目录；没有记录时为空
    pub source_ref: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LibraryFileNodeDto {
    pub name: String,
    /// 相对技能目录的路径，用 / 分隔
    pub rel_path: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub children: Vec<LibraryFileNodeDto>,
}

fn central_root() -> PathBuf {
    central_repo::app_install_skills_dir()
}

/// 技能名只允许是母库下的一级目录名，挡掉 `..`、分隔符和绝对路径
fn safe_skill_dir(skill_name: &str) -> anyhow::Result<PathBuf> {
    let trimmed = skill_name.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed == "."
        || trimmed == ".."
    {
        anyhow::bail!("INVALID_SKILL_NAME|{trimmed}");
    }
    let dir = central_root().join(trimmed);
    if !dir.is_dir() {
        anyhow::bail!("SKILL_NOT_FOUND|{trimmed}");
    }
    Ok(dir)
}

/// rel_path 必须落在技能目录内：逐段校验，不使用 `..`，不允许绝对路径
fn safe_rel_path(root: &Path, rel_path: &str) -> anyhow::Result<PathBuf> {
    let mut out = root.to_path_buf();
    for segment in rel_path.split(['/', '\\']) {
        let segment = segment.trim();
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            anyhow::bail!("INVALID_PATH|{rel_path}");
        }
        out.push(segment);
    }
    if !out.starts_with(root) {
        anyhow::bail!("INVALID_PATH|{rel_path}");
    }
    Ok(out)
}

fn scan_summary(dir: &Path) -> (usize, u64, u64) {
    let mut files = 0usize;
    let mut size = 0u64;
    let mut newest = 0u64;
    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        files += 1;
        if let Ok(meta) = entry.metadata() {
            size += meta.len();
            if let Ok(modified) = meta.modified() {
                if let Ok(dur) = modified.duration_since(std::time::UNIX_EPOCH) {
                    newest = newest.max(dur.as_millis() as u64);
                }
            }
        }
    }
    (files, size, newest)
}

fn build_tree(root: &Path, dir: &Path, depth: u32, max_depth: u32) -> Vec<LibraryFileNodeDto> {
    if depth > max_depth {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut nodes: Vec<LibraryFileNodeDto> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git" || name == "__pycache__" {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| name.clone());
        let meta = entry.metadata().ok();
        let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        nodes.push(LibraryFileNodeDto {
            name,
            rel_path: rel,
            is_dir,
            size_bytes: if is_dir {
                0
            } else {
                meta.as_ref().map(|m| m.len()).unwrap_or(0)
            },
            children: if is_dir {
                build_tree(root, &path, depth + 1, max_depth)
            } else {
                Vec::new()
            },
        });
    }
    // 目录在前，同类按名称排序
    nodes.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    nodes
}

/// 列出母库里的全部技能（只读）
#[tauri::command]
pub async fn list_central_library(
    store: tauri::State<'_, crate::core::skill_store::SkillStore>,
) -> Result<Vec<CentralSkillSummaryDto>, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<_> {
        let root = central_root();
        if !root.is_dir() {
            return Ok(Vec::new());
        }
        let records = store.list_skills().unwrap_or_default();
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&root)?.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let (files, size, newest) = scan_summary(&path);
            let source_ref = records
                .iter()
                .find(|s| s.name == name)
                .and_then(|s| s.source_ref.clone());
            out.push(CentralSkillSummaryDto {
                name,
                path: path.to_string_lossy().to_string(),
                file_count: files,
                size_bytes: size,
                updated_at: newest,
                source_ref,
            });
        }
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

/// 列出一个母库技能的文件树（只读，默认两层，最多四层）
#[tauri::command]
pub async fn list_library_files(
    skill_name: String,
    max_depth: Option<u32>,
) -> Result<Vec<LibraryFileNodeDto>, String> {
    let depth = max_depth.unwrap_or(2).min(4);
    tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<_> {
        let dir = safe_skill_dir(&skill_name)?;
        Ok(build_tree(&dir, &dir, 0, depth))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

/// 读取一个文本文件的内容（只读、限制体积、拒绝二进制）
#[tauri::command]
pub async fn read_library_file(skillName: String, relPath: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<String> {
        let dir = safe_skill_dir(&skillName)?;
        let file = safe_rel_path(&dir, &relPath)?;
        if !file.is_file() {
            anyhow::bail!("FILE_NOT_FOUND|{rel_path}");
        }
        // 同一目录下不允许跳出；safe_rel_path 已经逐段挡掉 `..`
        let canonical_dir = dir.canonicalize()?;
        let canonical_file = file.canonicalize()?;
        if !canonical_file.starts_with(&canonical_dir) {
            anyhow::bail!("INVALID_PATH|{rel_path}");
        }
        let meta = std::fs::metadata(&file)?;
        const MAX_BYTES: u64 = 512 * 1024;
        if meta.len() > MAX_BYTES {
            anyhow::bail!("FILE_TOO_LARGE|{} KB", meta.len() / 1024);
        }
        let bytes = std::fs::read(&file)?;
        if bytes.iter().take(4096).any(|b| *b == 0) {
            anyhow::bail!("BINARY_FILE|{rel_path}");
        }
        Ok(String::from_utf8_lossy(&bytes).to_string())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}
