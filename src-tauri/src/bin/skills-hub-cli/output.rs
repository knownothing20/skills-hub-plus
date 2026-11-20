//! 终端输出：状态表、目录差异、上下文对象。
//!
//! 不用第三方表格库——输出量小，对齐规则简单，少一个依赖少一处版本冲突。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use app_lib::core::peer_sync::version_labels;
use app_lib::core::skill_store::{SkillRecord, SkillStore};
use app_lib::core::stale_copies::scan_dir_latest_mtime;

use crate::paths;

/// 一次调用的共享状态。
pub struct Ctx {
    store: SkillStore,
    central_override: Option<PathBuf>,
    central_cache: Option<PathBuf>,
    pub dry_run: bool,
    pub as_json: bool,
}

impl Ctx {
    pub fn new(
        db: PathBuf,
        central_override: Option<PathBuf>,
        dry_run: bool,
        as_json: bool,
    ) -> Result<Self> {
        let store = paths::open_store(&db)?;
        Ok(Self {
            store,
            central_override,
            central_cache: None,
            dry_run,
            as_json,
        })
    }

    pub fn store(&self) -> &SkillStore {
        &self.store
    }

    /// 母库根目录（首次访问时解析并缓存）。
    ///
    /// 只在技能记录里的 `central_path` 失效时用作兜底（见 `central_dir_of`）。
    /// 正常路径下每个技能自己就有 `central_path`，不需要它。
    pub fn central_root(&mut self) -> Result<PathBuf> {
        if let Some(p) = &self.central_cache {
            return Ok(p.clone());
        }
        let resolved = paths::resolve_central(&self.store, self.central_override.clone())?;
        self.central_cache = Some(resolved.clone());
        Ok(resolved)
    }

    /// 取某个技能的母库目录。
    ///
    /// 优先用记录里的 `central_path`；为空或目录不存在时，退回 `--central`
    /// 指定的根目录 + 技能名。这样 `adopt`/`push` 不会因为一条脏记录就
    /// 完全不碰母库，而是退到一个合理位置并给出提示。
    pub fn central_dir_of(&mut self, skill: &SkillRecord) -> Result<PathBuf> {
        let from_record = PathBuf::from(&skill.central_path);
        if !skill.central_path.trim().is_empty() && from_record.is_dir() {
            return Ok(from_record);
        }
        let root = self.central_root()?;
        Ok(root.join(&skill.name))
    }
}

/// 一个技能在版本字母表里的位置。
///
/// 口径来自 `version_labels`：它在内部把**母库也算进去**（母库不在
/// `skill_targets` 表里），并按内容指纹分组、按组内最新时间降序打字母。
/// 直接复用它，是为了让「谁是新版本」只有一处定义——CLI 和 GUI 的卡片角标
/// 永远给同一个答案。
pub struct SkillVersionView {
    pub name: String,
    /// 每个位置 -> 版本字母（只有一种版本时为空）
    pub labels: BTreeMap<String, String>,
}

/// 汇总所有需要关注的技能：只要版本字母表非空，就说明存在两个以上不同内容。
pub fn collect_versions(ctx: &Ctx, skills: &[SkillRecord]) -> Vec<SkillVersionView> {
    let mut out = Vec::new();
    for skill in skills {
        let Ok(labels) = version_labels(ctx.store(), skill) else {
            continue;
        };
        if labels.is_empty() {
            continue;
        }
        out.push(SkillVersionView {
            name: skill.name.clone(),
            labels: labels.into_iter().collect(),
        });
    }
    out
}

/// 打印技能总览。只列出**存在差异**的技能，全部一致的收在最后一行汇总，
/// 否则 41 个技能刷屏之后真正要看的那几行反而找不着。
///
/// 「最新」的判定沿用 `version_labels` 的字母顺序：A 是最近被编辑的那一组，
/// 其余字母都比它旧。母库固定显示为 `母库`，与副本同列可比。
pub fn print_skill_table(ctx: &Ctx, skills: &[SkillRecord], views: &[SkillVersionView]) {
    if ctx.as_json {
        let items: Vec<_> = views
            .iter()
            .map(|view| {
                serde_json::json!({
                    "skill": view.name,
                    "positions": view.labels.iter().map(|(k, v)| serde_json::json!({
                        "position": k,
                        "version": v,
                    })).collect::<Vec<_>>()
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({ "differing": items, "total_skills": skills.len() })
        );
        return;
    }

    if views.is_empty() {
        println!(
            "全部 {} 个技能在各软件之间内容一致，无需同步。",
            skills.len()
        );
        return;
    }

    println!(
        "以下技能存在版本差异（共 {} 个技能，A 为最新）：",
        views.len()
    );
    println!();
    for view in views {
        println!("● {}", view.name);
        for (position, letter) in &view.labels {
            let role = if position == "central" {
                "母库"
            } else {
                position.as_str()
            };
            println!("    {:<34} {}", role, letter);
        }
        println!();
    }
    let consistent = skills.len().saturating_sub(views.len());
    println!("其余 {consistent} 个技能一致。");
    println!();
    println!("把最新版铺到所有位置（含母库）：skills-hub-cli pull");
    println!("只看某个技能的差异：              skills-hub-cli diff <技能名>");
}

/// 展开某个技能各位置的文件差异。
///
/// 先把每份副本的文件清单指纹算出来，再按指纹分组：同组内容相同，跨组才有差异。
/// 这样即使 9 份副本，也只显示「几组」，而不是列 9 遍。
/// 展开某个技能各位置的文件差异。
///
/// `central` 由调用方传进来，而不是在这里直接读 `skill.central_path`——
/// 那份记录可能是空的或失效的，`Ctx::central_dir_of` 已经统一做了兜底。
/// 两处各读一次字段，迟早会出现「diff 显示的母库」和「pull 写进去的母库」
/// 不是同一个目录。
///
/// 先把每份副本的文件清单指纹算出来，再按指纹分组：同组内容相同，跨组才有
/// 差异。这样即使 9 份副本，也只显示「几组」，而不是列 9 遍。
pub fn diff_dirs(ctx: &Ctx, skill: &SkillRecord, central: &Path) -> Result<()> {
    let store = ctx.store();
    let targets = store.list_skill_targets(&skill.id)?;

    struct Place {
        label: String,
        path: PathBuf,
        mtime: u64,
        files: usize,
        fingerprint: String,
    }

    let mut places: Vec<Place> = Vec::new();

    // 母库作为首个位置参与比较（它不在 skill_targets 里，必须单独加）。
    if central.is_dir() {
        let (mtime, files) = scan_dir_latest_mtime(central);
        places.push(Place {
            label: "母库".to_string(),
            fingerprint: fingerprint(central),
            path: central.to_path_buf(),
            mtime,
            files,
        });
    }
    for t in &targets {
        let p = PathBuf::from(&t.target_path);
        if !p.is_dir() {
            continue;
        }
        let (mtime, files) = scan_dir_latest_mtime(&p);
        let label = if t.instance.is_empty() {
            t.tool.clone()
        } else {
            format!("{}#{}", t.tool, t.instance)
        };
        places.push(Place {
            label,
            fingerprint: fingerprint(&p),
            path: p,
            mtime,
            files,
        });
    }

    println!("技能：{}", skill.name);
    println!();
    println!("{:<22} {:<8} {:<10} 路径", "位置", "文件数", "最新修改");
    for p in &places {
        println!(
            "{:<22} {:<8} {:<10} {}",
            p.label,
            p.files,
            format_time(p.mtime),
            p.path.display()
        );
    }
    println!();

    // 按指纹分组
    let mut groups: std::collections::BTreeMap<&str, Vec<&Place>> =
        std::collections::BTreeMap::new();
    for p in &places {
        groups.entry(p.fingerprint.as_str()).or_default().push(p);
    }

    if groups.len() <= 1 {
        println!("所有位置内容一致。");
        return Ok(());
    }

    println!("内容分成 {} 组（同组内容完全相同）：", groups.len());
    let group_list: Vec<Vec<&Place>> = groups.into_values().collect();
    for (idx, group) in group_list.iter().enumerate() {
        let labels: Vec<&str> = group.iter().map(|p| p.label.as_str()).collect();
        // 组内取最新时间的那份当代表，用来判断这组是不是最近被改过。
        let newest = group.iter().map(|p| p.mtime).max().unwrap_or(0);
        println!();
        println!(
            "  组 {}（{}）：{}",
            idx + 1,
            format_time(newest),
            labels.join("、")
        );
    }

    println!();
    println!("要指定以某组为准：");
    println!("  skills-hub-cli adopt {} --from <工具名>", skill.name);
    println!("（多实例工具需再加 --instance，例如 --from doubao_work --instance \"Profile 2\"）");
    Ok(())
}

/// 目录指纹：文件名 + 内容哈希的汇总。用 `content_hash` 同一套忽略规则，
/// 保证脚本与 GUI 看到的是同一批文件。
fn fingerprint(dir: &Path) -> String {
    app_lib::core::content_hash::hash_dir(dir).unwrap_or_default()
}

fn format_time(ms: u64) -> String {
    if ms == 0 {
        return "-".to_string();
    }
    // 只显示相对时间，绝对值对「谁更新」这个判断没有帮助。
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    if now == 0 || ms >= now {
        return "刚刚".to_string();
    }
    let secs = (now - ms) / 1000;
    match secs {
        0..=59 => "刚刚".to_string(),
        60..=3599 => format!("{} 分钟前", secs / 60),
        3600..=86399 => format!("{} 小时前", secs / 3600),
        _ => format!("{} 天前", secs / 86400),
    }
}
