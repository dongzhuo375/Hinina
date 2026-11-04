# useSettingsForm（设置页配置草稿与事务控制器）

> 源文件：`src/composables/useSettingsForm.ts`

## 职责

集中持有设置页中央 `SettingsForm` 草稿、加载/校验/保存/放弃，以及 OJ 实例清单与显式/自动切换。OJ/Editor/Layout 三个配置 section 编辑同一份 reactive 草稿（Theme 只读占位）；`baseline`、`dirty` 和保存载荷始终从该对象派生，避免父子双份表单。

## 核心类型/函数

| 名称 | 签名/类型 | 用途 |
|------|-----------|------|
| `SettingsForm` / `SettingsFormPatch` | interface / `Partial<SettingsForm>` | 数字字段以原始字符串保存；补丁仅用于局部更新，不替换 reactive 对象身份 |
| `parseIntStrict` | `(raw) => number \| null` | 严格非负整数解析，拒绝空串、小数、负号、科学计数法 |
| `clampRatio` | `(value) => number` | 分栏比例钳位到 `[0.30,0.70]` 并保留两位小数 |
| `form` / `baseline` / `dirty` | reactive / ref / computed | 中央草稿、上次加载或保存成功的 JSON 基线、dirty 判定 |
| `errors` / `isValid` / `canSave` | computed | URL 与各整数字段实时校验；`autoSave=false` 时隐藏的间隔字段不参与校验、不阻止保存，也不进入保存载荷（保留已存值） |
| `load` / `populate` | fn | `load` 先等共享保存落地（`configService.whenSaveSettled()`；等待结束后再捕获 `configService.saveEpoch`，避免保存结束的 +1 造成固定双读），再经 `readSettledConfig` 读取磁盘配置并回填表单和实例清单：撞上在途 OJ 切换时同样先等它落地再读，避免把保存/切换前的旧值展示出来；读完后若又有保存开始则重读。加载后按 `configService.latestOjSwitchResult()` 恢复历史持久化失败警告 |
| `save` / `discard` | fn | 保存前冻结完整提交快照；成功后 baseline 只前移到该快照，保存期间的新编辑仍为 dirty。放弃时恢复基线并清除切换提示。**锁定字段不参与前端校验、也不按表单值改写**：保存载荷仍是完整 AppConfig（未改动字段保持读入值，数字字段不做 `Number()` 转换以免 NaN 拖垮保存），锁定语义由后端落实：`update_config` 用旧配置覆盖锁定字段（评审十 #3、评审二十九 #7）——**真值由后端 `launch_args` 自持**（`enforced = !settings`；2026-11-04 G1）。保存主体走 **`configService.runSaveTxn` 保存事务**：`saving`（= `saveInFlight`）在入队时同步置位、覆盖 `save → 自动切换` 整段（评审三十四 #2），释放/代际/等待由服务层负责，编辑器偏好/分栏比例等其它保存入口与本事务**同一队列串行**（2026-11-04 G4 下沉：原模块级 `sharedSaving`/`saveEpoch`/`saveWaiters` 已删）；新挂载控制器在旧控制器保存完成前不得保存/切换，避免较旧的保存意图覆盖用户较新的显式选择。结算期间 `switching=true`：`canSave` 需 `!switching`，保存与在途切换互斥（评审二十九 #1）；切换失败只回滚选择、保留地址草稿（#3）。保存/切换在途时 **OJ 域输入由 `OjServerSection` 整体禁用**（评审三十三 #3：完成后的磁盘重置会覆盖该域）；控制器卸载后不再回写表单，但保存新建实例触发的自动切换仍执行。**普通保存成功会把当前 active 落盘**：`ConfigService.updateConfig` 统一解除同一目标的 `persist-failed` 共享结果，`clearResolvedPersistedWarning` 只负责清本控制器可见的警告（评审三十六） |
| `switching`（模块级共享） | `Ref<boolean>` | 跨控制器共享：设置页卸载重挂后，新控制器也能看到旧控制器在途的切换并禁用保存/切换（评审三十三 #4）。**发起者会 await 其它控制器的 foreign reload 后才解除本标志**（评审三十四 #1）——重读期间输入保持禁用，`copyOjDomain` 不会覆盖用户输入；foreign reload 失败时用切换代际（`ojSwitchEpoch`）确认目标仍是当前代际，否则不执行旧目标对齐 |
| `saving`（服务级共享） | `Readonly<Ref<boolean>>` | `configService.saveInFlight` 的转发：保存门禁（**排队或在途**）状态，`save → 自动切换` 是一个操作、经 `runSaveTxn` 整段持锁（评审三十四 #2；2026-11-04 G4 下沉，模块级机器已删） |
| `latestOjSwitchResult`（服务级） | `configService.latestOjSwitchResult()` | 最近一次已生效切换的共享结果由 `ConfigService` 持有（评审三十五/三十六）：切换可能在无观察者时完成，晚挂载控制器由 `load` 据此恢复 persist-failed 警告；后续成功切换或**任意一次把同一 active 成功落盘的 `updateConfig`**（含解题页编辑器偏好/分栏比例）会把它改为 `success`（警告解除）；普通失败不覆盖 |
| `SwitchOutcome`（内部类型） | `'success' \| 'persist-failed'` | 切换结果随观察者通知与 `latestOjSwitchResult()` 传递；`persist-failed`（已生效但持久化失败）在其它控制器重读后仍会设置「重启后可能回退」警告（评审三十四 #3） |
| `readSettledConfig`（内部） | `() => Promise<AppConfig>` | 先等 `configService.whenOjSwitchSettled()`（无在途时同步通过），再用 `ojSwitchEpoch` 复核读取期间是否发生切换；变了就重读。保证新控制器在事务完成后拿到新配置 |
| `ojOptions` | computed | 实时下拉候选；实例清单与 persistedActive 为 controller 内部状态 |
| 签名 | `useSettingsForm(maintainerLocked?: Readonly<Ref<boolean>>)` | 维护者层是否锁定（普通启动 true）；默认 `ref(false)`（不锁）供独立使用 |
| `onSwitchOj` / `performSwitch`（内部） | fn | 显式切换与保存新实例后的自动切换共用；自动切换失败回滚 activeOj，用户可重新选择重试。**钥匙模式（`--settings`）且当前路由仍是设置页时**切换成功后留在设置页（会话清理照旧、不导航）——踢回登录页等于重新锁死（评审八 P2）；`switch_oj` 是异步的，期间已导航离开则不再停留、照常回登录页（豁免只属于设置页本身，评审九 P2-1）；成功后重置/对齐 OJ 域并重算 baseline；重载失败时**承认切换已生效**（后端/registry/磁盘均已是新 OJ）——表单对齐新 OJ 的已知地址、OJ 域字段清单单一来源 `OJ_DOMAIN_KEYS`（脏判定/重置/对齐三处共用，评审二十四 #3）、OJ 域草稿回到磁盘值（避免 A 的比赛引用等写进 B）、基线按「磁盘值 + 新 active/url」重算（不出现假 dirty），提示「切换已生效，但配置刷新失败…」，绝不把 UI 回滚成「界面 A、实际 B」（评审二十二/#1、评审二十三）；停留时**只重置 OJ 域字段并按磁盘重算 baseline**（`resetOjFieldsFromDisk`）：编辑器/布局等全局草稿**保留**（评审十九 #1——整体 `load()` 会丢掉维护者刚改的字体/超时，与回滚分支的「保留编辑」不一致），dirty 只反映真正未保存的全局编辑，旧 OJ 的 OJ 域草稿不会写到新 OJ（评审十 #4）；OJ 域有未保存修改时给一次「已从磁盘重新加载」说明，不再静默丢弃（评审二十一 #2）。**卸载后的控制器不写回表单**（评审三十三 #4）；切换生效后等待其它仍挂载的控制器重读，并把 `success` / `persist-failed` 结果一并传过去（评审三十四 #1/#3）；`OjSwitchPersistFailed` 的分流经 `configService.isOjSwitchPersistFailed`（`IpcError` 类型化判据，不再裸读 `variant`，2026-11-04 G8） |
| `SETTINGS_INPUT` / `inputClass` | 常量/fn | 共享输入框基础样式与错误态拼接 |


## 直接依赖

- `vue`（`reactive` / `ref` / `computed` / `getCurrentScope` / `onScopeDispose`）
- `vue-router`（普通模式切换成功后 `replace(Login)`；钥匙模式留在设置页）
- `@/services/config.service`（`configService` + `isOjSwitchPersistFailed` 错误判据；保存门禁消费：`runSaveTxn` / `saveInFlight` / `saveEpoch` / `whenSaveSettled`）
- `@/stores/systemStore`（`isKeyedSettingsRoute(routeName)` —— 切换后的「停留设置页」判据；组合进程级钥匙状态与路由身份，2026-11-04 H3 起收进 store 单一入口）
- `@/stores/session`（`resetSessionForOjSwitch`）
- `@/stores/authStore`（`resetForOjSwitch`，与领域清理成对调用）
- `@/composables/useTransientValue`（已保存提示）
- `@/utils/language` / `editor` / `error` / `oj`
- `@/types/config`

## 被依赖

- `views/SettingsView.vue`
- `components/settings/OjServerSection.vue` / `EditorSection.vue` / `LayoutSection.vue`（编辑草稿）与 `ThemeSection.vue`（复用 `SETTINGS_INPUT`）
- `composables/__tests__/useSettingsForm.spec.ts`

## 逻辑流程

1. `load()` 失效缓存并读取完整 `AppConfig`，回填草稿、OJ 实例和 baseline。
2. 编辑只改中央 reactive 草稿；`errors` 与 `dirty/canSave` 实时派生。
3. `save()` 在首个 await 前冻结表单、target/isNewInstance 与提交快照，经 `updateConfig` 写入该快照；普通保存**不写 `draft.oj.active`**，已有实例不被重新 enabled。自动保存间隔只在开关开启且 `parseIntStrict` 解析成功时写入（`canSave` 已保证其值域），关闭时保留已存值——非法隐藏输入不得变成 NaN→null 让整次保存失败。
4. 保存成功后才把 baseline 前移到提交快照，并用 `useTransientValue` 显示 3 秒反馈；保存期间的新编辑保持 dirty。
5. 新建实例自动切换失败时同时回滚 activeOj 与该 OJ 的持久化地址，并把 baseline 更新为“已保存配置 + 回滚后的选择”；随后无关保存不会把新实例 URL 写入旧实例，也不产生假 dirty。保存失败保留草稿与错误，切换意图不残留到下一次保存。`discard()` 恢复提交快照并清除 switch 提示。

## 测试

`src/composables/__tests__/useSettingsForm.spec.ts` 覆盖：严格校验、隐藏字段校验豁免与隐藏非法间隔不进入载荷（保留已存值）、有效间隔写入、保存期间编辑保持 dirty、缺失 active 实例不借用 URL、禁用实例不被重新启用、完整保存载荷与 active 不变、新实例自动切换及失败回滚重试、保存失败无残留切换意图、已配置 OJ 失败回滚，以及**跨控制器卸载/重挂**（新控制器等待在途切换/保存后读到新值、旧控制器不再回写表单、foreign reload 在共享 busy 内完成、失败对齐遵守切换代际、持久化失败警告跨控制器保留（含切换在途时新挂载的 load 不清掉警告、无观察者完成时晚挂载控制器从共享结果恢复警告、成功切换或把同一 active 成功落盘的普通保存后不再提示）、等待保存落地只读一次、旧保存的自动切换不被新选择覆盖）。spec 的 config.service mock 以**同构孪生**复刻 save 门禁（`saveState`/`runSaveTxn`：入队置位、链式排队、队尾复位）与切换侧 `switchState`——真实排队/代际语义由 `config.service.spec` 的真实实例套件锁定（含变异验证），mock 只保证 composable 测试可见一致的observable 行为。`SettingsView.spec.ts` 与 `SettingsView.behavior.spec.ts` 继续提供真实组合根回归（含保存/切换在途时整个 OJ 域禁用的正向与失败恢复）。
