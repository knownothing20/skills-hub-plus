use std::fs;
use std::path::Path;

use crate::core::skill_store::{SkillRecord, SkillStore, SkillTargetRecord};
use crate::core::stale_copies::{
    refresh_stale_copy, scan_stale_copy_targets, set_auto_refresh_stale_copies,
    startup_stale_refresh, StaleVerdict,
};

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

fn seed_target(store: &SkillStore, skill_id: &str, tool: &str, target_dir: &Path, mode: &str) {
    store
        .upsert_skill_target(&SkillTargetRecord {
            id: format!("target-{tool}"),
            skill_id: skill_id.to_string(),
            tool: tool.to_string(),
            scope: "global".to_string(),
            project_path: None,
            target_path: target_dir.to_string_lossy().to_string(),
            mode: mode.to_string(),
            status: "ok".to_string(),
            last_error: None,
            synced_at: None,
        })
        .unwrap();
}

fn verdict_for(
    statuses: &[crate::core::stale_copies::StaleCopyStatus],
    tool: &str,
) -> StaleVerdict {
    statuses
        .iter()
        .find(|status| status.tool == tool)
        .map(|status| status.verdict)
        .expect("expected a stale status entry")
}

#[test]
fn sanitize_component_replaces_path_separators() {
    assert_eq!(
        super::sanitize_component("agent2api-deploy"),
        "agent2api-deploy"
    );
    assert_eq!(super::sanitize_component("a/b\\c"), "a_b_c");
}

#[test]
fn identical_copy_is_not_reported() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path());
    let source = dir.path().join("source-skill");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), b"hello").unwrap();

    // fs::copy 保留 mtime，与真实同步行为一致
    let target = dir.path().join("copy-skill");
    fs::create_dir_all(&target).unwrap();
    fs::copy(source.join("SKILL.md"), target.join("SKILL.md")).unwrap();

    seed_skill(&store, "s1", &source);
    seed_target(&store, "s1", "antigravity", &target, "copy");

    let statuses = scan_stale_copy_targets(&store).unwrap();
    assert!(
        statuses.is_empty(),
        "identical copy must not be reported, got {statuses:?}"
    );
}

#[test]
fn newer_source_reports_behind_and_refreshes() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path());
    let central_root = dir.path().join("central");
    fs::create_dir_all(&central_root).unwrap();

    let source = dir.path().join("source-skill");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), b"old").unwrap();

    let target = dir.path().join("copy-skill");
    fs::create_dir_all(&target).unwrap();
    fs::copy(source.join("SKILL.md"), target.join("SKILL.md")).unwrap();

    seed_skill(&store, "s1", &source);
    seed_target(&store, "s1", "antigravity", &target, "copy");

    // 让主 skill 明确变新（超出 ±2s 容差）
    std::thread::sleep(std::time::Duration::from_millis(2100));
    fs::write(source.join("SKILL.md"), b"newer content").unwrap();

    let statuses = scan_stale_copy_targets(&store).unwrap();
    assert_eq!(verdict_for(&statuses, "antigravity"), StaleVerdict::Behind);

    // 手动刷新：备份 + 替换
    refresh_stale_copy(&store, &central_root, "s1", "antigravity").unwrap();
    assert_eq!(
        fs::read_to_string(target.join("SKILL.md")).unwrap(),
        "newer content"
    );
    let backup = central_root
        .parent()
        .unwrap()
        .join("backup")
        .join("stale-copies")
        .join("s1");
    assert!(
        backup.join("SKILL.md").exists(),
        "backup must keep the old copy"
    );

    let statuses = scan_stale_copy_targets(&store).unwrap();
    assert!(statuses.is_empty(), "refreshed copy must be clean");
}

#[test]
fn extra_file_in_copy_is_hand_edited() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path());
    let central_root = dir.path().join("central");
    fs::create_dir_all(&central_root).unwrap();

    let source = dir.path().join("source-skill");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), b"base").unwrap();

    let target = dir.path().join("copy-skill");
    fs::create_dir_all(&target).unwrap();
    fs::copy(source.join("SKILL.md"), target.join("SKILL.md")).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(2100));
    fs::write(source.join("SKILL.md"), b"updated").unwrap();
    // 副本里出现主 skill 没有的文件 —— 手改铁证
    fs::write(target.join("my-notes.md"), b"user created this").unwrap();

    seed_skill(&store, "s1", &source);
    seed_target(&store, "s1", "antigravity", &target, "copy");

    let statuses = scan_stale_copy_targets(&store).unwrap();
    assert_eq!(
        verdict_for(&statuses, "antigravity"),
        StaleVerdict::HandEdited
    );

    // 手改副本不允许自动刷新
    let err = refresh_stale_copy(&store, &central_root, "s1", "antigravity").unwrap_err();
    assert!(err.to_string().contains("STALE_STATE_CHANGED"));
    assert_eq!(
        fs::read_to_string(target.join("my-notes.md")).unwrap(),
        "user created this"
    );
}

#[test]
fn newer_copy_is_hand_edited() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path());

    let source = dir.path().join("source-skill");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), b"base").unwrap();

    let target = dir.path().join("copy-skill");
    fs::create_dir_all(&target).unwrap();
    fs::copy(source.join("SKILL.md"), target.join("SKILL.md")).unwrap();

    // 副本比主 skill 更新：用户在工具里改过
    std::thread::sleep(std::time::Duration::from_millis(2100));
    fs::write(target.join("SKILL.md"), b"edited inside the tool").unwrap();

    seed_skill(&store, "s1", &source);
    seed_target(&store, "s1", "antigravity", &target, "copy");

    let statuses = scan_stale_copy_targets(&store).unwrap();
    assert_eq!(
        verdict_for(&statuses, "antigravity"),
        StaleVerdict::HandEdited
    );
}

#[test]
fn unclear_mtime_with_different_content_is_conflict() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path());

    let source = dir.path().join("source-skill");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), b"source version").unwrap();

    let target = dir.path().join("copy-skill");
    fs::create_dir_all(&target).unwrap();
    // 内容不同但 mtime 相差在容差内 —— 分不清，只能标记冲突
    fs::write(target.join("SKILL.md"), b"another version").unwrap();

    seed_skill(&store, "s1", &source);
    seed_target(&store, "s1", "antigravity", &target, "copy");

    let statuses = scan_stale_copy_targets(&store).unwrap();
    assert_eq!(
        verdict_for(&statuses, "antigravity"),
        StaleVerdict::Conflict
    );
}

#[test]
fn junction_targets_and_disabled_targets_are_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path());

    let source = dir.path().join("source-skill");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), b"base").unwrap();

    let copy_target = dir.path().join("copy-skill");
    fs::create_dir_all(&copy_target).unwrap();
    fs::write(copy_target.join("SKILL.md"), b"stale").unwrap();

    let disabled_target = dir.path().join("disabled-skill");
    fs::create_dir_all(&disabled_target).unwrap();
    fs::write(disabled_target.join("SKILL.md"), b"stale").unwrap();

    seed_skill(&store, "s1", &source);
    seed_target(&store, "s1", "antigravity", &copy_target, "copy");
    seed_target(&store, "s1", "codex", &disabled_target, "copy");
    store
        .update_skill_target_status("s1", "codex", "global", None, "disabled")
        .unwrap();

    // codex 目录 mtime 更新也不会出现：disabled 目标不参与
    let statuses = scan_stale_copy_targets(&store).unwrap();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].tool, "antigravity");
}

#[test]
fn startup_refresh_respects_the_setting_and_keeps_hand_edited() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path());
    let central_root = dir.path().join("central");
    fs::create_dir_all(&central_root).unwrap();

    let source = dir.path().join("source-skill");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), b"old").unwrap();

    let behind = dir.path().join("behind-skill");
    fs::create_dir_all(&behind).unwrap();
    fs::copy(source.join("SKILL.md"), behind.join("SKILL.md")).unwrap();

    let hand_edited = dir.path().join("hand-edited-skill");
    fs::create_dir_all(&hand_edited).unwrap();
    fs::copy(source.join("SKILL.md"), hand_edited.join("SKILL.md")).unwrap();

    seed_skill(&store, "s1", &source);
    seed_target(&store, "s1", "antigravity", &behind, "copy");
    seed_target(&store, "s1", "zcode", &hand_edited, "copy");

    std::thread::sleep(std::time::Duration::from_millis(2100));
    fs::write(source.join("SKILL.md"), b"newer").unwrap();
    fs::write(hand_edited.join("user-note.md"), b"hand edit").unwrap();

    // 开关关闭：只报数，不刷新
    set_auto_refresh_stale_copies(&store, false).unwrap();
    let (refreshed, remaining) = startup_stale_refresh(&store, &central_root).unwrap();
    assert_eq!((refreshed, remaining), (0, 2));
    assert_eq!(fs::read_to_string(behind.join("SKILL.md")).unwrap(), "old");

    // 开关开启：Behind 被刷新，HandEdited 保留
    set_auto_refresh_stale_copies(&store, true).unwrap();
    let (refreshed, remaining) = startup_stale_refresh(&store, &central_root).unwrap();
    assert_eq!((refreshed, remaining), (1, 1));
    assert_eq!(
        fs::read_to_string(behind.join("SKILL.md")).unwrap(),
        "newer"
    );
    assert!(hand_edited.join("user-note.md").exists());
    assert_eq!(
        fs::read_to_string(hand_edited.join("SKILL.md")).unwrap(),
        "old"
    );
}
