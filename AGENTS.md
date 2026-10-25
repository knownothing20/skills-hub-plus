# Skills Hub Plus — 开发规范(AI Agent 必读)

本文件约束所有 AI 编码代理与协作者在本仓库的开发行为。动手前先通读;与旧习惯冲突时,**以本文件为准**。

## 🚫 硬性红线(违反即事故)

1. **禁止本地编译出包**:不运行 `npm run tauri:dev` / `tauri build` / `build_release*`。程序一律由 GitHub Actions 云端编译(见下文流程)。`src-tauri\target\` 保持删除状态,谁生成谁负责删干净。
2. **禁止移动/重命名**:`skills\`(母版库,几十个工具 junction 指向它)、`dist-app\`(程序路径解析依赖相对位置)、`package.json`/`.git`(exe 定位母版库的锚点)。根目录其余文件的"位置"也基本是工具锚点,不做目录美化。
3. **禁止触碰私密目录内容**:`skills\`、`backup\`、`release\` 被 gitignore,是用户数据,任何代码/脚本/文档不得使其进入 git。
4. **GitHub Actions 节约纪律**:不频繁触发构建。日常推送让 CI 自动校验;**只有需要新版 exe 时才手动跑一次 Build Windows**;纯文档/脚本类推送用"禁用 CI → push → 启用 CI"避免无效运行:
   ```bash
   gh workflow disable "CI" && git push origin main && gh workflow enable "CI"
   ```
5. **git 身份**:全部提交署名 `knownothing20 <137740968+knownothing20@users.noreply.github.com>`(已配置,勿改)。

## ✅ 标准开发流程(唯一正确流程)

```
1. 开发     在本地改代码(前端 + Rust)
2. 本地验证 ① npx tsc -b            → 0 错误
           ② npx vitest run        → 全部通过(不许减少既有测试)
           ③ 改了 Rust → dev\verify-rust.bat(check+fmt+测试编译,自动清缓存)
3. 提交推送 git add/commit/push(信息格式见下文)
4. 线上编译 【仅当需要新 exe】GitHub → Actions → Build Windows (manual) → Run workflow
5. 更新程序 下载产物 release-assets-x64 →
           portable.exe 覆盖 dist-app\skills-hub.exe(先退出运行中的程序)→
           旧版挪入 release\<日期或版本>\ 归档 → 根目录快捷方式自动指向最新
```

本地验证的 Rust 环境注意:必须在 MSVC 环境下跑 cargo(脚本已处理);直接在 Git Bash 裸跑 cargo 会因 GNU `link` 抢占链接器而报假错。Windows 本机**无法执行** Rust 测试二进制(上游已知 `STATUS_ENTRYPOINT_NOT_FOUND` 问题),测试执行交给 CI 的 Linux 环境,本地只做编译验证。

## 📁 目录布局

| 区域 | 内容 | 规则 |
|---|---|---|
| 程序区 | `dist-app\`(当前程序)`skills\`(母版库)`backup\`\ `release\`(构建归档) | 位置锁定;后三者 gitignored |
| 开发区 | `dev\`(辅助/验证脚本)`src\` `src-tauri\` `public\` `scripts\`(npm 脚本)`node_modules\` `dist\` | 正常开发 |
| 文档区 | `README.md` `CHANGELOG.md` `docs\` `LICENSE` + `AGENTS.md`/`CLAUDE.md` | 平台锚点,留根目录 |
| 配置区 | `.github\`(workflows + 社区文档)`\.gitignore` 构建配置若干 | 锚点文件勿动 |

## 🏗 架构要点(改动前必知)

- **Tauri 2 + React 19**;前端↔后端经 Tauri IPC:前端 `invokeTauri<T>('cmd')`(src/hooks/useTauriInvoke.ts),命令定义在 `src-tauri/src/commands/*.rs`(8 个领域子模块),**新命令需两处注册**:commands 模块导出 + `lib.rs` 的 invoke_handler。
- **前端 hooks 架构**:App.tsx 已拆为 16 个领域 hook(src/hooks/*)。新功能优先做成新 hook,不往 App.tsx 塞大块逻辑;**hook 调用顺序有依赖关系**(如 useTagOperations 产出 loadTags 供下游用),调整顺序前先理依赖,顺序错误会 TS2448 编译错。
- **后端分层**:`commands/`(DTO 转换、无业务)→ `core/`(纯业务,可独立测试);异步命令用 `spawn_blocking`;错误 `anyhow::Result` + `format_anyhow_error()`。
- **i18n**:文案一律 `t('key')`;语言文件在 `src/i18n/locales/en.ts` 与 `zh.ts`,**新增 key 两个文件必须同步**(有 resources 数量一致性测试盯着)。
- **DTO 双端同步**:Rust DTO(commands/)与 TS DTO(src/components/skills/types.ts)字段必须一致。
- 工具适配器注册表在 `core/tool_adapters/mod.rs`(50+ 工具);母版库路径逻辑在 `core/central_repo.rs`(exe 相对定位,这就是 dist-app\ 不能挪的原因)。
- 版本号三处同步:`package.json` / `src-tauri/tauri.conf.json` / `Cargo.toml`(`npm run version:check`)。

## 📐 编码规范(摘要)

- TypeScript 严格模式,`noUnusedLocals/Parameters` 全开——未使用即编译错;组件 PascalCase,props 类型 `XxxProps`;样式进 `App.css`/`figma.css`,语义类名 kebab-case。
- Rust:snake_case;新 core 模块记得 `core/mod.rs` 导出;测试用 `tempfile` 造数据,不依赖 symlink(跨平台);提交前 `cargo fmt --all`。
- 改 UI 前先读 `docs/UI-DESIGN-GUIDELINES.md`(仅界面任务需要)。
- 只做加法与必要修改,不顺手重构无关代码。

## 📝 提交与发布

- 提交信息:`type(scope): 摘要`,type ∈ feat/fix/docs/chore/style/refactor;一段正文说明动机与非显而易见的决策。
- 发布前列表:`npm run check:privacy`(隐私审计,CI 与 build 脚本也会跑)。
- 云端构建产物:`Skills-Hub-Plus-<ref>-Windows-x64-portable.exe`(裸 exe,替换 dist-app 用)与 `-setup.exe`(NSIS 安装包)。
