# configStore（配置服务的前端消费入口）

> 源文件：`src/stores/configStore.ts`（2026-11-09，严格分层欠账清偿：View/component/composable 对 `config.service` 的全部直连收编到本 store，`Architecture.md` 登记的分层欠账就此闭合）

## 职责

View / component / composable 访问**配置服务**（应用配置读取与缓存失效、设置页保存事务、OJ 切换、OJ 基址、编辑器偏好、分栏比例、评测轮询参数）的唯一入口。`services/config.service` 只面向本 store、其它 service（`contest.service`）与 store 内部消费；视图不再直接 import 该 service。

**本 store 不复制状态**：保存队列、切换队列、代际计数、最近切换结果与配置缓存仍由 `configService` 持有（跨控制器共享的单一真值），这里只做消费入口的转发。

## 转发面按 service 状态的支撑形态分两类

这是本 store 最容易写错的地方（`configStore.spec` 有对应锁定用例）：

- **由响应式 ref/computed 支撑的**（`settingsSaveInFlight`、`ojSwitchInFlight`）走 **getter**：Pinia getter 是 computed，求值期间对 service ref 的读取被依赖追踪，值翻转后自动重新求值（与 `systemStore.isKeyed` 同理）。消费方需要 Ref 语义时用 `computed(() => store.settingsSaveInFlight)` 包回——不要直接解构 store（拿到的是展开后的布尔快照）。
- **由普通字段支撑的**（`ojSwitchEpoch`、`settingsSaveEpoch`、`latestOjSwitchResult`）必须走 **action（方法）**：computed 对无响应式依赖的求值结果会**永久缓存**——getter 化会把「代际比对/最近结果读取」钉死在首次读到的值上，`useSettingsForm` 的重读循环与持久化失败提示会静默失效。方法每次调用直读 service，无缓存。

## 为什么是逐个显式转发

与 `systemStore` 同一理由：① View 只能看到 store 的**有类型方法**，拿不到 service 实例——否则 `store.service.x()` 又绕回了 View→Service，严格口径失效；② 每个方法都可被 spec 直接断言（转发目标与回传值）；③ service 的公开面没有全部转发——无视图消费的成员（`activeOjBaseUrl`、`getDefaultLanguage`、`whenSaveSettled`、`saveInFlight`/`saveEpoch`）留给内部与其它 store 消费，新增视图消费面时再显式补转发。

**action 一律非 async、直接 return service 调用（零微任务插入）**：设置页「invalidate → getConfig → 其它读取」的启动顺序与保存/切换的排队时序被既有用例锁定，转发层不得加 await 壳（与 `systemStore` 的 async 写法刻意不同，理由见上）。

**`runSaveTxn` 的两个特殊点**：① 本文件因此列入 `scripts/check-save-gate.mjs` 的 `TXN_ALLOWLIST`（门禁按标识符扫描，转发体出现 `runSaveTxn` 必须显式登记）；② 转发用 `(tx) => fn(tx)` 包一层而非裸参数直传——保存门禁的静态扫描无法解析「裸参数直传」的回调形态（fail closed 视为违规），内联箭头让回调保持可分析且语义不变。

## 被依赖

- `components/editor/CodeEditor.vue` — `getEditorPrefs` / `updateEditorPrefs`（+ `EditorPrefs` 类型再导出）
- `components/layout/TopBar.vue`、`components/problem/ProblemStatement.vue`、`views/AnnouncementsView.vue` — `getOjBaseUrl`（题面/简介/公告图片基址改写）
- `components/problem/QuickSubmitDialog.vue` —（经 `CodeEditor` 间接）编辑器偏好
- `views/ProblemSolveView.vue` — `getSplitRatio` / `saveSplitRatio`（+ `EditorPrefs` 类型）
- `views/SubmissionDetailView.vue` — `getPollSchedule`（收敛轮询参数）
- `composables/useSettingsForm.ts` — 设置页配置读取/保存/OJ 切换全链（`getConfig` / `invalidate` / `switchOj` / `runSaveTxn` / `whenOjSwitchSettled` / `whenSettingsSaveSettled` / `settingsSaveInFlight` / `ojSwitchEpoch` / `ojSwitchInFlight` / `settingsSaveEpoch` / `latestOjSwitchResult` / `isOjSwitchPersistFailed`）

## 测试

`src/stores/__tests__/configStore.spec.ts`：真 pinia + mock `config.service`（真实 Vue ref 打桩）。锁定三件事——① ref 支撑的 getter 响应式转发（`settingsSaveInFlight` 经 `computed` 包裹链仍跟随翻转）；② **普通字段必须逐次直读**（翻转后读新值——若有人把它们改成 getter，该用例即变异红）；③ action 参数与回传直透、`runSaveTxn` 透传回调与 `options` 且回调收到 service 给的 `tx`。消费方 spec 沿用「真 store + 既有 service mock」路径（store 纯转发，mock 面不变）；`useSettingsForm.spec` 以真 pinia 实例化本 store、其 config.service 同构孪生 mock 不变。
