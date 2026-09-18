# config（应用配置跨端契约类型）

> 源文件：`src/types/config.ts`

## 职责

应用配置的跨端契约类型，与 Rust `core::entity::config::AppConfig`（camelCase 序列化）对齐；前端只读消费。

## 核心类型/函数

| 名称 | 形状 | 关键语义 |
|------|------|----------|
| `AppConfig` | `{ user, oj, editor, theme, layout }` | 五组配置 |
| `UserConfig` | `{ lastUsername }` | 登录用户名（自动填充）。旧 `lastOjType` 已移除：OJ 选择是应用级状态，迁移至 `oj.active`（Rust 加载路径归一） |
| `OjInstance` | `{ id, baseUrl, enabled, options: Record<string, unknown> }` | 单个 OJ 实例的连接配置（对应 Rust `OjInstance`）：`id` 与适配器工厂 id 一致（如 "HOJ"）；`baseUrl` 为站点根（不带 `/api` 等前缀）；`enabled = false` 时后端不注册该实例的 Provider；`options` 为 OJ 私有旋钮（弱类型，值域由各 OJ 自行约定） |
| `OjConfig` | `{ active, instances, contestRef, contestPassword: string \| null, timeoutSecs, pollIntervalSecs, pollTimeoutSecs, cacheTtlSecs, cacheProblemStatement: boolean }` | `active` 为当前 OJ 实例 id（须指向 instances 中一条）；时长字段一律**秒**；`contestRef` 为**不透明字符串**的比赛引用（HOJ 为数字串，其它 OJ 可能是任意资源 ID），**空串 = 未配置**（简报链路据此走 `unconfigured` 态，后端 `load_configured_contest` 据此报错提示配置）；`contestPassword` 私有赛用，公开赛为 null；`cacheProblemStatement` 为题面缓存开关（Rust 侧默认 `true`，前端回填时 `?? true` 兜底旧配置） |
| `EditorConfig` | `{ fontSize, tabSize, autoSave, autoSaveIntervalSecs, defaultLanguage }` | autoSaveIntervalSecs 秒；`defaultLanguage` 值域为 **HOJ 显示名**（默认 "C++"，与提交契约同源；历史 Monaco id 由 Rust `normalize_language_display_name` 与前端 `normalizeHojLanguage` 双向兜底归一），消费方为 workspaceStore 新建工作区的默认语言 |
| `ThemeConfig` | `{ themeName, editorTheme }` | Rust 默认 dark / vs-dark；当前前端固定浅色（App.vue `:theme="null"`、Monaco `theme: 'vs'`），字段预留 |
| `LayoutConfig` | `{ sidebarWidth, splitRatio }` | 像素 / 0–1 比例；当前前端未消费（分栏比例由 ProblemSolveView 本地 state 管理），字段预留 |

## 直接依赖

无（纯类型声明文件）

## 被依赖

- `bridge/config.bridge.ts`、`services/config.service.ts`（派生 `getOjBaseUrl` / `activeOjBaseUrl` / `switchOj` / `getPollSchedule`）、`stores/submissionStore.ts` 与 `views/LoginView.vue`（间接经 config.service）、`views/SettingsView.vue`（OJ 实例表单）——均仅类型引用

## 逻辑流程

无（纯类型定义）。

设计要点：

- 消费一律经 `services/config.service.ts`（进程内缓存 Promise + 兜底值），View/Store
  不直接调 config.bridge；兜底值与 Rust 默认值保持一致（poll 2s/300s 等）。
- 前端实际消费的子集：`oj.instances` + `oj.active`（题面图片基址/简报经
  `configService.activeOjBaseUrl` 解析当前实例地址；设置页「当前 OJ」下拉候选与选中值）、
  `oj.contestRef`（简报）、`oj.pollIntervalSecs`/`pollTimeoutSecs`（评测轮询）。
