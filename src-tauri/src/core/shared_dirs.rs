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
/// 只收录已确认支持该目录的工具，宁可少列也不要误报「可用」。
/// - `agents`：适配器里这个目录就是它的全局技能目录
/// - `claude_code` / `codex` / `cursor` / `antigravity`：Agent Skills 开放标准
/// - `doubao`：程序内含 `filter additional local skills shadowed by .agents/skills`
pub const SHARED_DIR_READERS: &[&str] = &[
    "agents",
    "claude_code",
    "codex",
    "cursor",
    "antigravity",
    "doubao",
];

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
