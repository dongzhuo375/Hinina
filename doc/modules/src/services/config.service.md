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
| `SaveTxn.updateConfig` | `(mutate: (draft: AppConfig) => void) => Promise<AppConfig>` | **唯一的配置写入通道**（`runSaveTxn` 回调内调用）：读当前配置 → structuredClone 副本上应用变更 → 整体写回后端（`update_config` 是整体替换语义）→ **返回后端实际提交的配置**（active 取后端写锁内值）→ 失效缓存；写回失败同样失效缓存（避免缓存与磁盘漂移）并抛出。设置页保存、`updateEditorPrefs`、解题页分栏比例**全部经同一队列**——并发的读-改-写不交错（丢更新修复，2026-11-04 G4）。**公开的排队入口有三个**：`runSaveTxn` 与两个语义化包装 `updateEditorPrefs` / `saveSplitRatio`——它们都会等门禁，所以在事务回调里调用任意一个都会排队等自己（自死锁）。唯一不排队的写入通道是回调参数 `tx.updateConfig`。禁止写法由脚本门禁 `scripts/check-save-gate.mjs`（`npm run check:save-gate`，CI frontend job 执行；判定用例在 `scripts/__tests__/check-save-gate.spec.ts`）锁定：回调正文出现任何排队入口即失败（**引用级**判定——名字出现即违规，调用语法形态不穷举）；**并沿同文件局部辅助函数做传递追踪**（回调 → `performSwitch` → 排队入口 会报出完整调用链；**引用式传参同样追**——`list.map(persistOne)`；可追踪形态：`function` 声明、函数表达式常量、**赋值箭头**（类字段 `h = () => …` / `this.x = …`）、类/对象方法（单行注解，含泛型））；新增 `runSaveTxn` 调用方需加白名单。**已知局限**：跨文件辅助函数、别名与动态调用、多行表达式体（截断到 `;`/换行）、注解**跨换行**或含条件 `?` 的类/对象方法（**单行**返回注解可追踪——泛型与 `(): () => void` 括号形态都进函数表；`sameLineBodyOpen` 遇换行/`;`/`)`/`,`/`}`/`?` 即放弃）、泛型实参跨换行（`readAngles` 只认单行）、返回类型含 `{` 的 `function` 声明（`function f(): Array<{...}> {`——泛型里的 `{` 被当成正文起点；单行不含 `{` 的注解如 `(): () => void` 可追踪）、对象**属性**箭头（`{ h: () => … }` 冒号形态——**赋值**箭头已可追踪）不在文本扫描能力内，仍需评审把关；排队保存请保持在可追踪形态（方法注解不跨行、`function` 声明注解不含 `{`，或块体/表达式体箭头）。`switchObservers` 监听器（在事务回调内被 await）同样不得引用排队入口；**注册形态逐出现点判定**：声明 `= new Set` / `.add(…)`（含 `?.`）/ `.delete(…)` / `[...x]` 展开为良性，别名（`const reg = switchObservers.add`）/ `add.bind` / 方括号取值 / 整体赋值为现成 Set 一律 fail closed。两个包装定义按「参数 → 同行注解 → 块」的**定义形态**逐出现点定位（`indexOf` 曾被更早的同名文本抢中而看错块）。**2026-11-11 起的强化**：`stripComments` 已感知正则字面量（除法/正则启发式含控制括号语境，`/[/*]/` 与 `if (ok) /[/*]/.test(v)` 不再截断扫描）；`.vue` 扫描面含模板可执行表达式（指令绑定/动态参数/插值，HTML 实体解码）；动态 import 说明符非字面量时 fail closed。「锁定字段不动」由后端按 `launch_args` 自持真值执行，IPC 载荷不携带锁定声明（评审二十六 #2 与 2026-11-04 G1）。**成功写回后按后端实际提交的 active 判定**：若与最近一次 `persist-failed` 切换目标相同，会解除该共享结果（评审三十六：任意保存入口都算回退风险解除；保存与切换交错时不会按旧快照误判） |
| `ConfigService.getOjBaseUrl` | `() => Promise<string>` | OJ 基址，用于题面/简介/公告相对图片 URL 改写；取当前 OJ 实例（`oj.active` 匹配）的 `baseUrl`（经 `activeOjBaseUrl`）；失败返回空串 |
| `ConfigService.activeOjBaseUrl` | `(config: AppConfig) => string` | 同步帮手：解析配置中当前 OJ 实例的服务端地址，供已持有配置的调用方复用。`active` 未命中启用实例列表时回退第一个启用实例（正常配置经 Rust 归一不会未命中，此回退仅兜底 IPC 写读竞态）；无启用实例返回空串 |
| `ConfigService.switchOj` | `(ojId: string) => Promise<void>` | 切换当前 OJ（设置页「当前 OJ」下拉的显式动作）：调 bridge `switchOj`（后端在**单一写事务**内：补注册 → 校验已注册且在启用实例列表 → 切 Registry → 显式清各 Service 的 OJ 域缓存 → 持久化 `oj.active` → 发布 `CoreEvent::OjSwitched`）→ 本地缓存失效（后续读取拿到新 active）。并发调用**排队**执行。失败原样上抛；**经 `isOjSwitchPersistFailed` 判定的持久化失败也先失效缓存**（后端已切 registry/内存，陈旧缓存会把旧 active 写回，评审二十六 P1）。其余失败由调用方按「未切换」处理 |
| `ConfigService.ojSwitchEpoch` | `number`（getter） | 切换代际：开始与结束各 +1；设置页读取配置时据此检测「读取期间发生过切换」并重读（评审三十三 #4） |
| `ConfigService.ojSwitchInFlight` | `boolean`（getter） | 是否有切换在途（跨控制器） |
| `ConfigService.whenOjSwitchSettled` | `() => Promise<void>` | 等待在途切换落地；切换失败也视为落地（不向等待者抛出切换错误） |
| `SaveTxn` | `{ updateConfig(mutate): Promise<AppConfig> }` | 保存事务句柄：`runSaveTxn` 回调内的**唯一不排队**写入通道（直通）。公开的排队入口有三个（`runSaveTxn` + 两个语义化包装），回调内调用任意一个都会自死锁——规则由静态门禁锁定：**识别不了的引用式回调按违规处理（fail closed）**，而不是静默跳过（2026-11-04 G4 下沉；评审后去掉超时看门狗，改为源码扫描约束） |
| `ConfigService.saveEpoch` | `number`（getter） | 保存代际：**任意**保存事务入队与结束各 +1（排队中的也算开始）。供全量队列观测；设置页加载用 `settingsSaveEpoch` |
| `ConfigService.saveInFlight` | `Readonly<Ref<boolean>>` | **任意**写入口的保存队列状态（排队或在途均为 true；由队列 tail **派生**）。供观测/测试；设置页 `load` **不**等它（后台写入悬挂不得把设置页卡在 loading） |
| `ConfigService.settingsSaveInFlight` | `Readonly<Ref<boolean>>` | **设置页发起**的保存事务是否在途（`runSaveTxn(..., { settingsPage: true })`，由 `settingsSaveDepth > 0` **派生**，无独立布尔量）。设置页的 `saving`（UI「保存中…」、保存按钮禁用、与 OJ 切换的双向互斥）取它——后台写入（编辑器偏好/分栏比例）只排保存队列，不该让设置页下拉框与保存按钮产生反应 |
| `ConfigService.settingsSaveEpoch` | `number`（getter） | **设置页保存**代际：设置页事务入队与结束各 +1，后台写入不参与。设置页 `load` 据此检测「读取期间发生过设置页保存」并重读 |
| `ConfigService.whenSaveSettled` | `() => Promise<void>` | 等待**调用时刻**已排队/在途的**任意**保存事务落地（不向其后的新任务延伸；需要覆盖「等待期间新开始的保存」的调用方自己复核代际）；不向等待者抛出保存错误 |
| `ConfigService.whenSettingsSaveSettled` | `() => Promise<void>` | 等待**设置页发起**的保存事务落地（无在途时同步 resolved）。设置页 `load` 用它：只等设置页自己的保存——后台写入可能长时间不返回，等它们会把加载永久卡住（2026-11-04 评审恢复旧等待范围） |
| `ConfigService.runSaveTxn` | `<T>(fn: (tx: SaveTxn) => Promise<T>, options?: { settingsPage?: boolean }) => Promise<T>` | 保存事务：整个回调期间持有保存门禁，覆盖「保存 → 自动切换」复合操作（旧实现靠设置页 composable 的模块级 `saving` 圈住，编辑器偏好/分栏比例等第二保存入口绕过它）；回调内只能走 `tx.updateConfig`——**不得嵌套 `runSaveTxn`，也不得调用 `updateEditorPrefs` / `saveSplitRatio`（含 `?.()` 可选调用）**（两个包装同样排队，与嵌套事务等价）。规则由 `scripts/check-save-gate.mjs` 锁定（判定用例见 `scripts/__tests__/check-save-gate.spec.ts`，含传递调用、环检测、**引用式回调 fail closed**、可选调用、`switchObservers` 监听器规则）。回调内 `switchOj` 安全（独立队列）。`options.settingsPage` 标记事务来源：推进 `settingsSaveEpoch` 并置位 `settingsSaveInFlight`（2026-11-04 G4 下沉） |
| `ConfigService.latestOjSwitchResult` | `() => { target, outcome: 'success' \| 'persist-failed' } \| null` | 最近一次**已生效**切换的共享结果：`persist-failed` 让晚挂载设置页恢复「重启可能回退」警告；后续成功切换或任意一次保存成功落盘同一 active 时解除（评审三十五/三十六） |
| `ConfigService.getPollSchedule` | `() => Promise<PollSchedule>` | 轮询间隔与总超时；失败或非法配置回退 2s / 300s |
| `ConfigService.getEditorPrefs` | `() => Promise<EditorPrefs>` | 字号 / Tab 宽度 / 编辑器主题（钳位上下界取自 `utils/editor`，越界与未知主题回退 14 / 4 / `'vs'`）；CodeEditor 挂载时消费 |
| `ConfigService.updateEditorPrefs` | `(patch: Partial<EditorPrefs>) => Promise<void>` | **解题页编辑器设置弹层落盘入口**：只写传入字段（读-改-写保留其余配置）；主题落 `theme.editorTheme`（旧版 `themeName` 已随 theme 域删除），落盘前经 `normalizeEditorTheme` 归一 |
| `ConfigService.getDefaultLanguage` | `() => Promise<string>` | 默认语言（**HOJ 显示名**，如 "C++"）；经 `normalizeHojLanguage` 归一，历史配置遗留的 Monaco id（'cpp'）映射回显示名，空值回退 "C++"，其它非空值（Go/Rust…）原样保留 |
| `ConfigService.getSplitRatio` | `() => Promise<number>` | 解题页初始分栏比例（非法回退 0.48） |
| `ConfigService.saveSplitRatio` | `(ratio: number) => Promise<void>` | **解题页拖拽结束后的分栏比例落盘入口**：经 `runSaveTxn` 只写 `layout.splitRatio`（后台写入，不标记 `settingsPage`，不进入设置页门禁） |
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
- **`stores/configStore.ts` — View / component / composable 的唯一消费入口**（（经 store 消费）：
  OJ 基址、编辑器偏好、分栏比例、轮询参数、设置页保存事务与 OJ 切换全部经 store 纯转发；
  视图不再直接 import 本 service。store 只做消费入口——保存队列/切换队列/代际/最近结果
  仍由本服务持有；**代际与最近结果是普通字段，store 以 action 方法逐次直读**，勿改成 getter）
- `components/problem/ProblemStatement.vue`、`components/editor/CodeEditor.vue`、`views/ProblemSolveView.vue`、`composables/useSettingsForm.ts`、`views/AnnouncementsView.vue`、`views/SubmissionDetailView.vue`、`components/layout/TopBar.vue` — 均经 `stores/configStore` 消费（题面图片基址、编辑器偏好读写、分栏比例、设置页配置/保存/切换、轮询参数）

## 逻辑流程

```
getConfig()
  ├─ 缓存命中 → 直接返回同一 Promise（并发去重）
  └─ 未命中   → config.bridge.getConfig()
                 ├─ 成功 → 写入缓存
                 └─ 失败 → 清空缓存（下次调用重试）并抛出

tx.updateConfig(mutate)（唯一写入通道：只在 runSaveTxn 回调内可用）
  入队时已 epoch+1 / 队列 busy=true
  getConfig() → structuredClone 副本 → mutate(draft)
  → bridge.updateConfig(draft)（整体替换；锁定真值只在后端）
  → finally invalidate()（成败都失效，防缓存与磁盘漂移）

runSaveTxn(fn, { settingsPage? })（保存事务：设置页「save → 自动切换」的复合临界区）
  入队即 epoch+1（关掉「已排队、busy 仍 false」窗口的切换插队竞态）；
  settingsPage=true 时另推进 settingsSaveEpoch、settingsSaveDepth+1
  → 等前一个保存事务落地（失败也算落地）→ fn(tx)（tx.updateConfig 直通；
    事务外入口在此期间排队，不发 bridge）
  → finally：epoch+1；settingsPage 时 depth-1，归零才释放 whenSettingsSaveSettled
    的等待者；只有队尾且无人再排队才 busy=false（inFlight 由 tail 派生）

SerialQueue（私有）
  保存与切换共用同一实现：入队建 gate 作为新队尾 → 等前一个队尾 →
  执行 → finally release()，且只有自己仍是队尾才清 tail/inFlight

updateEditorPrefs(patch)
  → runSaveTxn(tx => tx.updateConfig(draft => 只写 patch 中出现的字段))
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
  保存经 `runSaveTxn` 事务句柄写后端 + 失效缓存，其余读取方下次调用即拿到新值。
- **P55 值域统一（后经语言权威值改造修订）**：`defaultLanguage` 值域为 **HOJ 显示名**
  （与提交契约同源，见 `utils/language`）；历史 Monaco id 由前后端各自的归一函数
  映射回显示名（前端 `normalizeHojLanguage`、Rust `normalize_language_display_name`）。
  Rust 端默认值 `light` / `vs` / `0.48` 不变。
- 兜底值与 Rust `core::entity::config` 的默认值保持一致（`poll_interval=2s`、`poll_timeout=300s`、字号 14、Tab 4、编辑器主题 `vs`、分栏 0.48）。
- **编辑器偏好值域归 `utils/editor`**：字号 8–32、Tab 1–8（候选档位 2/4/8）、主题 `vs`/`vs-dark` ——
  本服务只取该模块常量做钳位，不复制字面量。
  `updateEditorPrefs` 落盘前对主题做归一，未知主题名不会写进配置文件（Rust 端 `sanitize`
  还会再收敛一次）；**部分写语义**：只写传入字段，未传入的偏好保持磁盘原值。
- **已知限制（单队列头阻塞）**：所有写入口共用一个队列——后台写入（编辑器偏好/
  分栏比例）悬挂会把**排在它之后**的设置页保存一起卡住：设置页停在「保存中…」、
  OJ 切换被拒、`load()` 的 `whenSettingsSaveSettled` 一并等待。IPC 悬挂概率低
  （`update_config` 是本地命令），但这是新引入的可用性耦合；反方向（设置页保存
  慢 → 后台写入排队）则是有意设计（丢更新防护）。拆双队列会重新打开「设置页保存
  与后台写并发读-改-写丢更新」的窗口（G4 的动机），故记录为已知限制而非修复。

## 测试

`src/services/__tests__/config.service.spec.ts`：缓存并发去重与失败重试、派生参数透传/越界回退/读取失败兜底（含 `editorTheme` 未知值回退 `vs`）、事务句柄整体写回 + 缓存失效 + 副本隔离 + 失败仍失效缓存、updateEditorPrefs 只写传入字段 + 主题落 `theme.editorTheme` + 非法主题名归一后落盘。OJ 实例化新增：`getOjBaseUrl`/`activeOjBaseUrl` 取 active 实例地址、active 未命中回退第一个启用实例、禁用实例不参与匹配与回退（enabled = 后端不注册）、读取失败返回空串；`switchOj` 调 bridge 且切换后本地缓存失效（下次读取重新拉后端真值）、bridge 失败原样上抛（设置页据此回滚下拉显示）。并发基座：真实 `ConfigService` 的并发切换串行（第二次等第一次落地）、`whenOjSwitchSettled` 覆盖队尾且队尾失败也解除、代际开始/结束各 +1、最近生效结果记录（普通失败不覆盖）与保存成功且 active 匹配时解除 persist-failed。**save 门禁（真实实例 + 延迟 bridge）**：并发保存串行（第二个事务在第一个落地后才发 bridge——顺序断言记在事务体内部，调用方 `.then` 的先后只取决于 Promise 包装层数、不是契约）、`saveEpoch` 入队/结束各 +1、`saveInFlight` 覆盖排队与在途且队尾落地才复位、`whenSaveSettled` 覆盖队尾且队尾失败也解除、`runSaveTxn` 持锁跨后续编排（事务外发起的第二保存排队到事务落地——丢更新修复的锁定，变异验证过）、事务失败也释放（代际/busy/等待/缓存失效）；`settingsSaveInFlight` 只标记设置页事务（后台写入期间保持空闲）；设置页作用域：`settingsSaveEpoch` 只随设置页事务推进、`whenSettingsSaveSettled` 在设置页事务落地即解除（不等悬挂的后台写入——设置页加载只等自己的保存）；`saveInFlight`/`settingsSaveInFlight` 均为派生状态（队列 tail / 深度），无手动同步的布尔量。**保存门禁的静态约束**（`scripts/check-save-gate.mjs` 脚本门禁 + `scripts/__tests__/check-save-gate.spec.ts` 判定用例；放在 `src/` 外是因 `src/**` 无 Node 类型且 `?raw` 全量 import 会污染覆盖率）：`runSaveTxn` 只允许出现在白名单文件（新增调用方须显式复核）、任何事务回调正文不得引用 `runSaveTxn`/`updateEditorPrefs`/`saveSplitRatio`（**引用级判定**：调用形态 `?.()` / `.call` / `.apply` / 括号成员 / `.bind` / 标签模板无法穷举，名字出现即违规；两个包装同样排队，等价于嵌套事务）、**回调形态无法静态分析时按违规处理（fail closed——引用式/别名/跨文件回调不再被静默跳过）**、两个包装必须经 `runSaveTxn` 且不得直连 `updateConfigNow`、`switchObservers` 监听器（在事务回调内被 await）同样不得引用排队入口；每条规则都有判定用例（含引用式回调、其余调用形态、仅引用、观察者经辅助函数间接引用、单行源码不挂住 CLI），回调内插入排队入口/包装改直连的变异验证均转红。类型化判据：真实 `IpcError`（经 `parseAppError` 构造）为真，普通 `Error` 与裸 `{ variant }` 对象为假；裸 variant 对象按普通失败处理（不记录 persist-failed、不失效缓存）。
