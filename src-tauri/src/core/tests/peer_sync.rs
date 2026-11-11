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
