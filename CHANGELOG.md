# Changelog

All notable changes to this project will be documented in this file.

## [2.0.13] - 2026-11-27

### Added

- **技能自声明版本号（卡片 + 详情页都显示）**：A/B 字母只说得出「哪份内容更新」，
  说不出「这是哪个版本」。`tietu-track-positioning` 就是典型——母库 `0.6.0`、
  `workbuddy` 是 `0.6.1`，光看字母只能猜谁是谁。现在直接读技能自己的版本号：

  | 来源 | 说明 |
  |---|---|
  | `manifest.json` 顶层 `version` | 发布元数据，优先 |
  | `SKILL.md` frontmatter 的 `version:` | 次选，只扫 frontmatter 区间 |

  **覆盖率只有 8/48**（其余 40 个技能两个文件里都没写版本号），所以按你的要求
  **没声明的一律显示 `v0`**，不留空位——若按「没有就不渲染」处理，40 行会参差不齐。
  `v0` 用弱化灰，真实版本号用主色，一眼能分出来。

  布局约束：徽章 `flex: 0 0 auto` + `min-width: 34px`，**绝不参与标题行的弹性分配**，
  技能名（`flex: 1 1 0`）仍优先吃满剩余宽度并省略。给 `flex: 1` 会和名字抢宽度，
  导致长名字被截得更早——`web-access`、`digital-human-video-editing` 这类长名会先暴露。

### Fixed

- **`agents/openai.yaml` 缺失时版本号永远读不到**：`try_read_skill_ui_metadata`
  原先在该文件 NotFound 时直接 `return SkillUiMetadata::default()`，而版本号来自
  另两个文件（`manifest.json` / `SKILL.md`），与它无关。48 个技能里只有 1 个有
  `openai.yaml`，不修这条路径，版本号功能等于完全不生效。现在先读版本号，再决定
  是否提前返回。同理，`openai.yaml` 解析出错时的容错分支也会单独再读一次版本号，
  不因 icon 元数据坏掉而连带丢弃版本信息。

- **版本号解析对脏数据保守**：`version:` 后面可能跟引号、行尾注释或一段描述。
  `sanitize_version` 只接受「以数字开头、仅含 `[alnum . - + _]`、长度 ≤24」的形状，
  不合格一律当作没有版本号（显示 v0），不把脏字符串塞进徽章把布局撑坏。
  另外只在 frontmatter 区间内匹配、且跳过缩进行——正文里的示例配置
  （`version: 2`）与子对象里的 `metadata.version` 都不会被误当成技能版本。

  Rust 侧新增 8 条测试锁住这些语义，其中包括「没有 openai.yaml 也能读到版本号」
  这条回归防线。

## [2.0.12] - 2026-11-27

### Fixed

- **「生成 / 更新」判定一直判错，真实编辑全被说成「刚装好」**：
  卡片原先用 `updated_at - created_at > 1s` 推断「更新过」，前提是
  「内容一改 `updated_at` 就动」。这个前提不成立——`skills.updated_at`
  只在安装、启用/停用、设为母版、重建镜像这类**元数据或结构**操作时写入，
  而真正的内容变更（用户改技能里的 md、`sync.py pull` 拉下新版本）走
  `peer_sync::reconcile` 的采纳路径，那条路径只写 `skill_baselines` 和
  `skill_baseline_history`，**从头到尾没有回写 `skills.updated_at`**。

  实测反例：`xiaohongshu-ai-creator` 入库于 11-25 21:39:58，内容在
  11-26 21:02:13 改过（13 个文件），21:12:09 被轮询检测到并扩散到各副本。
  但它的 `created_at == updated_at == 11-25 21:39:58`，旧逻辑判为「生成」，
  卡片还显示「1 天前」——而它明明是几十分钟前刚改的。

  改为由后端下发两个字段，前端不再猜：
  - `content_changed_at`：内容最后变更时间，取 `skill_baseline_history`
    里该技能最新一条指纹的时间；**没有历史则取母库文件的最新 mtime**
    （真实内容写入时间），两者都取不到才退回入库时间。
  - `content_edited`：是否真的被编辑过。历史里只有一条 = 建立基线时落的
    初次快照，不算编辑；两条及以上才说明至少发生过一次采纳。走 mtime 兜底时，
    文件比入库晚超过 1 分钟才算编辑过。

  为什么兜底不能取 `updated_at` 或入库时间：17 个技能在历史表里零记录
  （历史表是 2.0.x 才引入的）。它们的 `updated_at` 大多是同一次批量迁移留下的
  `11-11 08:14`，不代表内容变更；而文件 mtime 是真的——实测其中 13 个与
  `updated_at` 精确到秒完全一致（那次批量操作确实写过文件内容），
  `agentport` 的 mtime 更晚（`11-21 09:10`，之后又被编辑过）。
  若退回入库时间，这 17 个会全部显示成「09-21 生成」，是假的；
  用 mtime 后全部如实显示「11-11 更新」及各自真实时间。

  `skill_baseline_history` 是唯一真实记录「内容何时变过」的地方，但它混着
  两套时间单位：早期版本用 `as_secs()` 写入（31 行，如 `1795683097`），
  现在用 `as_millis()`（53 行，如 `1795683097000`）。早前把秒级值当毫秒
  解析，换算出 `1970-01-22 02:48` 这种鬼时间，误以为数据损坏。现统一按
  `1e11` 为界归一化到毫秒（秒级今天是 1.7e9，毫秒级 1.7e12，界线两侧距离
  足够远，不会误判）。

  全库 48 个技能实测结果：43 个判为「更新」，5 个（`browser-operator`、
  `digital-human-video-editing`、`web-access`、`zlib-safe`、`openscreen-dev`）
  判为「生成」——这 5 个的历史都只有 1 条，且是安装后几分钟写入的初次基线，
  符合「装完没动过」的语义。零历史的 17 个走 mtime 兜底，全部如实判为「更新」
  （11-11 及之后各有真实修改），不会被误说成「9 月刚装好」。

- **详情页的时间同样取自失效的 `updated_at`**：`SkillDetailView` 顶部那枚
  时钟图标旁显示的是 `skill.updated_at`，与卡片同一个问题。一并改用
  `content_changed_at`。

### Added

- **卡片时间戳加图标区分「生成」与「更新」**：同样是「23 小时前」，可能是
  刚装进来没动过，也可能是刚被更新并扩散到了各工具，语义完全不同。用
  `FilePlus2`（生成）与 `Pencil`（更新）两个图标区分，并各配 tooltip。

  布局约束：图标与文字排进 `.skill-updated` 这个 `inline-flex`，图标用
  `flex: 0 0 13px` 占固定槽位，文字 `flex: 0 1 auto`。列表视图网格的 time 列
  从 64px 增到 74px 容纳图标。实测列线仍然严格对齐（`timeLeftUnique:[742]`、
  `toolsLeftUnique:[823]`、`widthsUnique:[74]`、无溢出），没有出现历史上
  踩过的「工具区左边界整排右移」问题。

### Changed

- **启用/停用不再刷新 `updated_at`**：`set_skill_enabled` 与
  `enable_skill_with_targets_atomically` 里的 `UPDATE skills SET ... updated_at = ?`
  去掉了 `updated_at` 赋值。开关是状态而非内容变更，写进去只会污染时间线
  （虽然判定已不依赖该字段，但仍不该写脏数据）。

## [2.0.10] - 2026-11-26

### Changed

- **日志改为本地时间、加大容量并保留归档**：`tauri-plugin-log` 的默认值是
  `TimezoneStrategy::UseUtc` + `max_file_size(40KB)` + `RotationStrategy::KeepOne`。
  三者叠加的后果是：时间戳比本地时间慢 8 小时（日志写 `12:42`，实际 `20:42`），
  而且日志一满 40KB 就被 `fs::remove_file` **整个删除**，不留任何归档——出了问题
  回头去查，历史已经没了。实测日志目录里只剩一个 240 字节的文件，就两行。

  改为本地时间、单文件 5MB、保留 10 份归档。注意 `RotationStrategy::KeepSome`
  才开始做「重命名为带日期」的归档，`KeepOne` 是直接删。

- **同步日志补齐可追溯信息**：
  - `peer_sync` 采纳改动时只打「更新 N 个副本」的数量，其实 `updated` 里本来就是
    各落点的标签字符串，白白丢掉。改为打印具体落点，事后能直接看出是哪几份被覆盖。
  - 定时轮询原先只在 `adopted > 0 || conflicts > 0` 时才写一行，长时间无日志时
    **无法区分「没有改动」和「同步线程已经死了」**。改为每轮必打一行，兼作心跳。
  - `stale_copies` 自动刷新副本成功时**完全不写日志**（只有失败才 `warn`），导致
    「界面报落后、过一会儿自己好了」这类现象事后无从追查。现在成功也记技能、工具
    与备份落点；启动扫描还会汇总一行「异常几个、可自动刷新几个、已刷新几个、剩余几个」。

### Added

- **设置页新增「运行日志」入口**：一键打开日志目录，不用再人肉找
  `%LOCALAPPDATA%\io.github.mcncarl.skillshub\logs`。新增 `get_log_dir` 命令，
  指向与 `TargetKind::LogDir` 相同的目录，避免界面与真实写入位置不一致。

## [2.0.9] - 2026-11-25

### Fixed

- **「只显示有问题的技能」长期挂着已修好的技能**：点健康栏「落后 N」进入筛选时，
  存的是**点击那一刻**的名单快照（`new Set(problemSkillNamesRef.current)`），
  而启动自动刷新会在界面起来后几秒内把落后副本收回母库。数据修好后快照不会
  跟着变，列表于是长期显示一批**实际已无问题**的技能，横幅还写着「6 个」，
  点进去每一项都正常——只有手动 F5 才消失。实测：启动刷新在 21:44:42 把 6 个
  双语描述同步回母库并刷新基线，21:49 的界面仍列着这 6 个。

  改为只存「是否启用」的开关，过滤集合每次渲染都取实时的 `problemSkillNames`；
  问题清零时自动放开筛选，避免停在「只显示有问题的技能（0 个）」的空列表上。
  判定逻辑抽成纯函数 `resolveProblemFilter` 并加 4 条测试锁住语义，防止又被
  改回快照。

- **启动刷新后卡片仍挂着刷新前的版本字母/角标**：启动刷新只重读了技能列表与
  健康栏，没有重算 `get_peer_version_map`，于是「已自动同步 N 个」的提示与卡片
  上的旧字母自相矛盾。现在刷新成功后会一并重算版本字母。

- **母库角标点击处理里有不可达代码**：`toggleProblemFilter()` 之后紧跟一个
  `return;`，其后的 `toast.warning(...)` 永远不会执行（TS 不报错，但读代码时
  容易误以为它在工作）。已删除。

## [2.0.8] - 2026-11-21

### Fixed

- **工具图标挤在左侧、右侧一大片空白**：工具列原本是 `minmax(184px, 1fr)`，
  `1fr` 让它吃掉全部剩余宽度（1706px 窗口下实测 939px），而一行图标只占
  一两百 px，右边就挂着几百 px 死白；里层 `.skill-tool-avatars` 的
  `flex: 1 1 auto` 又把图标容器撑满格子，图标只能靠左排。
  现在工具列收成 `minmax(0, 296px)`（10 个图标刚好放下），系数改成
  `flex: 0 1 auto` 让图标容器只占实际宽度，多出来的宽度全部转给名称列
  （那里是真实描述文字，加宽会被文字填满，不会变成新的空白）。
  实测 1706px 下工具格从 939px 收到 296px，8 图标行右侧空白从约 700px 降到 61px。

  这里没有直接把工具列改成 `auto` / `fit-content`：列表每行是**独立的 grid
  容器**，按内容收缩会让各行算出不同列宽，右侧开关 / 操作 / 箭头三列跟着逐行
  偏移 —— 等于把 2.0.6 修好的「列线错位」挪到另一个位置重演。用固定上限
  `minmax(0, 296px)` 则只取决于容器宽度，同屏各行始终一致；窄窗口下工具列
  先被压缩、图标转横向滚动，不会把整行挤出容器。1080px 断点同步改为 224px。
  实测 1672 / 1000 / 900px 三种宽度，工具区左边界与右侧三列 X 坐标**各自唯一**。

## [2.0.7] - 2026-11-21

### Fixed

- **多账号工具只显示 1 个账号**（豆包）：`auto_reconcile_skill_targets` 只扫
  `home.join(relative_skills_dir)`，而豆包这个适配器的相对目录是 `DoubaoWork/skills`
  ——实测该目录存在但**完全为空**，真正的账号目录在
  `DoubaoWork/User Data/<Profile>/...` 下。于是非主账号的副本从来没被登记
  （实测 Profile 5 磁盘上有 7 个技能，库里只有 1 个），而且它写死
  `instance: String::new()`，把登记到的那份也标成了主实例。界面因此只能显示
  「1 个账号」。现在多账号工具走 `discover_root_instances` 逐账号展开，
  每个账号目录一条记录；单实例工具行为不变。

## [2.0.6] - 2026-11-21

### Fixed

- **「母库和主 skill 不同，却一个版本字母都没有」**：`mirror_check` 判「母库落后」
  用的是 `content_hash::hash_dir`，它把**目录条目**也喂进哈希、且不限单文件大小；
  而算 A/B 字母的 `peer_sync::snapshot_with_time` 只统计 5MB 以下的文件。同一批目录
  沿两套口径算出相反结论时，界面就一边报母库落后（红色 `!`）、一边说「只有一种版本」。
  现在 `mirror_check` 改用 `snapshot_with_time`，全应用只剩一套内容口径；并补了一条
  回归测试锁住「空目录不得让两个判定分叉」。
- **卡片角标会停在旧结论上**：窗口重新获得焦点时的重扫只刷新健康栏和副本落后，
  漏掉了母库落后与版本字母。于是母库角标还是启动时算的「!」，字母表已按新内容
  算过（空表）——用户看到的就是上面那种自相矛盾。四者现在同一时刻一起重扫。
- **「这个 agent 图标把样式顶开了」**：`.list-view` 的 identity 列用
  `minmax(200px, 256px)`、meta 列用 `fit-content(150px)`，两列都跟着内容伸缩。
  长名字行把工具区左边界从 x=801 顶到 x=832，整排图标右移 31px，行与行对不齐。
  两列改为固定宽度，只让 tools 列（`minmax(184px, 1fr)`）弹性。
  1080px 断点原本只给了 6 列，而 `grid-template-areas` 有 8 个区域——列数与区域数
  不匹配会让整套区域失效，一并补齐为 8 列。
- **已登出的账号仍被算进「已铺 N/M」的分母**：分母只判目录存在，注销账号后
  卡片仍显示旧总数。现在只统计 `signed_in === true` 的账号，已登出的单独提示。

## [2.0.5] - 2026-11-21

### Fixed

- **「版本字母全是 A，却显示副本落后」**：两个判定口径不一致。算 A/B 的
  `peer_sync::snapshot_with_time` 会先 `sort_by(rel)` 再算指纹，而判落后的
  `hash_dir` 直接按 `WalkDir` 的原始枚举顺序喂 hasher。枚举顺序由文件系统决定
  （NTFS 基本按名称、ext4/APFS 按哈希序、网络盘没谱），于是内容逐字节相同的
  两份副本也会算出不同指纹。界面因此自相矛盾，而且自动刷新又会判定「内容其实
  一致」而不动手——红色感叹号点多少次都消不掉。现在 `hash_dir` 先收集排序再算，
  与版本字母看同一件事。
- **统计条的「落后 N」会长期停在旧数字**：落后是扫盘得出的，而盘上的内容会被
  别的软件改（在 Codex 里存一下文件、技能自己写日志），本进程看不见。原来只在
  列表变化时重扫，切走改完再切回来仍是旧结论。现在窗口重新可见（`focus` /
  `visibilitychange`）时补一次重扫，比定时轮询省，且正好覆盖这个真实用法。

## [2.0.4] - 2026-11-21

### Added

- **`skill-hub-sync` 同步工具随仓库分发**：放在 `tools/skill-hub-sync/`
  （`sync.py` + `SKILL.md` + `README.md`），与母库 `skills/` 物理分开。
  任一软件里复制这份技能即可使用，不必安装 Skills Hub 的命令行工具；
  子命令 `status` / `diff` / `pull` / `push` / `adopt`。
- **母库作为一等同步参与者**：`skill_targets` 表里没有母库记录，只遍历副本表
  会安静地漏掉它。同步工具显式把母库插在位置列表首位（显示名「母库」），
  既能作为来源、也能作为目标。
- **「无基线」与「基线过期」两种状态**：内容全一致但库里没有基线时，
  不能再报「一致」（一致是相对基线而言的）；基线对不上任何位置时只刷新基线，
  不复制任何文件。两者都不会白搬目录。

### Fixed

- 单元测试假数据路径里的真实用户名改为占位符 `Example`
  （`SkillCard.test.tsx`、`mirrorErrors.test.ts`）。
- **版本号同步漏掉 `src-tauri/Cargo.lock`**：`version.mjs` 原来只管
  `package.json` + `tauri.conf.json` + `Cargo.toml` 三处，而 `Cargo.lock` 里
  本项目自己的 `[[package]]` 条目也要跟着改。现在 `sync` 会一并更新，
  `check` 也会在它不一致时报错退出（退出码 1）。

## [2.0.3] - 2026-11-19

### Fixed

- **副本永远拿不到 `.skillignore`**：规则文件在默认忽略清单里，既不参与内容
  快照比对、也不参与整目录复制，因此「两边规则不一致」在内容层完全看不出来。
  更要紧的是它还会被同步顺序卡死——全量同步只要判定内容一致就直接跳过整份副本，
  规则补发根本轮不到。现在规则下发提成独立步骤，每轮按技能执行一次，
  与内容是否变化无关；合并口径是并集，副本自己的条目原样保留。
  缺规则的后果是下一次整目录替换会把 `local/`、`logs/` 这类私有内容
  当作旧内容一起送进回收站（2026-11 的 agentport 配置就是这么丢的）。
- **已登出的账号仍在接收同步**：多账号目录里已登出的那份也在被写入副本，
  既占空间又会在下次登录后变成难以解释的「旧版本」。现在只推给可同步的实例；
  账号全部登出时退回主实例，保证技能不会凭空消失。

### Added

- **覆盖母库前先做安全检查**：原件目录为空、或目标路径不在技能库根目录下时
  直接拒绝，并给出可读的中文原因（而不是一个裸错误码）。
- **启动时清理母库残留**：历史失败的更新操作会在技能库根目录留下
  `.skills-hub-*` 暂存目录，长时间累积占空间。现在启动时清理超过一小时的残留。

### Changed

- 两个右键菜单合并为同一个浮层实现，补齐 `role="menu"`、方向键移动焦点、
  Esc 关闭等键盘操作；没有登记原件的技能不再弹出「覆盖母库」菜单。
- 账号目录的账号名回退顺序明确为 `账号名 → 目录配置名 → 租户名 → 目录名`，
  租户名与账号无关，必须排最后。

## [2.0.2] - 2026-11-19

### Added

- **母库镜像可以手动覆盖**：右键技能卡片的母库图标 → 「用原件覆盖母库」，
  整目录替换为原件内容，`local/` 等私有内容按 `.skillignore` 保留。
  母库刷新是全库唯一没有自动同步的方向，原先只靠一个随版本号消失的角标，
  现在挂到恒存在的母库图标上，不依赖任何状态判定。
- **多账号目录显示真实账号名与数字 ID**：账号列表从软件自己的配置
  （`Preferences` → `saman.last_user`、`Local State` → `profile.info_cache`）
  读出账号名与 `user_id`。已登出的账号灰显并标「未登录」，仍如实计入账号总数
  ——目录还在就说明技能还被写进去、还占空间。
  兜底顺序为 `last_user.name → profile.info_cache.name → tenant.username → 目录名`；
  飞书租户名（如「额额额」）与账号名无关，必须排在最后。
- **技能列表新增「来源」列**：母库徽章和主 skill 所在软件的图标并排放在同一列
  （左边母库、右边主 skill，带皇冠与版本字母），一眼能看出这份技能的原件在哪。
  卡片视图同样显示这两个图标。
- **列表不再浪费一整列空白**：标签列改为按内容收缩（`fit-content`），
  宽屏下不再顶到 200px 上限而在徽章与来源图标之间空出 120px；
  腾出的宽度全部给工具图标区，一行能多显示一倍多的图标。

### Fixed

- **母库落后的提示被版本字母盖掉**：角标里版本字母优先于感叹号渲染，导致
  「母库落后于原件」看不出来。现在落后状态一律把角标染成橙色。
- **`SkillRootInstance` 缺 `instance` 字段**：只有展示名 `label`，与
  `skill_targets.instance` 的口径不一致，前端无法与目标记录对齐。已补齐。
- **账号名回退成飞书租户名**：`Local State` 少找了一层目录（读的是 `User Data`
  的父目录而不是它自己），`profile.info_cache.name` 从未命中，于是 `Default`
  显示成「额额额」、`Profile 2` 显示成「leon」。`Preferences` 的路径本来就是对的。

### Changed

- 覆盖母库后立即重算副本落后状态，不必再手动刷新一次列表。

## [2.0.1] - 2026-11-12

同步只留一种方式：**整目录复制**。软链接（symlink / junction / auto）全部移除。

软链接和 2.0 的对等模型是矛盾的：链接让「N 份副本」退化成「1 份文件」，
「谁改谁最新」、版本字母、基线比对全都失去意义。与其留一个会出怪状态的
开关，不如彻底删掉。

### Removed

- **同步模式枚举**：`auto / symlink / junction` 三个变体删除，只保留 `copy`。
  `SyncMode` 枚举保留（历史序列化数据仍可读），一律按复制处理。
- **链接创建路径**：`sync_dir_hybrid*`、`try_link_dir`、`try_junction`、
  `create_enable_staged_symlink/junction`、`create_directory_symlink/junction`
  等函数全部删除；`junction` crate 依赖移除。
- **Tauri 命令**：`set_tool_sync_mode`、`get_tool_sync_modes`、
  `set_default_sync_mode` 删除；`get_default_sync_mode` 恒返回 `copy`。
- **per-tool 默认模式表**（`tool_sync_mode_v1` 设置与 Antigravity/Cursor 特判）
  整体删除——不再有需要特判的工具，因为链接路径已不存在。
- **界面**：工具卡上的「软链接 · 复制」下拉、设置页「同步模式」面板、
  自定义工具表单的同步方式选择、右键菜单的链接分支全部移除；
  相关 i18n 文案与样式一并清理。

### Compatibility

- 数据库 `skill_targets.mode` 字段保留作历史记录；读到 `symlink` / `junction`
  / `auto` 时一律按 `copy` 解释，下一轮对等同步会把旧链接换成实体目录。
- 设置里的 `default_sync_mode`、`tool_sync_mode_v1` 键不再读取，可留在库里。

## [2.0.0] - 2026-11-12

这一版把「一个原件 + 一堆副本」改成了「多份平级的副本 + 一份基线」，
解决多软件、多账号同时维护同一个技能的场景。

### Added

- **母库页面**：侧边栏新增只读浏览，三栏（技能 → 文件树 → 内容预览），
  Markdown 渲染，frontmatter 单独显示。
- **版本字母**：把同一技能的副本按内容分组，A 是最新（红色），B/C 灰色，
  标在工具图标左下角；只有一版时不显示。母库也参与分组。
- **一键修母库**：技能名旁的「母库落后 · 点击修复」用原件刷新镜像。
- **点数字筛选**：点健康栏的「落后」「母库落后」只显示有问题的技能。
- **托盘告警**：出现冲突时托盘图标换成带感叹号的版本，窗口不可见时弹系统通知。
- **刷新入口**：健康栏右侧按钮 + `F5` / `Ctrl+R`，一次重读列表、健康、落后、版本。
- **问题置顶**：断链、落后、手改、母库落后的技能永远排在列表最前。
- **单实例**：第二个进程只激活已有窗口，不再出现两个实例同时改数据库。

### Changed

- **同步模型改为对等**：不再有「主 skill 决定一切」。每个技能记一份基线
  （上次一致时的文件指纹），谁偏离基线谁就是最新，由它推给其它副本；
  只有多份偏离且内容不同时才判冲突、停下来问人。
- **托盘菜单精简**：12 项 → 4 项（显示 / 打开母库 / 立即同步 / 退出）。
- **「同步此软件」按状态拆分**：原件更新 → 用原件覆盖此工具；副本更新 →
  用这份作为最新（推给全部）；缺失 → 安装到此工具。
- **主 skill 的语义**改为「最初创建的位置」，只用于记录起源，不再是权威。

### Fixed

- **落后判定按内容优先**：复制、还原备份、编辑器保存都会刷新 mtime，之前
  会被误判成「手改」并标红。现在先比内容，一致就不报。
- **多份副本内容相同不再算冲突**：之前会被当作分叉，现在视为同一版本并同步。
- **母库感叹号位置**：移到左下角，和工具图标一致（左上主 skill、右上账号数、
  右下复制模式、左下异常）。

### Safety

- 同步前要求同一指纹连续两轮扫描都出现，避免把正在写入的半成品扩散出去。
- 忽略 `__pycache__`、`*.pyc`、`.DS_Store`、`~$*` 等生成物与临时文件；
  单文件超过 5 MB 或单技能超过 500 个文件则跳过自动同步。
- 保留最近 5 版基线指纹；副本退回历史版本时判为回滚，不扩散。

## [1.0.0] - 2026-11-05

First release under this fork's own version line. Upstream baseline was `v0.9.1`;
the feature set below has long outgrown it, so the app no longer reports an
upstream version it does not match.

### Fork changes

- Use `~/.agents/skills` as the fixed source of truth and keep tool directories as validated sync targets.
- Move removed or replaced managed content to recoverable Trash instead of permanently deleting it.
- Distinguish updateable external sources from centrally managed self-references, and report skipped updates separately from failures.
- Read optional Skill-owned icons from `agents/openai.yaml` instead of maintaining a built-in publisher/avatar registry.
- Disable automatic binary releases and in-app self-updates for this source-only fork.
- **Full-stack architecture refactoring 2.0**:
  - **Frontend**: Extracted 16 domain-specific hooks (`useAppSettings`, `useAutoUpdate`, `useDiscoveryScan`, `useBulkOperations`, `useSkillFilters`, `useTagOperations`, `useSkillImport`, `useToolSync`, `useSkillScope`, `useSkillLifecycle`, `useSystemSettings`, `useInstallScopeTargets`, `useImportPlan`, `useExploreSkills`, etc.), slashing `App.tsx` by 60.2% (from 4,149 to 1,651 lines) with 110 automated unit tests passing.
  - **Backend**: Decoupled monolithic `commands/mod.rs` (3,225 lines) into 8 focused domain submodules (`common`, `tool_commands`, `auto_update_commands`, `setting_commands`, `onboarding_commands`, `install_commands`, `sync_commands`, `skill_commands`), reducing `mod.rs` to 26 lines of aggregation with zero warnings and sub-second cargo checks.
- **Real-time symlink penetration**: Documented and verified that editing files in linked tool directories directly updates the central master library and propagates to other symlinked tools in real time.
- **Incremental backup and disaster recovery**: Integrated one-click snapshot backups and automated background backups on reverse-promotion to central.
- **Owner-aware sync**: skills that record an external owning folder (`source_type=local` + `source_ref`) now use that folder as the authoritative source for tool sync, auto-reconcile, re-enable and target comparison; the central copy becomes a mirror written only by the update action. Falls back to `central_path` whenever no owner is recorded, so managed skills behave exactly as before.
- **Software-level shield with per-skill exceptions**: `ignored_tools_v1` blocks an entire tool from discovery, sync and reconciliation, and `tool_skill_whitelist_v1` (entries shaped `tool_key/skill_name`) lets named skills inside a blocked tool keep syncing. Disk contents are never modified, so unblocking restores the previous state exactly. Shared skill folders (e.g. `.agents/skills`) stop being scanned only when every tool that owns them is blocked, so blocking one tool cannot silently disable another.
- **Doubao desktop and Qoder CN adapters**: the built-in catalog grows to 54 tools. Doubao reads its Chromium profile workspace (`%LOCALAPPDATA%\Doubao\User Data\Default\.doubao\agent_mode\workspace\.user_skills`) and is registered separately from Doubao Work; Qoder CN uses `~/.qoder-cn/skills` and stays separate from the international `~/.qoder/skills`.
- **Settings visibility for scan exclusions and blocked software**: dedicated cards list which software is excluded from discovery, which is fully blocked, and which skills are whitelisted as exceptions, each with an inline unblock action.
- **Tool names never leak raw keys**: health toasts, bulk progress, sync prompts and confirm dialogs now resolve unknown tool keys through the `tools.<key>` dictionary with the key as fallback, instead of printing the raw key when the running backend has no adapter for that tool.

## [0.9.1] - 2026-08-29

### Changed
- **Bilingual GitHub release notes**: The release workflow now extracts both Chinese and English changelog entries, including localized platform installation notes in GitHub Releases and app updater metadata. In-app update dialogs display only the section matching the current interface language, while legacy and incomplete release-note formats continue to display in full.
- **App update completion flow**: Available updates now use a prominent title-bar action that expands from an icon to a localized label on hover or keyboard focus. After installation, users can restart immediately from the completion dialog or close it and restart later from the persistent title-bar action.

### Fixed
- **Skill detail compatibility on older macOS versions**: Opening a Skill Markdown file no longer blanks the entire app in WKWebView versions that do not support regular-expression lookbehind. The detail renderer now detects this capability and falls back to standard Markdown while preserving full GFM rendering on supported systems ([#108](https://github.com/qufei1993/skills-hub/issues/108)).
- **Tag rename dialogs in Tauri WebView**: Tag rename now uses an in-app input dialog instead of the unsupported browser `window.prompt`, restoring tag renaming on macOS. The remaining shared-directory confirmation also uses the existing in-app dialog instead of `window.confirm` ([#109](https://github.com/qufei1993/skills-hub/issues/109)).
- **Responsive Skill detail workspace**: Replaced the fixed-width detail drawer with a full workspace view that adapts to the main window, keeps the file tree and Markdown content readable, and presents scope, synced tools, tags, source, update time, and file count in a compact responsive header. Source paths can be copied directly, while duplicated `name` and `description` frontmatter fields are omitted from the Markdown metadata table ([#111](https://github.com/qufei1993/skills-hub/issues/111)).
- **Accurate Skill status**: Skill details, cards, and dashboard summaries now distinguish source errors, healthy syncs, partial failures, failed syncs, and not-yet-synced states. Source update and target sync failures are persisted, the dashboard links active issues to their update details, and clicking a failed tool retries the sync instead of attempting to disable it. Skill cards and details now use the same currently detected tool set, with cards showing the synchronized count explicitly.
- **Windows junction cleanup and existing Skill imports**: Disabling, unsyncing, or deleting a Skill now removes Windows directory junctions without touching their targets. Discovery can also import byte-identical copies that already exist in central storage by reusing the managed record, while normal duplicate local installs and conflicting content remain rejected ([#110](https://github.com/qufei1993/skills-hub/pull/110)).
- **Windows automatic update schedules**: The default 24-hour interval and other whole-day intervals now register with `schtasks` as daily schedules instead of invalid hourly modifiers above 23. Supported minute and hourly intervals retain their native schedule types, while unsupported long non-whole-day intervals are rejected before task creation ([#102](https://github.com/qufei1993/skills-hub/issues/102)).

## [0.9.0] - 2026-08-23

### Added
- **DeepSeek Harness tool adapter**: Added global and project-level Skill sync through `~/.dsh/skills` and `.dsh/skills`. Skills Hub detects a default DeepSeek Harness installation through `~/.dsh` and displays the DeepSeek product icon throughout tool management and sync flows.
- **Discovery scan settings**: Added persistent per-directory scan source controls with a permanent Settings entry and a contextual shortcut beside discovered Skills. Saving refreshes discovery results without changing managed or synced Skills.

### Changed
- **Built-in tool catalog**: Increased the built-in adapter count to 47 and documented DeepSeek Harness path rules in the English and Chinese support matrices.
- **Discovery validation**: Tool directories are now reported as importable Skills only when they contain a `SKILL.md` file, reducing unrelated discovery results.

## [0.8.1] - 2026-08-06

### Fixed
- **Central storage guidance**: When a local Skill import conflicts with a folder placed directly in the Skills Hub-managed central storage directory, the error now explains that the directory is managed by the app and tells the user how to move and re-import the Skill ([#103](https://github.com/qufei1993/skills-hub/issues/103)).

## [0.8.0] - 2026-07-17

### Added
- **Card and list views**: My Skills can switch between responsive card and compact list layouts while preserving selection and filtering state.
- **Collapsible desktop navigation**: The sidebar can collapse to icon-only navigation, giving the workspace more room at smaller window sizes.
- **Recognizable tool identities**: Built-in AI coding tools use their product icons consistently across tool management, installation, and Skill sync status.
- **Custom tool identity and sync modes**: Custom tools support avatars and explicit automatic, symbolic link, Windows junction, or copy modes. Legacy configurations default to no avatar and automatic mode.
- **Custom tool editing**: Existing custom tools can update their name, avatar, global/project directories, and sync mode. Active targets migrate to the new directory or representation.
- **Title bar update status**: The app checks for updates after startup and shows an actionable icon when a new version is available, reusing the existing release notes and installation flow.

### Changed
- **Desktop UI redesign**: My Skills, Explore, Tags, Tools, Updates, Settings, and installation flows now share a consistent desktop layout, spacing system, scrolling behavior, and responsive rules.
- **Installation workflow**: Online, Git repository, and local folder installation use a unified structure with visible tag, tool, scope, summary, and action areas.
- **Skills Hub branding**: Replaced the application logo and generated desktop icons with the new balanced Skills Hub mark for consistent use in the app, installer, and operating system launcher.
- **Tool card action hierarchy**: Custom tool cards keep enable/disable in the top-right corner and move edit/delete actions into a separate footer.
- **GitHub project entry**: The open-source project link moved to the GitHub & network settings section and opens in the system default browser.
- **Skill card column sizing**: Card view keeps a stable column width instead of stretching a single filtered result across the content area.

### Fixed
- **Startup loading overlay**: Initial discovery of importable skills now runs silently in the background instead of briefly showing the “Installing Skills” dialog.
- **Safe custom target migration**: Directory and sync-mode changes protect shared physical targets and reject invalid configuration when a destination conflicts or migration fails.
- **External link permission**: Added a Tauri opener permission scoped to the Skills Hub repository, fixing the GitHub project page failing to open.

## [0.7.1] - 2026-07-10

### Fixed
- **Unified network proxy**: App update checks, update downloads and installation, release notes, and Explore online search now honor the configured network proxy ([PR #91](https://github.com/qufei1993/skills-hub/pull/91)).

## [0.7.0] - 2026-07-05

### Added
- **Install scope selection**: Choose global or project-level sync before installing a skill, including batch installs from local folders and Git repositories.
- **Bulk skill management**: Select multiple skills from My Skills and apply batch operations for tags, target tools, enable/disable, and delete.
- **Skill enable/disable controls**: Temporarily disable a skill without deleting it, then re-enable it later with its previous tool sync settings restored.
- **Management Center**: Manage tags, tool configuration, and Skills auto update from one dedicated area.
- **Tool management**: Enable or disable built-in tool targets so unused tools no longer appear in skill cards, install flows, or sync actions.
- **Custom tool directories**: Add custom tool targets with global skills directories and optional project-level skills directories.
- **Scheduled Skills auto update**: Configure system-level background updates for managed skills, trigger an immediate update, and review run results in the Management Center.
- **GitHub network proxy settings**: Configure a local proxy for GitHub API requests, featured skills, GitHub downloads, and Git update flows.

### Changed
- **Settings page organization**: Settings now focuses on application preferences such as interface language, appearance, storage, cache, GitHub access, network proxy, and app updates.
- **Management UI layout**: Tags, tools, and update pages now share consistent workspace spacing and tab navigation.
- **Bulk tool sync behavior**: Batch tool changes now apply the selected tool list as the final target state for the selected skills.
- **Auto update results**: Update status, counts, failures, start time, finish time, and duration are now shown inline instead of relying on a separate progress dialog.
- **Install modal layout**: The add-skill flow uses a compact install scope selector and keeps modal actions visible when candidate lists are long.

### Fixed
- **Local batch install review**: Fixed local skill directory batch installation and candidate selection edge cases.
- **Project scope grouping**: Tools that share the same project skills directory are grouped by target directory so project sync remains consistent.
- **Modal overlap**: Fixed cases where long modal content could overlap or hide footer actions.
- **Scheduler lint stability**: Cleaned up platform-specific scheduler code paths so all Rust lint checks pass across targets.

## [0.6.3] - 2026-06-27

### Fixed
- **Antigravity global sync path**: Updated Antigravity 2.0 global skill sync to `~/.gemini/config/skills` so synced skills are discovered by current Antigravity versions ([#79](https://github.com/qufei1993/skills-hub/issues/79)).
- **Windows updater metadata**: Added Windows updater signatures and `windows-x86_64` / `windows-aarch64` platform entries to `updater.json`, fixing update checks on Windows ([#78](https://github.com/qufei1993/skills-hub/issues/78), [PR #82](https://github.com/qufei1993/skills-hub/pull/82)).

## [0.6.2] - 2026-06-19

### Added
- **WorkBuddy tool adapter**: Added global skill sync support via `~/.workbuddy/skills/` ([PR #73](https://github.com/qufei1993/skills-hub/pull/73)).
- **CodeWhale tool adapter**: Added global and project-level skill sync via `~/.codewhale/skills/` and `.codewhale/skills/` ([#70](https://github.com/qufei1993/skills-hub/issues/70), [PR #74](https://github.com/qufei1993/skills-hub/pull/74)).
- **Claude Code plugin skill discovery**: Existing user-level Claude Code plugin skills can now be discovered and imported, including custom skill paths declared in `.claude-plugin/plugin.json` ([#69](https://github.com/qufei1993/skills-hub/issues/69), [PR #75](https://github.com/qufei1993/skills-hub/pull/75)).

## [0.6.1] - 2026-05-16

### Fixed
- **Window close behavior**: Closing the main window now exits the app instead of hiding it in the background, fixing cases where the app kept running but could not be reopened from the Dock or taskbar ([PR #68](https://github.com/qufei1993/skills-hub/pull/68)).

## [0.6.0] - 2026-05-05

### Added
- **Skill tags**: Add custom tags to managed skills for easier organization and filtering.
- **Tags page**: Manage tags from a dedicated Tags page, including create, rename, delete, and quick navigation back to filtered My Skills views.
- **Tag filtering**: Filter My Skills by one or more tags with OR matching, including a virtual `Untagged` filter for skills without tags.
- **Per-skill tag editor**: Edit a skill's tag assignments directly from the skill card.
- **Import search**: Search discovered skill candidates by name, description, or path before importing from a local directory or Git repository.

### Changed
- **My Skills filter bar**: Removed the manual refresh button; install, delete, sync, and tag-edit flows already refresh the list automatically.

### Fixed
- **Chinese filter bar layout**: Removing the refresh button fixes the cramped button layout in Chinese.
- **Discovered skills review**: The discovered skills review dialog now supports search and keeps selection counts aligned with filtered results.

## [0.5.0] - 2026-04-16

### Added
- **Project-level skill sync**: Skills can now be synced to selected project directories instead of only global tool directories.
- **Skill scope controls**: My Skills cards now show a scope badge (`Global` / project count) and include a scope modal for switching between global and project sync.
- **Scope filtering**: My Skills can be filtered by All / Global / Project scope.
- **Hermes Agent adapter**: Added global sync support for Hermes Agent via `~/.hermes/skills` ([#54](https://github.com/qufei1993/skills-hub/issues/54)).

### Changed
- **My Skills filter bar**: The section title now displays the total skill count, search is more compact, and filter controls stay on one line in the default window.
- **Default window size**: Increased the default desktop window from `800x600` to `960x680`.
- **macOS close behavior**: Closing the main window now hides it instead of quitting the app; reopening from the Dock restores and focuses the window.
- **Project sync support matrix**: Project-level sync is now treated as an explicit per-tool capability; tools without a confirmed project skills directory are global-only.

### Fixed
- **Import takeover for identical skills**: Importing an existing skill can now safely take over same-name targets when the content hash matches.
- **Unsynced tool re-enable entry**: Tool buttons that were unsynced from a skill remain visible so they can be re-enabled.
- **SKILL.md metadata parsing**: YAML block scalar descriptions in frontmatter now render correctly in skill cards and detail views.

## [0.4.3] - 2026-04-11

### Added
- **Copaw tool adapter**: Support for Copaw AI coding tool (thanks @LeonDevLifeLog [PR#50](https://github.com/qufei1993/skills-hub/pull/50)).

### Fixed
- **Git skill install & frontmatter rendering**: Fixed issues with Git-based skill installation and frontmatter metadata rendering.
- **Git skill discovery for container paths**: Fixed skill discovery failing when repository uses container-style directory paths.

## [0.4.2] - 2026-04-06

### Fixed
- **New tools modal style**: "New tools detected" dialog now uses consistent header/footer structure (`modal-header` + `modal-footer`) matching all other modals, fixing missing padding and border separators ([#46](https://github.com/qufei1993/skills-hub/issues/46)).
- **Git skill name derivation**: Installing a Git skill from a repo root (subpath `"."`) now correctly derives the name from the repository URL instead of using `"."` as the display name.

## [0.4.1] - 2026-03-21

### Added
- **Frontmatter metadata table**: Markdown files with YAML frontmatter now render a GitHub-style metadata table at the top of the skill detail view.

## [0.4.0] - 2026-03-20

### Added
- **In-app update check**: Check for updates directly within Settings, download and install without leaving the app ([#33](https://github.com/qufei1993/skills-hub/issues/33)).
- **QoderWork tool adapter**: Support for QoderWork desktop AI agent (`~/.qoderwork/skills/`) ([#34](https://github.com/qufei1993/skills-hub/issues/34)).

### Changed
- **Settings promoted to full page**: Settings moved from a modal dialog to a dedicated page view, consistent with My Skills / Explore navigation pattern.
- **Curated skills aggregation**: Explore page now sources skills from a curated list of 7 high-quality repositories.

### Fixed
- Language toggle briefly flashing "Installing Skills..." loading overlay on Explore page.

## [0.3.0] - 2026-03-15

### Added
- **Explore page**: Explore promoted from a modal tab to an independent page with My Skills / Explore top-level navigation.
- **Featured skills**: Explore page displays curated skills from ClawHub API (updated daily via GitHub Actions) with frontend filtering and one-click install.
- **Online skill search**: Real-time search via skills.sh API (triggered at 2+ characters, 500ms debounce), results deduplicated against the featured list and shown in separate sections.
- **Skill detail view**: Click a skill name to browse its files with a file tree, Markdown rendering (GFM + frontmatter stripping), and syntax highlighting (40+ languages, light/dark theme adaptive).
- **Skill description field**: Description extracted from SKILL.md frontmatter at install time, stored in database, and displayed on My Skills cards.
- **GitHub Token setting**: Optional GitHub Token input in settings to increase API rate limit from 60 to 5,000 requests/hour.
- **MoltBot tool adapter**: Added standalone MoltBot tool support after OpenClaw rename/split.

### Fixed
- Git install deriving skill name as "skills" when URL points to a `skills/` subdirectory, causing duplicated sync paths ([#28](https://github.com/qufei1993/skills-hub/issues/28)).
- GitHub API rate-limit errors now display the exact reset time instead of a generic message.
- Windows "Access Denied" OS error 5 when syncing to tools ([#20](https://github.com/qufei1993/skills-hub/issues/20)).
- Git repo directory structures not correctly recognized as skills ([#18](https://github.com/qufei1993/skills-hub/issues/18), [#8](https://github.com/qufei1993/skills-hub/issues/8)).
- Repos using `.claude/skills/` directory format not detected ([#27](https://github.com/qufei1993/skills-hub/issues/27)).
- OpenClaw path updated from `.moltbot/skills` to `.openclaw/skills` ([#29](https://github.com/qufei1993/skills-hub/issues/29)).

### Changed
- My Skills list: tool badges now only show synced tools, collapsing to `+N more` beyond 5.
- Manual Add modal simplified to Local Directory / Git Repository tabs only (Explore tab removed).
- Multi-skill repo online install now auto-matches target skill (exact → unique-contains → fallback to manual picker).

## [0.2.0] - 2026-02-01

### Added
- **Windows platform support**: Full support for Windows build and release (thanks @jrtxio [PR#6](https://github.com/qufei1993/skills-hub/pull/6)).
- Support and display for many new tools (e.g., Kimi Code CLI, Augment, OpenClaw, Cline, CodeBuddy, Command Code, Continue, Crush, Junie, iFlow CLI, Kiro CLI, Kode, MCPJam, Mistral Vibe, Mux, OpenClaude IDE, OpenHands, Pi, Qoder, Qwen Code, Trae/Trae CN, Zencoder, Neovate, Pochi, AdaL).
- UI confirmation and linked selection for tools that share the same global skills directory.
- Local import multi-skill discovery aligned with Git rules, with a selection list and invalid-item reasons.
- New local import commands for listing candidates and installing a selected subpath with SKILL.md validation.

### Changed
- Antigravity global skills directory updated to `~/.gemini/antigravity/global_skills`.
- OpenCode global skills directory corrected to `~/.config/opencode/skills`.
- Tool status now includes `skills_dir`; frontend tool list/sync is driven by backend data and deduped by directory.
- Sync/unsync now updates records across tools sharing a skills directory to avoid duplicate filesystem work and inconsistent state.
- Local import flow now scans candidates first; single valid candidate installs directly, multi-candidate opens selection.

## [0.1.1] - 2026-01-26

### Changed
- GitHub Actions release workflow for macOS packaging and uploading `updater.json` (`.github/workflows/release.yml`).
- Cursor sync now always uses directory copy due to Cursor not following symlinks when discovering skills: https://forum.cursor.com/t/cursor-doesnt-follow-symlinks-to-discover-skills/149693/4
- Managed skill update now re-syncs copy-mode targets using copy-only overwrite, and forces Cursor targets to copy to avoid accidental relinking.

## [0.1.0] - 2026-01-25

### Added
- Initial release of Skills Hub desktop app (Tauri + React).
- Central repository for Skills; sync to multiple AI coding tools (symlink/junction preferred, copy fallback).
- Local import from folders.
- Git import via repository URL or folder URL (`/tree/<branch>/<path>`), with multi-skill selection and batch install.
- Sync and update: copy-mode targets can be refreshed; managed skills can be updated from source.
- Migration intake: scan existing tool directories, import into central repo, and one‑click sync.
- New tool detection and optional sync.
- Basic settings: storage path, language, and theme.
- Git cache with cleanup (days) and freshness window (seconds).

### Build & Release
- Local packaging scripts for macOS (dmg), Windows (msi/nsis), Linux (deb/appimage).
- GitHub Actions build validation and tag-based draft releases (release notes pulled from `CHANGELOG.md`).

### Performance
- Git import and batch install optimizations: cached clones reduce repeated fetches; timeouts and non‑interactive git improve stability.

[Unreleased]: https://github.com/mcncarl/skills-hub/commits/main
[0.9.1]: https://github.com/qufei1993/skills-hub/compare/v0.9.0...v0.9.1
[0.9.0]: https://github.com/qufei1993/skills-hub/compare/v0.8.1...v0.9.0
[0.8.1]: https://github.com/qufei1993/skills-hub/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/qufei1993/skills-hub/compare/v0.7.1...v0.8.0
[0.7.1]: https://github.com/qufei1993/skills-hub/compare/v0.7.0...v0.7.1
