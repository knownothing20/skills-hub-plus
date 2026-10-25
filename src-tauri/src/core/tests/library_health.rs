use crate::core::library_health::{
    build_library_health_report, classify_target, is_idle, skill_last_modified_secs, BrokenTarget,
    IDLE_THRESHOLD_DAYS,
};
use crate::core::skill_store::{SkillRecord, SkillStore, SkillTargetRecord};

fn make_store() -> (tempfile::TempDir, SkillStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("test.db");
    let store = SkillStore::new(db);
    store.ensure_schema().expect("ensure_schema");
    (dir, store)
}

fn make_skill(id: &str, name: &str, central_path: &str) -> SkillRecord {
    SkillRecord {
        id: id.to_string(),
        name: name.to_string(),
        description: None,
        source_type: "local".to_string(),
        source_ref: Some("/tmp/source".to_string()),
        source_subpath: None,
        source_revision: None,
        central_path: central_path.to_string(),
        content_hash: None,
        created_at: 1,
        updated_at: 1,
        last_sync_at: None,
        last_seen_at: 1,
        enabled: true,
        status: "ok".to_string(),
    }
}

fn make_target(
    id: &str,
    skill_id: &str,
    tool: &str,
    target_path: &str,
    status: &str,
) -> SkillTargetRecord {
    SkillTargetRecord {
        id: id.to_string(),
        skill_id: skill_id.to_string(),
        tool: tool.to_string(),
        scope: "global".to_string(),
        project_path: None,
        target_path: target_path.to_string(),
        mode: "copy".to_string(),
        status: status.to_string(),
        last_error: None,
        synced_at: None,
    }
}

#[test]
fn classify_target_accepts_real_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let central = dir.path().join("central");
    let target_dir = dir.path().join("tool").join("skill-a");
    std::fs::create_dir_all(&central).unwrap();
    std::fs::create_dir_all(&target_dir).unwrap();
    let record = make_target(
        "t1",
        "s1",
        "cursor",
        target_dir.to_string_lossy().as_ref(),
        "ok",
    );

    assert_eq!(classify_target(&central, &record), None);
}

#[test]
fn classify_target_reports_missing_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let central = dir.path().join("central");
    std::fs::create_dir_all(&central).unwrap();
    let record = make_target(
        "t1",
        "s1",
        "cursor",
        dir.path().join("tool/missing").to_string_lossy().as_ref(),
        "ok",
    );

    assert_eq!(
        classify_target(&central, &record),
        Some(BrokenTarget {
            tool: "cursor".to_string(),
            path: dir
                .path()
                .join("tool/missing")
                .to_string_lossy()
                .to_string(),
            reason: "missing".to_string(),
        })
    );
}

#[test]
fn classify_target_ignores_disabled_targets_at_caller_level() {
    // The report builder skips disabled rows entirely; classification itself
    // only depends on the filesystem entry.
    let dir = tempfile::tempdir().expect("tempdir");
    let central = dir.path().join("central");
    std::fs::create_dir_all(&central).unwrap();
    let record = make_target(
        "t1",
        "s1",
        "cursor",
        dir.path().join("tool/missing").to_string_lossy().as_ref(),
        "disabled",
    );
    assert_eq!(
        classify_target(&central, &record).unwrap().reason,
        "missing"
    );
}

#[test]
fn idle_requires_no_valid_targets_and_stale_mtime() {
    let now = 10_000_000;
    let threshold = IDLE_THRESHOLD_DAYS * 24 * 60 * 60;

    assert!(is_idle(0, now - threshold - 1, now));
    assert!(!is_idle(0, now - threshold, now));
    assert!(!is_idle(0, now - threshold - 1, 0));
    assert!(!is_idle(1, now - threshold - 1, now));
}

#[test]
fn skill_last_modified_reads_skill_md_mtime() {
    let dir = tempfile::tempdir().expect("tempdir");
    let skill_dir = dir.path().join("skill-a");
    std::fs::create_dir_all(&skill_dir).unwrap();
    assert_eq!(skill_last_modified_secs(&skill_dir), 0);

    std::fs::write(skill_dir.join("SKILL.md"), "# demo").unwrap();
    assert!(skill_last_modified_secs(&skill_dir) > 0);
}

#[test]
fn build_report_counts_synced_broken_and_idle_skills() {
    let (dir, store) = make_store();
    let central = dir.path().join("central");
    let skill_a_dir = central.join("skill-a");
    let skill_b_dir = central.join("skill-b");
    std::fs::create_dir_all(&skill_a_dir).unwrap();
    std::fs::create_dir_all(&skill_b_dir).unwrap();
    std::fs::write(skill_a_dir.join("SKILL.md"), "# a").unwrap();
    std::fs::write(skill_b_dir.join("SKILL.md"), "# b").unwrap();

    let valid_target_dir = dir.path().join("tool").join("skill-a");
    std::fs::create_dir_all(&valid_target_dir).unwrap();

    store
        .upsert_skill(&make_skill(
            "a",
            "skill-a",
            skill_a_dir.to_string_lossy().as_ref(),
        ))
        .unwrap();
    store
        .upsert_skill(&make_skill(
            "b",
            "skill-b",
            skill_b_dir.to_string_lossy().as_ref(),
        ))
        .unwrap();
    store
        .upsert_skill_target(&make_target(
            "ta",
            "a",
            "cursor",
            valid_target_dir.to_string_lossy().as_ref(),
            "ok",
        ))
        .unwrap();
    store
        .upsert_skill_target(&make_target(
            "tb",
            "a",
            "claude",
            dir.path().join("tool/gone").to_string_lossy().as_ref(),
            "ok",
        ))
        .unwrap();
    store
        .upsert_skill_target(&make_target(
            "tc",
            "b",
            "cursor",
            dir.path().join("tool/disabled").to_string_lossy().as_ref(),
            "disabled",
        ))
        .unwrap();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let report = build_library_health_report(&central, &store, now).unwrap();

    assert_eq!(report.summary.total, 2);
    assert_eq!(report.summary.synced_skills, 1);
    assert_eq!(report.summary.broken_targets, 1);
    assert_eq!(report.summary.idle_skills, 0);

    let health_a = report
        .skills
        .iter()
        .find(|skill| skill.name == "skill-a")
        .unwrap();
    assert_eq!(health_a.synced_tool_count, 1);
    assert_eq!(health_a.broken.len(), 1);
    assert_eq!(health_a.broken[0].reason, "missing");
    assert!(health_a.last_modified > 0);
    assert!(!health_a.idle);

    let health_b = report
        .skills
        .iter()
        .find(|skill| skill.name == "skill-b")
        .unwrap();
    assert_eq!(health_b.synced_tool_count, 0);
    assert!(health_b.broken.is_empty());
    assert!(!health_b.idle, "recent mtime must not be idle yet");
}
