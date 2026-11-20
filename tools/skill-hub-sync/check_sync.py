#!/usr/bin/env python3
"""Skills Hub 同步自检（只读）。

用途：轻量只读自检，回答「谁和大部队不一致」。写操作请用同目录的 `sync.py`
（它才读基线、才判方向）。本脚本**不写任何文件**——只打印一份报告。

口径与 Rust 侧保持一致：
  * 忽略 DEFAULT_IGNORES 与每个位置的 `.skillignore`（私有目录不算内容）；
  * 母库作为一等位置参与比较（它在 skill_targets 表里没有记录）；
  * 按「文件相对路径 + 内容」算指纹分组，不用 mtime 判方向。

用法：
  python check_sync.py                 # 全库
  python check_sync.py <技能名>        # 只看名字含 <技能名> 的技能
  python check_sync.py --json          # 机器可读
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sqlite3
import sys
from pathlib import Path

APP_IDENTIFIER = "io.github.mcncarl.skillshub"

# 与 content_hash.rs 的 DEFAULT_IGNORES 对齐
DEFAULT_IGNORES = [
    ".git",
    ".DS_Store",
    "Thumbs.db",
    ".gitignore",
    ".skillignore",
    "node_modules",
    "__pycache__",
    "venv",
    ".venv",
    ".pytest_cache",
    ".cache",
    ".tmp",
    ".agentport-tmp",
    ".codex-tmp",
    "tmp",
]

IGNORED_SUFFIXES = (".pyc", ".pyo", ".pyd", ".tmp", ".temp", ".swp", ".swx", ".bak", ".orig")
MAX_FILE_BYTES = 5 * 1024 * 1024


def config_root() -> Path:
    if sys.platform == "win32":
        return Path(os.environ["APPDATA"])
    if sys.platform == "darwin":
        return Path.home() / "Library" / "Application Support"
    return Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))


def default_db() -> Path:
    return config_root() / APP_IDENTIFIER / "skills_hub.db"


def load_ignore_patterns(root: Path) -> list[str]:
    patterns = [p.strip("/") for p in DEFAULT_IGNORES]
    ignore_file = root / ".skillignore"
    if ignore_file.is_file():
        for line in ignore_file.read_text(encoding="utf-8", errors="ignore").splitlines():
            trimmed = line.strip().lstrip("\ufeff").strip()
            if not trimmed or trimmed.startswith("#"):
                continue
            clean = trimmed.replace("\\", "/").strip("/")
            if clean and clean not in patterns:
                patterns.append(clean)
    return patterns


def matches_rule(pattern: str, rel: str, name: str) -> bool:
    if not pattern:
        return False
    if name == pattern or rel == pattern:
        return True
    if rel.startswith(f"{pattern}/") or f"/{pattern}/" in rel or rel.endswith(f"/{pattern}"):
        return True
    if pattern.startswith("*"):
        suffix = pattern[1:]
        return name.endswith(suffix) or rel.endswith(suffix)
    if pattern.endswith("*"):
        return name.startswith(pattern[:-1])
    return False


def is_ignored(rel: str, name: str, patterns: list[str]) -> bool:
    if rel.startswith(".git/") or "__pycache__" in rel or ".pytest_cache" in rel:
        return True
    if name in (".DS_Store", "Thumbs.db", "desktop.ini") or name.startswith("~$"):
        return True
    if name.lower().endswith(IGNORED_SUFFIXES):
        return True
    return any(matches_rule(p, rel, name) for p in patterns)


def dir_snapshot(root: Path) -> dict[str, str]:
    """相对路径 -> 内容哈希。私有内容按 .skillignore 排除。"""
    out: dict[str, str] = {}
    if not root.is_dir():
        return out
    patterns = load_ignore_patterns(root)
    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        rel = path.relative_to(root).as_posix()
        if is_ignored(rel, path.name, patterns):
            continue
        try:
            if path.stat().st_size > MAX_FILE_BYTES:
                continue
            out[rel] = hashlib.sha256(path.read_bytes()).hexdigest()
        except OSError:
            continue
    return out


def fingerprint(snapshot: dict[str, str]) -> str:
    hasher = hashlib.sha256()
    for rel in sorted(snapshot):
        hasher.update(rel.encode())
        hasher.update(snapshot[rel].encode())
    return hasher.hexdigest()


def latest_mtime(root: Path) -> int:
    newest = 0
    if not root.is_dir():
        return 0
    patterns = load_ignore_patterns(root)
    for path in root.rglob("*"):
        if not path.is_file():
            continue
        rel = path.relative_to(root).as_posix()
        if is_ignored(rel, path.name, patterns):
            continue
        try:
            newest = max(newest, int(path.stat().st_mtime))
        except OSError:
            continue
    return newest


def positions_for(conn: sqlite3.Connection, skill_id: str, central: str) -> list[dict]:
    rows = conn.execute(
        "SELECT tool, instance, target_path FROM skill_targets WHERE skill_id = ?",
        (skill_id,),
    ).fetchall()
    out = []
    seen = set()
    for tool, instance, path in rows:
        label = tool if not instance else f"{tool}#{instance}"
        out.append({"label": label, "path": path, "kind": "copy"})
        seen.add(Path(path).resolve() if Path(path).is_dir() else Path(path))
    # 母库是一等位置：表里没有它，必须显式加
    central_path = Path(central)
    if central_path.is_dir() and central_path.resolve() not in seen:
        out.insert(0, {"label": "母库", "path": central, "kind": "central"})
    return out


def report(db: Path, needle: str | None, as_json: bool) -> int:
    if not db.is_file():
        print(f"找不到数据库：{db}", file=sys.stderr)
        return 2
    conn = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    skills = conn.execute("SELECT id, name, central_path FROM skills ORDER BY name").fetchall()
    if needle:
        low = needle.lower()
        skills = [s for s in skills if low in s[1].lower()]
        if not skills:
            print(f"没有名字包含「{needle}」的技能。", file=sys.stderr)
            return 2

    differing = []
    for skill_id, name, central in skills:
        places = positions_for(conn, skill_id, central)
        if not places:
            continue
        snaps = {}
        for place in places:
            snap = dir_snapshot(Path(place["path"]))
            if not snap:
                continue
            snaps[place["label"]] = {
                "fp": fingerprint(snap),
                "files": len(snap),
                "mtime": latest_mtime(Path(place["path"])),
                "path": place["path"],
                "kind": place["kind"],
            }
        groups: dict[str, list[str]] = {}
        for label, info in snaps.items():
            groups.setdefault(info["fp"], []).append(label)
        if len(groups) <= 1:
            continue
        # A 组 = 组内最新修改时间最大的那一组
        ordered = sorted(
            groups.items(),
            key=lambda kv: max(snaps[label]["mtime"] for label in kv[1]),
            reverse=True,
        )
        differing.append(
            {
                "skill": name,
                "groups": [
                    {"letter": chr(ord("A") + i), "labels": labels}
                    for i, (_fp, labels) in enumerate(ordered)
                ],
                "positions": {
                    label: {"files": info["files"], "path": info["path"]}
                    for label, info in snaps.items()
                },
            }
        )

    if as_json:
        print(json.dumps({"total_skills": len(skills), "differing": differing},
                         ensure_ascii=False, indent=2))
    else:
        if not differing:
            print(f"全部 {len(skills)} 个技能在各位置内容一致，无需同步。")
        else:
            print(f"以下技能存在版本差异（A 为最新），共 {len(differing)} 个：\n")
            for item in differing:
                print(f"● {item['skill']}")
                for group in item["groups"]:
                    print(f"    组 {group['letter']}: {'、'.join(group['labels'])}")
                print()
            print(f"其余 {len(skills) - len(differing)} 个技能一致。")
            print("\n这只是报告。收回改动请用 sync.py pull（或按 SKILL.md 手工流程）。")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Skills Hub 同步自检（只读）")
    parser.add_argument("needle", nargs="?", help="只看名字含该片段的技能")
    parser.add_argument("--db", default=str(default_db()), help="数据库路径")
    parser.add_argument("--json", action="store_true", help="输出 JSON")
    args = parser.parse_args()
    return report(Path(args.db), args.needle, args.json)


if __name__ == "__main__":
    raise SystemExit(main())