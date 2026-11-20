//! `skills-hub-cli`：Skills Hub 的命令行同步工具。
//!
//! 存在的理由：GUI 里同步一个技能要点开卡片、右键、逐个判断谁新，改五六个技能
//! 就是十几次点击。这个工具把同一套核心逻辑（`app_lib::core`）暴露成一条命令，
//! 让「在别的软件改完 skill，回终端收一下」成为可能。
//!
//! 与 GUI 的关系：**共用同一份实现**，不是另写一套同步算法。DB 路径、`.skillignore`
//! 语义、对等模型的三方比对全部一致，因此两者永远不会给出不同结论。

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use app_lib::core::peer_sync::{adopt_from, reconcile_skill, ReconcileOutcome};

mod output;
mod paths;
mod resolve;

use output::{collect_versions, diff_dirs, print_skill_table, Ctx};

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
  skills-hub-cli status                  全库差异一览（A 为最新，母库也参与比较）
  skills-hub-cli pull  [技能名...]       把最新的一份铺到所有位置，母库也在其中
  skills-hub-cli push  [技能名...]       把母库内容推给内容不一致的副本
  skills-hub-cli diff  <技能名>          列出各位置的内容分组，供你决定
  skills-hub-cli adopt <技能名> --from <工具名|母库> [--instance <实例>]
                                         指定以某个位置为准，推到母库和其余所有副本

说明：
  「位置」包括母库（D:\\GitHub\\skill-hub\\skills\\<名字>）和各软件的副本。
  pull 只有在一份内容偏离基线时才动手；多份各改各的会停下让你用 adopt 指定。
  默认不写任何文件之外的东西；私有目录（local/、node_modules…）按 .skillignore 原地保留。

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

/// `status`：列出所有存在版本差异的技能。
///
/// 口径与 GUI 卡片角标一致——都走 `version_labels`，它把**母库**也算成一个位置。
/// 早期版本用的是 `scan_stale_copy_targets`，那条链路只比「创造者软件目录 →
/// 各副本」，母库不在任何一侧，于是「母库落后了」永远显示不出来。
fn cmd_status(ctx: &mut Ctx) -> anyhow::Result<ExitCode> {
    let skills = ctx.store().list_skills()?;
    let views = collect_versions(ctx, &skills);

    print_skill_table(ctx, &skills, &views);

    if ctx.as_json {
        return Ok(ExitCode::SUCCESS);
    }
    if views.is_empty() {
        return Ok(ExitCode::SUCCESS);
    }
    println!();
    println!(
        "共 {} 个技能，其中 {} 个存在多版本。",
        skills.len(),
        views.len()
    );
    Ok(ExitCode::SUCCESS)
}

/// `pull`：你的主诉求——在别的软件改完，把改动收回来并铺开。
///
/// 口径：对每个技能跑一遍对等比对（`reconcile_skill`）。只有一份副本偏离基线时
/// 它会被采纳并推给其他所有位置（含母库），这正是「在 Codex 里改了，其它软件
/// 跟着更新」。多份副本各改各的（`Conflict`）时**不自动决定**，列出来用 `adopt`
/// 指定。
///
/// `require_stable` 必须传 `false`：那个「连续两轮看到同一指纹才动手」的确认机制
/// 靠的是一个**进程内**的静态表（`pending_slots`），而 CLI 每次调用都是一个新
/// 进程，表永远是空的——传 `true` 会让每一次 `pull` 都停在「待确认」，看起来
/// 什么也没做。GUI 的「立即同步」按钮同样是传 `false`（见 `reconcile_skill_now`），
/// 因为两者都是用户显式发起的一次动作。回滚检测不受影响，仍在函数内部生效。
fn cmd_pull(ctx: &mut Ctx, names: &[String]) -> anyhow::Result<ExitCode> {
    let targets = resolve::select_skills(ctx.store(), names)?;
    if targets.is_empty() {
        println!("没有匹配的技能。");
        return Ok(ExitCode::SUCCESS);
    }

    // `--dry-run` 不能只是「跳过循环」——那样什么也不打印，用户无法判断会动什么。
    // 这里用 `scan_peers` 只读地把「当前位置谁偏离」列出来，由人自己判断。
    if ctx.dry_run {
        println!("（--dry-run：只显示将要做什么，不写入任何文件）");
        println!();
        let mut any = false;
        for skill in &targets {
            let peers = app_lib::core::peer_sync::scan_peers(ctx.store(), skill)?;
            let edited: Vec<_> = peers
                .iter()
                .filter(|p| p.state == app_lib::core::peer_sync::PeerState::Edited)
                .collect();
            if edited.is_empty() {
                continue;
            }
            any = true;
            println!("● {}", skill.name);
            for p in &edited {
                let label = if p.instance.is_empty() {
                    p.tool.clone()
                } else {
                    format!("{} / {}", p.tool, p.instance)
                };
                println!(
                    "    {label} 偏离基线（{} 个文件改动）",
                    p.changed_files.len()
                );
            }
            if edited.len() == 1 {
                println!("    → 只有一份偏离，pull 会以它为准，铺到母库与其余所有位置");
            } else {
                println!(
                    "    → {} 份偏离，pull 不会自动动手（会判冲突），需用 adopt 指定",
                    edited.len()
                );
            }
        }
        if !any {
            println!("所有技能都与基线一致，pull 不会做任何事。");
        }
        return Ok(ExitCode::SUCCESS);
    }

    let (mut adopted, mut conflicts, mut unchanged) = (0usize, 0usize, 0usize);
    let mut conflict_names: Vec<String> = Vec::new();

    for skill in &targets {
        // 母库记录脏了（路径为空或目录不在）时，reconcile_skill 会看不到母库，
        // 于是「收回母库」这一步静默失效。这里先确认母库在，才继续。
        let central = ctx.central_dir_of(skill)?;
        if !central.is_dir() {
            eprintln!(
                "❌ {} 的母库目录不存在：{}\n   先用 --central 指定母库根目录，或检查该技能记录。",
                skill.name,
                central.display()
            );
            continue;
        }
        // 先把规则对齐（双向并集），再比内容。顺序不能反：母库可能缺
        // `.skillignore`（agentport 就是这样），此时副本里的 `local/` 会被当成
        // 技能内容复制进母库。规则先到位，私有目录才会被正确判为「不是内容」。
        app_lib::core::peer_sync::push_ignore_rules_to_targets(ctx.store(), skill);
        match reconcile_skill(ctx.store(), skill, false) {
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
            })
        );
    } else {
        println!();
        println!("采纳 {adopted} 个，无变化 {unchanged} 个，需你决定 {conflicts} 个。");
    }
    Ok(ExitCode::SUCCESS)
}

/// `push`：把**母库**的内容推给内容不一致的副本。
///
/// 与 `pull` 的区别是方向：`push` 不采纳副本改动，只修「母库更新了但副本没跟上」。
///
/// 这里**刻意不用** `scan_stale_copy_targets`：那条链路的基准是 `source_ref`
/// （创造者软件的原始目录），不是母库。用它做 push 会把「原件的旧内容」推给副本，
/// 母库里较新的改动反而推不出去——正好与命令意图相反。这里直接拿母库做源，
/// 语义干净：母库是什么，副本就变成什么。
fn cmd_push(ctx: &mut Ctx, names: &[String]) -> anyhow::Result<ExitCode> {
    let selected = resolve::select_skills(ctx.store(), names)?;
    if selected.is_empty() {
        println!("没有匹配的技能。");
        return Ok(ExitCode::SUCCESS);
    }

    // (技能名, 工具标签, 副本路径, 母库路径)
    let mut pending: Vec<(String, String, PathBuf, PathBuf)> = Vec::new();
    for skill in &selected {
        // 用 central_dir_of 而不是直接读记录：记录里的 central_path 可能是空的
        // 或已失效，那样这里会静默跳过该技能、看起来「全部一致」。
        let central = ctx.central_dir_of(skill)?;
        if !central.is_dir() {
            eprintln!(
                "❌ {} 的母库目录不存在：{}（跳过）",
                skill.name,
                central.display()
            );
            continue;
        }
        // 规则先对齐：母库若缺 `.skillignore`，副本的 `local/` 会被当成内容推过去，
        // 反过来母库的私有目录也一样。见 `push_ignore_rules_to_targets` 的说明。
        app_lib::core::peer_sync::push_ignore_rules_to_targets(ctx.store(), skill);
        let central_fp = fingerprint_of(&central);
        for target in ctx.store().list_skill_targets(&skill.id)? {
            let path = PathBuf::from(&target.target_path);
            if !path.is_dir() || path == central {
                continue;
            }
            if fingerprint_of(&path) == central_fp {
                continue;
            }
            let label = if target.instance.is_empty() {
                target.tool.clone()
            } else {
                format!("{}#{}", target.tool, target.instance)
            };
            pending.push((skill.name.clone(), label, path, central.clone()));
        }
    }

    if pending.is_empty() {
        println!("所有副本都与母库一致，无需推送。");
        return Ok(ExitCode::SUCCESS);
    }
    if ctx.dry_run {
        for (name, label, path, _) in &pending {
            println!("将推送 {name}：母库 → {label}（{}）", path.display());
        }
        println!("（--dry-run：未写入任何文件）");
        return Ok(ExitCode::SUCCESS);
    }

    let mut pushed = 0usize;
    for (name, label, path, central) in &pending {
        match app_lib::core::sync_engine::sync_dir_copy_with_overwrite(central, path, true) {
            Ok(_) => {
                pushed += 1;
                println!("✅ {name}：母库 → {label}");
            }
            Err(err) => eprintln!("❌ {name}：母库 → {label} 失败：{err:#}"),
        }
    }
    println!();
    println!("推送 {pushed} 个副本。");
    Ok(ExitCode::SUCCESS)
}

/// 目录指纹，口径与 `version_labels` 一致（应用 `.skillignore`、忽略生成物）。
fn fingerprint_of(dir: &Path) -> String {
    app_lib::core::content_hash::hash_dir(dir).unwrap_or_default()
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
///
/// `--from` 除了工具名，也接受 `母库`（或 `central`）——把母库那一版铺回所有副本，
/// 这是最保守的「以我为准」形式，也是唯一能主动回退副本改动的方式。
fn cmd_adopt(ctx: &mut Ctx, args: &[String]) -> anyhow::Result<ExitCode> {
    let Some(name) = args.first() else {
        eprintln!("用法：skills-hub-cli adopt <技能名> --from <工具名|母库> [--instance <实例>]");
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
        eprintln!("必须用 --from 指定以哪个工具的那份副本为准（也接受 `母库`）。");
        return Ok(ExitCode::FAILURE);
    };

    let skill = resolve::find_skill_by_name(ctx.store(), name)?;
    let path = if resolve::is_central_alias(&tool) {
        ctx.central_dir_of(&skill)?
    } else {
        resolve::target_path_of(ctx.store(), &skill, &tool, &instance)?
    };

    if ctx.dry_run {
        println!("将以 {} 为准，源路径：{}", tool, path.display());
        println!("（--dry-run：未写入任何文件）");
        return Ok(ExitCode::SUCCESS);
    }

    let updated = adopt_from(ctx.store(), &skill, &path)?;
    println!("✅ 以 {tool} 为准，更新了 {updated} 处位置（含母库）。");
    if updated == 0 {
        println!("   所有位置本来就与 {tool} 一致，无需改动。");
    }
    Ok(ExitCode::SUCCESS)
}
