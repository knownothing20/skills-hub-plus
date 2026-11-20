//! `skills-hub-cli`：Skills Hub 的命令行同步工具。
//!
//! 存在的理由：GUI 里同步一个技能要点开卡片、右键、逐个判断谁新，改五六个技能
//! 就是十几次点击。这个工具把同一套核心逻辑（`app_lib::core`）暴露成一条命令，
//! 让「在别的软件改完 skill，回终端收一下」成为可能。
//!
//! 与 GUI 的关系：**共用同一份实现**，不是另写一套同步算法。DB 路径、`.skillignore`
//! 语义、对等模型的三方比对全部一致，因此两者永远不会给出不同结论。

use std::path::PathBuf;
use std::process::ExitCode;

use app_lib::core::peer_sync::{adopt_from, reconcile_skill, ReconcileOutcome};
use app_lib::core::stale_copies::{scan_stale_copy_targets, StaleVerdict};

mod output;
mod paths;
mod resolve;

use output::{diff_dirs, print_skill_table, Ctx};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("错误：{err}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> &'static str {
    "skills-hub-cli —— Skills Hub 命令行同步

用法：
  skills-hub-cli status                  全库差异一览（谁新、谁旧、谁冲突）
  skills-hub-cli pull  [技能名...]       把副本的改动收回来：以最新副本为准，铺到母库与其它副本
  skills-hub-cli push  [技能名...]       把母库内容推给落后副本（只处理能确定方向的）
  skills-hub-cli diff  <技能名>          列出各副本之间的差异，供你决定
  skills-hub-cli adopt <技能名> --from <工具名> [--instance <实例>]
                                         指定以某一份副本为准，推到其余所有位置

选项：
  --db <路径>       指定数据库（默认自动定位 Skills Hub 的 skills_hub.db）
  --central <路径>  指定母库根目录（默认从数据库设置读取）
  --dry-run         只显示要做什么，不写任何文件
  --json            以 JSON 输出，便于脚本处理
  -h, --help        显示本帮助"
}

fn run(args: &[String]) -> anyhow::Result<ExitCode> {
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{}", usage());
        return Ok(ExitCode::SUCCESS);
    }

    // 先扫出全局选项，剩下的第一个词才是子命令。
    let mut db_override: Option<PathBuf> = None;
    let mut central_override: Option<PathBuf> = None;
    let mut dry_run = false;
    let mut as_json = false;
    let mut rest: Vec<String> = Vec::new();
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--db" => db_override = iter.next().map(PathBuf::from),
            "--central" => central_override = iter.next().map(PathBuf::from),
            "--dry-run" => dry_run = true,
            "--json" => as_json = true,
            _ => rest.push(arg.clone()),
        }
    }

    let Some(command) = rest.first() else {
        println!("{}", usage());
        return Ok(ExitCode::SUCCESS);
    };
    let mut ctx = Ctx::new(
        paths::resolve_db(db_override)?,
        central_override,
        dry_run,
        as_json,
    )?;

    match command.as_str() {
        "status" => cmd_status(&mut ctx),
        "pull" => cmd_pull(&mut ctx, &rest[1..]),
        "push" => cmd_push(&mut ctx, &rest[1..]),
        "diff" => cmd_diff(&mut ctx, &rest[1..]),
        "adopt" => cmd_adopt(&mut ctx, &rest[1..]),
        other => {
            eprintln!("未知命令：{other}\n\n{}", usage());
            Ok(ExitCode::FAILURE)
        }
    }
}

/// `status`：把每个技能的每一份副本按「相对母库」的结论归类。
fn cmd_status(ctx: &mut Ctx) -> anyhow::Result<ExitCode> {
    let store = ctx.store();
    let skills = store.list_skills()?;
    let stale = scan_stale_copy_targets(store)?;

    print_skill_table(ctx, &skills, &stale);

    let behind = stale
        .iter()
        .filter(|s| s.verdict == StaleVerdict::Behind)
        .count();
    let hand_edited = stale
        .iter()
        .filter(|s| s.verdict == StaleVerdict::HandEdited)
        .count();
    let conflict = stale
        .iter()
        .filter(|s| s.verdict == StaleVerdict::Conflict)
        .count();

    if !ctx.as_json {
        println!();
        println!(
            "共 {} 个技能；副本落后 {behind} 个，副本有独有改动 {hand_edited} 个，方向不明 {conflict} 个。",
            skills.len()
        );
        if hand_edited > 0 {
            println!(
                "有独有改动的副本用 `skills-hub-cli pull` 收回，或用 `diff <技能名>` 先看差异。"
            );
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// `pull`：你的主诉求——在别的软件改完，把改动收回来并铺开。
///
/// 口径：对每个技能跑一遍对等比对（`reconcile_skill`）。只有一份副本偏离基线时
/// 它会被采纳并推给其他所有位置，这正是「在 Codex 里改了，其它软件跟着更新」。
/// 多份副本各改各的（`Conflict`）时**不自动决定**，列出来等你用 `adopt` 指定。
fn cmd_pull(ctx: &mut Ctx, names: &[String]) -> anyhow::Result<ExitCode> {
    let targets = resolve::select_skills(ctx.store(), names)?;
    if targets.is_empty() {
        println!("没有匹配的技能。");
        return Ok(ExitCode::SUCCESS);
    }

    let (mut adopted, mut conflicts, mut unchanged) = (0usize, 0usize, 0usize);
    let mut conflict_names: Vec<String> = Vec::new();

    for skill in &targets {
        if ctx.dry_run {
            continue;
        }
        match reconcile_skill(ctx.store(), skill, true) {
            Ok(ReconcileOutcome::Adopted { from, updated }) => {
                adopted += 1;
                if !ctx.as_json {
                    println!(
                        "✅ {} ← 采纳自 {from}，更新 {} 处副本",
                        skill.name,
                        updated.len()
                    );
                }
            }
            Ok(ReconcileOutcome::Conflict { candidates, .. }) => {
                conflicts += 1;
                conflict_names.push(skill.name.clone());
                if !ctx.as_json {
                    println!(
                        "⚠️  {} 有 {} 份副本各改各的（{}），不自动处理；用 `diff {}` 看差异后用 `adopt` 指定",
                        skill.name,
                        candidates.len(),
                        candidates.join(" / "),
                        skill.name
                    );
                }
            }
            Ok(ReconcileOutcome::TooManyFiles { count }) => {
                unchanged += 1;
                if !ctx.as_json {
                    println!("⏭️  {} 文件过多（{count}），跳过", skill.name);
                }
            }
            Ok(ReconcileOutcome::Rollback { from }) => {
                conflicts += 1;
                conflict_names.push(skill.name.clone());
                if !ctx.as_json {
                    println!("⚠️  {} 的 {from} 看起来是回滚，已跳过", skill.name);
                }
            }
            Ok(ReconcileOutcome::Initialized { from }) => {
                unchanged += 1;
                if !ctx.as_json {
                    println!("ℹ️  {} 首次建立基线，来源 {from}", skill.name);
                }
            }
            Ok(ReconcileOutcome::NoChange) => unchanged += 1,
            Err(err) => {
                eprintln!("❌ {} 处理失败：{err:#}", skill.name);
            }
        }
    }

    if ctx.as_json {
        println!(
            "{}",
            serde_json::json!({
                "adopted": adopted,
                "conflicts": conflict_names,
                "unchanged": unchanged,
                "dry_run": ctx.dry_run,
            })
        );
    } else if ctx.dry_run {
        println!("（--dry-run：未写入任何文件）");
    } else {
        println!();
        println!("采纳 {adopted} 个，无变化 {unchanged} 个，需你决定 {conflicts} 个。");
    }
    Ok(ExitCode::SUCCESS)
}

/// `push`：只把母库的内容推给明确落后的副本。
///
/// 与 `pull` 的区别是方向：`push` 不采纳副本改动，只修「母库更新了但副本没跟上」。
fn cmd_push(ctx: &mut Ctx, names: &[String]) -> anyhow::Result<ExitCode> {
    let selected = resolve::select_skills(ctx.store(), names)?;
    let selected_ids: Vec<&str> = selected.iter().map(|s| s.id.as_str()).collect();
    let stale = scan_stale_copy_targets(ctx.store())?;

    let behind: Vec<_> = stale
        .iter()
        .filter(|s| s.verdict == StaleVerdict::Behind)
        .filter(|s| names.is_empty() || selected_ids.iter().any(|id| *id == s.skill_id))
        .collect();

    if behind.is_empty() {
        println!("没有需要推送的副本（母库不比任何副本新）。");
        return Ok(ExitCode::SUCCESS);
    }
    if ctx.dry_run {
        for s in &behind {
            println!("将推送 {} → {}（{}）", s.skill_name, s.tool, s.target_path);
        }
        println!("（--dry-run：未写入任何文件）");
        return Ok(ExitCode::SUCCESS);
    }

    let central_root = ctx.central_root()?;
    let mut refreshed = 0usize;
    for s in &behind {
        match app_lib::core::stale_copies::refresh_stale_copy(
            ctx.store(),
            &central_root,
            &s.skill_id,
            &s.tool,
        ) {
            Ok(()) => {
                refreshed += 1;
                println!("✅ {} → {}（{}）", s.skill_name, s.tool, s.target_path);
            }
            Err(err) => eprintln!("❌ {} → {} 失败：{err:#}", s.skill_name, s.tool),
        }
    }
    println!();
    println!("推送 {refreshed} 个副本。");
    Ok(ExitCode::SUCCESS)
}

/// `diff`：把某个技能各位置的文件清单与内容差异摊开，供人选。
fn cmd_diff(ctx: &mut Ctx, names: &[String]) -> anyhow::Result<ExitCode> {
    let Some(name) = names.first() else {
        eprintln!("用法：skills-hub-cli diff <技能名>\n可先用 `status` 看有哪些技能不一致。");
        return Ok(ExitCode::FAILURE);
    };
    let skill = resolve::find_skill_by_name(ctx.store(), name)?;
    diff_dirs(ctx, &skill)?;
    Ok(ExitCode::SUCCESS)
}

/// `adopt`：明确指定以谁为准。
fn cmd_adopt(ctx: &mut Ctx, args: &[String]) -> anyhow::Result<ExitCode> {
    let Some(name) = args.first() else {
        eprintln!("用法：skills-hub-cli adopt <技能名> --from <工具名> [--instance <实例>]");
        return Ok(ExitCode::FAILURE);
    };
    let mut from_tool: Option<String> = None;
    let mut instance = String::new();
    let mut iter = args.iter().skip(1).peekable();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--from" => from_tool = iter.next().cloned(),
            "--instance" => instance = iter.next().cloned().unwrap_or_default(),
            _ => {}
        }
    }
    let Some(tool) = from_tool else {
        eprintln!("必须用 --from 指定以哪个工具的那份副本为准。");
        return Ok(ExitCode::FAILURE);
    };

    let skill = resolve::find_skill_by_name(ctx.store(), name)?;
    let path = resolve::target_path_of(ctx.store(), &skill, &tool, &instance)?;

    if ctx.dry_run {
        println!("将以 {} 为准，源路径：{}", tool, path.display());
        println!("（--dry-run：未写入任何文件）");
        return Ok(ExitCode::SUCCESS);
    }

    let updated = adopt_from(ctx.store(), &skill, &path)?;
    println!("✅ 以 {tool} 为准，更新了 {updated} 处副本。");
    if updated == 0 {
        println!("   注意：只有这一份副本时无处可推；请再确认母库是否已对齐（见 `status`）。");
    }
    Ok(ExitCode::SUCCESS)
}
