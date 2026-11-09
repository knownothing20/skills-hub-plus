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
