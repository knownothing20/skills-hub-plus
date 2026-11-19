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
