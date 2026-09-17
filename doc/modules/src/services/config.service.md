# config.service（前端配置服务）

> 源文件：`src/services/config.service.ts`

## 职责

前端读取 Rust 端 `AppConfig` 的唯一入口，并派生出各层需要的展示/调度参数，屏蔽 IPC 细节与配置字段兜底逻辑。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `PollSchedule` | `{ intervalMs: number; timeoutMs: number }` | 评测轮询调度参数 |
| `EditorPrefs` | `{ fontSize: number; tabSize: number; editorTheme: string }` | 编辑器偏好（字号 / Tab 宽度 / Monaco 主题） |
| `ConfigService.getConfig` | `() => Promise<AppConfig>` | 读取配置（进程内缓存，并发共享同一次 IPC） |
| `ConfigService.invalidate` | `() => void` | 使缓存失效（设置页保存后 / 需要磁盘真值时） |
| `ConfigService.updateConfig` | `(mutate: (draft: AppConfig) => void) => Promise<AppConfig>` | **设置页保存唯一入口**：读当前配置 → structuredClone 副本上应用变更 → 整体写回后端（`update_config` 是整体替换语义）→ 失效缓存；写回失败同样失效缓存（避免缓存与磁盘漂移）并抛出 |
| `ConfigService.getOjBaseUrl` | `() => Promise<string>` | OJ 基址，用于题面/简介/公告相对图片 URL 改写；失败返回空串 |
| `ConfigService.getPollSchedule` | `() => Promise<PollSchedule>` | 轮询间隔与总超时；失败或非法配置回退 2s / 300s |
| `ConfigService.getEditorPrefs` | `() => Promise<EditorPrefs>` | 字号 / Tab 宽度 / 编辑器主题（钳位上下界取自 `utils/editor`，越界与未知主题回退 14 / 4 / `'vs'`）；CodeEditor 挂载时消费 |
| `ConfigService.updateEditorPrefs` | `(patch: Partial<EditorPrefs>) => Promise<void>` | **解题页编辑器设置弹层落盘入口**：只写传入字段（读-改-写保留其余配置）；主题落 `theme.editorTheme` 且**不触碰 `theme.themeName`**，落盘前经 `normalizeEditorTheme` 归一 |
| `ConfigService.getDefaultLanguage` | `() => Promise<string>` | 默认语言（**HOJ 显示名**，如 "C++"）；经 `normalizeHojLanguage` 归一，历史配置遗留的 Monaco id（'cpp'）映射回显示名，空值回退 "C++"，其它非空值（Go/Rust…）原样保留 |
| `ConfigService.getSplitRatio` | `() => Promise<number>` | 解题页初始分栏比例（非法回退 0.48） |
| `normalizeLanguageId` | `(raw: string \| undefined \| null) => string` | 显示名/大小写变体 → Monaco language id；无法识别回退 `'cpp'`（P55：保证喂给 Monaco 的恒为合法值） |
| `configService` | 单例 | 全局唯一实例 |

## 直接依赖

- `@/bridge/config.bridge`（`get_config` / `update_config`）
- `@/types/config`（仅类型）
- `@/utils/language`（`normalizeHojLanguage`）、`@/utils/editor`（偏好默认值、钳位上下界与主题归一）
- `@/utils/logger`（`createLogger` —— 作用域日志）

## 被依赖

- `services/contest.service.ts` — 登录页匿名简报需要 `contestId` 与 OJ 基址
- `stores/submissionStore.ts` — `startPolling()` 读取轮询调度参数
- `stores/workspaceStore.ts` — `loadWorkspace()` 读取默认语言
- `stores/announcementStore.ts`（经 announcement.service 间接）
- `components/problem/ProblemStatement.vue` — 题面图片基址
- `components/editor/CodeEditor.vue` — 字号 / Tab 宽度 / 编辑器主题（读 + 弹层改动落盘）
- `views/ProblemSolveView.vue` — 初始分栏比例读取与拖拽回写
- `views/SettingsView.vue` — 配置读取与保存

## 逻辑流程

```
getConfig()
  ├─ 缓存命中 → 直接返回同一 Promise（并发去重）
  └─ 未命中   → config.bridge.getConfig()
                 ├─ 成功 → 写入缓存
                 └─ 失败 → 清空缓存（下次调用重试）并抛出

updateConfig(mutate)
  getConfig() → structuredClone 副本 → mutate(draft)
  → bridge.updateConfig(draft)（整体替换）
  → finally invalidate()（成败都失效，防缓存与磁盘漂移）

updateEditorPrefs(patch)
  → updateConfig(draft => 只写 patch 中出现的字段)
      fontSize/tabSize → editor.*；editorTheme → theme.editorTheme（先 normalizeEditorTheme）

getOjBaseUrl() / getPollSchedule() / getEditorPrefs() / getDefaultLanguage() / getSplitRatio()
  └─ 内部吞掉异常并返回安全兜底值：派生参数缺失只影响局部展示/节奏，不阻断主流程
```

设计要点：

- **分层约定**：View / Store 不得直接调用 `config.bridge`，配置读写一律经本服务。
- **不引入 store**：缓存 Promise 而非结果，使多处调用共享一次 IPC；设置页保存经
  `updateConfig` 写后端 + 失效缓存，其余读取方下次调用即拿到新值。
- **P55 值域统一（后经语言权威值改造修订）**：`defaultLanguage` 值域为 **HOJ 显示名**
  （与提交契约同源，见 `utils/language`）；历史 Monaco id 由前后端各自的归一函数
  映射回显示名（前端 `normalizeHojLanguage`、Rust `normalize_language_display_name`）。
  Rust 端默认值 `light` / `vs` / `0.48` 不变。
- 兜底值与 Rust `core::entity::config` 的默认值保持一致（`poll_interval=2s`、`poll_timeout=300s`、字号 14、Tab 4、编辑器主题 `vs`、分栏 0.48）。
- **编辑器偏好值域归 `utils/editor`**：字号 8–32、Tab 1–8（候选档位 2/4/8）、主题 `vs`/`vs-dark` ——
  本服务只取该模块常量做钳位，不复制字面量。
  `updateEditorPrefs` 落盘前对主题做归一，未知主题名不会写进配置文件（Rust 端 `sanitize`
  还会再收敛一次）；**部分写语义**：只写传入字段，未传入的偏好保持磁盘原值。

## 测试

`src/services/__tests__/config.service.spec.ts`：normalizeLanguageId 全表、缓存并发去重与失败重试、派生参数透传/越界回退/读取失败兜底（含 `editorTheme` 未知值回退 `vs`）、updateConfig 整体写回 + 缓存失效 + 副本隔离 + 失败仍失效缓存、updateEditorPrefs 只写传入字段 + 主题落 `theme.editorTheme` 且不动 `themeName` + 非法主题名归一后落盘。
