# skill-hub-sync（同步工具，随仓库分发）

这是 Skills Hub 的**同步工具**，跟着仓库走。它不含任何用户技能内容。

## 为什么不放在 `skills/`

`skills/` 是**母库**，装的是用户自己的技能（可能含私有项目、内网路径、
凭据约定）。那样整个目录都在 `.gitignore` 里，一个都不入库。

同步工具是**代码**，不是技能内容，所以单独放在 `tools/`，与母库彻底分开：

```
tools/skill-hub-sync/     ← 本目录，入库，任何人 clone 都有
skills/skill-hub-sync/    ← 母库里的那一份，含在 .gitignore 里，不入库
```

两份**内容相同**，改完一边要同步到另一边。但**不要**把它们当成同一个技能去
`sync.py pull` —— 母库记录的是 `skills/` 那一份，`tools/` 这份只是分发用。

## 怎么用

```bash
# 找到你当前软件里的那一份（各软件路径不同，用自己这份最稳）
ls ~/.codex/skills/skill-hub-sync/sync.py

SYNC="py ~/.codex/skills/skill-hub-sync/sync.py"
$SYNC status           # 只读：谁偏离了基线
$SYNC pull             # 收回：别处的改动 → 母库和所有位置
$SYNC push             # 推出：母库 → 全部副本
$SYNC adopt X --from 母库
```

只看某一个技能：`$SYNC status <关键字>`。

**Python 命令名**按环境挑：Windows 用 `py`，有 python 用 `python`，
macOS/Linux 用 `python3`。**只用标准库**，不需要 pip 装东西。

## 它会读什么、写什么

只读本地 SQLite（`<配置目录>/io.github.mcncarl.skillshub/skills_hub.db`）
和本地技能目录。**没有任何网络调用。**

写操作（`pull`/`push`/`adopt`）会先备份到
`~/.workbuddy-ai/skill-sync-backup/<时间戳>-<动作>/`，里面有 `MANIFEST.txt`
记录每个备份目录原本对应哪个位置。

## 已知限制

- 没有「文件数超过 500 就跳过」的保护（GUI 的 Rust 实现有）。
- 没有「连续两轮确认」机制 → 编辑器存盘存一半时跑 `pull` 可能收进半成品。
  **改完稍等一下再同步。**

细节见 `SKILL.md`。