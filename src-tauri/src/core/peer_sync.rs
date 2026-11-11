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

/// 单文件上限：超过的不参与同步，避免把模型权重、视频之类的东西带进全库。
const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
/// 单技能文件数上限：超过就不自动同步，防止脚本生成的临时产物扩散。
pub const MAX_FILES_PER_SKILL: usize = 500;

/// 生成物和临时文件：不参与比对，也不参与同步。
const IGNORED_SUFFIXES: &[&str] = &[
    ".pyc", ".pyo", ".pyd", ".tmp", ".temp", ".swp", ".swx", ".bak", ".orig",
];
const IGNORED_NAMES: &[&str] = &[".DS_Store", "Thumbs.db", "desktop.ini"];

/// 判断一个相对路径是否该被忽略
fn is_ignored_rel(rel: &str) -> bool {
    if rel.starts_with(".git/") || rel.contains("__pycache__") || rel.contains(".pytest_cache") {
        return true;
    }
    let name = rel.rsplit('/').next().unwrap_or(rel);
    if IGNORED_NAMES
        .iter()
        .any(|ignored| name.eq_ignore_ascii_case(ignored))
    {
        return true;
    }
    // Office 保存时产生的锁文件：~$xxx.docx
    if name.starts_with("~$") {
        return true;
    }
    let lower = name.to_ascii_lowercase();
    IGNORED_SUFFIXES
        .iter()
        .any(|suffix| lower.ends_with(suffix))
}

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
    /// 偏离的那份命中历史版本：看起来是回滚，不扩散
    Rollback { from: String },
    /// 文件太多，按保护策略跳过
    TooManyFiles { count: usize },
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
        if is_ignored_rel(&rel) {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.len() > MAX_FILE_BYTES {
            log::warn!("peer_sync: 跳过超大文件 {rel}（{} KB）", meta.len() / 1024);
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

/// 整个目录的指纹：把每个文件的相对路径和 hash 串起来再哈希一次
fn snapshot_fingerprint(snap: &Snapshot) -> String {
    let mut hasher = Sha256::new();
    for (rel, (hash, _size)) in snap {
        hasher.update(rel.as_bytes());
        hasher.update(hash.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

/// 上一轮扫描看到的「待采纳」指纹。
///
/// 只有当同一个指纹连续两轮出现时才真正同步——避免编辑器保存到一半
/// 或者脚本正在生成文件时，把半成品扩散到所有副本。
fn pending_slots() -> &'static std::sync::Mutex<std::collections::HashMap<String, String>> {
    static PENDING: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, String>>,
    > = std::sync::OnceLock::new();
    PENDING.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
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
pub fn reconcile_skill(
    store: &SkillStore,
    skill: &SkillRecord,
    require_stable: bool,
) -> Result<ReconcileOutcome> {
    let baseline = baseline_map(store, &skill.id);
    let targets = store.list_skill_targets(&skill.id)?;

    let mut peers: Vec<(String, PathBuf, Snapshot)> = Vec::new();
    for target in &targets {
        let path = PathBuf::from(&target.target_path);
        let snap = snapshot(&path);
        if snap.is_empty() {
            continue;
        }
        if snap.len() > MAX_FILES_PER_SKILL {
            log::warn!(
                "[peer_sync] {} 的 {} 有 {} 个文件，超过上限 {}，跳过自动同步",
                skill.name,
                target.target_path,
                snap.len(),
                MAX_FILES_PER_SKILL
            );
            return Ok(ReconcileOutcome::TooManyFiles { count: snap.len() });
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
            let _ = store.push_baseline_history(&skill.id, &snapshot_fingerprint(&snap), 5);
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

    // 只剩一个版本：先确认它不是「回滚」，再确认它已经稳定
    let (from_label, src_path, src_snap) = edited[0].clone();
    let candidate_fp = snapshot_fingerprint(&src_snap);

    // 命中历史上出现过的版本 → 这是回滚，不是新编辑，绝不扩散
    if let Ok(history) = store.get_baseline_history(&skill.id, 5) {
        if history.iter().any(|known| known == &candidate_fp) {
            log::warn!(
                "[peer_sync] {} 的 {} 回到了历史版本，判为回滚，不扩散",
                skill.name,
                from_label
            );
            return Ok(ReconcileOutcome::Rollback { from: from_label });
        }
    }

    // 二次确认：同一个指纹要连续两轮都看到才动手，避免扩散写了一半的内容
    if require_stable {
        let mut slots = pending_slots()
            .lock()
            .map_err(|_| anyhow::anyhow!("pending lock poisoned"))?;
        match slots.get(&skill.id) {
            Some(previous) if previous == &candidate_fp => {
                slots.remove(&skill.id);
            }
            _ => {
                slots.insert(skill.id.clone(), candidate_fp.clone());
                log::info!(
                    "[peer_sync] {} 待确认（{} 偏离，等下一轮复核）",
                    skill.name,
                    from_label
                );
                return Ok(ReconcileOutcome::NoChange);
            }
        }
    }
    // 旧基线先进历史，供下一轮识别「回滚」
    let old_fp = snapshot_fingerprint(&baseline);
    let _ = store.push_baseline_history(&skill.id, &old_fp, 5);

    let entries: Vec<(String, String, i64)> = src_snap
        .iter()
        .map(|(rel, (hash, size))| (rel.clone(), hash.clone(), *size))
        .collect();
    store.replace_skill_baseline(&skill.id, &entries)?;
    let _ = store.push_baseline_history(&skill.id, &candidate_fp, 5);

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
        match reconcile_skill(store, skill, true) {
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
            Ok(ReconcileOutcome::Rollback { from }) => {
                conflicts += 1;
                log::warn!(
                    "[peer_sync] {} 的 {} 退回历史版本，已跳过（如确需生效请手动处理）",
                    skill.name,
                    from
                );
            }
            Ok(ReconcileOutcome::TooManyFiles { count }) => {
                log::warn!("[peer_sync] {} 文件过多（{count}），跳过", skill.name);
                unchanged += 1;
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
    ordered.sort_by_key(|entry| std::cmp::Reverse(entry.1));
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
        if is_ignored_rel(&rel) {
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

/// 指定某个副本为准：把它推给所有其它副本，并把它记为新的基线。
///
/// 用于「这份才是最新的」——比逐个点同步更符合真实意图。
pub fn adopt_from(store: &SkillStore, skill: &SkillRecord, source_path: &Path) -> Result<usize> {
    let snap = snapshot(source_path);
    if snap.is_empty() {
        anyhow::bail!("EMPTY_SOURCE|{}", source_path.display());
    }

    let entries: Vec<(String, String, i64)> = snap
        .iter()
        .map(|(rel, (hash, size))| (rel.clone(), hash.clone(), *size))
        .collect();
    store.replace_skill_baseline(&skill.id, &entries)?;

    let mut updated = 0usize;
    for target in store.list_skill_targets(&skill.id)? {
        let path = PathBuf::from(&target.target_path);
        if path == source_path {
            continue;
        }
        if snapshot(&path) == snap {
            continue;
        }
        match sync_dir_copy_with_overwrite(source_path, &path, true) {
            Ok(_) => updated += 1,
            Err(err) => log::warn!(
                "[peer_sync] 采用 {} 时更新 {} 失败: {err:#}",
                source_path.display(),
                target.target_path
            ),
        }
    }
    Ok(updated)
}

#[cfg(test)]
#[path = "tests/peer_sync.rs"]
mod tests;
