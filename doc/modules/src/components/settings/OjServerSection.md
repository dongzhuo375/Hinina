# OjServerSection（OJ 服务器分组）

> 源文件：`src/components/settings/OjServerSection.vue`

## 职责

展示 OJ 实例、地址、比赛引用、超时、轮询、缓存和题面缓存字段，并上报草稿补丁与 OJ 切换意图。保存和切换事务仍由 `useSettingsForm` 统一执行，本组件不直接调用 service/store/router。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `form` | prop `SettingsForm` | 中央 reactive 草稿；文本/选择用 `v-model` 编辑同一对象，不替换对象身份 |
| `errors` | prop `Record<string,string \| null>` | URL 与数字字段错误；模板直接读 `props.errors`，保证 computed 替换后仍更新 |
| `options` | prop `ReturnType<typeof ojSelectOptions>` | OJ 下拉候选；模板直接读 `props.options`，保存新实例后 configured 标签实时更新 |
| `switchError` / `switchNotice` | prop `string \| null` | 切换错误与引导提示，直接读 `props` 保持响应式 |
| `inputClass` | prop fn | 共享输入框错误态样式；基础样式直接 import `SETTINGS_INPUT` |
| `update:form` | emit `SettingsFormPatch` | switch 按钮的局部草稿补丁；文本/选择用 `v-model` 直接编辑同一 reactive 草稿并保留 IME composition |
| `switch-oj` | emit | `activeOj` v-model 更新后通知父级执行切换 |

## 直接依赖

- `SettingsSectionFrame`
- `@/utils/oj`（`ojBaseUrlHint` / `ojSelectOptions` 类型）
- `@/composables/useSettingsForm`（`SETTINGS_INPUT` 常量和类型）

## 被依赖

- `views/SettingsView.vue`

## 逻辑流程

输入只产生 `update:form` 补丁，父级 `Object.assign` 到同一 reactive 草稿。`activeOj` 变化先 emit 补丁，再 emit `switch-oj`；未配置 OJ 的引导、失败回滚和保存新建实例后的自动切换全部留在 `useSettingsForm`。

## 测试

由 `useSettingsForm` 单测和 `SettingsView.behavior.spec.ts` 的组合根测试覆盖：保存、创建后自动切换、已配置切换失败回滚及错误提示。
