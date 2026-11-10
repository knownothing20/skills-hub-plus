//! 开放标准共享技能目录（`~/.agents/skills`）及其读取者。
//!
//! 这个目录不是某一个软件的私有目录：它遵循 Agent Skills 开放标准，
//! 会被多个工具**追加扫描**（先读自己的目录，再读这里）。豆包还会用它
//! 遮蔽（shadow）自己 Profile 里的同名技能。
//!
//! 因此「技能放在这里」和「给某个工具建了链接」是两种不同的分发形态：
//! 前者一份内容对所有读取者生效，且不受 Skills Hub 的同步控制。

use std::path::{Path, PathBuf};

/// 相对 home 的共享目录路径。
pub const AGENTS_SKILLS_RELATIVE: &str = ".agents/skills";

/// 会读取 `~/.agents/skills` 的工具 key（与 tool_adapters 的 id 一致）。
///
/// 只列**有实证**的读取者：
///
/// - `agents`：这就是它自己的技能根目录
/// - `doubao`：程序里能查到 `filter additional local skills shadowed by .agents/skills`
///
/// Claude Code 曾经被列在这里（按"开放标准"推断），但实测不成立：
/// `~/.claude/skills` 里没有的那个技能，它的 `/` 命令也搜不到；而同一个技能
/// 存在于 `.claude/skills` 时才可用。codex / cursor / antigravity 同样没有验证过。
///
/// 宁可少报：界面上画一条"共享可达"的虚线，用户就会以为那个工具能用，
/// 实际搜不到比不提示更糟。
pub const SHARED_DIR_READERS: &[&str] = &["agents", "doubao"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedSkillDir {
    pub path: PathBuf,
    pub label: &'static str,
    pub readers: Vec<&'static str>,
}

/// 当前认定的共享目录清单。新增公共约定时在这里追加。
pub fn shared_skill_dirs(home: &Path) -> Vec<SharedSkillDir> {
    vec![SharedSkillDir {
        path: home.join(AGENTS_SKILLS_RELATIVE),
        label: "Agent Skills 开放标准目录",
        readers: SHARED_DIR_READERS.to_vec(),
    }]
}

fn normalize(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_lowercase()
}

/// 判断某个路径是否落在共享目录内（相等或其后代）。
pub fn shared_dir_containing(home: &Path, path: &Path) -> Option<SharedSkillDir> {
    let target = normalize(path);
    shared_skill_dirs(home).into_iter().find(|dir| {
        let base = normalize(&dir.path);
        target == base || target.starts_with(&format!("{base}/"))
    })
}

#[cfg(test)]
#[path = "tests/shared_dirs.rs"]
mod tests;
