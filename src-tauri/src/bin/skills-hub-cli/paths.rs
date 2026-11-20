//! 定位 DB 与母库根目录。
//!
//! 不能用 `default_db_path(app)`——那个要 `AppHandle`，CLI 里没有 Tauri 运行时。
//! Tauri 的 `app_config_dir` 在 Windows 上就是 `%APPDATA%\<identifier>`，
//! 所以这里照同样口径拼出来，保证和 GUI 打开的是同一个库。

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use app_lib::core::skill_store::SkillStore;

/// Tauri 的应用标识，取自 `tauri.conf.json` 的 `identifier`。
const APP_IDENTIFIER: &str = "io.github.mcncarl.skillshub";

/// 解析数据库路径：优先命令行，其次 `%APPDATA%` 下的标准位置。
pub fn resolve_db(override_path: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = override_path {
        if !p.is_file() {
            return Err(anyhow!("指定的数据库不存在：{}", p.display()));
        }
        return Ok(p);
    }
    let config_dir = config_root().ok_or_else(|| {
        anyhow!("无法确定配置目录（Windows 读 APPDATA，其他系统读 XDG_CONFIG_HOME/HOME）")
    })?;
    let db = config_dir.join(APP_IDENTIFIER).join("skills_hub.db");
    if !db.is_file() {
        return Err(anyhow!(
            "找不到 Skills Hub 数据库：{}\n如果装在不同位置，请用 --db 指定。",
            db.display()
        ));
    }
    Ok(db)
}

/// 各平台的配置根目录，口径与 Tauri 的 `app_config_dir` 的父目录一致。
fn config_root() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(PathBuf::from)
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|h| h.join("Library").join("Application Support"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }
}

/// 母库根目录：优先命令行，其次数据库里的 `central_repo_path` 设置。
pub fn resolve_central(store: &SkillStore, override_path: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = override_path {
        if !p.is_dir() {
            return Err(anyhow!("指定的母库目录不存在：{}", p.display()));
        }
        return Ok(p);
    }
    if let Some(raw) = store.get_setting("central_repo_path")? {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            let p = PathBuf::from(trimmed);
            if p.is_dir() {
                return Ok(p);
            }
        }
    }
    // 设置里没有时，退回「所有技能 central_path 的公共父目录」。
    let skills = store.list_skills()?;
    let first = skills
        .iter()
        .map(|s| PathBuf::from(&s.central_path))
        .find(|p| p.is_dir())
        .ok_or_else(|| anyhow!("无法确定母库位置，请用 --central 指定"))?;
    Ok(first.parent().unwrap_or(Path::new(".")).to_path_buf())
}

/// 打开数据库并确保 schema 存在（老库升级后也能直接跑）。
pub fn open_store(db: &Path) -> Result<SkillStore> {
    let store = SkillStore::new(db.to_path_buf());
    store
        .ensure_schema()
        .with_context(|| format!("打开数据库失败：{}", db.display()))?;
    Ok(store)
}
