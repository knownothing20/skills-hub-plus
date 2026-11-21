use super::*;
use crate::core::auto_update::managed_skill_update_capability;
use crate::core::skill_store::{SkillRecord, SkillStore, SkillTargetRecord};
use crate::core::sync_engine::SyncMode;
use crate::core::tool_adapters::{save_tool_config, CustomToolConfig, ToolConfig};

fn make_store() -> (tempfile::TempDir, SkillStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = SkillStore::new(dir.path().join("test.db"));
    store.ensure_schema().expect("ensure_schema");
    (dir, store)
}

#[test]
fn format_anyhow_error_passthrough_prefixes() {
    for message in [
        "MULTI_SKILLS|abc",
        "TARGET_EXISTS|/tmp/skill",
        "TOOL_NOT_INSTALLED|cursor",
        "TOOL_NOT_WRITABLE|Cursor|/tmp/skills",
    ] {
        assert_eq!(format_anyhow_error(anyhow::anyhow!(message)), message);
    }
}

#[test]
fn format_anyhow_error_redacts_clone_temp_path() {
    let err = anyhow::anyhow!("clone https://example.com/a/b into /tmp/skills-hub-git-123");
    let msg = format_anyhow_error(err);
    assert!(msg.contains("已省略临时目录"));
    assert!(!msg.contains("/tmp/skills-hub-git-123"));
}

#[test]
fn format_anyhow_error_github_hint_auth() {
    let err = anyhow::anyhow!("git clone https://github.com/a/b failed: authentication failed");
    let msg = format_anyhow_error(err);
    assert!(msg.contains("无法访问该仓库"));
}

#[test]
fn expand_home_path_basic() {
    let home = dirs::home_dir().expect("home");
    assert_eq!(expand_home_path("~").unwrap(), home);
    assert_eq!(expand_home_path("~/abc").unwrap(), home.join("abc"));
}

#[test]
fn expand_home_path_empty_is_error() {
    let err = expand_home_path("  ").unwrap_err().to_string();
    assert!(err.contains("storage path is empty"));
}

#[test]
fn saving_custom_tool_config_creates_enabled_skills_dir() {
    let (dir, store) = make_store();
    let existing = dir.path().join("existing-skills");
    std::fs::create_dir_all(&existing).unwrap();
    let created = dir.path().join("created-skills");
    assert!(!created.exists());

    save_tool_config(
        &store,
        ToolConfig {
            disabled_builtin_tools: Vec::new(),
            custom_tools: vec![
                CustomToolConfig {
                    key: "custom_existing".to_string(),
                    label: "Existing".to_string(),
                    avatar: Some("data:image/png;base64,AA==".to_string()),
                    skills_dir: existing.to_string_lossy().to_string(),
                    project_skills_dir: None,
                    sync_mode: SyncMode::Copy,
                    enabled: true,
                },
                CustomToolConfig {
                    key: "custom_created".to_string(),
                    label: "Created".to_string(),
                    avatar: None,
                    skills_dir: created.to_string_lossy().to_string(),
                    project_skills_dir: None,
                    sync_mode: SyncMode::Copy,
                    enabled: true,
                },
            ],
        },
    )
    .unwrap();
    assert!(created.is_dir());

    let tools = runtime_tools(&store, true).unwrap();
    let existing_tool = tools
        .iter()
        .find(|tool| tool.key == "custom_existing")
        .unwrap();
    let created_tool = tools
        .iter()
        .find(|tool| tool.key == "custom_created")
        .unwrap();

    assert!(existing_tool.enabled);
    assert!(existing_tool.installed);
    assert_eq!(
        existing_tool.avatar.as_deref(),
        Some("data:image/png;base64,AA==")
    );
    assert_eq!(existing_tool.sync_mode, SyncMode::Copy);
    assert!(created_tool.enabled);
    assert!(created_tool.installed);
    assert_eq!(created_tool.sync_mode, SyncMode::Copy);
}

#[test]
fn normalize_scope_defaults_to_global_and_rejects_unknown() {
    assert_eq!(normalize_scope(None).unwrap(), "global");
    assert_eq!(normalize_scope(Some("global")).unwrap(), "global");
    assert_eq!(normalize_scope(Some("project")).unwrap(), "project");
    assert!(normalize_scope(Some("workspace")).is_err());
}

#[test]
fn recent_projects_are_deduped_ordered_and_limited() {
    let (_dir, store) = make_store();
    let project_root = tempfile::tempdir().unwrap();
    let mut paths = Vec::new();
    for i in 0..9 {
        let path = project_root.path().join(format!("project-{i}"));
        std::fs::create_dir_all(&path).unwrap();
        paths.push(path);
    }

    for path in &paths {
        save_recent_project_impl(&store, path.to_string_lossy().as_ref()).unwrap();
    }

    let recent = get_recent_projects_impl(&store).unwrap();
    assert_eq!(recent.len(), 8);
    assert_eq!(recent[0], paths[8].to_string_lossy());
    assert_eq!(recent[7], paths[1].to_string_lossy());
    assert!(!recent.contains(&paths[0].to_string_lossy().to_string()));

    save_recent_project_impl(&store, paths[3].to_string_lossy().as_ref()).unwrap();
    let recent = get_recent_projects_impl(&store).unwrap();
    assert_eq!(recent.len(), 8);
    assert_eq!(recent[0], paths[3].to_string_lossy());
    assert_eq!(
        recent
            .iter()
            .filter(|item| *item == &paths[3].to_string_lossy())
            .count(),
        1
    );
}

#[test]
fn save_recent_project_rejects_missing_directory() {
    let (_dir, store) = make_store();
    let missing = tempfile::tempdir().unwrap().path().join("missing-project");
    let err = save_recent_project_impl(&store, missing.to_string_lossy().as_ref())
        .unwrap_err()
        .to_string();
    assert!(err.contains("projectPath must be an existing directory"));
}

#[test]
fn get_managed_skills_impl_maps_targets() {
    let (dir, store) = make_store();
    let skill = SkillRecord {
        id: "s1".to_string(),
        name: "S1".to_string(),
        description: None,
        source_type: "local".to_string(),
        source_ref: Some(
            dir.path()
                .join("missing-source")
                .to_string_lossy()
                .to_string(),
        ),
        source_subpath: None,
        source_revision: None,
        central_path: "/tmp/central".to_string(),
        content_hash: None,
        created_at: 1,
        updated_at: 2,
        last_sync_at: None,
        last_seen_at: 1,
        enabled: true,
        status: "ok".to_string(),
    };
    store.upsert_skill(&skill).unwrap();

    let target = SkillTargetRecord {
        id: "t1".to_string(),
        instance: String::new(),
        skill_id: "s1".to_string(),
        tool: "cursor".to_string(),
        scope: "global".to_string(),
        project_path: None,
        target_path: "/tmp/target".to_string(),
        mode: "copy".to_string(),
        status: "error".to_string(),
        last_error: Some("permission denied".to_string()),
        synced_at: None,
    };
    store.upsert_skill_target(&target).unwrap();
    let tag = store.create_tag("Frontend").unwrap();
    store.set_skill_tags("s1", &[tag.id]).unwrap();

    let out = get_managed_skills_impl(&store).unwrap();
    assert_eq!(out.len(), 1);
    assert!(out[0].enabled);
    assert_eq!(out[0].tags.len(), 1);
    assert_eq!(out[0].tags[0].name, "Frontend");
    assert_eq!(out[0].targets.len(), 1);
    assert_eq!(out[0].targets[0].tool, "cursor");
    assert_eq!(out[0].targets[0].scope, "global");
    assert_eq!(out[0].targets[0].status, "error");
    assert_eq!(
        out[0].targets[0].last_error.as_deref(),
        Some("permission denied")
    );
    assert!(out[0].targets[0].project_path.is_none());
    assert_eq!(out[0].status, "error");
    assert!(out[0].has_external_source);
    assert!(!out[0].updateable);
}

#[test]
fn get_managed_skills_impl_embeds_standard_icon_without_exposing_asset_path() {
    let (dir, store) = make_store();
    let central = dir.path().join("metadata-icon-skill");
    std::fs::create_dir_all(central.join("agents")).unwrap();
    std::fs::create_dir_all(central.join("assets")).unwrap();
    std::fs::write(
        central.join("agents/openai.yaml"),
        "interface:\n  icon_small: \"./assets/icon.svg\"\n  brand_color: \"#3B82F6\"\n",
    )
    .unwrap();
    std::fs::write(
        central.join("assets/icon.svg"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1 1\"></svg>",
    )
    .unwrap();
    store
        .upsert_skill(&SkillRecord {
            id: "metadata-icon".to_string(),
            name: "metadata-icon".to_string(),
            description: None,
            source_type: "managed".to_string(),
            source_ref: None,
            source_subpath: None,
            source_revision: None,
            central_path: central.to_string_lossy().to_string(),
            content_hash: None,
            created_at: 1,
            updated_at: 2,
            last_sync_at: None,
            last_seen_at: 1,
            enabled: true,
            status: "ok".to_string(),
        })
        .unwrap();

    let out = get_managed_skills_impl(&store).unwrap();
    assert!(out[0]
        .icon_data_url
        .as_deref()
        .unwrap()
        .starts_with("data:image/svg+xml;base64,"));
    assert_eq!(out[0].brand_color.as_deref(), Some("#3B82F6"));
    let serialized = serde_json::to_string(&out).unwrap();
    assert!(!serialized.contains("assets/icon.svg"));
}

#[test]
fn invalid_icon_metadata_does_not_change_skill_health() {
    let (dir, store) = make_store();
    let central = dir.path().join("invalid-icon-skill");
    std::fs::create_dir_all(central.join("agents")).unwrap();
    std::fs::write(
        central.join("agents/openai.yaml"),
        "interface:\n  icon_small: \"../outside.png\"\n",
    )
    .unwrap();
    store
        .upsert_skill(&SkillRecord {
            id: "invalid-icon".to_string(),
            name: "invalid-icon".to_string(),
            description: None,
            source_type: "managed".to_string(),
            source_ref: None,
            source_subpath: None,
            source_revision: None,
            central_path: central.to_string_lossy().to_string(),
            content_hash: None,
            created_at: 1,
            updated_at: 2,
            last_sync_at: None,
            last_seen_at: 1,
            enabled: true,
            status: "ok".to_string(),
        })
        .unwrap();

    let out = get_managed_skills_impl(&store).unwrap();
    assert!(out[0].icon_data_url.is_none());
    assert_eq!(out[0].status, "ok");
}

#[test]
fn managed_icon_data_url_budget_drops_the_overflow_and_all_later_icons() {
    let mut remaining = 10usize;
    let mut first = Some("123456".to_string());
    apply_managed_icon_data_url_budget(&mut first, &mut remaining);
    assert!(first.is_some());
    assert_eq!(remaining, 4);

    let mut overflow = Some("12345".to_string());
    apply_managed_icon_data_url_budget(&mut overflow, &mut remaining);
    assert!(overflow.is_none());
    assert_eq!(remaining, 0);

    let mut later = Some("1".to_string());
    apply_managed_icon_data_url_budget(&mut later, &mut remaining);
    assert!(later.is_none());
}

#[test]
fn managed_list_budget_keeps_earlier_icon_and_falls_back_later_without_health_error() {
    let (dir, store) = make_store();
    for (id, updated_at) in [("first-icon", 2), ("second-icon", 1)] {
        let central = dir.path().join(id);
        std::fs::create_dir_all(central.join("agents")).unwrap();
        std::fs::create_dir_all(central.join("assets")).unwrap();
        std::fs::write(
            central.join("agents/openai.yaml"),
            "interface:\n  icon_small: './assets/icon.svg'\n",
        )
        .unwrap();
        std::fs::write(
            central.join("assets/icon.svg"),
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1 1\"><path d=\"M0 0\"/></svg>",
        )
        .unwrap();
        store
            .upsert_skill(&SkillRecord {
                id: id.to_string(),
                name: id.to_string(),
                description: None,
                source_type: "managed".to_string(),
                source_ref: None,
                source_subpath: None,
                source_revision: None,
                central_path: central.to_string_lossy().to_string(),
                content_hash: None,
                created_at: 1,
                updated_at,
                last_sync_at: None,
                last_seen_at: 1,
                enabled: true,
                status: "ok".to_string(),
            })
            .unwrap();
    }

    let unlimited = get_managed_skills_impl(&store).unwrap();
    let first_icon_bytes = unlimited[0].icon_data_url.as_ref().unwrap().len();
    let budgeted = get_managed_skills_impl_with_icon_budget(&store, first_icon_bytes).unwrap();
    assert_eq!(budgeted[0].name, "first-icon");
    assert!(budgeted[0].icon_data_url.is_some());
    assert_eq!(budgeted[1].name, "second-icon");
    assert!(budgeted[1].icon_data_url.is_none());
    assert!(budgeted.iter().all(|skill| skill.status == "ok"));
}

#[test]
fn get_managed_skills_impl_redacts_unsafe_git_source_refs() {
    let (dir, store) = make_store();
    let central = dir.path().join("unsafe-git");
    std::fs::create_dir(&central).unwrap();
    let mut skill = SkillRecord {
        id: "unsafe-git".to_string(),
        name: "unsafe-git".to_string(),
        description: None,
        source_type: "git".to_string(),
        source_ref: Some("https://secret-token@example.com/repo.git".to_string()),
        source_subpath: None,
        source_revision: None,
        central_path: central.to_string_lossy().to_string(),
        content_hash: None,
        created_at: 1,
        updated_at: 2,
        last_sync_at: None,
        last_seen_at: 1,
        enabled: true,
        status: "ok".to_string(),
    };
    store.upsert_skill(&skill).unwrap();

    let out = get_managed_skills_impl(&store).unwrap();
    assert_eq!(out.len(), 1);
    assert!(out[0].source_ref.is_none());
    assert!(!out[0].updateable);
    assert_eq!(out[0].status, "error");
    let serialized = serde_json::to_string(&out).unwrap();
    assert!(!serialized.contains("secret-token"));

    skill.source_ref = Some("anthropics/skills".to_string());
    store.upsert_skill(&skill).unwrap();
    let out = get_managed_skills_impl(&store).unwrap();
    assert_eq!(out[0].source_ref.as_deref(), Some("anthropics/skills"));
    assert!(out[0].updateable);
}

#[test]
fn managed_skill_status_keeps_existing_local_sources_healthy() {
    let source = tempfile::tempdir().unwrap();
    let central = tempfile::tempdir().unwrap();
    let skill = SkillRecord {
        id: "s1".to_string(),
        name: "S1".to_string(),
        description: None,
        source_type: "local".to_string(),
        source_ref: Some(source.path().to_string_lossy().to_string()),
        source_subpath: None,
        source_revision: None,
        central_path: central.path().to_string_lossy().to_string(),
        content_hash: None,
        created_at: 1,
        updated_at: 1,
        last_sync_at: None,
        last_seen_at: 1,
        enabled: true,
        status: "ok".to_string(),
    };

    let capability = managed_skill_update_capability(&skill);
    assert_eq!(managed_skill_status(&skill, &capability), "ok");
}

#[test]
fn record_skill_target_failure_persists_error_status() {
    let (_dir, store) = make_store();
    let skill = SkillRecord {
        id: "s1".to_string(),
        name: "S1".to_string(),
        description: None,
        source_type: "local".to_string(),
        source_ref: Some("/tmp/src".to_string()),
        source_subpath: None,
        source_revision: None,
        central_path: "/tmp/central".to_string(),
        content_hash: None,
        created_at: 1,
        updated_at: 2,
        last_sync_at: None,
        last_seen_at: 1,
        enabled: true,
        status: "ok".to_string(),
    };
    store.upsert_skill(&skill).unwrap();

    record_skill_target_failure(
        &store,
        "s1",
        "cursor",
        "global",
        None,
        std::path::Path::new("/tmp/target"),
        SyncMode::Copy,
        "permission denied",
        "",
    )
    .unwrap();

    let target = store
        .get_skill_target("s1", "cursor", "global", None)
        .unwrap()
        .unwrap();
    assert_eq!(target.status, "error");
    assert_eq!(target.last_error.as_deref(), Some("permission denied"));
    assert_eq!(target.mode, "copy");
    assert!(target.synced_at.is_none());
}

#[test]
fn test_discovered_skill_preserves_physical_entity_without_converting_to_link() {
    let (dir, store) = make_store();

    let central_root = dir.path().join("central");
    std::fs::create_dir_all(&central_root).unwrap();

    let tool_skills = dir.path().join("client-skills");
    std::fs::create_dir_all(&tool_skills).unwrap();

    save_tool_config(
        &store,
        ToolConfig {
            disabled_builtin_tools: Vec::new(),
            custom_tools: vec![CustomToolConfig {
                key: "test_client_tool".to_string(),
                label: "Client Tool".to_string(),
                avatar: None,
                skills_dir: tool_skills.to_string_lossy().to_string(),
                project_skills_dir: None,
                sync_mode: SyncMode::Copy,
                enabled: true,
            }],
        },
    )
    .unwrap();

    let client_skill_dir = tool_skills.join("discovered-skill");
    std::fs::create_dir_all(&client_skill_dir).unwrap();
    std::fs::write(
        client_skill_dir.join("SKILL.md"),
        "---\nname: discovered-skill\n---\n# Real entity\n",
    )
    .unwrap();
    std::fs::write(
        client_skill_dir.join("logic.py"),
        "print('original physical content')\n",
    )
    .unwrap();

    assert!(client_skill_dir.is_dir());
    assert!(!client_skill_dir.is_symlink());

    let central_skill = central_root.join("discovered-skill");
    std::fs::create_dir_all(&central_skill).unwrap();
    std::fs::write(
        central_skill.join("SKILL.md"),
        "---\nname: discovered-skill\n---\n# Real entity\n",
    )
    .unwrap();
    std::fs::write(
        central_skill.join("logic.py"),
        "print('original physical content')\n",
    )
    .unwrap();

    store
        .upsert_skill(&SkillRecord {
            id: "discovered-skill-id".to_string(),
            name: "discovered-skill".to_string(),
            description: None,
            source_type: "local".to_string(),
            source_ref: Some(client_skill_dir.to_string_lossy().to_string()),
            source_subpath: None,
            source_revision: None,
            central_path: central_skill.to_string_lossy().to_string(),
            content_hash: None,
            created_at: 1,
            updated_at: 1,
            last_sync_at: None,
            last_seen_at: 1,
            enabled: true,
            status: "ok".to_string(),
        })
        .unwrap();

    // 场景 1: 前端导入新发现技能，向来源工具显式要求 copy
    let res = sync_skill_to_tool_impl(
        &central_root,
        &store,
        &central_skill.to_string_lossy(),
        "discovered-skill-id",
        "test_client_tool",
        "discovered-skill",
        Some(true),
        Some(true),
        Some("global"),
        None,
        Some("copy"),
        None,
        "",
    )
    .unwrap();

    assert_eq!(res.mode_used, "copy");
    assert!(client_skill_dir.is_dir());
    assert!(!client_skill_dir.is_symlink());
    assert_eq!(
        std::fs::read_to_string(client_skill_dir.join("logic.py")).unwrap(),
        "print('original physical content')\n"
    );

    let target_record = store
        .get_skill_target("discovered-skill-id", "test_client_tool", "global", None)
        .unwrap()
        .expect("target record exists");
    assert_eq!(target_record.mode, "copy");
    assert_eq!(target_record.status, "ok");

    // 场景 2: 即使未传 syncMode 且系统设置为 junction，只要是 is_source_path 物理实体，也坚决保留为实体副本
    store.set_setting("default_sync_mode", "junction").unwrap();
    let res2 = sync_skill_to_tool_impl(
        &central_root,
        &store,
        &central_skill.to_string_lossy(),
        "discovered-skill-id",
        "test_client_tool",
        "discovered-skill",
        Some(true),
        Some(true),
        Some("global"),
        None,
        None,
        None,
        "",
    )
    .unwrap();

    assert_eq!(res2.mode_used, "copy");
    assert!(client_skill_dir.is_dir());
    assert!(!client_skill_dir.is_symlink());
}

/// 软件被卸载后，它的目标记录仍然存在，但目录已经不在了。
///
/// 这是 2026-11 豆包卸载后的真实状态：`status` 还是 ok，`target_path` 指向
/// 一个随软件消失的目录。校验必须放行这种记录，否则用户既取消不掉、也删
/// 不掉，会一直卡在「断链 N」里。
#[test]
fn vanished_target_is_validated_for_cleanup_instead_of_failing_on_canonicalize() {
    let (_dir, store) = make_store();
    let central = tempfile::tempdir().unwrap();

    // 用真实的工具根，这样「记录里的父目录」与「解析出的根」一致——
    // 与生产环境里那条豆包记录的结构完全相同。
    let tool = runtime_tools(&store, true)
        .unwrap()
        .into_iter()
        .find(|candidate| candidate.key == "codex")
        .expect("codex 应出现在运行时工具列表里");
    let gone_target = tool.skills_dir.join("demo-skill");
    assert!(!gone_target.exists(), "测试前提：目标目录必须不存在");

    store
        .upsert_skill(&SkillRecord {
            id: "demo".to_string(),
            name: "demo-skill".to_string(),
            description: None,
            source_type: "managed".to_string(),
            source_ref: None,
            source_subpath: None,
            source_revision: None,
            central_path: central.path().to_string_lossy().to_string(),
            content_hash: None,
            created_at: 1,
            updated_at: 1,
            last_sync_at: None,
            last_seen_at: 1,
            enabled: true,
            status: "ok".to_string(),
        })
        .unwrap();
    let target = SkillTargetRecord {
        id: "t1".to_string(),
        instance: String::new(),
        skill_id: "demo".to_string(),
        tool: "codex".to_string(),
        scope: "global".to_string(),
        project_path: None,
        target_path: gone_target.to_string_lossy().to_string(),
        mode: "copy".to_string(),
        status: "ok".to_string(),
        last_error: None,
        synced_at: Some(1),
    };

    // 关键断言：校验通过，且返回的路径与记录一致（不会被悄悄改成别处）。
    let (root, path) =
        validated_managed_target(&store, &target, "demo-skill").expect("失效目标应可校验");
    assert_eq!(path, gone_target, "返回的目标路径必须与记录一致");
    assert_eq!(root, tool.skills_dir, "根目录应为该工具的真实技能根");
}

/// 名字对不上的记录仍要被拒——放宽只针对「目录不存在」，不放过张冠李戴。
#[test]
fn vanished_target_with_mismatched_name_is_still_rejected() {
    let (_dir, store) = make_store();
    let central = tempfile::tempdir().unwrap();
    store
        .upsert_skill(&SkillRecord {
            id: "demo".to_string(),
            name: "demo-skill".to_string(),
            description: None,
            source_type: "managed".to_string(),
            source_ref: None,
            source_subpath: None,
            source_revision: None,
            central_path: central.path().to_string_lossy().to_string(),
            content_hash: None,
            created_at: 1,
            updated_at: 1,
            last_sync_at: None,
            last_seen_at: 1,
            enabled: true,
            status: "ok".to_string(),
        })
        .unwrap();
    let tool = runtime_tools(&store, true)
        .unwrap()
        .into_iter()
        .find(|candidate| candidate.key == "codex")
        .expect("codex 应出现在运行时工具列表里");
    let target = SkillTargetRecord {
        id: "t1".to_string(),
        instance: String::new(),
        skill_id: "demo".to_string(),
        tool: "codex".to_string(),
        scope: "global".to_string(),
        project_path: None,
        // 磁盘上的名字与记录里的技能名不一致。
        target_path: tool
            .skills_dir
            .join("other-skill")
            .to_string_lossy()
            .to_string(),
        mode: "copy".to_string(),
        status: "ok".to_string(),
        last_error: None,
        synced_at: Some(1),
    };

    assert!(
        validated_managed_target(&store, &target, "demo-skill").is_err(),
        "名字不匹配的记录必须继续被拒绝"
    );
}

/// 多账号工具的副本必须**按账号分别登记**，不能挤进主实例。
///
/// 这条锁住 2026-11-21 的「豆包只显示 1 个账号」：`auto_reconcile_skill_targets`
/// 原先只扫 `home.join(relative_skills_dir)`（豆包这里是空目录 `~/DoubaoWork/skills`，
/// 真正的账号目录在 `User Data/<Profile>/...`），并且写死 `instance: String::new()`。
/// 结果是非主账号目录里的技能从来没被登记（Profile 5 磁盘 7 个、库里 1 个），
/// 界面只能显示「1 个账号」，用户以为同步把东西弄丢了。
#[test]
fn auto_reconcile_registers_skill_once_per_account() {
    let dir = tempfile::tempdir().unwrap();
    let store = SkillStore::new(dir.path().join("test.db"));
    store.ensure_schema().unwrap();

    // 母库目录（resolve_skill_source 会读 central_path）
    let central = dir.path().join("central/demo-skill");
    std::fs::create_dir_all(&central).unwrap();
    std::fs::write(central.join("SKILL.md"), b"hello").unwrap();

    store
        .upsert_skill(&SkillRecord {
            id: "sid".to_string(),
            name: "demo-skill".to_string(),
            description: None,
            source_type: "local".to_string(),
            source_ref: Some(central.to_string_lossy().to_string()),
            source_subpath: None,
            source_revision: None,
            central_path: central.to_string_lossy().to_string(),
            content_hash: None,
            created_at: 1,
            updated_at: 2,
            last_sync_at: None,
            last_seen_at: 1,
            enabled: true,
            status: "ok".to_string(),
        })
        .unwrap();

    // 关键断言：登记时按 instance 区分，同一个工具可以有多条记录。
    let now = 1234;
    let p4 = dir.path().join("u/Profile 4/.doubaowork/skills/demo-skill");
    let p5 = dir.path().join("u/Profile 5/.doubaowork/skills/demo-skill");
    std::fs::create_dir_all(&p4).unwrap();
    std::fs::create_dir_all(&p5).unwrap();

    let skill = store.get_skill_by_id("sid").unwrap().unwrap();
    assert!(
        register_reconciled_target(&store, &skill, "doubao_work", "Profile 4", &p4, now).unwrap()
    );
    assert!(
        register_reconciled_target(&store, &skill, "doubao_work", "Profile 5", &p5, now).unwrap()
    );

    let targets = store.list_skill_targets("sid").unwrap();
    let instances: Vec<&str> = targets.iter().map(|t| t.instance.as_str()).collect();
    assert!(
        instances.contains(&"Profile 4") && instances.contains(&"Profile 5"),
        "两个账号应各有一条记录，实际：{instances:?}"
    );
    assert_eq!(
        store
            .get_skill_target_instance("sid", "doubao_work", "global", None, "Profile 5")
            .unwrap()
            .map(|t| t.target_path),
        Some(p5.to_string_lossy().to_string()),
        "Profile 5 的记录必须指向它自己的目录，而不是主实例的"
    );

    // 再跑一次同样的登记应短路（status 已 ok），不重复计数。
    assert!(
        !register_reconciled_target(&store, &skill, "doubao_work", "Profile 4", &p4, now).unwrap(),
        "已登记且 ok 的目标不应重复计数"
    );
}
