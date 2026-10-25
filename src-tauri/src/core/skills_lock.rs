use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::content_hash::hash_dir;
use super::skill_store::{SkillRecord, SkillStore};

/// Version of the skills.lock manifest format produced and accepted here.
pub const LOCK_FORMAT_VERSION: i64 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockTargetEntry {
    pub tool: String,
    pub scope: String,
    pub target_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockSkillEntry {
    pub name: String,
    pub enabled: bool,
    pub tags: Vec<String>,
    pub source_ref: Option<String>,
    pub content_hash: String,
    pub targets: Vec<LockTargetEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillsLockFile {
    pub version: i64,
    pub exported_at: String,
    pub skills: Vec<LockSkillEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportLockResult {
    pub path: String,
    pub skill_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockDrift {
    pub name: String,
    pub lock_hash: String,
    pub local_hash: String,
}

/// v1 import is report-only: nothing is installed or deleted. The report lists
/// what a manifest expects versus what this machine currently has.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockDiffReport {
    pub lock_path: String,
    /// Skills present in the manifest but missing on this machine.
    pub missing_here: Vec<LockSkillEntry>,
    /// Skills present on this machine but absent from the manifest (reported only, never removed).
    pub extra_here: Vec<String>,
    /// Skills whose content hash differs between manifest and machine.
    pub drifted: Vec<LockDrift>,
    /// Skills with identical content whose target tool list differs.
    pub targets_changed: Vec<String>,
}

/// In-memory snapshot of one local skill used by the pure diff logic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalSkillSnapshot {
    pub name: String,
    pub content_hash: String,
    /// Distinct (tool, scope) pairs this skill currently syncs to.
    pub targets: BTreeSet<(String, String)>,
}

/// Export every managed skill (name, enabled, tags, source, content hash,
/// targets) as a portable `skills.lock` JSON manifest.
pub fn export_skills_lock<R: tauri::Runtime>(
    _app: &tauri::AppHandle<R>,
    store: &SkillStore,
    path: &str,
) -> Result<ExportLockResult> {
    let lock = build_skills_lock(store, &now_rfc3339_utc())?;
    write_lock_file(&lock, Path::new(path))?;
    Ok(ExportLockResult {
        path: path.to_string(),
        skill_count: lock.skills.len(),
    })
}

/// Read a `skills.lock` manifest and compare it with the current library.
/// v1 never mutates the library; it only returns the diff report.
pub fn import_skills_lock<R: tauri::Runtime>(
    _app: &tauri::AppHandle<R>,
    store: &SkillStore,
    path: &str,
) -> Result<LockDiffReport> {
    let raw = std::fs::read_to_string(Path::new(path))
        .with_context(|| format!("failed to read skills.lock file: {}", path))?;
    let lock = parse_skills_lock(&raw)?;
    let local = collect_local_snapshots(store)?;
    Ok(build_lock_diff_report(path, &lock, local))
}

/// Build the manifest from the store. Skills are sorted by name so repeated
/// exports of an unchanged library produce byte-identical files.
pub fn build_skills_lock(store: &SkillStore, exported_at: &str) -> Result<SkillsLockFile> {
    let mut records = store.list_skills()?;
    records.sort_by(|a, b| a.name.cmp(&b.name));

    let mut skills = Vec::with_capacity(records.len());
    for record in &records {
        let tags = store
            .get_skill_tags(&record.id)?
            .into_iter()
            .map(|tag| tag.name)
            .collect::<Vec<_>>();

        let targets = store
            .list_skill_targets(&record.id)?
            .into_iter()
            .map(|target| LockTargetEntry {
                tool: target.tool,
                scope: target.scope,
                target_path: target.target_path,
            })
            .collect::<Vec<_>>();

        skills.push(LockSkillEntry {
            name: record.name.clone(),
            enabled: record.enabled,
            tags,
            source_ref: record.source_ref.clone(),
            content_hash: current_content_hash(record),
            targets,
        });
    }

    Ok(SkillsLockFile {
        version: LOCK_FORMAT_VERSION,
        exported_at: exported_at.to_string(),
        skills,
    })
}

/// Pretty-print the manifest to disk as JSON.
pub fn write_lock_file(lock: &SkillsLockFile, path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(lock).context("serialize skills.lock manifest")?;
    std::fs::write(path, json)
        .with_context(|| format!("failed to write skills.lock file: {}", path.display()))
}

/// Parse and validate a manifest. Returns clear errors for invalid JSON or an
/// unsupported format version.
pub fn parse_skills_lock(raw: &str) -> Result<SkillsLockFile> {
    let lock: SkillsLockFile = serde_json::from_str(raw)
        .with_context(|| "invalid skills.lock file: unable to parse JSON manifest")?;
    if lock.version != LOCK_FORMAT_VERSION {
        anyhow::bail!(
            "unsupported skills.lock version: {} (this app supports version {})",
            lock.version,
            LOCK_FORMAT_VERSION
        );
    }
    Ok(lock)
}

/// Gather the current state of every managed skill: fresh content hash of its
/// central directory plus its (tool, scope) target pairs.
pub fn collect_local_snapshots(store: &SkillStore) -> Result<Vec<LocalSkillSnapshot>> {
    let records = store.list_skills()?;
    let mut snapshots = Vec::with_capacity(records.len());
    for record in &records {
        let targets = store
            .list_skill_targets(&record.id)?
            .into_iter()
            .map(|target| (target.tool, target.scope))
            .collect::<BTreeSet<_>>();
        snapshots.push(LocalSkillSnapshot {
            name: record.name.clone(),
            content_hash: current_content_hash(record),
            targets,
        });
    }
    Ok(snapshots)
}

/// Pure diff between a manifest and local snapshots. Names are matched exactly;
/// target comparison uses (tool, scope) pairs so manifests stay portable across
/// machines with different home directories.
pub fn build_lock_diff_report(
    lock_path: &str,
    lock: &SkillsLockFile,
    local: Vec<LocalSkillSnapshot>,
) -> LockDiffReport {
    let local_by_name: BTreeMap<String, LocalSkillSnapshot> = local
        .into_iter()
        .map(|snapshot| (snapshot.name.clone(), snapshot))
        .collect();
    let lock_names: BTreeSet<&str> = lock.skills.iter().map(|s| s.name.as_str()).collect();

    let mut missing_here = Vec::new();
    let mut drifted = Vec::new();
    let mut targets_changed = Vec::new();

    for entry in &lock.skills {
        let Some(snapshot) = local_by_name.get(&entry.name) else {
            missing_here.push(entry.clone());
            continue;
        };

        if entry.content_hash != snapshot.content_hash {
            drifted.push(LockDrift {
                name: entry.name.clone(),
                lock_hash: entry.content_hash.clone(),
                local_hash: snapshot.content_hash.clone(),
            });
            continue;
        }

        let lock_targets: BTreeSet<(String, String)> = entry
            .targets
            .iter()
            .map(|target| (target.tool.clone(), target.scope.clone()))
            .collect();
        if lock_targets != snapshot.targets {
            targets_changed.push(entry.name.clone());
        }
    }

    let extra_here = local_by_name
        .keys()
        .filter(|name| !lock_names.contains(name.as_str()))
        .cloned()
        .collect::<Vec<_>>();

    LockDiffReport {
        lock_path: lock_path.to_string(),
        missing_here,
        extra_here,
        drifted,
        targets_changed,
    }
}

/// Fresh content fingerprint of a skill's central directory. Falls back to the
/// last hash recorded in the database when the directory cannot be read (for
/// example a missing central copy), so such skills still surface as drift.
fn current_content_hash(record: &SkillRecord) -> String {
    let central_path = Path::new(&record.central_path);
    if central_path.is_dir() {
        if let Ok(hash) = hash_dir(central_path) {
            return hash;
        }
    }
    record.content_hash.clone().unwrap_or_default()
}

/// Format unix seconds as an RFC3339 UTC timestamp (e.g. `2026-01-01T00:00:00Z`).
pub fn format_rfc3339_utc(unix_secs: u64) -> String {
    let days = unix_secs / 86_400;
    let secs_of_day = unix_secs % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year,
        month,
        day,
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60
    )
}

pub fn now_rfc3339_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format_rfc3339_utc(secs)
}

/// Howard Hinnant's `civil_from_days`: days since 1970-01-01 to (y, m, d).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
#[path = "tests/skills_lock.rs"]
mod tests;
