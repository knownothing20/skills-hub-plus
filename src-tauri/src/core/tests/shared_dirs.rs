use std::path::Path;

use crate::core::shared_dirs::{shared_dir_containing, shared_skill_dirs, SHARED_DIR_READERS};

fn home() -> &'static Path {
    Path::new("C:/Users/example")
}

#[test]
fn recognizes_the_agents_skills_dir_and_its_children() {
    let base = home().join(".agents/skills");
    let dir = shared_dir_containing(home(), &base).expect("base dir must be shared");
    assert_eq!(dir.readers, SHARED_DIR_READERS.to_vec());

    let child = base.join("pinokio");
    let child_dir = shared_dir_containing(home(), &child).expect("child must inherit sharing");
    assert_eq!(child_dir.path, base);
}

#[test]
fn treats_a_lookalike_path_as_private() {
    let other = home().join(".agents/skills-backup/x");
    assert!(shared_dir_containing(home(), &other).is_none());
}

#[test]
fn normalizes_separators_and_case() {
    let mixed = Path::new("C:\\Users\\Example\\.Agents\\Skills\\pinokio");
    assert!(shared_dir_containing(home(), mixed).is_some());
}

#[test]
fn lists_the_agents_dir_only_once() {
    let dirs = shared_skill_dirs(home());
    assert_eq!(dirs.len(), 1);
    assert!(dirs[0].path.ends_with(".agents/skills"));
}

#[test]
fn private_tool_dirs_are_not_shared() {
    let codex = home().join(".codex/skills/pinokio");
    assert!(shared_dir_containing(home(), &codex).is_none());
}
