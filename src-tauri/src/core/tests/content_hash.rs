use std::fs;

use crate::core::content_hash::hash_dir;

#[test]
fn hash_changes_with_content_and_ignores_git_dir() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();

    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(root.join("a.txt"), b"hello").unwrap();
    fs::write(root.join("sub/b.txt"), b"world").unwrap();

    let h1 = hash_dir(root).unwrap();

    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(root.join(".git/ignored"), b"ignored").unwrap();
    let h2 = hash_dir(root).unwrap();
    assert_eq!(h1, h2, "应忽略 .git 内容");

    fs::write(root.join("a.txt"), b"hello2").unwrap();
    let h3 = hash_dir(root).unwrap();
    assert_ne!(h2, h3);
}

#[test]
fn hash_respects_custom_skillignore() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();

    fs::create_dir_all(root.join("local")).unwrap();
    fs::write(root.join("SKILL.md"), b"# skill").unwrap();
    fs::write(root.join(".skillignore"), b"local/\n*.key\n").unwrap();
    fs::write(root.join("local/connections.json"), b"key1").unwrap();

    let h1 = hash_dir(root).unwrap();

    // 修改 local/connections.json 或新增 key 文件，哈希应该依然一致（被忽略）
    fs::write(root.join("local/connections.json"), b"key2_different").unwrap();
    fs::write(root.join("secret.key"), b"private_secret").unwrap();
    let h2 = hash_dir(root).unwrap();
    assert_eq!(h1, h2, "应忽略 .skillignore 中指定的 local/ 与 *.key");

    // 修改被追踪的真实代码文件，哈希应该改变
    fs::write(root.join("SKILL.md"), b"# skill updated").unwrap();
    let h3 = hash_dir(root).unwrap();
    assert_ne!(h2, h3, "修改未被忽略的真实代码文件，哈希必须改变");
}

/// 内容相同、只是**创建顺序**不同，指纹必须相同。
///
/// 这条锁的是 2.0.4 修掉的真实缺陷：`hash_dir` 早先按 `WalkDir` 的原始枚举
/// 顺序喂 hasher，而枚举顺序由文件系统决定。NTFS 上碰巧按名称返回，所以本机
/// 看不出问题；换成 ext4/APFS 或网络盘，「两份内容逐字节相同」就会算出两个
/// 指纹，界面于是同时显示「版本全是 A」和「副本落后」——自相矛盾，且自动刷新
/// 判定内容已一致而拒绝动手，红标永远消不掉。
#[test]
fn hash_is_independent_of_creation_order() {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = dir.path().join("a");
    let b = dir.path().join("b");
    fs::create_dir_all(a.join("sub")).unwrap();
    fs::create_dir_all(b.join("sub")).unwrap();

    // a：正序创建；b：逆序创建。内容完全一样。
    for (root, name) in [(&a, "alpha.md"), (&a, "zeta.md")] {
        fs::write(root.join(name), format!("body of {name}")).unwrap();
    }
    for (root, name) in [(&b, "zeta.md"), (&b, "alpha.md")] {
        fs::write(root.join(name), format!("body of {name}")).unwrap();
    }
    fs::write(a.join("sub/inner.txt"), b"inner").unwrap();
    fs::write(b.join("sub/inner.txt"), b"inner").unwrap();

    assert_eq!(
        hash_dir(&a).unwrap(),
        hash_dir(&b).unwrap(),
        "创建顺序不同不应影响指纹——顺序必须由路径决定，不能由文件系统决定"
    );
}

/// 与 `peer_sync::snapshot_with_time`（算版本字母 A/B 用的那套）口径一致：
/// 同一份内容，两套指纹算法必须同时判「一致」。这是「A 全是 A，却报落后」
/// 那个自相矛盾状态的回归闸。
#[test]
fn hash_matches_peer_snapshot_semantics_on_identical_dirs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = dir.path().join("a");
    let b = dir.path().join("b");
    fs::create_dir_all(&a).unwrap();
    fs::create_dir_all(&b).unwrap();
    for root in [&a, &b] {
        fs::write(root.join("SKILL.md"), b"# skill").unwrap();
        fs::write(root.join(".skillignore"), b"local/\n").unwrap();
        fs::create_dir_all(root.join("local")).unwrap();
        // 私有内容各写各的，必须不影响指纹
        fs::write(
            root.join("local/secret.json"),
            root.to_string_lossy().as_bytes(),
        )
        .unwrap();
    }
    assert_eq!(hash_dir(&a).unwrap(), hash_dir(&b).unwrap());
}
