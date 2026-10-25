use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::core::skill_store::{SkillRecord, SkillStore, SkillTargetRecord};
use crate::core::skills_lock::{
    build_lock_diff_report, build_skills_lock, collect_local_snapshots, format_rfc3339_utc,
    parse_skills_lock, write_lock_file, LocalSkillSnapshot, LockSkillEntry, LockTargetEntry,
    SkillsLockFile, LOCK_FORMAT_VERSION,
};

fn make_store() -> (tempfile::TempDir, SkillStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = SkillStore::new(dir.path().join("test.db"));
    store.ensure_schema().expect("ensure_schema");
    (dir, store)
}

fn insert_skill(
    store: &SkillStore,
    root: &Path,
    name: &str,
    body: &str,
    targets: &[(&str, &str, &str)],
) {
    let central = root.join(name);
    fs::create_dir_all(&central).unwrap();
    fs::write(central.join("SKILL.md"), body).unwrap();

    store
        .upsert_skill(&SkillRecord {
            id: name.to_string(),
            name: name.to_string(),
            description: None,
            source_type: "managed".to_string(),
            source_ref: Some(format!("https://example.com/{name}")),
            source_subpath: None,
            source_revision: None,
            central_path: central.to_string_lossy().to_string(),
            content_hash: None,
            created_at: 1,
            updated_at: 2,
            last_sync_at: None,
            last_seen_at: 1,
            enabled: true,
            status: "active".to_string(),
        })
        .unwrap();

    for (index, (tool, scope, target_path)) in targets.iter().enumerate() {
        store
            .upsert_skill_target(&SkillTargetRecord {
                id: format!("{name}-target-{index}"),
                skill_id: name.to_string(),
                tool: tool.to_string(),
                scope: scope.to_string(),
                project_path: None,
                target_path: target_path.to_string(),
                mode: "copy".to_string(),
                status: "synced".to_string(),
                last_error: None,
                synced_at: None,
            })
            .unwrap();
    }
}

#[test]
fn rfc3339_formats_known_timestamps() {
    assert_eq!(format_rfc3339_utc(0), "1970-01-01T00:00:00Z");
    assert_eq!(format_rfc3339_utc(86_400), "1970-01-02T00:00:00Z");
    // 2026-01-01T00:00:00Z
    assert_eq!(format_rfc3339_utc(1_767_225_600), "2026-01-01T00:00:00Z");
    // Leap day: 2024-02-29T12:30:45Z
    assert_eq!(format_rfc3339_utc(1_709_209_845), "2024-02-29T12:30:45Z");
}

#[test]
fn parse_accepts_valid_manifest_and_rejects_bad_input() {
    let lock = SkillsLockFile {
        version: LOCK_FORMAT_VERSION,
        exported_at: "2026-01-01T00:00:00Z".to_string(),
        skills: vec![LockSkillEntry {
            name: "alpha".to_string(),
            enabled: true,
            tags: vec!["prod".to_string()],
            source_ref: None,
            content_hash: "abc".to_string(),
            targets: vec![LockTargetEntry {
                tool: "cursor".to_string(),
                scope: "global".to_string(),
                target_path: "/home/user/.cursor/skills/alpha".to_string(),
            }],
        }],
    };
    let raw = serde_json::to_string(&lock).unwrap();
    let parsed = parse_skills_lock(&raw).unwrap();
    assert_eq!(parsed, lock);

    let bad_json = parse_skills_lock("{ not json");
    assert!(bad_json.is_err());
    assert!(bad_json
        .unwrap_err()
        .to_string()
        .contains("invalid skills.lock"));

    let future =
        parse_skills_lock(r#"{ "version": 99, "exported_at": "x", "skills": [] }"#).unwrap_err();
    assert!(future
        .to_string()
        .contains("unsupported skills.lock version: 99"));
}

#[test]
fn export_writes_manifest_that_round_trips_and_matches_library() {
    let (dir, store) = make_store();
    insert_skill(
        &store,
        dir.path(),
        "alpha",
        "---\nname: alpha\n---\nhello",
        &[("cursor", "global", "/home/a/.cursor/skills/alpha")],
    );
    insert_skill(
        &store,
        dir.path(),
        "beta",
        "---\nname: beta\n---\nworld",
        &[],
    );

    let lock = build_skills_lock(&store, "2026-01-02T03:04:05Z").unwrap();
    assert_eq!(lock.version, LOCK_FORMAT_VERSION);
    assert_eq!(lock.exported_at, "2026-01-02T03:04:05Z");
    // Sorted by name for deterministic exports.
    let names: Vec<&str> = lock.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["alpha", "beta"]);
    assert_eq!(lock.skills[0].targets.len(), 1);
    assert_eq!(lock.skills[0].targets[0].tool, "cursor");
    assert_eq!(
        lock.skills[0].source_ref.as_deref(),
        Some("https://example.com/alpha")
    );

    let lock_path = dir.path().join("skills.lock");
    write_lock_file(&lock, &lock_path).unwrap();
    let parsed = parse_skills_lock(&fs::read_to_string(&lock_path).unwrap()).unwrap();
    assert_eq!(parsed, lock);

    // A manifest exported from this very library must diff to all-empty.
    let snapshots = collect_local_snapshots(&store).unwrap();
    let report = build_lock_diff_report(lock_path.to_string_lossy().as_ref(), &parsed, snapshots);
    assert!(report.missing_here.is_empty());
    assert!(report.extra_here.is_empty());
    assert!(report.drifted.is_empty());
    assert!(report.targets_changed.is_empty());
    assert_eq!(report.lock_path, lock_path.to_string_lossy());
}

#[test]
fn export_falls_back_to_stored_hash_when_central_dir_is_missing() {
    let (dir, store) = make_store();
    insert_skill(&store, dir.path(), "ghost", "old-content", &[]);
    // Record a stored hash, then delete the central directory.
    let record = store
        .list_skills()
        .unwrap()
        .into_iter()
        .find(|s| s.name == "ghost")
        .unwrap();
    let mut updated = record.clone();
    updated.content_hash = Some("stored-hash".to_string());
    store.upsert_skill(&updated).unwrap();
    fs::remove_dir_all(dir.path().join("ghost")).unwrap();

    let lock = build_skills_lock(&store, "2026-01-01T00:00:00Z").unwrap();
    let entry = lock.skills.iter().find(|s| s.name == "ghost").unwrap();
    assert_eq!(entry.content_hash, "stored-hash");
}

#[test]
fn diff_classifies_missing_extra_drifted_and_target_changes() {
    let lock = SkillsLockFile {
        version: LOCK_FORMAT_VERSION,
        exported_at: "2026-01-01T00:00:00Z".to_string(),
        skills: vec![
            LockSkillEntry {
                name: "shared".to_string(),
                enabled: true,
                tags: vec![],
                source_ref: None,
                content_hash: "hash-shared".to_string(),
                targets: vec![LockTargetEntry {
                    tool: "cursor".to_string(),
                    scope: "global".to_string(),
                    target_path: "/machine-a/.cursor/skills/shared".to_string(),
                }],
            },
            LockSkillEntry {
                name: "gone".to_string(),
                enabled: false,
                tags: vec![],
                source_ref: None,
                content_hash: "hash-gone".to_string(),
                targets: vec![],
            },
            LockSkillEntry {
                name: "edited".to_string(),
                enabled: true,
                tags: vec![],
                source_ref: None,
                content_hash: "hash-old".to_string(),
                targets: vec![],
            },
            LockSkillEntry {
                name: "retargeted".to_string(),
                enabled: true,
                tags: vec![],
                source_ref: None,
                content_hash: "hash-retargeted".to_string(),
                targets: vec![LockTargetEntry {
                    tool: "cursor".to_string(),
                    scope: "global".to_string(),
                    target_path: "/machine-a/.cursor/skills/retargeted".to_string(),
                }],
            },
        ],
    };

    let local = vec![
        LocalSkillSnapshot {
            name: "shared".to_string(),
            content_hash: "hash-shared".to_string(),
            // Same (tool, scope) pair, different absolute path: portable, not a change.
            targets: BTreeSet::from([("cursor".to_string(), "global".to_string())]),
        },
        LocalSkillSnapshot {
            name: "edited".to_string(),
            content_hash: "hash-new".to_string(),
            targets: BTreeSet::new(),
        },
        LocalSkillSnapshot {
            name: "retargeted".to_string(),
            content_hash: "hash-retargeted".to_string(),
            targets: BTreeSet::from([("claude_code".to_string(), "global".to_string())]),
        },
        LocalSkillSnapshot {
            name: "bonus".to_string(),
            content_hash: "hash-bonus".to_string(),
            targets: BTreeSet::new(),
        },
    ];

    let report = build_lock_diff_report("C:\\locks\\skills.lock", &lock, local);

    assert_eq!(report.lock_path, "C:\\locks\\skills.lock");

    let missing: Vec<&str> = report
        .missing_here
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    assert_eq!(missing, vec!["gone"]);
    assert!(!report.missing_here[0].enabled);
    assert_eq!(report.extra_here, vec!["bonus"]);

    assert_eq!(report.drifted.len(), 1);
    assert_eq!(report.drifted[0].name, "edited");
    assert_eq!(report.drifted[0].lock_hash, "hash-old");
    assert_eq!(report.drifted[0].local_hash, "hash-new");

    assert_eq!(report.targets_changed, vec!["retargeted"]);
}

#[test]
fn diff_reports_drift_after_local_content_changes() {
    let (dir, store) = make_store();
    insert_skill(&store, dir.path(), "alpha", "v1-content", &[]);

    let lock = build_skills_lock(&store, "2026-01-01T00:00:00Z").unwrap();

    // Modify the central content, then compare the stale manifest.
    fs::write(dir.path().join("alpha").join("SKILL.md"), "v2-content").unwrap();
    let snapshots = collect_local_snapshots(&store).unwrap();
    let report = build_lock_diff_report("skills.lock", &lock, snapshots);

    assert!(report.missing_here.is_empty());
    assert!(report.extra_here.is_empty());
    assert_eq!(report.drifted.len(), 1);
    assert_eq!(report.drifted[0].name, "alpha");
    assert_ne!(report.drifted[0].lock_hash, report.drifted[0].local_hash);
}
