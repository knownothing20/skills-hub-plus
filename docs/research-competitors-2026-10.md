# 竞品调研与功能借鉴分析(2026-10)

> 调研范围:GitHub 上 Agent Skills 管理/同步类产品及相邻生态。目的:为 Skills Hub Plus 确定差异化定位与功能路线图。本文含两部分:调研结论 + 对照本仓库代码的落地性分析。

## 一、竞品概览

| 产品 | 星数 | 定位 | 与本项目的重叠 |
|---|---|---|---|
| [vercel-labs/skills](https://github.com/vercel-labs/skills)(+ [skills.sh](https://skills.sh)) | ~31.5k | 事实标准:CLI + 目录网站,75+ agent | 安装/更新/多工具写入 |
| [xingkongliang/skills-manager](https://github.com/xingkongliang/skills-manager) | ~6.8k | 最直接竞品,同为 Tauri+React+Rust+SQLite | 核心 sync/备份/版本追踪几乎全重合 |
| [qufei1993/skills-hub](https://github.com/qufei1993/skills-hub)(本项目上游) | ~1.8k | 概念同源 | 100%(见《值得借鉴的上游功能》) |
| [crossoverJie/SkillDeck](https://github.com/crossoverJie/SkillDeck) | ~588 | macOS 原生,主打用量统计 | 同步/分配;用量洞察不重叠 |
| [anthropics/skills](https://github.com/anthropics/skills) / [obra/superpowers](https://github.com/obra/superpowers) | 184k / 305k | 官方库与知名框架 | 生态锚点,非竞品 |

**赛道结论**:桌面 GUI + 中央库 + 多工具同步在 2026 年已是标配;差异化只能靠**后处理能力**(健康度、用量、发现、清单复现)。

**本项目独有卖点**:**反向提升**(在工具里改好的 skill 一键推回母版)——目前无任何竞品实现,应当放大宣传。

## 二、借鉴功能落地性分析(对照本仓库代码)

### F1. 技能卡片健康度徽章 + 断链检测 ⭐ 推荐先做

- **来源**:skills-manager 的状态徽章;Skills Desktop 的断链修复
- **现有基础**:`tool_adapters/mod.rs` 的 `DetectedSkill` 已含 `is_link` / `link_target` 字段(detect_link 已实现);`getSkillSyncState` / `getToolSyncState` 已驱动 UI 状态
- **差距**:①按 skill 聚合各目标工具的链接有效性(link_target 是否仍解析到母版);②顶部全库健康摘要(断链数/过期数/未分配数);③闲置检测(90 天未动且未分配任何工具)
- **工程量:小**(Rust 加一个聚合扫描命令 + 前端徽章组件)
- **顺带实现 F5 简化版**(闲置清理建议)

### F2. 内置发现页接入 skills.sh 生态

- **来源**:vercel skills 的排行榜(All Time / Trending 24h / Hot);SkillDeck 的 Registry 浏览器
- **现有基础**:ExplorePage + `featured_skills.rs` 的"远端 JSON + 编译期内置兜底"框架已就绪——但目前数据源是 mcncarl 仓库的 featured-skills.json(别人的数据,更新依赖上游)
- **差距**:①调研 skills.sh 公开 API(需确认端点与稳定性);②新增数据源适配;③条目"一键装入 Hub"(git 稀疏拉取 SKILL.md 到母版,再走既有同步——与现有 Git 导入共用通道)
- **工程量:中**。第一步可只换数据源,一键安装第二步做
- **收益**:发现入口是竞品中增长最强的功能,且摆脱对上游数据源的依赖

### F3. skills.lock 清单导入导出(多机/团队复现)

- **来源**:vercel Packs、Homebrew Brewfile、Scoop export
- **现有基础**:`backup/manifest.json` 已记录每个 skill 的 name/enabled/tags/source_ref/targets(tool+path+scope)——**结构上已是 lock 文件的八成**
- **差距**:①补来源版本信息(git commit/tag,若无则记 content hash);②独立的"导出清单 / 按清单导入"命令(导入可大量复用备份恢复逻辑);③导入时的漂移报告(本地有而清单没有 → 建议,不自动删)
- **工程量:小-中**
- **收益**:与"拷文件夹迁移"互补——文件夹拷贝带全部内容,lock 清单适合"新机器只装想要的"和团队共享

### F4. 操作历史 + 一键撤销 + 工具能力降级矩阵

- **来源**:skills-manager 的 Undo/Ref 降级;上游 qufei1993 的"Cursor 不支持 symlink 自动降级"
- **现有基础**:降级逻辑已有硬编码特判(sync_engine.rs 中 Cursor 强制 copy);备份体系可承接"覆盖前快照"
- **差距**:①SQLite 操作日志表(时间/动作/源/目标/备份指针);②撤销命令(逆操作 + 从备份还原);③能力矩阵显性化(每工具一份能力描述,UI 标注降级原因)
- **工程量:中**
- **收益**:把同步从"一次性动作"升级为可审计、可反悔的操作流;现有"反向提升 + 备份"体系与之天然衔接

### F5. 用量洞察(先简化版)

- **来源**:SkillDeck Usage Insights(唯一做"用后数据"的竞品)
- **简化版**:F1 的闲置检测即覆盖(启用工具数 + 最近修改时间 + 90 天阈值)
- **完整版**:Claude Code hook 上报调用事件 → 本地聚合调用次数/热度排序。**工程量大**,等简化版验证需求后再议

## 三、合并路线图(含上游借鉴项)

| 优先级 | 事项 | 来源 | 工程量 |
|---|---|---|---|
| P0 | F1 健康度徽章 + 闲置检测(含 F5 简化版) | skills-manager / SkillDeck | 小 |
| P0 | F3 skills.lock 导入导出 | vercel / Homebrew | 小-中 |
| P1 | 多设备母版库同步 | 上游 v0.10.0 | 大 |
| P1 | F2 发现页接入 skills.sh(先换数据源) | vercel / SkillDeck | 中 |
| P2 | F4 操作历史与撤销 | skills-manager | 中 |
| P2 | 批量更新所选技能 / 状态与工具过滤 | 上游 v1.0.0 / v1.0.1 | 中 |
| P3 | 应用内回收站(30 天恢复) | 上游 v0.10.0 | 中 |
| P3 | F5 用量洞察完整版(hook 埋点) | SkillDeck | 大 |

> 排序原则:先做"小工程量 × 高频日常体验"(F1/F3),再做"根治迁移"(多设备同步),发现与审计随后。

## 四、差异化定位声明

1. **反向提升是独有卖点**——竞品全部是单向(库→工具),唯本项目支持把工具侧改进安全推回母版。
2. **便携自包含**——程序+母版同目录,60MB 拷走即用;竞品均依赖用户目录中央库。
3. **后处理能力矩阵**(健康度/用量/清单)是赛道公认空白,借 F1/F3/F5 补齐后即形成完整闭环。
