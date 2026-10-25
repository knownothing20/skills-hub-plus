# Skills Hub Plus

跨平台桌面应用(Tauri + React):把所有 **Agent Skill 集中在一个母版库**里管理,并同步到本机每一个 AI 编程工具——*"装一次,处处同步"*。

> 血缘说明:基于 [qufei1993/skills-hub](https://github.com/qufei1993/skills-hub)(MIT 协议,经 mcncarl/skills-hub)的独立分支,现以 **Skills Hub Plus** 名义继续维护。
>
> English readme:[`../README.md`](../README.md)

## 截图

| 我的技能 | 工具管理 |
|:---:|:---:|
| ![我的技能卡片视图](assets/my-skills-card-view.png) | ![工具管理](assets/tools-management.png) |
| **探索在线技能** | **设置** |
| ![探索在线技能](assets/explore-online-skills.png) | ![设置](assets/settings-app-preferences.png) |

## 功能

- **集中母版库** — 所有 skill 只存一份,放在程序旁边的 `skills/` 文件夹,不再散落在各工具目录里。
- **多工具同步** — 优先用 NTFS junction / 软链接安装到各工具目录,工具不支持链接时(如 Cursor)自动退回物理复制。支持 Claude Code、Codex、Cursor、OpenCode、WorkBuddy、ZCode 等。
- **任意端编辑,全局即时生效** — junction/软链模式下,在任何 AI 工具里改 skill 就是直接改母版,其他所有工具零操作即时同步。
- **工具三态模型** — 🎨 原生直接可用(零拷贝)/ 🟢 专属物理安装(绿环)/ ⚪ 未同步(一键同步)。
- **多端版本比对与反向提升** — 按 mtime 和文件树比对每个工具与母版的差异,工具里改好的内容一键"设为母版"推回母版库,`.skillignore` 自动过滤私密文件。
- **备份与一键恢复** — 一键全量备份(清单 + SQLite 数据库 + skill 快照),破坏性操作前自动增量备份,可恢复到任意历史快照。
- **定时自动更新** — git 来源的 skill 按计划自动更新。
- **云端编译** — GitHub Actions 手动触发即可在云端编出 Windows x64 便携版 exe,本地无需装 Rust 工具链(见下文"构建")。

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

新电脑上:整个文件夹拷过去,双击 `dist-app\skills-hub.exe` 即可——首次启动自动"收养"母版库里的全部 skill,然后逐个工具点一次同步重建链接(junction 是每台机器各自的,需要重建)。**运行程序完全不需要** `node_modules/` 和 `src-tauri/target/`。

## 构建

- **云端编译(推荐)**:GitHub → **Actions → Build Windows (manual)** → *Run workflow* → 从运行页面下载 `release-assets-x64`。里面是便携版 `*-portable.exe`(可直接替换 `dist-app\skills-hub.exe`)和 NSIS 安装包。公开仓库标准 Runner 免费,缓存命中后约 3 分钟出结果。
- **本地编译**:`npm install` 后执行 `npm run tauri:build`(或 `build_release.ps1`)。需要 Node 20+、Rust/MSVC 环境,编译缓存约占 14 GB 硬盘——这正是推荐云端编译的原因。

## 文档

- English readme:[`../README.md`](../README.md)
- 更新日志:[`../CHANGELOG.md`](../CHANGELOG.md) / [`CHANGELOG.zh.md`](CHANGELOG.zh.md)
- UI 设计规范:[`UI-DESIGN-GUIDELINES.zh.md`](UI-DESIGN-GUIDELINES.zh.md)

## 许可

MIT — 见 [LICENSE](../LICENSE),保留上游出处说明。
