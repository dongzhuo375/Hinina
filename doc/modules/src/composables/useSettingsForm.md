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
| `errors` / `isValid` / `canSave` | computed | URL 与各整数字段实时校验；`autoSave=false` 时间隔仍参与校验 |
| `load` / `populate` | fn | 先 `configService.invalidate()` 再读取磁盘配置并回填表单和实例清单 |
| `save` / `discard` | fn | 经 `configService.updateConfig` 整体读-改-写；成功后推进 baseline，放弃时恢复基线 |
| `ojInstances` / `ojOptions` / `persistedActive` | ref/computed | 实例清单、下拉候选、已持久化 active（失败回滚） |
| `performSwitch` / `onSwitchOj` | fn | 显式切换与保存新实例后的自动切换共用；成功后重置会话并导航 Login，失败写错误 |
| `INPUT` / `inputClass` | 常量/fn | 配置输入框基础样式与错误态拼接 |
| `LANGUAGE_OPTIONS` / `TAB_SIZES` | readonly 数组 | 来自语言/编辑器权威 utils 的展示候选 |

## 直接依赖

- `vue`（`reactive` / `ref` / `computed` / `onScopeDispose`）
- `vue-router`（切换成功后 `replace(Login)`）
- `@/services/config.service`
- `@/stores/session`（`resetSessionForOjSwitch`）
- `@/utils/language` / `editor` / `error` / `oj`
- `@/types/config`

## 被依赖

- `views/SettingsView.vue`
- `components/settings/OjServerSection.vue` / `EditorSection.vue` / `LayoutSection.vue`（编辑草稿）与 `ThemeSection.vue`（复用 `SETTINGS_INPUT`）
- `composables/__tests__/useSettingsForm.spec.ts`

## 逻辑流程

1. `load()` 失效缓存并读取完整 `AppConfig`，回填草稿、OJ 实例和 baseline。
2. 编辑只改中央 reactive 草稿；`errors` 与 `dirty/canSave` 实时派生。
3. `save()` 在首个 await 前固定 `target/isNewInstance`，经 `updateConfig` 写入完整配置；普通保存**不写 `draft.oj.active`**。
4. 保存成功后才推进 baseline、更新实例清单并显示 3 秒反馈；新建实例随后调用 `performSwitch`。
5. 保存失败保留草稿与错误，自动切换意图不得残留到下一次保存。`discard()` 从 baseline 恢复。
6. `savedTimer` 由 `onScopeDispose` 清理。

## 测试

`src/composables/__tests__/useSettingsForm.spec.ts` 覆盖：严格校验与 autoSave 禁用不豁免、完整保存载荷与 active 不变、新实例自动切换、保存失败无残留切换意图、已配置 OJ 失败回滚。`SettingsView.spec.ts` 与 `SettingsView.behavior.spec.ts` 继续提供真实组合根回归。
