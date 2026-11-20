//! 对等同步的行为测试。
//!
//! 重点覆盖「多个副本同时偏离基线」这种真实场景：
//! 用户在两个工具里做了同样的改动，第三份没动——第三份必须被更新，
//! 而不是在下一轮被当成「唯一偏离」反过来把旧内容扩散出去。

use std::fs;
use std::path::Path;

use crate::core::peer_sync::{reconcile_skill, ReconcileOutcome};
use crate::core::skill_store::{SkillRecord, SkillStore, SkillTargetRecord};

fn make_store(dir: &Path) -> SkillStore {
    let store = SkillStore::new(dir.join("test.db"));
    store.ensure_schema().unwrap();
    store
}

fn seed_skill(store: &SkillStore, id: &str, source_dir: &Path) {
    store
        .upsert_skill(&SkillRecord {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            source_type: "local".to_string(),
            source_ref: Some(source_dir.to_string_lossy().to_string()),
            source_subpath: None,
            source_revision: None,
            central_path: source_dir.to_string_lossy().to_string(),
            content_hash: None,
            created_at: 1,
            updated_at: 2,
            last_sync_at: None,
            last_seen_at: 1,
            enabled: true,
            status: "ok".to_string(),
        })
        .unwrap();
}

fn seed_target(store: &SkillStore, skill_id: &str, tool: &str, target_dir: &Path) {
    store
        .upsert_skill_target(&SkillTargetRecord {
            id: format!("target-{tool}"),
            instance: String::new(),
            skill_id: skill_id.to_string(),
            tool: tool.to_string(),
            scope: "global".to_string(),
            project_path: None,
            target_path: target_dir.to_string_lossy().to_string(),
            mode: "copy".to_string(),
            status: "ok".to_string(),
            last_error: None,
            synced_at: None,
        })
        .unwrap();
}

fn write_skill(dir: &Path, body: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("SKILL.md"), body).unwrap();
    fs::write(dir.join("notes.md"), "shared notes\n").unwrap();
}

fn read_body(dir: &Path) -> String {
    fs::read_to_string(dir.join("SKILL.md")).unwrap()
}

/// 三份副本：初始一致 → 两份改成同样内容 → 第三份应被同步 → 再跑一轮不应再变。
#[test]
fn identical_drift_propagates_and_does_not_revert_on_second_round() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let store = make_store(root);

    let a = root.join("a");
    let b = root.join("b");
    let c = root.join("c");
    write_skill(&a, "version one\n");
    write_skill(&b, "version one\n");
    write_skill(&c, "version one\n");
    seed_skill(&store, "demo", &a);
    seed_target(&store, "demo", "tool_a", &a);
    seed_target(&store, "demo", "tool_b", &b);
    seed_target(&store, "demo", "tool_c", &c);

    let skill = store.get_skill_by_id("demo").unwrap().unwrap();

    // 第一轮：建立基线（三份一致，没有改动）
    let first = reconcile_skill(&store, &skill, false).unwrap();
    assert!(
        matches!(first, ReconcileOutcome::Initialized { .. }),
        "第一轮应该是建立基线，实际是 {first:?}"
    );

    // 用户在 a 和 b 里做了同样的改动，c 没动
    write_skill(&a, "version two\n");
    write_skill(&b, "version two\n");

    let second = reconcile_skill(&store, &skill, false).unwrap();
    assert!(
        matches!(second, ReconcileOutcome::Adopted { .. }),
        "两份内容相同的偏离应该被采纳，实际是 {second:?}"
    );
    assert_eq!(
        read_body(&c),
        "version two\n",
        "没有偏离的 c 必须收到新版本，否则下一轮它会被当成唯一偏离"
    );

    // 第二轮：所有副本都已经是 version two，不应该再有任何动作
    let third = reconcile_skill(&store, &skill, false).unwrap();
    assert!(
        matches!(third, ReconcileOutcome::NoChange),
        "第二轮应该是无变化，实际是 {third:?}"
    );
    assert_eq!(read_body(&a), "version two\n");
    assert_eq!(read_body(&b), "version two\n");
    assert_eq!(read_body(&c), "version two\n");
}

/// 三份各不相同：必须停下判冲突，一个字都不能写。
#[test]
fn three_different_versions_do_not_overwrite_anything() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let store = make_store(root);

    let a = root.join("a");
    let b = root.join("b");
    let c = root.join("c");
    write_skill(&a, "base\n");
    write_skill(&b, "base\n");
    write_skill(&c, "base\n");
    seed_skill(&store, "demo", &a);
    seed_target(&store, "demo", "tool_a", &a);
    seed_target(&store, "demo", "tool_b", &b);
    seed_target(&store, "demo", "tool_c", &c);
    let skill = store.get_skill_by_id("demo").unwrap().unwrap();
    let _ = reconcile_skill(&store, &skill, false).unwrap();

    write_skill(&a, "from a\n");
    write_skill(&b, "from b\n");
    write_skill(&c, "from c\n");

    let outcome = reconcile_skill(&store, &skill, false).unwrap();
    assert!(
        matches!(outcome, ReconcileOutcome::Conflict { .. }),
        "三份各不相同应判冲突，实际是 {outcome:?}"
    );
    assert_eq!(read_body(&a), "from a\n");
    assert_eq!(read_body(&b), "from b\n");
    assert_eq!(read_body(&c), "from c\n");
}

/// 副本被换回历史版本：判为回滚，不扩散。
#[test]
fn reverting_to_a_known_version_is_treated_as_rollback() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let store = make_store(root);

    let a = root.join("a");
    let b = root.join("b");
    write_skill(&a, "version one\n");
    write_skill(&b, "version one\n");
    seed_skill(&store, "demo", &a);
    seed_target(&store, "demo", "tool_a", &a);
    seed_target(&store, "demo", "tool_b", &b);
    let skill = store.get_skill_by_id("demo").unwrap().unwrap();

    // 建基线 → 改成 v2 并被采纳
    let _ = reconcile_skill(&store, &skill, false).unwrap();
    write_skill(&a, "version two\n");
    let adopted = reconcile_skill(&store, &skill, false).unwrap();
    assert!(
        matches!(adopted, ReconcileOutcome::Adopted { .. }),
        "v2 应被采纳"
    );

    // 把 a 换回 v1（历史里出现过）→ 回滚，不该推给 b
    write_skill(&a, "version one\n");
    let outcome = reconcile_skill(&store, &skill, false).unwrap();
    assert!(
        matches!(outcome, ReconcileOutcome::Rollback { .. }),
        "换回历史版本应判回滚，实际是 {outcome:?}"
    );
    assert_eq!(read_body(&b), "version two\n", "回滚不应扩散到其它副本");
}

/// 文件数超上限：直接跳过，不比对也不写盘。
#[test]
fn too_many_files_is_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let store = make_store(root);

    let a = root.join("a");
    fs::create_dir_all(&a).unwrap();
    for index in 0..(crate::core::peer_sync::MAX_FILES_PER_SKILL + 5) {
        fs::write(a.join(format!("f{index}.txt")), "x").unwrap();
    }
    seed_skill(&store, "demo", &a);
    seed_target(&store, "demo", "tool_a", &a);
    let skill = store.get_skill_by_id("demo").unwrap().unwrap();

    let outcome = reconcile_skill(&store, &skill, false).unwrap();
    assert!(
        matches!(outcome, ReconcileOutcome::TooManyFiles { .. }),
        "超过文件上限应跳过，实际是 {outcome:?}"
    );
}

/// 各软件的私有目录（`.skillignore` 命中项）不能被算成「另一个版本」。
///
/// 真实场景：agentport 在每个软件副本里有自己的 `local/` 配置目录，内容各不
/// 相同。如果指纹把它们算进去，同一份技能会在版本表里裂成 A/B/C/D…，
/// 版本字母彻底失去参考价值。
#[test]
fn version_labels_ignore_private_dirs_excluded_by_skillignore() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let store = make_store(root);

    let owner = root.join("owner");
    write_skill(&owner, "same body\n");
    fs::write(owner.join(".skillignore"), "local\nlogs\n").unwrap();
    fs::create_dir_all(owner.join("local")).unwrap();
    fs::write(owner.join("local/connections.json"), "{\"a\":1}").unwrap();

    let peer = root.join("peer");
    write_skill(&peer, "same body\n");
    fs::write(peer.join(".skillignore"), "local\nlogs\n").unwrap();
    fs::create_dir_all(peer.join("local")).unwrap();
    // 内容明显不同：这是软件自己的私有配置，不是技能内容。
    fs::write(
        peer.join("local/connections.json"),
        "{\"completely\":\"different\"}",
    )
    .unwrap();

    seed_skill(&store, "agentport", &owner);
    seed_target(&store, "agentport", "tool_a", &owner);
    seed_target(&store, "agentport", "tool_b", &peer);
    let skill = store.get_skill_by_id("agentport").unwrap().unwrap();

    let labels = crate::core::peer_sync::version_labels(&store, &skill).unwrap();
    assert!(
        labels.is_empty(),
        "私有目录差异不该产生版本分组，实际拿到 {labels:?}"
    );
}

/// 反过来：真正的技能内容差异仍然必须分组，别把功能改瞎。
#[test]
fn version_labels_still_split_on_real_content_differences() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let store = make_store(root);

    let newest = root.join("newest");
    write_skill(&newest, "newer body\n");
    let older = root.join("older");
    write_skill(&older, "older body\n");
    fs::write(older.join("SKILL.md"), "older body\n").unwrap();

    // 明确拉开修改时间，保证排序有依据。
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(newest.join("SKILL.md"), "newer body\n").unwrap();

    seed_skill(&store, "demo", &newest);
    seed_target(&store, "demo", "tool_new", &newest);
    seed_target(&store, "demo", "tool_old", &older);
    let skill = store.get_skill_by_id("demo").unwrap().unwrap();

    let labels = crate::core::peer_sync::version_labels(&store, &skill).unwrap();
    assert_eq!(
        labels.len(),
        2,
        "两个不同版本应各有一个字母，实际 {labels:?}"
    );
    assert_eq!(labels.get("tool_new").map(String::as_str), Some("A"));
    assert!(labels.get("tool_old").is_some_and(|label| label != "A"));
}

/// 内容完全一致时，reconcile_skill 会直接返回 NoChange——规则下发不能因此被跳过。
/// `.skillignore` 在 DEFAULT_IGNORES 里，既不进快照也不进复制，所以「内容没变」
/// 并不代表规则到位。母库后补的规则必须能走到副本，否则下次整目录替换会把
/// `local/` 这类私有内容当旧内容一起送进回收站。
#[test]
fn reconcile_all_pushes_ignore_rules_even_when_content_matches() {
    let tmp = tempfile::tempdir().unwrap();
    let store = make_store(tmp.path());
    let central = tmp.path().join("central");
    let copy = tmp.path().join("copy");

    write_skill(&central, "same body\n");
    write_skill(&copy, "same body\n");
    // 母库有规则，副本没有。
    fs::write(central.join(".skillignore"), "local\nlogs\n").unwrap();

    seed_skill(&store, "demo", &central);
    seed_target(&store, "demo", "tool_copy", &copy);

    // 内容一致时 reconcile_skill 不复制任何东西（首轮是 Initialized，之后是
    // NoChange），两种情况都不该妨碍规则下发。这里只关心规则是否到位，
    // 不锁定具体计数。
    crate::core::peer_sync::reconcile_all(&store).unwrap();

    let rule_path = copy.join(".skillignore");
    assert!(rule_path.is_file(), "副本必须拿到规则文件");
    let rule = fs::read_to_string(&rule_path).unwrap();
    let has = |p: &str| {
        rule.lines()
            .any(|line| line.trim().trim_end_matches('/') == p)
    };
    assert!(has("local"), "母库规则要下发到副本: {rule}");
    assert!(has("logs"), "母库规则要下发到副本: {rule}");
}

/// 副本自己的规则不能被下发覆盖掉（求并集）。
#[test]
fn reconcile_all_keeps_copy_own_ignore_rules() {
    let tmp = tempfile::tempdir().unwrap();
    let store = make_store(tmp.path());
    let central = tmp.path().join("central");
    let copy = tmp.path().join("copy");

    write_skill(&central, "same body\n");
    write_skill(&copy, "same body\n");
    fs::write(central.join(".skillignore"), "local\n").unwrap();
    fs::write(copy.join(".skillignore"), "runtime\n").unwrap();

    seed_skill(&store, "demo", &central);
    seed_target(&store, "demo", "tool_copy", &copy);
    crate::core::peer_sync::reconcile_all(&store).unwrap();

    let rule = fs::read_to_string(copy.join(".skillignore")).unwrap();
    let has = |p: &str| {
        rule.lines()
            .any(|line| line.trim().trim_end_matches('/') == p)
    };
    assert!(has("local"), "母库规则要补进来: {rule}");
    assert!(has("runtime"), "副本自己的规则不能丢: {rule}");
}

/// 采纳某个副本时，母库必须一起更新。
///
/// 母库不在 `skill_targets` 里，早期实现只遍历副本表，于是「在别的软件改完、
/// 收回母库」这个最常见的场景完全失效：副本之间对齐了，母库却停在旧内容。
#[test]
fn adopt_from_updates_central_and_all_copies() {
    let tmp = tempfile::tempdir().unwrap();
    let store = make_store(tmp.path());
    let central = tmp.path().join("central");
    let copy_a = tmp.path().join("copy_a");
    let copy_b = tmp.path().join("copy_b");

    write_skill(&central, "old body\n");
    write_skill(&copy_a, "new body\n");
    write_skill(&copy_b, "old body\n");

    seed_skill(&store, "demo", &central);
    seed_target(&store, "demo", "tool_a", &copy_a);
    seed_target(&store, "demo", "tool_b", &copy_b);

    let skill = store.get_skill_by_id("demo").unwrap().unwrap();
    let updated = crate::core::peer_sync::adopt_from(&store, &skill, &copy_a).unwrap();

    assert_eq!(
        read_body(&central),
        "new body\n",
        "母库必须被更新——只改副本等于没收回改动"
    );
    assert_eq!(read_body(&copy_b), "new body\n", "其它副本也要跟上");
    assert_eq!(read_body(&copy_a), "new body\n", "源自身不应被改动");
    assert_eq!(updated, 2, "母库 + 另一个副本 = 2 处");
}

/// 源就在母库时不能自己覆盖自己——那会先把目录搬进回收站再复制回来。
#[test]
fn adopt_from_skips_central_when_source_is_central() {
    let tmp = tempfile::tempdir().unwrap();
    let store = make_store(tmp.path());
    let central = tmp.path().join("central");
    let copy = tmp.path().join("copy");

    write_skill(&central, "central body\n");
    write_skill(&copy, "stale body\n");

    seed_skill(&store, "demo", &central);
    seed_target(&store, "demo", "tool_copy", &copy);

    let skill = store.get_skill_by_id("demo").unwrap().unwrap();
    let updated = crate::core::peer_sync::adopt_from(&store, &skill, &central).unwrap();

    assert_eq!(read_body(&central), "central body\n", "母库内容不该被动");
    assert!(central.join("SKILL.md").exists(), "母库不能被搬走");
    assert_eq!(read_body(&copy), "central body\n", "副本应跟上母库");
    assert_eq!(updated, 1, "只应更新那一个副本");
}

// ---------------------------------------------------------------------------
// 以下三条覆盖「母库参与对等比对」这个补丁。母库不在 skill_targets 表里，
// 早期实现扫不到它，于是单副本技能在别的软件改完之后整条链路推不动。
// ---------------------------------------------------------------------------

/// 把母库纳入 peers：母库条目必须是母库的真实路径，并且排在最前。
#[test]
fn reconcile_includes_central_as_a_peer() {
    let tmp = tempfile::tempdir().unwrap();
    let store = make_store(tmp.path());
    let central = tmp.path().join("central");
    let copy = tmp.path().join("copy");

    write_skill(&central, "same body\n");
    write_skill(&copy, "same body\n");

    seed_skill(&store, "demo", &central);
    seed_target(&store, "demo", "tool_copy", &copy);

    let skill = store.get_skill_by_id("demo").unwrap().unwrap();
    // 内容一致 → 首轮建立基线，来源应是母库（它在 peers 里排第一）。
    let outcome = reconcile_skill(&store, &skill, false).unwrap();
    match outcome {
        ReconcileOutcome::Initialized { from } => {
            assert_eq!(
                from,
                crate::core::peer_sync::CENTRAL_LABEL,
                "基线应取母库，而不是任意副本"
            );
        }
        other => panic!("应建立基线，实际 {other:?}"),
    }
}

/// 用户的真实场景：只有一份副本，在别的软件里改了它。
/// 母库必须收到这份改动——这正是「在 Codex 改完，回终端 pull 一下」。
#[test]
fn reconcile_pulls_single_copy_change_back_into_central() {
    let tmp = tempfile::tempdir().unwrap();
    let store = make_store(tmp.path());
    let central = tmp.path().join("central");
    let only_copy = tmp.path().join("only_copy");

    write_skill(&central, "old body\n");
    write_skill(&only_copy, "old body\n");

    seed_skill(&store, "demo", &central);
    seed_target(&store, "demo", "tool_only", &only_copy);

    let skill = store.get_skill_by_id("demo").unwrap().unwrap();
    // 建基线
    let _ = reconcile_skill(&store, &skill, false).unwrap();
    assert_eq!(read_body(&central), "old body\n");

    // 用户在唯一副本里改了内容
    write_skill(&only_copy, "edited in another app\n");

    let outcome = reconcile_skill(&store, &skill, false).unwrap();
    assert!(
        matches!(outcome, ReconcileOutcome::Adopted { .. }),
        "唯一副本的改动应被采纳，实际 {outcome:?}"
    );
    assert_eq!(
        read_body(&central),
        "edited in another app\n",
        "母库必须收到副本的改动——否则改动永远出不了那个软件"
    );
}

/// 母库被直接改（例如 GUI 里在母库里编辑）：必须扩散到副本，
/// 而不是被当成「哪个副本都没偏离」而无事发生。
#[test]
fn reconcile_propagates_central_edit_to_copies() {
    let tmp = tempfile::tempdir().unwrap();
    let store = make_store(tmp.path());
    let central = tmp.path().join("central");
    let copy_a = tmp.path().join("copy_a");
    let copy_b = tmp.path().join("copy_b");

    write_skill(&central, "v1\n");
    write_skill(&copy_a, "v1\n");
    write_skill(&copy_b, "v1\n");

    seed_skill(&store, "demo", &central);
    seed_target(&store, "demo", "tool_a", &copy_a);
    seed_target(&store, "demo", "tool_b", &copy_b);

    let skill = store.get_skill_by_id("demo").unwrap().unwrap();
    let _ = reconcile_skill(&store, &skill, false).unwrap();

    write_skill(&central, "v2 from central\n");

    let outcome = reconcile_skill(&store, &skill, false).unwrap();
    match outcome {
        ReconcileOutcome::Adopted { from, .. } => {
            assert_eq!(from, crate::core::peer_sync::CENTRAL_LABEL, "来源应是母库");
        }
        other => panic!("母库改动应被采纳，实际 {other:?}"),
    }
    assert_eq!(read_body(&copy_a), "v2 from central\n");
    assert_eq!(read_body(&copy_b), "v2 from central\n");
}

/// 母库目录不存在时不能把整个技能判成异常，也不能把它塞进 peers。
#[test]
fn reconcile_tolerates_missing_central_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let store = make_store(tmp.path());
    let copy = tmp.path().join("copy");
    write_skill(&copy, "only copy body\n");

    // 母库路径指向一个不存在的目录
    let missing = tmp.path().join("no-such-central");
    seed_skill(&store, "demo", &missing);
    seed_target(&store, "demo", "tool_copy", &copy);

    let skill = store.get_skill_by_id("demo").unwrap().unwrap();
    let outcome = reconcile_skill(&store, &skill, false).unwrap();
    assert!(
        matches!(outcome, ReconcileOutcome::Initialized { .. }),
        "母库缺失时应退回用副本建基线，实际 {outcome:?}"
    );
    assert_eq!(read_body(&copy), "only copy body\n", "副本不该被清空");
}

// ---------------------------------------------------------------------------
// 规则文件的双向对齐。只从母库单向下发的写法在「母库自己缺规则」时会失守，
// agentport 就是真实例子：8 个副本都有 local/logs/runtime，母库一个都没有。
// ---------------------------------------------------------------------------

/// 母库缺规则、副本有规则 → 对齐后母库必须也拿到，否则副本的 private 目录
/// 会被当成技能内容复制进母库。
#[test]
fn ignore_rules_flow_from_copies_back_into_central() {
    let tmp = tempfile::tempdir().unwrap();
    let store = make_store(tmp.path());
    let central = tmp.path().join("central");
    let copy = tmp.path().join("copy");

    write_skill(&central, "same body\n");
    write_skill(&copy, "same body\n");
    // 只有副本有规则（真实场景：母库从未写过 .skillignore）
    fs::write(copy.join(".skillignore"), "local\nlogs\nruntime\n").unwrap();

    seed_skill(&store, "demo", &central);
    seed_target(&store, "demo", "tool_copy", &copy);

    let skill = store.get_skill_by_id("demo").unwrap().unwrap();
    crate::core::peer_sync::push_ignore_rules_to_targets(&store, &skill);

    let central_rule = fs::read_to_string(central.join(".skillignore"))
        .expect("母库必须补上规则，否则副本的私有目录会漏进母库");
    for want in ["local", "logs", "runtime"] {
        assert!(
            central_rule.lines().any(|l| l.trim() == want),
            "母库缺少 {want}: {central_rule}"
        );
    }
}

/// 双向对齐不能删掉任何一方已有的条目（并集语义）。
#[test]
fn ignore_rules_merge_is_union_not_replacement() {
    let tmp = tempfile::tempdir().unwrap();
    let store = make_store(tmp.path());
    let central = tmp.path().join("central");
    let copy_a = tmp.path().join("copy_a");
    let copy_b = tmp.path().join("copy_b");

    write_skill(&central, "same body\n");
    write_skill(&copy_a, "same body\n");
    write_skill(&copy_b, "same body\n");
    fs::write(central.join(".skillignore"), "node_modules\n").unwrap();
    fs::write(copy_a.join(".skillignore"), "local\n").unwrap();
    fs::write(copy_b.join(".skillignore"), "runtime\n").unwrap();

    seed_skill(&store, "demo", &central);
    seed_target(&store, "demo", "tool_a", &copy_a);
    seed_target(&store, "demo", "tool_b", &copy_b);

    let skill = store.get_skill_by_id("demo").unwrap().unwrap();
    crate::core::peer_sync::push_ignore_rules_to_targets(&store, &skill);

    for (label, dir) in [("母库", &central), ("copy_a", &copy_a), ("copy_b", &copy_b)] {
        let rule = fs::read_to_string(dir.join(".skillignore")).unwrap();
        for want in ["node_modules", "local", "runtime"] {
            assert!(
                rule.lines().any(|l| l.trim() == want),
                "{label} 少了 {want}（并集不该删条目）: {rule}"
            );
        }
    }
}

/// `snapshot` 必须应用 `.skillignore`：否则每个软件各自的 private 目录会被
/// 当成「内容差异」，同一技能永远显示不一致，还会被复制到其它副本。
#[test]
fn snapshot_excludes_paths_listed_in_skillignore() {
    let tmp = tempfile::tempdir().unwrap();
    let store = make_store(tmp.path());
    let central = tmp.path().join("central");
    let copy = tmp.path().join("copy");

    write_skill(&central, "same body\n");
    write_skill(&copy, "same body\n");
    for dir in [&central, &copy] {
        fs::write(dir.join(".skillignore"), "local\n").unwrap();
        fs::create_dir_all(dir.join("local")).unwrap();
    }
    // 两边私有内容故意不同
    fs::write(central.join("local/c.json"), "{\"who\":\"central\"}").unwrap();
    fs::write(copy.join("local/c.json"), "{\"who\":\"copy\"}").unwrap();

    seed_skill(&store, "demo", &central);
    seed_target(&store, "demo", "tool_copy", &copy);

    let skill = store.get_skill_by_id("demo").unwrap().unwrap();
    let _ = reconcile_skill(&store, &skill, false).unwrap();
    // 建基线后再跑一轮：内容应当一致（私有目录不参与），不能判冲突。
    let outcome = reconcile_skill(&store, &skill, false).unwrap();
    assert!(
        matches!(outcome, ReconcileOutcome::NoChange),
        "私有目录差异不该被判成内容差异，实际 {outcome:?}"
    );
    assert_eq!(
        fs::read_to_string(copy.join("local/c.json")).unwrap(),
        "{\"who\":\"copy\"}",
        "副本的私有内容不该被覆盖"
    );
}
