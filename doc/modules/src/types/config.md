# config（应用配置跨端契约类型）

> 源文件：`src/types/config.ts`

## 职责

应用配置的跨端契约类型，与 Rust `core::entity::config::AppConfig`（camelCase 序列化）对齐；前端只读消费。

## 核心类型/函数

| 名称 | 形状 | 关键语义 |
|------|------|----------|
| `AppConfig` | `{ user, oj, editor, theme, layout }` | 五组配置 |
| `UserConfig` | `{ lastOjType, lastUsername }` | 上次登录的 OJ 类型与用户名（自动填充） |
| `OjConfig` | `{ hojUrl, timeoutSecs, pollIntervalSecs, pollTimeoutSecs, cacheTtlSecs, cacheProblemStatement: boolean, contestId: number, contestPassword: string \| null }` | 时长字段一律**秒**；`contestId` 为数字，**0 = 未配置**（简报链路据此走 `unconfigured` 态，后端 `load_configured_contest` 据此报错提示配置）；`contestPassword` 私有赛用，公开赛为 null；`cacheProblemStatement` 为题面缓存开关（Rust 侧默认 `true`，前端回填时 `?? true` 兜底旧配置） |
| `EditorConfig` | `{ fontSize, tabSize, autoSave, autoSaveIntervalSecs, defaultLanguage }` | autoSaveIntervalSecs 秒；`defaultLanguage` 值域为 **HOJ 显示名**（默认 "C++"，与提交契约同源；历史 Monaco id 由 Rust `normalize_language_display_name` 与前端 `normalizeHojLanguage` 双向兜底归一），消费方为 workspaceStore 新建工作区的默认语言 |
| `ThemeConfig` | `{ themeName, editorTheme }` | Rust 默认 dark / vs-dark；当前前端固定浅色（App.vue `:theme="null"`、Monaco `theme: 'vs'`），字段预留 |
| `LayoutConfig` | `{ sidebarWidth, splitRatio }` | 像素 / 0–1 比例；当前前端未消费（分栏比例由 ProblemSolveView 本地 state 管理），字段预留 |

## 直接依赖

无（纯类型声明文件）

## 被依赖

- `bridge/config.bridge.ts`、`services/config.service.ts`（派生 `getOjBaseUrl` / `getPollSchedule`）、`stores/submissionStore.ts` 与 `views/LoginView.vue`（间接经 config.service）——均仅类型引用

## 逻辑流程

无（纯类型定义）。

设计要点：

- 消费一律经 `services/config.service.ts`（进程内缓存 Promise + 兜底值），View/Store
  不直接调 config.bridge；兜底值与 Rust 默认值保持一致（poll 2s/300s 等）。
- 前端实际消费的子集：`oj.hojUrl`（图片基址/简报）、`oj.contestId`（简报）、
  `oj.pollIntervalSecs`/`pollTimeoutSecs`（评测轮询）；其余字段为设置界面（未实现）预留。
