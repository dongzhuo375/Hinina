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
| `load` / `populate` | fn | 先 `configService.invalidate()` 再读取磁盘配置并回填表单和实例清单 |
| `save` / `discard` | fn | 保存前冻结完整提交快照；成功后 baseline 只前移到该快照，保存期间的新编辑仍为 dirty。放弃时恢复基线并清除切换提示。**锁定字段不参与前端校验、也不按表单值改写**：保存载荷仍是完整 AppConfig（未改动字段保持读入值，数字字段不做 `Number()` 转换以免 NaN 拖垮保存），锁定语义由后端 `maintainer_locked` 用旧配置覆盖锁定字段落实（评审十 #3、评审二十九 #7）。结算期间 `switching=true`：`canSave` 需 `!switching`，保存与在途切换互斥（评审二十九 #1）；切换失败只回滚选择、保留地址草稿（#3） |
| `ojOptions` | computed | 实时下拉候选；实例清单与 persistedActive 为 controller 内部状态 |
| 签名 | `useSettingsForm(maintainerLocked?: Readonly<Ref<boolean>>)` | 维护者层是否锁定（普通启动 true）；默认 `ref(false)`（不锁）供独立使用 |
| `onSwitchOj` / `performSwitch`（内部） | fn | 显式切换与保存新实例后的自动切换共用；自动切换失败回滚 activeOj，用户可重新选择重试。**钥匙模式（`--settings`）且当前路由仍是设置页时**切换成功后留在设置页（会话清理照旧、不导航）——踢回登录页等于重新锁死（评审八 P2）；`switch_oj` 是异步的，期间已导航离开则不再停留、照常回登录页（豁免只属于设置页本身，评审九 P2-1）；`persistedActive` 在重载成功后提交；重载失败时**承认切换已生效**（后端/registry/磁盘均已是新 OJ）——表单对齐新 OJ 的已知地址、OJ 域字段清单单一来源 `OJ_DOMAIN_KEYS`（脏判定/重置/对齐三处共用，评审二十四 #3）、OJ 域草稿回到磁盘值（避免 A 的比赛引用等写进 B）、基线按「磁盘值 + 新 active/url」重算（不出现假 dirty），提示「切换已生效，但配置刷新失败…」，绝不把 UI 回滚成「界面 A、实际 B」（评审二十二/#1、评审二十三）；停留时**只重置 OJ 域字段并按磁盘重算 baseline**（`resetOjFieldsFromDisk`）：编辑器/布局等全局草稿**保留**（评审十九 #1——整体 `load()` 会丢掉维护者刚改的字体/超时，与回滚分支的「保留编辑」不一致），dirty 只反映真正未保存的全局编辑，旧 OJ 的 OJ 域草稿不会写到新 OJ（评审十 #4）；OJ 域有未保存修改时给一次「已从磁盘重新加载」说明，不再静默丢弃（评审二十一 #2） |
| `SETTINGS_INPUT` / `inputClass` | 常量/fn | 共享输入框基础样式与错误态拼接 |


## 直接依赖

- `vue`（`reactive` / `ref` / `computed`）
- `vue-router`（普通模式切换成功后 `replace(Login)`；钥匙模式留在设置页）
- `@/services/config.service`
- `@/services/system.service`（`launchFlags` 快照 —— 钥匙模式判据）
- `@/stores/session`（`resetSessionForOjSwitch`）
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

`src/composables/__tests__/useSettingsForm.spec.ts` 覆盖：严格校验、隐藏字段校验豁免与隐藏非法间隔不进入载荷（保留已存值）、有效间隔写入、保存期间编辑保持 dirty、缺失 active 实例不借用 URL、禁用实例不被重新启用、完整保存载荷与 active 不变、新实例自动切换及失败回滚重试、保存失败无残留切换意图、已配置 OJ 失败回滚。`SettingsView.spec.ts` 与 `SettingsView.behavior.spec.ts` 继续提供真实组合根回归。
