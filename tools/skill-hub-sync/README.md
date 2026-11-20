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

两份共享 `sync.py` / `SKILL.md` / `check_sync.py` 三个文件，**内容逐字节相同**
（母库里那份只是这三件，没有本 README —— 它是写给仓库读者的）。
改完一边记得同步到另一边。但**不要**把它们当成同一个技能去
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

## 复制到别的软件能用吗？——能，装哪儿都一样

**实测结论**：把本目录复制进第三个软件目录，从那里执行 `pull`，
母库和另一个软件都收到了改动。**不依赖装在哪个软件下。**

### 为什么不需要「每个软件配一次」

数据库是**整台机器共用一份**：

```
%APPDATA%\io.github.mcncarl.skillshub\skills_hub.db      （Windows）
```

它不存在于任何软件自己的目录里，所以 codex / claude / 豆包 / workbuddy
里跑的都是**同一个库、同一套基线**。实测：从 10 个位置（9 个软件目录 +
母库）各自用自己那份脚本、不带 `--db` 执行 `status`，10/10 都读到全部 44
个技能且无差异；8 个位置的 `sync.py` 哈希完全相同。

换句话说，**前提只有「这台机器上是这套安装」**，而不是「每个软件都要登记」。
skill 和 Skills Hub 本来就是捆绑的，这个前提天然成立。

### 唯一的边界：换一台全新的机器

新机器上如果**还没跑过 Skills Hub**，库不存在 → 脚本报「找不到数据库」退出；
库存在但没扫过这些技能 → 显示「0 个技能」，什么都不做。

这不是缺陷，是「不猜方向」的必然结果：方向靠基线判断，没有基线就没有
权威版本。这种情况下只有两条路：

1. 先跑一次 Skills Hub，让它扫描并登记技能（推荐，之后一切正常）；
2. 按 `SKILL.md` 第四节**手工**同步（备份 → 排除私有目录 → `cp -a` → 复检），
   方向由人定。

日常使用（本机、多个软件之间来回改）**完全不受这个边界影响**。

## 已知限制

- 没有「文件数超过 500 就跳过」的保护（GUI 的 Rust 实现有）。
- 没有「连续两轮确认」机制 → 编辑器存盘存一半时跑 `pull` 可能收进半成品。
  **改完稍等一下再同步。**

细节见 `SKILL.md`。