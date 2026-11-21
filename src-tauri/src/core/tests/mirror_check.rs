use std::fs;

use crate::core::mirror_check::scan_stale_mirrors;
use crate::core::skill_store::{SkillRecord, SkillStore};

fn store_in(dir: &std::path::Path) -> SkillStore {
    let store = SkillStore::new(dir.join("test.db"));
    store.ensure_schema().unwrap();
    store
}

fn seed(store: &SkillStore, id: &str, owner: &std::path::Path, mirror: &std::path::Path) {
    store
        .upsert_skill(&SkillRecord {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            source_type: "local".to_string(),
            source_ref: Some(owner.to_string_lossy().to_string()),
            source_subpath: None,
            source_revision: None,
            central_path: mirror.to_string_lossy().to_string(),
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

#[test]
fn identical_mirror_is_not_reported() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_in(dir.path());
    let owner = dir.path().join("owner/skill");
    let mirror = dir.path().join("central/skill");
    fs::create_dir_all(&owner).unwrap();
    fs::create_dir_all(&mirror).unwrap();
    fs::write(owner.join("SKILL.md"), b"same").unwrap();
    fs::copy(owner.join("SKILL.md"), mirror.join("SKILL.md")).unwrap();

    seed(&store, "s1", &owner, &mirror);
    assert!(scan_stale_mirrors(&store).unwrap().is_empty());
}

#[test]
fn older_mirror_is_reported_behind() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_in(dir.path());
    let owner = dir.path().join("owner/skill");
    let mirror = dir.path().join("central/skill");
    fs::create_dir_all(&owner).unwrap();
    fs::create_dir_all(&mirror).unwrap();
    fs::write(owner.join("SKILL.md"), b"v1").unwrap();
    fs::copy(owner.join("SKILL.md"), mirror.join("SKILL.md")).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(2100));
    fs::write(owner.join("SKILL.md"), b"v2 longer content").unwrap();

    seed(&store, "s1", &owner, &mirror);
    let stale = scan_stale_mirrors(&store).unwrap();
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].verdict, "behind");
}

#[test]
fn mirror_with_extra_file_is_hand_edited() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_in(dir.path());
    let owner = dir.path().join("owner/skill");
    let mirror = dir.path().join("central/skill");
    fs::create_dir_all(&owner).unwrap();
    fs::create_dir_all(&mirror).unwrap();
    fs::write(owner.join("SKILL.md"), b"v1").unwrap();
    fs::copy(owner.join("SKILL.md"), mirror.join("SKILL.md")).unwrap();
    fs::write(mirror.join("only-here.md"), b"mirror-only").unwrap();

    std::thread::sleep(std::time::Duration::from_millis(2100));
    fs::write(owner.join("SKILL.md"), b"v2").unwrap();

    seed(&store, "s1", &owner, &mirror);
    let stale = scan_stale_mirrors(&store).unwrap();
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].verdict, "hand_edited");
}

#[test]
fn same_content_with_different_mtime_is_not_reported() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_in(dir.path());
    let owner = dir.path().join("owner/skill");
    let mirror = dir.path().join("central/skill");
    fs::create_dir_all(&owner).unwrap();
    fs::create_dir_all(&mirror).unwrap();
    fs::write(owner.join("SKILL.md"), b"identical").unwrap();
    fs::copy(owner.join("SKILL.md"), mirror.join("SKILL.md")).unwrap();

    // 触碰镜像：内容不变但 mtime 变新
    std::thread::sleep(std::time::Duration::from_millis(2100));
    fs::write(mirror.join("SKILL.md"), b"identical").unwrap();

    seed(&store, "s1", &owner, &mirror);
    assert!(
        scan_stale_mirrors(&store).unwrap().is_empty(),
        "identical content must not be reported even if mtime differs"
    );
}

#[test]
fn non_local_skills_are_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_in(dir.path());
    store
        .upsert_skill(&SkillRecord {
            id: "git-skill".to_string(),
            name: "git-skill".to_string(),
            description: None,
            source_type: "git".to_string(),
            source_ref: Some("https://example.com/repo".to_string()),
            source_subpath: None,
            source_revision: None,
            central_path: dir.path().join("central").to_string_lossy().to_string(),
            content_hash: None,
            created_at: 1,
            updated_at: 2,
            last_sync_at: None,
            last_seen_at: 1,
            enabled: true,
            status: "ok".to_string(),
        })
        .unwrap();
    assert!(scan_stale_mirrors(&store).unwrap().is_empty());
}

/// 空目录 / 目录条目差异不能算「落后」。
///
/// 这是 2026-11-21 那个「母库和主 skill 不同，却一个版本字母都没有」的直接
/// 回归用例：`mirror_check` 原先用 `content_hash::hash_dir`，它把**目录条目**
/// 也喂进哈希；而算版本字母的 `peer_sync::snapshot_with_time` 只统计文件。
/// 两边对同一批目录给出相反结论时，界面就会同时出现「母库落后（红色 !）」
/// 和「只有一种版本（没有 A/B）」，用户无从判断该信哪个。
#[test]
fn empty_directories_do_not_split_from_version_labels() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_in(dir.path());
    let owner = dir.path().join("owner/skill");
    let mirror = dir.path().join("central/skill");
    fs::create_dir_all(owner.join("emptydir")).unwrap();
    fs::create_dir_all(mirror.join("emptydir")).unwrap();
    fs::write(owner.join("SKILL.md"), b"same").unwrap();
    fs::copy(owner.join("SKILL.md"), mirror.join("SKILL.md")).unwrap();

    seed(&store, "s1", &owner, &mirror);
    let stale = scan_stale_mirrors(&store).unwrap();
    let skill = store.get_skill_by_id("s1").unwrap().unwrap();
    let labels = crate::core::peer_sync::version_labels(&store, &skill).unwrap();

    assert!(
        stale.is_empty(),
        "内容一致时不该报母库落后：{:?}",
        stale.iter().map(|s| s.verdict.clone()).collect::<Vec<_>>()
    );
    assert!(
        labels.is_empty(),
        "只有一个版本时不该产出 A/B 字母：{labels:?}"
    );
}
