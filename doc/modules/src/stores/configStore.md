# configStore（配置服务的前端消费入口）

> 源文件：`src/stores/configStore.ts`（2026-11-09，严格分层欠账清偿：View/component/composable 对 `config.service` 的全部直连收编到本 store，`Architecture.md` 登记的分层欠账就此闭合）

## 职责

View / component / composable 访问**配置服务**（配置读取与缓存失效、设置页保存事务、OJ 切换、OJ 基址、编辑器偏好、分栏比例、评测轮询参数）的唯一入口。`services/config.service` 只面向本 store、其它 service（`contest.service`）与 store 内部消费；视图不再直接 import 该 service。

**本 store 不复制状态**：保存队列、切换队列、代际计数、最近切换结果与配置缓存仍由 `configService` 持有（跨控制器共享的单一真值），这里只做消费入口的转发。

**为什么是逐个显式转发**（而非把 service 对象挂在 store 上或做成动态门面）：① View 只能看到 store 的**有类型方法**，拿不到 service 实例——否则 `store.service.x()` 又绕回了 View→Service，严格口径失效；② 每个方法都可被 spec 直接断言（转发目标与回传值）；③ service 的公开面没有全部转发——无视图消费的成员（`activeOjBaseUrl`、`getDefaultLanguage`、`whenSaveSettled`、`saveInFlight`/`saveEpoch`）留给内部与其它 store 消费，新增视图消费面时再显式补转发。

## 核心类型/函数

| 名称 | 签名 | 说明 |
|---|---|---|
| `settingsSaveInFlight` | getter `() => boolean` | **设置页发起**的保存事务在途（覆盖「保存 → 自动切换」整段）。service 侧是 readonly computed ref——getter 内读 `.value` 被依赖追踪。消费方需要 Ref 语义时用 `computed(() => store.settingsSaveInFlight)` 包回（`useSettingsForm` 即此形态）；不要直接解构 store（拿到的是展开后的布尔快照） |
| `ojSwitchInFlight` | getter `() => boolean` | 是否有 OJ 切换在途。**支撑必须是 ref**：service 侧由 `switchQueue.inFlight`（computed ref）派生；转发或 mock 若读普通属性，getter 会对无响应式依赖的求值结果永久缓存（评审 P2 的死代码教训，`configStore.spec` 锁定） |
| `ojSwitchEpoch` | action `() => number` | 切换代际（每次切换开始与结束各 +1）。service 侧普通字段（非响应式）——**必须 action 逐次直读**：getter 化会被 computed 永久缓存，`useSettingsForm` 的重读循环静默失效 |
| `settingsSaveEpoch` | action `() => number` | 设置页保存代际（入队与结束各 +1；后台写入不参与）。同上，逐次直读 |
| `latestOjSwitchResult` | action `() => OjSwitchResult \| null` | 最近一次已生效切换的共享结果（service 以方法读普通字段，非响应式）。逐次直读；持久化失败是「已生效但重启可能回退」的既存事实，晚挂载控制器据此恢复提示 |
| `isOjSwitchPersistFailed` | action `(error: unknown) => boolean` | 「OJ 切换已生效但配置持久化失败」错误判据（判据单一来源在 service） |
| `getConfig` / `invalidate` | 见 service | 配置读取（进程内缓存、并发调用共享同一次 IPC）与缓存失效 |
| `switchOj` | `(ojId: string) => Promise<void>` | 切换当前 OJ（service 内并发排队串行；persist-failed 语义见 `config.service.md`） |
| `runSaveTxn` | `<T>(fn: (tx: SaveTxn) => Promise<T>, options?: { settingsPage?: boolean }) => Promise<T>` | 保存事务：回调内**只能**用 `tx.updateConfig`（排队入口在回调内调用即自死锁）。**两个特殊点**：① 本文件因此列入 `scripts/check-save-gate.mjs` 的 `TXN_ALLOWLIST`（门禁按标识符扫描，转发体出现 `runSaveTxn` 必须显式登记）；② 转发用 `(tx) => fn(tx)` 包一层而非裸参数直传——保存门禁的静态扫描无法解析「裸参数直传」的回调形态（fail closed），内联箭头让回调保持可分析且语义不变 |
| `whenOjSwitchSettled` / `whenSettingsSaveSettled` | `() => Promise<void>` | 等待在途切换 / 设置页保存落地（只覆盖调用时刻已排队者，其后由代际复查） |
| `getOjBaseUrl` / `getPollSchedule` / `getEditorPrefs` / `updateEditorPrefs` / `getSplitRatio` / `saveSplitRatio` | 见 service | 派生读取与后台写入口（OJ 基址、轮询参数、编辑器偏好读写、分栏比例读写），兜底值语义见 `config.service.md` |
| `EditorPrefs`（模块级再导出） | `export type` | 编辑器偏好类型单一来源在 service；CodeEditor / ProblemSolveView 经本 store 取类型，避免类型边直连 service |

**action 调用同步进入 service、不加 await 壳**：调用顺序与转发前一致（「invalidate → getConfig → 其它读取」的启动顺序与保存/切换排队时序被既有用例锁定）；返回的 Promise 仍经 Pinia action 包装、并非零微任务（实测多两跳——时序敏感断言须等终态而非数节拍）。与 `systemStore` 的 async 写法刻意不同。

## 直接依赖

- `@/services/config.service`（`configService` 单例 + `isOjSwitchPersistFailed` 判据；类型 `EditorPrefs` / `OjSwitchResult` / `PollSchedule` / `SaveTxn`）
- `@/types/config`（`AppConfig`，仅类型）
- `pinia`（`defineStore`）

## 被依赖

- `components/editor/CodeEditor.vue` — `getEditorPrefs` / `updateEditorPrefs`（+ `EditorPrefs` 类型）
- `components/layout/TopBar.vue`、`components/problem/ProblemStatement.vue`、`views/AnnouncementsView.vue` — `getOjBaseUrl`（简介/题面/公告图片基址改写）
- `views/ProblemSolveView.vue` — `getSplitRatio` / `saveSplitRatio`（+ `EditorPrefs` 类型）
- `views/SubmissionDetailView.vue` — `getPollSchedule`（收敛轮询参数）
- `composables/useSettingsForm.ts` — 设置页配置读取/保存/OJ 切换全链（`getConfig` / `invalidate` / `switchOj` / `runSaveTxn` / `whenOjSwitchSettled` / `whenSettingsSaveSettled` / `settingsSaveInFlight` / `ojSwitchEpoch` / `ojSwitchInFlight` / `settingsSaveEpoch` / `latestOjSwitchResult` / `isOjSwitchPersistFailed`）

## 逻辑流程

```
View/composable → useConfigStore()
  ├─ getter：settingsSaveInFlight / ojSwitchInFlight —— 读 configService 的 ref（computed 追踪，值翻转自动重估）
  ├─ action：ojSwitchEpoch() / settingsSaveEpoch() / latestOjSwitchResult() —— 普通字段逐次直读（无缓存）
  └─ action：getConfig / switchOj / runSaveTxn((tx) => fn(tx)) / whenXxxSettled / ... —— 直透 service（保存队列/代际/最近结果仍由 service 持有）
```

## 测试

`src/stores/__tests__/configStore.spec.ts`：真 pinia + mock `config.service`（真实 Vue ref 打桩）。锁定三件事——① ref 支撑的 getter 响应式转发（`settingsSaveInFlight` 经 `computed` 包裹链仍跟随翻转）；② 普通字段必须逐次直读（翻转后读新值；误改成 getter 会在调用形态直接抛错，本组用例另锁新鲜度）；③ action 参数与回传直透、`runSaveTxn` 透传回调与 `options` 且回调收到 service 给的 `tx`。消费方 spec 沿用「真 store + 既有 service mock」路径（store 纯转发，mock 面不变）；`useSettingsForm.spec` 以真 pinia 实例化本 store，其 config.service 同构孪生 mock 的 in-flight 支撑必须是 ref（评审 P2：读普通属性会把「新控制器等在途切换」测成死代码）。
