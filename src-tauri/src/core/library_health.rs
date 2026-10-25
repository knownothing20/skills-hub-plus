use std::path::{Path, PathBuf};

use super::central_repo::resolve_central_repo_path;
use super::skill_store::{SkillStore, SkillTargetRecord};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// A Skill is considered idle when its central SKILL.md has not been modified
/// for more than this many days and no valid sync target remains.
pub const IDLE_THRESHOLD_DAYS: u64 = 90;
const SECS_PER_DAY: u64 = 24 * 60 * 60;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthSummary {
    pub total: u64,
    pub synced_skills: u64,
    pub broken_targets: u64,
    pub idle_skills: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrokenTarget {
    pub tool: String,
    pub path: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillHealth {
    pub name: String,
    pub enabled: bool,
    /// Number of valid (existing) sync targets.
    pub synced_tool_count: u64,
    pub broken: Vec<BrokenTarget>,
    /// mtime (unix seconds) of SKILL.md inside the central Skill directory.
    pub last_modified: u64,
    /// No valid target at all and the central copy is older than the threshold.
    pub idle: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryHealthReport {
    pub summary: HealthSummary,
    pub skills: Vec<SkillHealth>,
}

pub fn library_health_report<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &SkillStore,
) -> Result<LibraryHealthReport> {
    let central_root = resolve_central_repo_path(app, store)?;
    let now_secs = unix_now_secs();
    build_library_health_report(&central_root, store, now_secs)
}

pub fn build_library_health_report(
    central_root: &Path,
    store: &SkillStore,
    now_secs: u64,
) -> Result<LibraryHealthReport> {
    let skills = store.list_skills()?;
    let mut total: u64 = 0;
    let mut synced_skills: u64 = 0;
    let mut broken_targets: u64 = 0;
    let mut idle_skills: u64 = 0;
    let mut health_skills = Vec::with_capacity(skills.len());

    for skill in skills {
        let targets = store.list_skill_targets(&skill.id)?;
        let mut synced_tool_count: u64 = 0;
        let mut broken = Vec::new();
        for target in targets.iter().filter(|target| target.status != "disabled") {
            match classify_target(central_root, target) {
                Some(broken_target) => broken.push(broken_target),
                None => synced_tool_count += 1,
            }
        }
        let last_modified = skill_last_modified_secs(Path::new(&skill.central_path));
        let idle = is_idle(synced_tool_count, last_modified, now_secs);

        total += 1;
        if synced_tool_count > 0 {
            synced_skills += 1;
        }
        broken_targets += broken.len() as u64;
        if idle {
            idle_skills += 1;
        }

        health_skills.push(SkillHealth {
            name: skill.name.clone(),
            enabled: skill.enabled,
            synced_tool_count,
            broken,
            last_modified,
            idle,
        });
    }

    Ok(LibraryHealthReport {
        summary: HealthSummary {
            total,
            synced_skills,
            broken_targets,
            idle_skills,
        },
        skills: health_skills,
    })
}

/// Classify one sync target. `None` means the target is valid; `Some` carries
/// the reason it is broken ("missing" or "broken_link").
pub fn classify_target(central_root: &Path, target: &SkillTargetRecord) -> Option<BrokenTarget> {
    let path = Path::new(&target.target_path);
    let broken = |reason: &str| {
        Some(BrokenTarget {
            tool: target.tool.clone(),
            path: target.target_path.clone(),
            reason: reason.to_string(),
        })
    };

    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return broken("missing"),
    };

    let (is_link, link_target) = super::tool_adapters::detect_link(path);
    if !is_link {
        // A plain directory is a healthy copy target; anything else is not.
        return if metadata.is_dir() {
            None
        } else {
            broken("missing")
        };
    }

    // Link targets must still resolve back into the central repo.
    let Some(raw_link_target) = link_target else {
        return broken("broken_link");
    };
    let candidate = if raw_link_target.is_absolute() {
        raw_link_target
    } else {
        let Some(parent) = path.parent() else {
            return broken("broken_link");
        };
        parent.join(&raw_link_target)
    };
    let resolved = match resolve_existing_path(&candidate) {
        Some(resolved) => resolved,
        None => return broken("broken_link"),
    };
    let central_canonical =
        resolve_existing_path(central_root).unwrap_or_else(|| central_root.to_path_buf());
    if resolved.starts_with(&central_canonical) {
        None
    } else {
        broken("broken_link")
    }
}

pub fn is_idle(synced_tool_count: u64, last_modified_secs: u64, now_secs: u64) -> bool {
    synced_tool_count == 0
        && last_modified_secs > 0
        && now_secs.saturating_sub(last_modified_secs) > IDLE_THRESHOLD_DAYS * SECS_PER_DAY
}

pub fn skill_last_modified_secs(central_skill_dir: &Path) -> u64 {
    let direct = central_skill_dir.join("SKILL.md");
    if let Ok(metadata) = std::fs::metadata(&direct) {
        return metadata_modified_secs(&metadata);
    }
    // Case-insensitive fallback for filesystems that store a different casing.
    if let Ok(entries) = std::fs::read_dir(central_skill_dir) {
        for entry in entries.flatten() {
            if entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case("SKILL.md")
            {
                if let Ok(metadata) = entry.metadata() {
                    return metadata_modified_secs(&metadata);
                }
            }
        }
    }
    0
}

fn metadata_modified_secs(metadata: &std::fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn unix_now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Canonicalize a path that must exist; falls back to stripping the Windows
/// verbatim prefix (`\\?\` / `\??\`) that `read_link` may return for junctions.
fn resolve_existing_path(path: &Path) -> Option<PathBuf> {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return Some(canonical);
    }
    let text = path.to_string_lossy();
    let stripped = text
        .strip_prefix(r"\\?\")
        .or_else(|| text.strip_prefix(r"\??\"))
        .unwrap_or(&text);
    let normalized = PathBuf::from(stripped);
    if normalized.exists() {
        Some(normalized)
    } else {
        None
    }
}

#[cfg(test)]
#[path = "tests/library_health.rs"]
mod tests;
