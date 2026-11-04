# config.service（前端配置服务）

> 源文件：`src/services/config.service.ts`

## 职责

前端读取 Rust 端 `AppConfig` 的唯一入口，并派生出各层需要的展示/调度参数，屏蔽 IPC 细节与配置字段兜底逻辑。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `PollSchedule` | `{ intervalMs: number; timeoutMs: number }` | 评测轮询调度参数 |
| `EditorPrefs` | `{ fontSize: number; tabSize: number; editorTheme: string }` | 编辑器偏好（字号 / Tab 宽度 / Monaco 主题） |
| `isOjSwitchPersistFailed` | `(error: unknown) => boolean` | 「切换已生效但持久化失败」的**唯一类型化判据**：`error instanceof IpcError && error.variant === 'OjSwitchPersistFailed'`。变体白名单在 `bridge`，裸读 `(e as { variant? }).variant` 会让非 IPC 异常（手写对象、未来包装器）冒充专用变体、把「未生效」误判成「已生效」（2026-11-04 G8）；设置页 `useSettingsForm` 与本服务共用 |
| `ConfigService.getConfig` | `() => Promise<AppConfig>` | 读取配置（进程内缓存，并发共享同一次 IPC） |
| `ConfigService.invalidate` | `() => void` | 使缓存失效（设置页保存后 / 需要磁盘真值时） |
| `ConfigService.updateConfig` | `(mutate: (draft: AppConfig) => void) => Promise<AppConfig>` | **通用保存入口（经保存门禁串行）**：`runSaveTxn(tx => tx.updateConfig(mutate))` 的快捷方式——读当前配置 → structuredClone 副本上应用变更 → 整体写回后端（`update_config` 是整体替换语义）→ **返回后端实际提交的配置**（active 取后端写锁内值）→ 失效缓存；写回失败同样失效缓存（避免缓存与磁盘漂移）并抛出。设置页保存、`updateEditorPrefs`、解题页分栏比例**全部经同一门禁**——并发的读-改-写不交错（丢更新修复，2026-11-04 G4）；复合操作（保存 → 自动切换）用 `runSaveTxn` 整段持锁。「锁定字段不动」由后端按 `launch_args` 自持真值执行，IPC 载荷不携带锁定声明（评审二十六 #2 与 2026-11-04 G1）。**成功写回后按后端实际提交的 active 判定**：若与最近一次 `persist-failed` 切换目标相同，会解除该共享结果（评审三十六：任意保存入口都算回退风险解除；保存与切换交错时不会按旧快照误判） |
| `ConfigService.getOjBaseUrl` | `() => Promise<string>` | OJ 基址，用于题面/简介/公告相对图片 URL 改写；取当前 OJ 实例（`oj.active` 匹配）的 `baseUrl`（经 `activeOjBaseUrl`）；失败返回空串 |
| `ConfigService.activeOjBaseUrl` | `(config: AppConfig) => string` | 同步帮手：解析配置中当前 OJ 实例的服务端地址，供已持有配置的调用方复用。`active` 未命中启用实例列表时回退第一个启用实例（正常配置经 Rust 归一不会未命中，此回退仅兜底 IPC 写读竞态）；无启用实例返回空串 |
| `ConfigService.switchOj` | `(ojId: string) => Promise<void>` | 切换当前 OJ（设置页「当前 OJ」下拉的显式动作）：调 bridge `switchOj`（后端在**单一写事务**内：补注册 → 校验已注册且在启用实例列表 → 切 Registry → 显式清各 Service 的 OJ 域缓存 → 持久化 `oj.active` → 发布 `CoreEvent::OjSwitched`）→ 本地缓存失效（后续读取拿到新 active）。并发调用**排队**执行。失败原样上抛；**经 `isOjSwitchPersistFailed` 判定的持久化失败也先失效缓存**（后端已切 registry/内存，陈旧缓存会把旧 active 写回，评审二十六 P1）。其余失败由调用方按「未切换」处理 |
| `ConfigService.ojSwitchEpoch` | `number`（getter） | 切换代际：开始与结束各 +1；设置页读取配置时据此检测「读取期间发生过切换」并重读（评审三十三 #4） |
| `ConfigService.ojSwitchInFlight` | `boolean`（getter） | 是否有切换在途（跨控制器） |
| `ConfigService.whenOjSwitchSettled` | `() => Promise<void>` | 等待在途切换落地；切换失败也视为落地（不向等待者抛出切换错误） |
| `SaveTxn` | `{ updateConfig(mutate): Promise<AppConfig> }` | 保存事务句柄：`runSaveTxn` 回调内的**唯一**保存通道（直通，不再二次排队）；在回调内调用公共 `updateConfig` 会排队等自己（死锁）（2026-11-04 G4 下沉） |
| `ConfigService.saveEpoch` | `number`（getter） | 保存代际：每次保存事务**入队与结束**各 +1（排队中的事务也算开始——意图已冻结，读取方不得忽略）；设置页 `load` 据此检测「读取期间发生过保存」并重读 |
| `ConfigService.saveInFlight` | `Readonly<Ref<boolean>>` | 保存门禁状态（**排队或在途均为 true**）：入队即同步置位，只有队尾落地且无人再排队才复位；响应式只读视图，设置页 `saving` 直接用它。入队即置位是为了关掉「已排队、busy 仍 false」窗口里显式切换插队、旧事务再按旧 target 自动切换的竞态 |
| `ConfigService.whenSaveSettled` | `() => Promise<void>` | 等待在途/排队中的保存事务落地（返回捕获时刻的**队尾**，与 `whenOjSwitchSettled` 队尾语义对称；不向等待者抛出保存错误） |
| `ConfigService.runSaveTxn` | `<T>(fn: (tx: SaveTxn) => Promise<T>) => Promise<T>` | 保存事务：整个回调期间持有保存门禁，覆盖「保存 → 自动切换」复合操作（旧实现靠设置页 composable 的模块级 `saving` 圈住，编辑器偏好/分栏比例等第二保存入口绕过它）；回调内 `switchOj` 安全（独立队列），公共 `updateConfig` 禁止（自等死锁）（2026-11-04 G4 下沉） |
| `ConfigService.latestOjSwitchResult` | `() => { target, outcome: 'success' \| 'persist-failed' } \| null` | 最近一次**已生效**切换的共享结果：`persist-failed` 让晚挂载设置页恢复「重启可能回退」警告；后续成功切换或任意 `updateConfig` 成功落盘同一 active 时解除（评审三十五/三十六） |
| `ConfigService.getPollSchedule` | `() => Promise<PollSchedule>` | 轮询间隔与总超时；失败或非法配置回退 2s / 300s |
| `ConfigService.getEditorPrefs` | `() => Promise<EditorPrefs>` | 字号 / Tab 宽度 / 编辑器主题（钳位上下界取自 `utils/editor`，越界与未知主题回退 14 / 4 / `'vs'`）；CodeEditor 挂载时消费 |
| `ConfigService.updateEditorPrefs` | `(patch: Partial<EditorPrefs>) => Promise<void>` | **解题页编辑器设置弹层落盘入口**：只写传入字段（读-改-写保留其余配置）；主题落 `theme.editorTheme`（旧版 `themeName` 已随 theme 域删除），落盘前经 `normalizeEditorTheme` 归一 |
| `ConfigService.getDefaultLanguage` | `() => Promise<string>` | 默认语言（**HOJ 显示名**，如 "C++"）；经 `normalizeHojLanguage` 归一，历史配置遗留的 Monaco id（'cpp'）映射回显示名，空值回退 "C++"，其它非空值（Go/Rust…）原样保留 |
| `ConfigService.getSplitRatio` | `() => Promise<number>` | 解题页初始分栏比例（非法回退 0.48） |
| `configService` | 单例 | 全局唯一实例 |

> 语言归一函数是 `@/utils/language` 的 `normalizeHojLanguage`（历史配置遗留的 Monaco id → HOJ 显示名），本服务消费它而非自定义导出（P55 语言域改为「权威值 = HOJ 显示名」后原 `normalizeLanguageId` 已删除）。

## 直接依赖

- `@/bridge/config.bridge`（`get_config` / `update_config`）
- `@/bridge`（`IpcError` —— 持久化失败判据的类型根基）
- `vue`（`ref` / `readonly` —— `saveInFlight` 的响应式只读视图，先例：`system.service` 的 `launchFlags` 快照）
- `@/types/config`（仅类型）
- `@/utils/language`（`normalizeHojLanguage`）、`@/utils/editor`（偏好默认值、钳位上下界与主题归一）
- `@/utils/logger`（`createLogger` —— 作用域日志）

## 被依赖

- `services/contest.service.ts` — 登录页匿名简报需要 `contestRef` 与 OJ 基址（经 `activeOjBaseUrl`）
- `stores/submissionStore.ts` — `startPolling()` 读取轮询调度参数
- `stores/workspaceStore.ts` — `loadWorkspace()` 读取默认语言
- `stores/announcementStore.ts`（经 announcement.service 间接）
- `components/problem/ProblemStatement.vue` — 题面图片基址
- `components/editor/CodeEditor.vue` — 字号 / Tab 宽度 / 编辑器主题（读 + 弹层改动落盘）
- `views/ProblemSolveView.vue` — 初始分栏比例读取与拖拽回写
- `composables/useSettingsForm.ts` — 设置页配置读取与保存、当前 OJ 切换（`switchOj`）；`views/SettingsView.vue` 仅作组合根

## 逻辑流程

```
getConfig()
  ├─ 缓存命中 → 直接返回同一 Promise（并发去重）
  └─ 未命中   → config.bridge.getConfig()
                 ├─ 成功 → 写入缓存
                 └─ 失败 → 清空缓存（下次调用重试）并抛出

updateConfig(mutate) / tx.updateConfig(mutate)
  公共入口先入保存门禁（runSaveTxn 排队串行；入队即 epoch+1 / busy=true）
  getConfig() → structuredClone 副本 → mutate(draft)
  → bridge.updateConfig(draft)（整体替换；锁定真值只在后端）
  → finally invalidate()（成败都失效，防缓存与磁盘漂移）

runSaveTxn(fn)（保存事务：设置页「save → 自动切换」的复合临界区）
  入队即 epoch+1 / busy=true（关掉「已排队、busy 仍 false」窗口的切换插队竞态）
  → 等前一个保存事务落地（失败也算落地）→ fn(tx)（tx.updateConfig 直通；
    事务外入口在此期间排队，不发 bridge）
  → finally：epoch+1；只有队尾且无人再排队才 busy=false

updateEditorPrefs(patch)
  → updateConfig(draft => 只写 patch 中出现的字段)
      fontSize/tabSize → editor.*；editorTheme → theme.editorTheme（先 normalizeEditorTheme）

switchOj(ojId)
  → bridge.switchOj(ojId)（后端在**单一写事务**内：补注册 → 校验已注册且在启用实例
    列表中 → 切 Registry → 清各 Service 的 OJ 域缓存 → 持久化 oj.active → 发
    CoreEvent::OjSwitched；与 update_config/reload 共用配置写事务锁）
  → 成功 invalidate()（下次读取拿到新 active）
  → isOjSwitchPersistFailed(e)：先 invalidate() 再抛出（否则旧 active 会被后续读取/保存
    写回；调用方按「已生效」对齐新 OJ，不回滚 UI）
  → 其它失败原样上抛（调用方回滚下拉选择）
  → 并发调用**排队**执行；`ojSwitchEpoch` / `whenOjSwitchSettled()` 供设置页跨控制器
    等待在途切换后再读配置（评审三十三 #4）

getOjBaseUrl() / getPollSchedule() / getEditorPrefs() / getDefaultLanguage() / getSplitRatio()
  └─ 内部吞掉异常并返回安全兜底值：派生参数缺失只影响局部展示/节奏，不阻断主流程
  （getOjBaseUrl 内部经 activeOjBaseUrl 取当前实例地址；activeOjBaseUrl 本身是同步纯函数，不吞异常）
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

`src/services/__tests__/config.service.spec.ts`：缓存并发去重与失败重试、派生参数透传/越界回退/读取失败兜底（含 `editorTheme` 未知值回退 `vs`）、updateConfig 整体写回 + 缓存失效 + 副本隔离 + 失败仍失效缓存、updateEditorPrefs 只写传入字段 + 主题落 `theme.editorTheme` + 非法主题名归一后落盘。OJ 实例化新增：`getOjBaseUrl`/`activeOjBaseUrl` 取 active 实例地址、active 未命中回退第一个启用实例、禁用实例不参与匹配与回退（enabled = 后端不注册）、读取失败返回空串；`switchOj` 调 bridge 且切换后本地缓存失效（下次读取重新拉后端真值）、bridge 失败原样上抛（设置页据此回滚下拉显示）。并发基座：真实 `ConfigService` 的并发切换串行（第二次等第一次落地）、`whenOjSwitchSettled` 覆盖队尾且队尾失败也解除、代际开始/结束各 +1、最近生效结果记录（普通失败不覆盖）与 `updateConfig` 成功且 active 匹配时解除 persist-failed。**save 门禁（真实实例 + 延迟 bridge）**：并发保存串行（第二个事务在第一个落地后才发 bridge）、`saveEpoch` 入队/结束各 +1、`saveInFlight` 覆盖排队与在途且队尾落地才复位、`whenSaveSettled` 覆盖队尾且队尾失败也解除、`runSaveTxn` 持锁跨后续编排（事务外 `updateConfig` 排队到事务落地——第二保存入口丢更新修复的锁定，变异验证过）、事务失败也释放（代际/busy/等待/缓存失效）。类型化判据：真实 `IpcError`（经 `parseAppError` 构造）为真，普通 `Error` 与裸 `{ variant }` 对象为假；裸 variant 对象按普通失败处理（不记录 persist-failed、不失效缓存）。
