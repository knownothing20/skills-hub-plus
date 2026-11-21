#!/usr/bin/env python3
"""Skills Hub 同步工具（skill 自带，不依赖 skills-hub-cli.exe）。

设计原则与 Rust 侧 `core/peer_sync.rs` **逐条对齐**，因为它读的是同一张库：

  * 方向由**基线**（`skill_baselines` 表）决定，不是按 mtime、也不是按「谁新」；
  * 母库（`skills.central_path`）作为**一等位置**参与，它不在 `skill_targets` 表里；
  * `.skillignore` + `DEFAULT_IGNORES` 决定什么算私有内容，私有内容不参与比对、
    也不会被覆盖；
  * 偏离基线的位置**只有一份**才动手；多份各改各的 → 判冲突、一个字节都不写；
  * 候选版本命中 `skill_baseline_history` → 判回滚，不扩散。

子命令：
  status           只读，列出每个技能各位置相对基线的状态
  diff <技能>      只读，列出具体哪些文件不一样
  pull             收回：唯一偏离基线的那个位置 → 推给其它所有位置（含母库）
  push             推出：母库 → 全部副本
  adopt <技能> --from <位置>   指定以某位置为准

用法：
  python sync.py status
  python sync.py status <技能名>
  python sync.py pull --dry-run
  python sync.py pull <技能名>
  python sync.py push
  python sync.py adopt <技能名> --from codex
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import sqlite3
import sys
import time
from pathlib import Path

APP_IDENTIFIER = "io.github.mcncarl.skillshub"
CENTRAL_LABEL = "母库"

# 与 content_hash.rs 的 DEFAULT_IGNORES 对齐
DEFAULT_IGNORES = [
    ".git", ".DS_Store", "Thumbs.db", ".gitignore", ".skillignore",
    "node_modules", "__pycache__", "venv", ".venv", ".pytest_cache",
    ".cache", ".tmp", ".agentport-tmp", ".codex-tmp", "tmp",
]
IGNORED_SUFFIXES = (".pyc", ".pyo", ".pyd", ".tmp", ".temp", ".swp", ".swx", ".bak", ".orig")
MAX_FILE_BYTES = 5 * 1024 * 1024
MAX_FILES_PER_SKILL = 20000


# ---------------------------------------------------------------- 路径与数据库

def config_root() -> Path:
    if sys.platform == "win32":
        return Path(os.environ["APPDATA"])
    if sys.platform == "darwin":
        return Path.home() / "Library" / "Application Support"
    return Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))


def default_db() -> Path:
    return config_root() / APP_IDENTIFIER / "skills_hub.db"


def connect(db: Path, writable: bool = False) -> sqlite3.Connection:
    if not db.is_file():
        raise SystemExit(f"找不到数据库：{db}")
    if writable:
        return sqlite3.connect(db)
    return sqlite3.connect(f"file:{db}?mode=ro", uri=True)


# ---------------------------------------------------------------- 忽略规则（与 Rust 同口径）

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


# ---------------------------------------------------------------- 快照与指纹

def snapshot(root: Path) -> dict[str, tuple[str, int]]:
    """相对路径 -> (sha256, 字节数)。私有内容按 .skillignore 排除。"""
    out: dict[str, tuple[str, int]] = {}
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
            size = path.stat().st_size
            if size > MAX_FILE_BYTES:
                continue
            out[rel] = (hashlib.sha256(path.read_bytes()).hexdigest(), size)
        except OSError:
            continue
    return out


def fingerprint(snap: dict[str, tuple[str, int]]) -> str:
    """与 Rust snapshot_fingerprint 一致：按相对路径排序后串起 rel+hash。"""
    hasher = hashlib.sha256()
    for rel in sorted(snap):
        hasher.update(rel.encode())
        hasher.update(snap[rel][0].encode())
    return hasher.hexdigest()


def diff_against(baseline: dict, snap: dict) -> list[str]:
    keys = set()
    for rel, value in snap.items():
        if baseline.get(rel) != value:
            keys.add(rel)
    for rel in baseline:
        if rel not in snap:
            keys.add(rel)
    return sorted(keys)


# ---------------------------------------------------------------- 位置枚举

def positions_of(conn: sqlite3.Connection, skill_id: str, central: str) -> list[dict]:
    """母库排第一（与 Rust 一致：建基线时默认以母库为准）。"""
    out: list[dict] = []
    central_path = Path(central) if central else None
    seen: set[str] = set()
    if central_path and central_path.is_dir():
        out.append({"label": CENTRAL_LABEL, "path": central_path, "kind": "central"})
        seen.add(str(central_path.resolve()).lower())
    rows = conn.execute(
        "SELECT tool, instance, target_path FROM skill_targets WHERE skill_id = ? ORDER BY tool, instance",
        (skill_id,),
    ).fetchall()
    for tool, instance, path in rows:
        if not path:
            continue
        p = Path(path)
        if str(p.resolve()).lower() in seen:
            continue
        viewer = tool if not instance else f"{tool}#{instance}"
        out.append({"label": viewer, "path": p, "kind": "copy"})
    return out


def load_baseline(conn: sqlite3.Connection, skill_id: str) -> dict[str, tuple[str, int]]:
    rows = conn.execute(
        "SELECT rel_path, hash, size FROM skill_baselines WHERE skill_id = ?", (skill_id,)
    ).fetchall()
    return {rel: (h, size) for rel, h, size in rows}


def load_history(conn: sqlite3.Connection, skill_id: str, keep: int = 5) -> list[str]:
    rows = conn.execute(
        "SELECT fingerprint FROM skill_baseline_history WHERE skill_id = ? "
        "ORDER BY created_at DESC LIMIT ?", (skill_id, keep)
    ).fetchall()
    return [r[0] for r in rows]


# ---------------------------------------------------------------- 状态判定（对齐 scan_peers）

IN_SYNC = "一致"
EDITED = "偏离"
MISSING = "缺失"
# 还没有基线时，谁都不能说「一致」——「一致」是相对基线而言的。
# 此时只描述事实：这个位置有多少文件，以及它和别处是否相同。
NOBASE = "无基线"


def evaluate(conn: sqlite3.Connection, skill_row) -> dict:
    skill_id, name, central, source_ref = skill_row
    places = positions_of(conn, skill_id, central)
    baseline = load_baseline(conn, skill_id)
    baseline_source = "skill_baselines"
    if not baseline and source_ref and Path(source_ref).is_dir():
        baseline = snapshot(Path(source_ref))
        baseline_source = "source_ref（尚无基线）"

    standings = []
    for place in places:
        snap = snapshot(place["path"])
        if not snap:
            state = MISSING
        elif not baseline:
            # 没有基线时不能一律判「一致」——「一致」是相对基线说的，
            # 此时既不能说一致也不能说偏离，只标「无基线」。
            state = NOBASE
        elif snap == baseline:
            state = IN_SYNC
        else:
            state = EDITED
        standings.append({
            "label": place["label"], "path": str(place["path"]), "kind": place["kind"],
            "state": state, "files": len(snap),
            "changed": diff_against(baseline, snap) if state == EDITED else [],
            "fp": fingerprint(snap), "snap": snap,
        })
    return {
        "id": skill_id, "name": name, "central": central,
        "baseline": baseline, "baseline_source": baseline_source,
        "baseline_empty": not baseline, "standings": standings,
    }


def classify(info: dict) -> dict:
    """把「谁偏离」归纳成可执行结论。

    除了 Rust 侧的 single/conflict，这里多识别两种真实存在的情况：

    **基线过期** —— 所有位置内容彼此相同、但与基线不同。这不是「需要同步」，
    而是「基线旧了」（典型成因：刚把同一份内容分发到所有位置，或升级后基线
    没跟着更新）。此时只需刷新基线，不应复制任何文件。

    **位置缺失** —— 内容全一致，但有登记过的位置目录不存在了。界面管这叫
    「断链」，也是用户在别处迁移／删掉副本后最常见的样子。它同样不是「内容
    需要同步」：**不能**让 `in_sync` 把它静默吞掉，否则这个技能永远补不回来，
    断链红字会一直挂着。（2026-11-21：tietu-track-positioning 就是这么卡住的
    —— agents 目录被删，脚本报「一致」，界面报「断链 1」。）
    """
    valid = [s for s in info["standings"] if s["state"] != MISSING]
    missing = [s for s in info["standings"] if s["state"] == MISSING]

    # 所有有效位置内容一致 → 无论基线是否存在，都不需要复制文件
    if valid and len({s["fp"] for s in valid}) == 1:
        if missing:
            # 有位置缺了：以「现有内容」为准把它补回去，同时刷新基线。
            return {"kind": "missing", "edited": [], "missing": missing,
                    "unanimous": valid[0]}
        if info["baseline_empty"]:
            return {"kind": "no_baseline", "edited": [], "unanimous": valid[0]}
        if valid[0]["fp"] != fingerprint(info["baseline"]):
            return {"kind": "baseline_stale", "edited": [], "unanimous": valid[0]}
        return {"kind": "in_sync", "edited": []}

    edited = [s for s in info["standings"] if s["state"] == EDITED]
    if not info["baseline"]:
        # 没有基线且各位置确实互不相同 → 必须让人指定
        return {"kind": "no_baseline", "edited": edited}
    if not edited:
        return {"kind": "in_sync", "edited": []}
    # 基线对不上任何位置：基线本身已失效（换了行尾、被外部工具重写、
    # 或手工改过库）。此时它没法用来判方向，必须说清楚，别只报「冲突」
    # 让用户以为是自己改乱了。
    baseline_stale = not any(s["state"] == IN_SYNC for s in info["standings"])
    first = edited[0]["fp"]
    if any(s["fp"] != first for s in edited):
        changed = sorted({f for s in edited for f in s["changed"]})
        return {"kind": "conflict", "edited": edited, "changed_files": changed,
                "baseline_detached": baseline_stale}
    return {"kind": "single", "edited": edited, "from": edited[0],
            "baseline_detached": baseline_stale}


# ---------------------------------------------------------------- 写操作

def backup(entries: list[tuple[str, Path]], note: str) -> Path:
    """备份一组位置，返回备份根目录。

    `entries` 是 (标签, 路径)，**不是**裸路径列表。因为同一个技能在多个软件里
    目录名都叫 `demo`/`<技能名>`，只按目录名存会撞成 `demo`、`demo-1`、`demo-2`，
    一旦出事根本分不清哪个备份对应哪个软件。这里用标签命名，标签里的 `#`
    （多账号实例）换成 `-`，非法字符统一替换。
    """
    stamp = time.strftime("%Y%m%d-%H%M%S")
    root = Path.home() / ".workbuddy-ai" / "skill-sync-backup" / f"{stamp}-{note}"
    root.mkdir(parents=True, exist_ok=True)
    manifest = []
    for label, raw in entries:
        src = Path(raw) if not isinstance(raw, Path) else raw
        if not src.is_dir():
            continue
        safe = "".join(c if (c.isalnum() or c in "-_.") else "-" for c in label)
        dest = root / safe
        n = 1
        while dest.exists():
            dest = root / f"{safe}__{n}"
            n += 1
        shutil.copytree(src, dest, symlinks=True)
        manifest.append(f"{safe}\t{src}")
    if manifest:
        (root / "MANIFEST.txt").write_text(
            "备份目录\t原位置\n" + "\n".join(manifest) + "\n", encoding="utf-8"
        )
    return root


def preserve_entries(target: Path) -> tuple[Path, list[tuple[Path, Path]]]:
    """把目标侧私有内容搬到暂存区，返回 (暂存目录, [(暂存位置, 原位)])。"""
    rules = load_ignore_patterns(target)
    staging = target.parent / f".{target.name}.sync-staging-{os.getpid()}"
    staging.mkdir(parents=True, exist_ok=True)
    moved: list[tuple[Path, Path]] = []
    for entry in sorted(target.iterdir()):
        if not is_ignored(entry.name, entry.name, rules):
            continue
        dest = staging / entry.name
        shutil.move(str(entry), str(dest))
        moved.append((dest, target / entry.name))
    return staging, moved


def restore_entries(moved: list[tuple[Path, Path]]) -> None:
    for src, dest in moved:
        if not src.exists():
            continue
        if dest.exists():
            if dest.is_dir():
                shutil.rmtree(dest)
            else:
                dest.unlink()
        shutil.move(str(src), str(dest))


def merge_ignore_rules(source: Path, target: Path) -> bool:
    rules_file = source / ".skillignore"
    if not rules_file.is_file():
        return False
    wanted = []
    for line in rules_file.read_text(encoding="utf-8", errors="ignore").splitlines():
        t = line.strip().lstrip("\ufeff").strip()
        if not t or t.startswith("#"):
            continue
        clean = t.replace("\\", "/").strip("/")
        if clean and clean not in wanted:
            wanted.append(clean)
    if not wanted:
        return False
    target_file = target / ".skillignore"
    if target_file.is_file():
        have = [l.strip().lstrip("\ufeff").strip().replace("\\", "/").strip("/")
                for l in target_file.read_text(encoding="utf-8", errors="ignore").splitlines()]
        have = [h for h in have if h and not h.startswith("#")]
        missing = [w for w in wanted if w not in have]
        if not missing:
            return False
        body = target_file.read_text(encoding="utf-8", errors="ignore").rstrip("\n")
        target_file.write_text(body + "\n" + "\n".join(missing) + "\n", encoding="utf-8")
        return True
    target_file.write_text("\n".join(wanted) + "\n", encoding="utf-8")
    return True


def replace_dir(source, target) -> None:
    """整目录替换，但保护目标侧私有内容（与 sync_dir_copy_with_overwrite 同语义）。"""
    source = Path(source) if not isinstance(source, Path) else source
    target = Path(target) if not isinstance(target, Path) else target
    if target.exists():
        staging, moved = preserve_entries(target)
        try:
            shutil.rmtree(target)
        except OSError as err:
            restore_entries(moved)
            shutil.rmtree(staging, ignore_errors=True)
            raise SystemExit(f"无法替换 {target}：{err}")
        shutil.copytree(source, target, symlinks=True)
        restore_entries(moved)
        shutil.rmtree(staging, ignore_errors=True)
    else:
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copytree(source, target, symlinks=True)
    merge_ignore_rules(source, target)


def _write_baseline(conn: sqlite3.Connection, info: dict,
                    snap: dict[str, tuple[str, int]]) -> None:
    """把 snap 写成该技能的新基线，旧指纹进历史（供回滚识别）。

    `skill_baseline_history` 上有 UNIQUE(skill_id, fingerprint) 约束，
    所以必须 `INSERT OR IGNORE`——同一个指纹被重复记录是正常情况
    （例如「刷新基线」时旧指纹可能已经在历史里了）。Rust 侧同样是
    `INSERT OR IGNORE` 并把历史裁到最近 N 条，这里逐条对齐。
    """
    old_fp = fingerprint(info["baseline"]) if info["baseline"] else None
    now = int(time.time())
    conn.execute("DELETE FROM skill_baselines WHERE skill_id = ?", (info["id"],))
    conn.executemany(
        "INSERT INTO skill_baselines (skill_id, rel_path, hash, size) VALUES (?,?,?,?)",
        [(info["id"], rel, h, size) for rel, (h, size) in snap.items()],
    )
    order = [(info["id"], old_fp, now), (info["id"], fingerprint(snap), now)]
    conn.executemany(
        "INSERT OR IGNORE INTO skill_baseline_history "
        "(skill_id, fingerprint, created_at) VALUES (?,?,?)",
        [row for row in order if row[1]],
    )
    # 与 Rust 一致：只保留最近 5 条，否则历史表会无限增长。
    conn.execute(
        "DELETE FROM skill_baseline_history WHERE skill_id = ? AND fingerprint NOT IN ("
        "  SELECT fingerprint FROM skill_baseline_history WHERE skill_id = ? "
        "  ORDER BY created_at DESC LIMIT 5)",
        (info["id"], info["id"]),
    )
    conn.commit()


def propagate(conn: sqlite3.Connection, info: dict, src: dict, others: list[dict],
              dry_run: bool) -> list[str]:
    """把 src 的内容铺到 others，并更新基线。返回成功写入的标签。"""
    done: list[str] = []
    if dry_run:
        return [s["label"] for s in others]
    for target in others:
        try:
            replace_dir(src["path"], target["path"])
            done.append(target["label"])
        except (OSError, SystemExit) as err:
            print(f"  ✗ {target['label']}：{err}", file=sys.stderr)
    _write_baseline(conn, info, src["snap"])
    return done


# ---------------------------------------------------------------- 子命令

def cmd_status(conn, needle: str | None, as_json: bool, db: Path) -> int:
    rows = conn.execute(
        "SELECT id, name, central_path, source_ref FROM skills ORDER BY name"
    ).fetchall()
    if needle:
        low = needle.lower()
        rows = [r for r in rows if low in r[1].lower()]
        if not rows:
            print(f"没有名字包含「{needle}」的技能。", file=sys.stderr)
            return 2

    report = []
    for row in rows:
        info = evaluate(conn, row)
        cls = classify(info)
        if cls["kind"] == "in_sync":
            continue
        report.append({"name": info["name"], "kind": cls["kind"],
                       "places": [{k: s[k] for k in ("label", "state", "files", "path", "changed")}
                                  for s in info["standings"]],
                       "from": cls.get("from", {}).get("label") if cls["kind"] == "single" else None,
                       "unanimous": cls.get("unanimous", {}).get("files"),
                       "baseline_detached": cls.get("baseline_detached", False),
                       "changed_files": cls.get("changed_files", [])})

    if as_json:
        print(json.dumps({"total": len(rows), "differing": report}, ensure_ascii=False, indent=2))
        return 0
    if not report:
        print(f"全部 {len(rows)} 个技能都与基线一致，无需同步。")
        return 0

    kind_text = {"conflict": "⚠️  冲突（多份各改各的，我不会动手）",
                 "single": "可同步（只有一份偏离基线）",
                 "missing": "⚠️  有位置缺失（界面叫「断链」；内容一致，缺的目录补回即可）",
                 "baseline_stale": "ℹ️  基线过期（各位置其实一致，只需刷新基线）",
                 "no_baseline": "ℹ️  尚无基线（需要指定以谁为准，或直接刷新）"}
    print(f"以下 {len(report)} 个技能需要处理：\n")
    for item in report:
        print(f"● {item['name']} —— {kind_text[item['kind']]}")
        for p in item["places"]:
            mark = {"一致": "  ", "偏离": "→ ", "缺失": "x ", "无基线": "? "}.get(p["state"], "  ")
            extra = f"  改了 {len(p['changed'])} 个文件" if p["changed"] else ""
            print(f"    {mark}{p['label']:<14} {p['state']}  {p['files']} 文件{extra}")
        if item["from"]:
            print(f"    结论：把「{item['from']}」推给其它位置（含母库）")
        if item["kind"] == "conflict":
            print(f"    涉及文件：{', '.join(item['changed_files'][:6])}"
                  + (" …" if len(item["changed_files"]) > 6 else ""))
        if item.get("baseline_detached"):
            print("    ⚠️  基线对不上任何位置（基线已失效：可能行尾被改、"
                  "被外部工具重写过）。请用 adopt 指定以谁为准")
        if item["kind"] in ("baseline_stale", "no_baseline") and item.get("unanimous"):
            print(f"    结论：全部位置内容相同（{item['unanimous']} 个文件），"
                  f"用 python sync.py pull 刷新基线即可，不会复制任何文件")
        if item["kind"] == "missing":
            gone = [p["label"] for p in item["places"] if p["state"] == "缺失"]
            print(f"    结论：现有位置内容一致，但 {'、'.join(gone)} 的目录不存在了。"
                  f"用 python sync.py pull 按现有内容补回（其余位置不动）")
        print()
    print(f"其余 {len(rows) - len(report)} 个技能一致。")
    print("这只是报告。收回改动：python sync.py pull   ；推出母库：python sync.py push")
    return 0


def cmd_diff(conn, needle: str, db: Path) -> int:
    rows = conn.execute(
        "SELECT id, name, central_path, source_ref FROM skills WHERE name LIKE ?", (f"%{needle}%",)
    ).fetchall()
    if not rows:
        print(f"没有名字包含「{needle}」的技能。", file=sys.stderr)
        return 2
    for row in rows:
        info = evaluate(conn, row)
        print(f"=== {info['name']} ===")
        print(f"基线来源：{info['baseline_source']}（{len(info['baseline'])} 个文件）\n")
        for s in info["standings"]:
            print(f"-- {s['label']} [{s['state']}] {s['path']}")
            for rel in s["changed"][:40]:
                in_base = rel in info["baseline"]
                in_here = rel in s["snap"]
                if not in_base:
                    print(f"     + {rel}（此位置新增）")
                elif not in_here:
                    print(f"     - {rel}（此位置没有）")
                else:
                    print(f"     ~ {rel}（内容不同）")
            if len(s["changed"]) > 40:
                print(f"     … 另有 {len(s['changed']) - 40} 个文件")
            print()
    return 0


def _load_targets(conn, needle):
    rows = conn.execute(
        "SELECT id, name, central_path, source_ref FROM skills ORDER BY name"
    ).fetchall()
    if needle:
        low = needle.lower()
        rows = [r for r in rows if low in r[1].lower()]
    return rows


def cmd_pull(conn, needle, dry_run: bool, db: Path, backup_dir: Path | None) -> int:
    """收回：唯一偏离基线的位置 → 其它所有位置。"""
    rows = _load_targets(conn, needle)
    if not rows:
        print(f"没有名字包含「{needle}」的技能。" if needle else "库里没有技能。", file=sys.stderr)
        return 2
    touched, skipped, refreshed = [], [], []
    for row in rows:
        info = evaluate(conn, row)
        cls = classify(info)
        if cls["kind"] == "in_sync":
            continue
        if cls["kind"] == "missing":
            # 内容全一致，但有位置目录没了（界面叫「断链」）。
            # 以现有内容为准把它补回来——缺的目录重建，其余位置一个字节都不动。
            src = cls["unanimous"]
            gone = cls["missing"]
            if dry_run:
                print(f"[干跑] {info['name']}：补回缺失位置 "
                      f"{'、'.join(s['label'] for s in gone)}"
                      f"（以 {src['label']} 为准，{src['files']} 文件）")
                refreshed.append(info["name"])
                continue
            # standings 里的 path 是字符串（evaluate 为了 JSON 输出转过了），
            # 备份只需要还存在的目录，缺失的那些本来就没东西可备份。
            backup_root = backup([(s["label"], Path(s["path"])) for s in gone
                                  if Path(s["path"]).is_dir()], "restore")
            done = propagate(conn, info, src, gone, False)
            print(f"⇡ {info['name']}：{src['label']} → 补回缺失的 "
                  f"{'、'.join(done)}（备份：{backup_root.name}）")
            refreshed.append(info["name"])
            continue
        if cls["kind"] in ("baseline_stale", "no_baseline") and cls.get("unanimous"):
            # 各位置内容已经一致，只有基线是旧的／缺的：只刷新基线，不碰文件。
            if dry_run:
                print(f"[干跑] {info['name']}：各位置已一致，只刷新基线（{cls['unanimous']['files']} 文件）")
                refreshed.append(info["name"])
                continue
            src = cls["unanimous"]
            _write_baseline(conn, info, src["snap"])
            print(f"⟳ {info['name']}：刷新基线（内容早已一致，未复制任何文件）")
            refreshed.append(info["name"])
            continue
        if cls["kind"] == "no_baseline":
            skipped.append((info["name"], "各位置内容互不相同且无基线，需先 adopt 指定以谁为准"))
            continue
        if cls["kind"] == "conflict":
            skipped.append((info["name"], f"冲突：{', '.join(s['label'] for s in cls['edited'])} 各自不同"))
            continue
        src = cls["from"]
        history = load_history(conn, info["id"])
        if fingerprint(src["snap"]) in history:
            skipped.append((info["name"], f"回滚：{src['label']} 回到了历史版本，不扩散"))
            continue
        others = [s for s in info["standings"] if s["label"] != src["label"] and s["state"] != MISSING]
        if not others:
            skipped.append((info["name"], f"只有 {src['label']} 一个有效位置"))
            continue
        if dry_run:
            print(f"[干跑] {info['name']}：{src['label']} → "
                  f"{'、'.join(s['label'] for s in others)}")
            touched.append(info["name"])
            continue
        if backup_dir is not None:
            backup([(src["label"], src["path"])] + [(s["label"], s["path"]) for s in others], "pull")
        done = propagate(conn, info, src, others, False)
        print(f"✓ {info['name']}：{src['label']} → {'、'.join(done)}")
        touched.append(info["name"])

    print()
    if touched:
        print(f"已同步 {len(touched)} 个技能" + ("（干跑，未写任何文件）" if dry_run else f"：{'、'.join(touched)}"))
    if refreshed:
        print(f"已刷新基线 {len(refreshed)} 个技能"
              + ("（干跑）" if dry_run else f"：{'、'.join(refreshed)}"))
    if not touched and not refreshed:
        print("没有需要同步的技能。")
    if skipped:
        print("\n跳过：")
        for name, why in skipped:
            print(f"  · {name}：{why}")
    return 0


def cmd_push(conn, needle, dry_run: bool, db: Path) -> int:
    """推出：母库 → 全部副本。"""
    rows = _load_targets(conn, needle)
    if not rows:
        print("没有匹配的技能。", file=sys.stderr)
        return 2
    touched = []
    for row in rows:
        skill_id, name, central, _src = row
        central_path = Path(central) if central else None
        if not central_path or not central_path.is_dir():
            print(f"  · {name}：母库目录不存在，跳过", file=sys.stderr)
            continue
        central_snap = snapshot(central_path)
        if not central_snap:
            print(f"  · {name}：母库是空的，跳过", file=sys.stderr)
            continue
        central_fp = fingerprint(central_snap)
        targets = positions_of(conn, skill_id, central)
        targets = [t for t in targets if t["kind"] == "copy"]
        todo = [t for t in targets
                if t["path"].is_dir() and fingerprint(snapshot(t["path"])) != central_fp]
        if not todo:
            continue
        if dry_run:
            print(f"[干跑] {name}：母库 → {'、'.join(t['label'] for t in todo)}")
            touched.append(name)
            continue
        backup([(CENTRAL_LABEL, central_path)] + [(t["label"], t["path"]) for t in todo], "push")
        for t in todo:
            try:
                replace_dir(central_path, t["path"])
                print(f"  ✓ {name} → {t['label']}")
            except (OSError, SystemExit) as err:
                print(f"  ✗ {name} → {t['label']}：{err}", file=sys.stderr)
        # 走 _write_baseline 而不是就地写：它会把旧基线指纹记进历史表，
        # 这样母库改回历史版本时能识别成「回滚」，不会又铺一遍。
        _write_baseline(conn, {"id": skill_id, "baseline": load_baseline(conn, skill_id)},
                        central_snap)
        touched.append(name)
    print()
    if touched:
        suffix = "（干跑，未写文件）" if dry_run else f"：{'、'.join(touched)}"
        print(f"已推出 {len(touched)} 个技能{suffix}")
    else:
        print("母库与所有副本已一致，无需推出。")
    return 0


def cmd_adopt(conn, needle, tool: str, instance: str, dry_run: bool, db: Path) -> int:
    """指定以某个位置为准，推给其它所有位置（含母库）。"""
    rows = _load_targets(conn, needle)
    if not rows:
        print(f"没有名字包含「{needle}」的技能。", file=sys.stderr)
        return 2
    for row in rows:
        info = evaluate(conn, row)
        want = tool
        if instance:
            want = f"{tool}#{instance}"
        if want.lower() == "central" or want == CENTRAL_LABEL:
            match = [s for s in info["standings"] if s["kind"] == "central"]
        else:
            match = [s for s in info["standings"]
                     if s["label"] == want or s["label"].lower() == want.lower()]
        if not match:
            print(f"{info['name']}：没有位置「{want}」。现有："
                  f"{'、'.join(s['label'] for s in info['standings'])}", file=sys.stderr)
            continue
        src = match[0]
        others = [s for s in info["standings"] if s["label"] != src["label"]]
        if dry_run:
            print(f"[干跑] {info['name']}：{src['label']} → {'、'.join(s['label'] for s in others)}")
            continue
        if src["state"] == MISSING:
            print(f"{info['name']}：{src['label']} 没有内容，不能作为源。", file=sys.stderr)
            continue
        backup([(src["label"], src["path"])] + [(s["label"], s["path"]) for s in others], "adopt")
        done = propagate(conn, info, src, others, False)
        print(f"✓ {info['name']}：{src['label']} → {'、'.join(done)}")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description="Skills Hub 同步（skill 自带，无需 CLI）")
    ap.add_argument("--db", default=str(default_db()), help="数据库路径")
    sub = ap.add_subparsers(dest="cmd", required=True)

    p_status = sub.add_parser("status", help="只读：谁偏离了基线")
    p_status.add_argument("needle", nargs="?")
    p_status.add_argument("--json", action="store_true")

    p_diff = sub.add_parser("diff", help="只读：具体哪些文件不同")
    p_diff.add_argument("skill")

    p_pull = sub.add_parser("pull", help="收回：唯一偏离的那份 → 全部其它位置")
    p_pull.add_argument("needle", nargs="?")
    p_pull.add_argument("--dry-run", action="store_true")

    p_push = sub.add_parser("push", help="推出：母库 → 全部副本")
    p_push.add_argument("needle", nargs="?")
    p_push.add_argument("--dry-run", action="store_true")

    p_adopt = sub.add_parser("adopt", help="以指定位置为准")
    p_adopt.add_argument("skill")
    p_adopt.add_argument("--from", dest="src", required=True,
                         help="位置名（母库 / tool / tool#实例）")
    p_adopt.add_argument("--instance", default="")
    p_adopt.add_argument("--dry-run", action="store_true")

    args = ap.parse_args()
    db = Path(args.db)

    if args.cmd == "status":
        return cmd_status(connect(db), args.needle, args.json, db)
    if args.cmd == "diff":
        return cmd_diff(connect(db), args.skill, db)

    conn = connect(db, writable=True)
    if args.cmd == "pull":
        return cmd_pull(conn, args.needle, args.dry_run, db, Path.home())
    if args.cmd == "push":
        return cmd_push(conn, args.needle, args.dry_run, db)
    if args.cmd == "adopt":
        return cmd_adopt(conn, args.skill, args.src, args.instance, args.dry_run, db)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())