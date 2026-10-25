# Skills Hub Plus

跨平台桌面应用(Tauri + React):把所有 **Agent Skill 集中在一个母版库**里管理,并同步到本机每一个 AI 编程工具——*"装一次,处处同步"*。

> **📍 本项目基于开源项目衍生** — 衍生自 **[qufei1993/skills-hub](https://github.com/qufei1993/skills-hub)**(MIT 协议,基线 `v0.9.1`,经 [mcncarl/skills-hub](https://github.com/mcncarl/skills-hub) 中转),现以 **Skills Hub Plus** 名义独立演进。核心同步架构、工具适配器与界面基础均来自上游,感谢上游作者的出色工作 👏——完整致谢见[文末](#致谢与许可)。上游仍在活跃开发(2026-10 已到 v1.0.1),差异见[《与上游的差异》](#与上游的差异),上游领先的部分见[《值得借鉴的上游功能》](#值得借鉴的上游功能)。
>
> English readme:[`../README.md`](../README.md)

## 截图

| 我的技能 | 批量操作 | 工具管理 |
|:---:|:---:|:---:|
| ![我的技能卡片视图](assets/my-skills-card-view.png) | ![批量操作](assets/my-skills-list-bulk-actions.png) | ![工具管理](assets/tools-management.png) |

| 探索在线技能 | 从 Git 添加 | 定时更新 |
|:---:|:---:|:---:|
| ![探索在线技能](assets/explore-online-skills.png) | ![从 Git 仓库添加技能](assets/add-skill-git-repository.png) | ![定时更新](assets/updates-scheduled-run.png) |

## 与上游的差异

本分支在上游 `v0.9.1` 基础上做出的改进:

| # | 改进 | 说明 |
|---|---|---|
| 1 | **反向"设为母版"** | 按 mtime + 文件树比对各工具与母版库的差异;在工具里改好的内容一键推回母版,`.skillignore` 自动过滤。上游只能单向(母版→工具)推送 |
| 2 | **工具三态模型** | 🎨 原生直接可用(零拷贝)/ 🟢 专属安装 / ⚪ 未同步;清除了幽灵工具,状态一目了然 |
| 3 | **便携式母版库** | 母版库放在程序旁的 `skills/` 目录(而非 `~/.agents/skills`),整个应用就是约 60 MB 的自包含文件夹,拷到别的电脑直接能用(见[迁移指南](#便携布局与迁移到其他电脑)) |
| 4 | **一键备份与恢复** | 全量备份(清单 + SQLite 数据库 + skill 快照);破坏性操作前自动增量备份;可恢复到任意历史快照 |
| 5 | **母版库安全守卫** | 安全 unsync 绝不误删母版文件;Cline 使用隔离的 `.cline/skills` 目录,不再劫持中央母版(修复 `UNSAFE_PATH` 崩溃) |
| 6 | **启动提速防卡顿** | 收养与描述回填移到后台线程;跳过对已有 skill 的冗余 SHA256 哈希 |
| 7 | **云端 CI 编译** | GitHub Actions 手动触发即可在云端编出 Windows x64 便携版 exe——本地零工具链、零 14 GB 编译缓存 |
| 8 | **隐私审计闸门** | `npm run check:privacy` 在每次发布构建前扫描跟踪文件、提交邮箱与凭据特征 |
| 9 | **架构 2.0** | 前端拆成 16 个领域 hook(`App.tsx` 减少 60%),后端 IPC 拆成 8 个模块,110 个单元测试 |
| 10 | **仅回收站删除** | 删除一律走系统回收站;skill 拥有自己的图标元数据 |

## 值得借鉴的上游功能

上游自我们的基线后已发布到 v1.0.1,以下是我们关注的差距,按对本分支的价值排序:

| 功能 | 上游版本起 | 对我们的意义 |
|---|---|---|
| **多设备母版库同步**(GitHub/GitLab/Gitee 远程仓库 + 三方合并) | v0.10.0 | 从根上解决"把 skill 带到其他电脑"——目前我们靠拷文件夹。**首选借鉴项** |
| **应用内回收站**(30 天可恢复,连配置一起还原) | v0.10.0 | 我们目前只删进系统回收站;应用内回收站更可控、可发现 |
| **合集 Collections**(浏览并一键安装 skills.sh 上的技能合集) | v1.0.0 | 一次装整套策划好的 skill 集 |
| **批量更新所选技能**(逐条进度、可重试) | v1.0.0 | 我们能批量同步,但还不能一次性按各自 git/local 源重拉更新 |
| **按状态/工具过滤**技能库 | v1.0.1 | 按启用状态、已同步工具筛选 |
| **Windows junction `\\?\` 规范路径修复** | v1.0.1 | 上游小修复,与我们"junction 优先"策略直接相关 |
| AI 技能管理 + 独立 CLI | v0.11.0 | 用 AI 对话管理技能;工程量大,低优先级 |
| Linux `.deb` / `.AppImage` 安装包 | v0.11.0 | 超出范围——本分支以 Windows 为主 |

## 功能总览

- **集中母版库** — 所有 skill 只存一份,放在程序旁的 `skills/` 文件夹
- **多工具同步** — 优先 junction/软链接,不支持的自动物理复制(如 Cursor);内置 50+ 工具适配器(Claude Code、Codex、Cursor、OpenCode、WorkBuddy、ZCode 等)
- **任意端编辑,全局即时生效** — junction 模式下,在任何 AI 工具里改 skill 就是直接改母版
- 版本比对与反向提升、备份与恢复、定时自动更新、云端编译——见上文专述

## 便携布局与迁移到其他电脑

整个应用拷走只需 **约 60 MB**:

```
skill-hub/            (仓库文件夹,放哪个盘都行)
├── dist-app/
│   └── skills-hub.exe   ← 程序本体(界面已内嵌,自包含)
├── skills/              ← 母版库,所有 skill 在这里
├── package.json / .git  ← 自动定位 skills/ 用的锚点
└── backup/              ← 可选:标签与配置的备份快照
```

新电脑上:整个文件夹拷过去,双击 `dist-app\skills-hub.exe`——首次启动自动"收养"母版库里的全部 skill,然后逐个工具点一次同步重建链接。**运行程序完全不需要** `node_modules/` 和 `src-tauri/target/`。

## 构建

- **云端编译(推荐)**:GitHub → **Actions → Build Windows (manual)** → *Run workflow* → 下载 `release-assets-x64`(便携版 exe + NSIS 安装包)。公开仓库免费,缓存命中后约 3 分钟。**按需触发,避免不必要的运行**。
- **本地编译**:`npm install` 后执行 `npm run tauri:build`(或 `build_release.ps1`)。需要 Node 20+、Rust/MSVC,编译缓存约占 14 GB 硬盘。

## 文档

- English readme:[`../README.md`](../README.md)
- 更新日志:[`../CHANGELOG.md`](../CHANGELOG.md) / [`CHANGELOG.zh.md`](CHANGELOG.zh.md)
- UI 设计规范:[`UI-DESIGN-GUIDELINES.zh.md`](UI-DESIGN-GUIDELINES.zh.md)

## 致谢与许可

- **[qufei1993/skills-hub](https://github.com/qufei1993/skills-hub)** — 本项目赖以衍生的原项目:核心同步架构、50+ 工具适配器、界面基础均来自上游。🙏
- [mcncarl/skills-hub](https://github.com/mcncarl/skills-hub) — 本代码线经过的中间分支。

MIT — 见 [LICENSE](../LICENSE),保留上游出处说明。
