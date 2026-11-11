//! 对等同步：技能不再有「主 skill」。
//!
//! 每个技能记住一份「基线」——上次所有副本一致时的文件指纹。之后哪个副本
//! 的内容偏离基线，哪个就承载了最新编辑，由它推给其它副本。多个副本同时
//! 偏离且内容不同，才算冲突，交给用户决定。
//!
//! 时间不参与判定，只用来提示「这份是什么时候改的」。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::core::skill_store::{SkillRecord, SkillStore};
use crate::core::sync_engine::sync_dir_copy_with_overwrite;

type Snapshot = BTreeMap<String, (String, i64)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerState {
    /// 与基线一致
    InSync,
    /// 内容偏离基线，说明有过本地编辑
    Edited,
    /// 目录不存在或为空
    Missing,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeerStanding {
    pub tool: String,
    pub instance: String,
    pub path: String,
    pub state: PeerState,
    pub changed_files: Vec<String>,
    pub file_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReconcileOutcome {
    /// 大家都和基线一致
    NoChange,
    /// 第一次建立基线
    Initialized { from: String },
    /// 采纳了某个副本的改动，并推给了其它副本
    Adopted { from: String, updated: Vec<String> },
    /// 多个副本各自改过且内容不同
    Conflict {
        candidates: Vec<String>,
        changed_files: Vec<String>,
    },
}

/// 目录内容快照：相对路径 -> (文件 hash, 字节数)
fn snapshot(root: &Path) -> Snapshot {
    let mut out = Snapshot::new();
    if !root.is_dir() {
        return out;
    }
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if rel.starts_with(".git/") || rel.contains("__pycache__") {
            continue;
        }
        let Ok(bytes) = std::fs::read(entry.path()) else {
            continue;
        };
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        out.insert(
            rel,
            (format!("{:x}", hasher.finalize()), bytes.len() as i64),
        );
    }
    out
}

fn baseline_map(store: &SkillStore, skill_id: &str) -> Snapshot {
    store
        .get_skill_baseline(skill_id)
        .unwrap_or_default()
        .into_iter()
        .map(|(rel, hash, size)| (rel, (hash, size)))
        .collect()
}

fn diff_against(baseline: &Snapshot, snap: &Snapshot) -> Vec<String> {
    let mut keys: BTreeSet<String> = BTreeSet::new();
    for (rel, value) in snap {
        if baseline.get(rel) != Some(value) {
            keys.insert(rel.clone());
        }
    }
    for rel in baseline.keys() {
        if !snap.contains_key(rel) {
            keys.insert(rel.clone());
        }
    }
    keys.into_iter().collect()
}

fn label_for(target: &crate::core::skill_store::SkillTargetRecord) -> String {
    if target.instance.is_empty() {
        target.tool.clone()
    } else {
        format!("{} / {}", target.tool, target.instance)
    }
}

/// 列出每个副本相对基线的位置（只读）
pub fn scan_peers(store: &SkillStore, skill: &SkillRecord) -> Result<Vec<PeerStanding>> {
    let mut baseline = baseline_map(store, &skill.id);
    // 还没有基线（刚升级或新技能）：先用来源目录的快照当参照，
    // 否则所有副本都会被判成「一致」，看不出谁偏离。
    if baseline.is_empty() {
        if let Some(raw) = skill.source_ref.as_deref() {
            baseline = snapshot(Path::new(raw));
        }
    }
    let mut out = Vec::new();
    for target in store.list_skill_targets(&skill.id)? {
        let path = PathBuf::from(&target.target_path);
        let snap = snapshot(&path);
        let state = if snap.is_empty() {
            PeerState::Missing
        } else if baseline.is_empty() || snap == baseline {
            PeerState::InSync
        } else {
            PeerState::Edited
        };
        let changed_files = if state == PeerState::Edited {
            diff_against(&baseline, &snap)
        } else {
            Vec::new()
        };
        out.push(PeerStanding {
            tool: target.tool,
            instance: target.instance,
            path: target.target_path,
            state,
            changed_files,
            file_count: snap.len(),
        });
    }
    Ok(out)
}

/// 对一个技能做一次对等同步
pub fn reconcile_skill(store: &SkillStore, skill: &SkillRecord) -> Result<ReconcileOutcome> {
    let baseline = baseline_map(store, &skill.id);
    let targets = store.list_skill_targets(&skill.id)?;

    let mut peers: Vec<(String, PathBuf, Snapshot)> = Vec::new();
    for target in &targets {
        let path = PathBuf::from(&target.target_path);
        let snap = snapshot(&path);
        if snap.is_empty() {
            continue;
        }
        peers.push((label_for(target), path, snap));
    }
    if peers.is_empty() {
        return Ok(ReconcileOutcome::NoChange);
    }

    // 还没有基线：优先用来源目录，否则用第一个非空副本，建立基线
    if baseline.is_empty() {
        let source = skill
            .source_ref
            .as_deref()
            .and_then(|raw| {
                let p = Path::new(raw);
                peers
                    .iter()
                    .find(|(_, path, _)| path.as_path() == p)
                    .cloned()
            })
            .or_else(|| peers.first().cloned());
        if let Some((label, _, snap)) = source {
            let entries: Vec<(String, String, i64)> = snap
                .iter()
                .map(|(rel, (hash, size))| (rel.clone(), hash.clone(), *size))
                .collect();
            store.replace_skill_baseline(&skill.id, &entries)?;
            return Ok(ReconcileOutcome::Initialized { from: label });
        }
        return Ok(ReconcileOutcome::NoChange);
    }

    let edited: Vec<(String, PathBuf, Snapshot)> = peers
        .iter()
        .filter(|(_, _, snap)| snap != &baseline)
        .cloned()
        .collect();
    if edited.is_empty() {
        return Ok(ReconcileOutcome::NoChange);
    }

    // 偏离基线的多个副本内容不一致 → 冲突，不做任何写入
    let first_snap = &edited[0].2;
    if edited.iter().any(|(_, _, snap)| snap != first_snap) {
        let mut changed: BTreeSet<String> = BTreeSet::new();
        for (_, _, snap) in &edited {
            for rel in diff_against(&baseline, snap) {
                changed.insert(rel);
            }
        }
        return Ok(ReconcileOutcome::Conflict {
            candidates: edited.iter().map(|(label, _, _)| label.clone()).collect(),
            changed_files: changed.into_iter().collect(),
        });
    }

    // 只剩一个版本：它承载了最新编辑，采纳并推给其它副本
    let (from_label, src_path, src_snap) = edited[0].clone();
    let entries: Vec<(String, String, i64)> = src_snap
        .iter()
        .map(|(rel, (hash, size))| (rel.clone(), hash.clone(), *size))
        .collect();
    store.replace_skill_baseline(&skill.id, &entries)?;

    let mut updated = Vec::new();
    for (label, path, snap) in &peers {
        if snap == &src_snap {
            continue;
        }
        match sync_dir_copy_with_overwrite(&src_path, path, true) {
            Ok(_) => updated.push(label.clone()),
            Err(err) => log::warn!("peer sync to {label} failed: {err:#}"),
        }
    }
    Ok(ReconcileOutcome::Adopted {
        from: from_label,
        updated,
    })
}

/// 对所有技能跑一遍，返回 (采纳数, 冲突数, 无变化数)
pub fn reconcile_all(store: &SkillStore) -> Result<(usize, usize, usize)> {
    let skills = store.list_skills()?;
    let (mut adopted, mut conflicts, mut unchanged) = (0usize, 0usize, 0usize);
    for skill in &skills {
        match reconcile_skill(store, skill) {
            Ok(ReconcileOutcome::Adopted { from, updated }) => {
                adopted += 1;
                log::info!(
                    "[peer_sync] {} 采纳来自 {} 的改动，更新 {} 个副本",
                    skill.name,
                    from,
                    updated.len()
                );
            }
            Ok(ReconcileOutcome::Conflict { candidates, .. }) => {
                conflicts += 1;
                log::warn!(
                    "[peer_sync] {} 冲突：{} 都改过且内容不同",
                    skill.name,
                    candidates.join(" / ")
                );
            }
            Ok(ReconcileOutcome::Initialized { from }) => {
                log::info!("[peer_sync] {} 建立基线，来源 {}", skill.name, from);
            }
            Ok(ReconcileOutcome::NoChange) => unchanged += 1,
            Err(err) => log::warn!("[peer_sync] {} 处理失败: {err:#}", skill.name),
        }
    }
    Ok((adopted, conflicts, unchanged))
}

/// 版本字母：把同一技能的副本按内容分组，按「组内最新修改时间」降序编号。
/// A 永远是最近被编辑的那一版；只有一个版本时不标字母。
pub fn version_labels(
    store: &SkillStore,
    skill: &SkillRecord,
) -> Result<std::collections::HashMap<String, String>> {
    let targets = store.list_skill_targets(&skill.id)?;

    // 先算出每个副本的快照与最新时间
    struct Peer {
        key: String,
        fingerprint: String,
        newest: u64,
    }
    let mut peers: Vec<Peer> = Vec::new();
    for target in &targets {
        let path = PathBuf::from(&target.target_path);
        if !path.is_dir() {
            continue;
        }
        let (fingerprint, newest) = snapshot_with_time(&path);
        if fingerprint.is_empty() {
            continue;
        }
        peers.push(Peer {
            key: target_key(&target.tool, &target.instance),
            fingerprint,
            newest,
        });
    }

    let mut by_fp: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for peer in &peers {
        let entry = by_fp.entry(peer.fingerprint.clone()).or_insert(0);
        if peer.newest > *entry {
            *entry = peer.newest;
        }
    }
    // 只有一个版本：不标
    if by_fp.len() <= 1 {
        return Ok(std::collections::HashMap::new());
    }

    let mut ordered: Vec<(String, u64)> = by_fp.into_iter().collect();
    ordered.sort_by(|a, b| b.1.cmp(&a.1));
    let mut label_of: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for (index, (fp, _)) in ordered.iter().enumerate() {
        let letter = char::from(b'A' + (index as u8 % 26)).to_string();
        label_of.insert(fp.clone(), letter);
    }

    let mut out = std::collections::HashMap::new();
    for peer in peers {
        if let Some(letter) = label_of.get(&peer.fingerprint) {
            out.insert(peer.key, letter.clone());
        }
    }
    Ok(out)
}

fn target_key(tool: &str, instance: &str) -> String {
    if instance.is_empty() {
        tool.to_string()
    } else {
        format!("{tool}#{instance}")
    }
}

/// 目录指纹（不含逐文件明细）与最新文件时间
fn snapshot_with_time(root: &Path) -> (String, u64) {
    let mut hasher = Sha256::new();
    let mut newest = 0u64;
    let mut found = false;
    let mut entries: Vec<(String, PathBuf)> = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if rel.starts_with(".git/") || rel.contains("__pycache__") {
            continue;
        }
        entries.push((rel, entry.path().to_path_buf()));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    for (rel, path) in entries {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        hasher.update(rel.as_bytes());
        hasher.update(&bytes);
        if let Ok(meta) = std::fs::metadata(&path) {
            if let Ok(modified) = meta.modified() {
                if let Ok(dur) = modified.duration_since(std::time::UNIX_EPOCH) {
                    newest = newest.max(dur.as_millis() as u64);
                }
            }
        }
        found = true;
    }
    if !found {
        return (String::new(), 0);
    }
    (format!("{:x}", hasher.finalize()), newest)
}
