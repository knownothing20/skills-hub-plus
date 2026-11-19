use std::fs;
use std::time::Duration;

use super::{cleanup_old_git_temp_dirs_in, mark_temp_dir, stale_central_residue};

#[test]
fn cleanup_removes_only_marked_prefixed_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path();

    let d1 = cache.join("skills-hub-git-1");
    let d2 = cache.join("skills-hub-git-2");
    let d3 = cache.join("other-3");

    fs::create_dir_all(&d1).unwrap();
    fs::create_dir_all(&d2).unwrap();
    fs::create_dir_all(&d3).unwrap();

    mark_temp_dir(&d1).unwrap();
    mark_temp_dir(&d3).unwrap();

    let removed = cleanup_old_git_temp_dirs_in(cache, Duration::from_secs(0)).unwrap();
    assert_eq!(removed, 1);
    assert!(!d1.exists());
    assert!(d2.exists(), "未标记的不应删除");
    assert!(d3.exists(), "前缀不匹配的不应删除");
}

#[test]
fn residue_selection_only_picks_hidden_staging_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    let residue = root.join(".skills-hub-install-abc");
    let regular = root.join("shengcai-shared-mcp");
    let legacy = root.join(".tabbit-stage-DEADBEEF");
    let loose_file = root.join(".skills-hub-note.txt");

    fs::create_dir_all(&residue).unwrap();
    fs::create_dir_all(&regular).unwrap();
    fs::create_dir_all(&legacy).unwrap();
    fs::write(&loose_file, b"x").unwrap();

    let picked = stale_central_residue(root, Duration::from_secs(0));
    assert_eq!(picked.len(), 1, "只应挑出 .skills-hub-* 目录");
    assert!(picked[0].ends_with(".skills-hub-install-abc"));
    assert!(regular.exists(), "正式技能目录绝不能进清理名单");
    assert!(legacy.exists(), "非本应用前缀的历史目录不在清扫范围");
}

#[test]
fn residue_selection_skips_recent_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let fresh = root.join(".skills-hub-update-fresh");
    fs::create_dir_all(&fresh).unwrap();

    // 一小时窗口内刚创建的目录必须保留，否则会撞上正在进行的安装。
    let picked = stale_central_residue(root, Duration::from_secs(3600));
    assert!(picked.is_empty(), "新建目录不应被清扫");
}

#[test]
fn residue_selection_handles_missing_root() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("does-not-exist");
    assert!(stale_central_residue(&missing, Duration::from_secs(0)).is_empty());
}
