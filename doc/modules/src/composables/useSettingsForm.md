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
| `save` / `discard` | fn | 保存前冻结完整提交快照；成功后 baseline 只前移到该快照，保存期间的新编辑仍为 dirty。放弃时恢复基线并清除切换提示 |
| `ojOptions` | computed | 实时下拉候选；实例清单与 persistedActive 为 controller 内部状态 |
| `onSwitchOj` / `performSwitch`（内部） | fn | 显式切换与保存新实例后的自动切换共用；自动切换失败回滚 activeOj，用户可重新选择重试 |
| `SETTINGS_INPUT` / `inputClass` | 常量/fn | 共享输入框基础样式与错误态拼接 |


## 直接依赖

- `vue`（`reactive` / `ref` / `computed`）
- `vue-router`（切换成功后 `replace(Login)`）
- `@/services/config.service`
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
