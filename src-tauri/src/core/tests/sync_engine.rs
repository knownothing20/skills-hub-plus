use std::fs;

use crate::core::sync_engine::{
    carry_over_skillignore, copy_dir_recursive, sync_dir_copy_with_overwrite,
    sync_dir_with_mode_with_overwrite, SyncMode,
};

#[test]
fn copy_dir_recursive_skips_git_dir() {
    let src_dir = tempfile::tempdir().unwrap();
    let dst_dir = tempfile::tempdir().unwrap();

    fs::create_dir_all(src_dir.path().join(".git")).unwrap();
    fs::create_dir_all(src_dir.path().join("sub")).unwrap();
    fs::write(src_dir.path().join("sub/a.txt"), b"ok").unwrap();
    fs::write(src_dir.path().join(".git/secret"), b"no").unwrap();

    copy_dir_recursive(src_dir.path(), dst_dir.path()).unwrap();
    assert!(dst_dir.path().join("sub/a.txt").exists());
    assert!(!dst_dir.path().join(".git").exists());
}

#[test]
fn failed_copy_overwrite_restores_previous_target() {
    let src_root = tempfile::tempdir().unwrap();
    let missing_source = src_root.path().join("missing-source");
    let dst_dir = tempfile::tempdir().unwrap();
    let target = dst_dir.path().join("managed-skill");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("version"), b"old").unwrap();

    let err =
        crate::core::sync_engine::sync_dir_copy_with_overwrite(&missing_source, &target, true)
            .unwrap_err();
    assert!(!err.to_string().is_empty());
    assert_eq!(fs::read(target.join("version")).unwrap(), b"old");
}

#[test]
fn explicit_copy_mode_copies_directory() {
    let src_dir = tempfile::tempdir().unwrap();
    fs::write(src_dir.path().join("a.txt"), b"ok").unwrap();
    let dst_dir = tempfile::tempdir().unwrap();
    let target = dst_dir.path().join("t");

    let out =
        sync_dir_with_mode_with_overwrite(SyncMode::Copy, src_dir.path(), &target, false).unwrap();

    assert_eq!(out.mode_used, SyncMode::Copy);
    assert_eq!(fs::read(target.join("a.txt")).unwrap(), b"ok");
}

#[cfg(unix)]
#[test]
fn copy_overwrite_replaces_broken_symlink_target() {
    use std::os::unix::fs::symlink;

    let src_dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(src_dir.path().join("s")).unwrap();
    fs::write(src_dir.path().join("s/a.txt"), b"ok").unwrap();

    let dst_dir = tempfile::tempdir().unwrap();
    let target = dst_dir.path().join("t");

    // Create a broken symlink at the target path.
    symlink(dst_dir.path().join("missing"), &target).unwrap();

    let out = crate::core::sync_engine::sync_dir_copy_with_overwrite(src_dir.path(), &target, true)
        .unwrap();

    assert!(matches!(out.mode_used, SyncMode::Copy));
    assert!(target.join("s/a.txt").exists());
    assert_eq!(fs::read(target.join("s/a.txt")).unwrap(), b"ok");
}

/// 目标目录里的「本地私有内容」必须在整目录替换后原样存活。
/// 2026-11 事故：AgentPort 的 local/ 私有配置和 node_modules 被覆盖同步一起清空。
#[test]
fn overwrite_keeps_ignore_listed_private_content() {
    let src_dir = tempfile::tempdir().unwrap();
    fs::write(src_dir.path().join("cli.js"), b"new-source").unwrap();
    fs::write(src_dir.path().join("SKILL.md"), b"new-skill").unwrap();

    let dst_root = tempfile::tempdir().unwrap();
    let target = dst_root.path().join("agentport");
    fs::create_dir_all(target.join("local")).unwrap();
    fs::create_dir_all(target.join("node_modules/axios")).unwrap();
    fs::create_dir_all(target.join("logs")).unwrap();
    fs::write(target.join("local/connections.json"), b"secret-token").unwrap();
    fs::write(target.join("node_modules/axios/index.js"), b"dep").unwrap();
    fs::write(target.join("logs/run.log"), b"trace").unwrap();
    fs::write(target.join("cli.js"), b"old-source").unwrap();
    // 私有内容清单：副本自己维护，同步时不得触碰
    fs::write(target.join(".skillignore"), b"local\nlogs\nruntime\n").unwrap();

    crate::core::sync_engine::sync_dir_copy_with_overwrite(src_dir.path(), &target, true).unwrap();

    assert_eq!(fs::read(target.join("cli.js")).unwrap(), b"new-source");
    assert_eq!(fs::read(target.join("SKILL.md")).unwrap(), b"new-skill");
    assert_eq!(
        fs::read(target.join("local/connections.json")).unwrap(),
        b"secret-token",
        "私有配置被同步清空了"
    );
    assert!(
        target.join("node_modules/axios/index.js").exists(),
        "依赖被清空了"
    );
    assert!(target.join("logs/run.log").exists(), "运行日志被清空了");
    assert!(target.join(".skillignore").exists(), "屏蔽清单被清空了");
}

/// 不在忽略清单里的多余文件仍然要删掉——同步的「对齐」语义不能丢。
#[test]
fn overwrite_still_prunes_entries_missing_from_source() {
    let src_dir = tempfile::tempdir().unwrap();
    fs::write(src_dir.path().join("keep.txt"), b"keep").unwrap();

    let dst_root = tempfile::tempdir().unwrap();
    let target = dst_root.path().join("managed-skill");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("keep.txt"), b"old").unwrap();
    fs::write(target.join("stale.txt"), b"should-go").unwrap();

    crate::core::sync_engine::sync_dir_copy_with_overwrite(src_dir.path(), &target, true).unwrap();

    assert_eq!(fs::read(target.join("keep.txt")).unwrap(), b"keep");
    assert!(!target.join("stale.txt").exists(), "源里没有的文件应被清理");
}

#[test]
fn overwrite_does_not_leave_staging_directories() {
    let src_dir = tempfile::tempdir().unwrap();
    fs::write(src_dir.path().join("a.txt"), b"a").unwrap();

    let dst_root = tempfile::tempdir().unwrap();
    let target = dst_root.path().join("managed-skill");
    fs::create_dir_all(target.join("local")).unwrap();
    fs::write(target.join("local/x.json"), b"x").unwrap();
    fs::write(target.join(".skillignore"), b"local\n").unwrap();

    crate::core::sync_engine::sync_dir_copy_with_overwrite(src_dir.path(), &target, true).unwrap();

    let leftovers: Vec<_> = fs::read_dir(dst_root.path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|name| name.starts_with(".skills-hub-preserve-"))
        .collect();
    assert!(leftovers.is_empty(), "暂存目录残留: {leftovers:?}");
}

#[test]
fn skillignore_is_carried_over_even_though_copy_skips_it() {
    let src = tempfile::tempdir().unwrap();
    let dst_root = tempfile::tempdir().unwrap();
    // 目标用普通子目录：tempdir 自己那层是点开头的，会被安全校验当隐藏名拒掉。
    let dst = dst_root.path().join("copy");

    fs::write(src.path().join(".skillignore"), b"local/\nlogs/\n").unwrap();
    fs::write(src.path().join("SKILL.md"), b"# demo").unwrap();
    fs::create_dir_all(src.path().join("local")).unwrap();
    fs::write(src.path().join("local/secret.bin"), b"private").unwrap();

    sync_dir_copy_with_overwrite(src.path(), &dst, true).unwrap();

    // .skillignore 自身在 DEFAULT_IGNORES 里，复制阶段会跳过，必须靠补发。
    let rule = fs::read_to_string(dst.join(".skillignore")).unwrap();
    // 规则条目落盘时会归一化（去掉首尾斜杠），与 content_hash 的比对口径一致。
    assert!(
        rule.lines().any(|line| line.trim() == "local"),
        "副本拿不到规则就无法保护私有内容: {rule}"
    );
    assert!(rule.lines().any(|line| line.trim() == "logs"));
    // 私有内容本身仍然不该被复制过去。
    assert!(!dst.join("local").exists());
    assert!(dst.join("SKILL.md").exists());
}

#[test]
fn carry_over_merges_instead_of_overwriting_local_rules() {
    let src = tempfile::tempdir().unwrap();
    let dst_root = tempfile::tempdir().unwrap();
    let dst = dst_root.path().join("copy");
    fs::create_dir_all(&dst).unwrap();

    fs::write(src.path().join(".skillignore"), b"local/\n").unwrap();
    fs::write(dst.join(".skillignore"), b"runtime/\n").unwrap();

    sync_dir_copy_with_overwrite(src.path(), &dst, true).unwrap();

    let rule = fs::read_to_string(dst.join(".skillignore")).unwrap();
    assert!(
        rule.lines().any(|line| line.trim() == "local"),
        "母库新增规则要补进来: {rule}"
    );
    assert!(
        rule.lines().any(|line| line.trim() == "runtime"),
        "副本自己的规则不能被覆盖掉，否则私有内容会跟着进回收站: {rule}"
    );
}

#[test]
fn carry_over_is_idempotent() {
    let src = tempfile::tempdir().unwrap();
    let dst = tempfile::tempdir().unwrap();
    fs::write(src.path().join(".skillignore"), b"local/\n").unwrap();
    fs::write(dst.path().join(".skillignore"), b"local/\n").unwrap();

    assert!(
        !carry_over_skillignore(src.path(), dst.path()),
        "规则已一致时不应重复写盘"
    );
    let rule = fs::read_to_string(dst.path().join(".skillignore")).unwrap();
    assert_eq!(rule.matches("local/").count(), 1, "不能产生重复行: {rule}");
}

#[test]
fn carry_over_ignores_comments_and_blank_lines() {
    let src = tempfile::tempdir().unwrap();
    let dst = tempfile::tempdir().unwrap();
    fs::write(src.path().join(".skillignore"), b"# comment\n\n/tmp/x/\n").unwrap();

    assert!(carry_over_skillignore(src.path(), dst.path()));
    let rule = fs::read_to_string(dst.path().join(".skillignore")).unwrap();
    assert!(!rule.contains("# comment"), "注释不应被搬过去: {rule}");
    assert!(rule.contains("tmp/x"), "首尾斜杠要归一化: {rule}");
}

#[test]
fn carry_over_noop_when_source_has_no_rules() {
    let src = tempfile::tempdir().unwrap();
    let dst = tempfile::tempdir().unwrap();
    assert!(!carry_over_skillignore(src.path(), dst.path()));
    assert!(!dst.path().join(".skillignore").exists());
}
