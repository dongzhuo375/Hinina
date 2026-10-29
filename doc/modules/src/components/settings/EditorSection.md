# EditorSection（编辑器配置分组）

> 源文件：`src/components/settings/EditorSection.vue`

## 职责

展示字号、Tab、默认语言、自动保存及间隔字段，并通过补丁事件编辑中央草稿。自身不直接写配置，也不制造编辑器热更新；最终保存由 `useSettingsForm` 统一执行。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `form` / `errors` | props | 中央草稿与字号/自动保存间隔错误 |
| `inputClass` / `input` | props | 共享输入框样式 |
| `LANGUAGE_OPTIONS` / `TAB_SIZES` | 常量 | 直接取 `utils/language` / `utils/editor` 权威候选 |
| `update:form` | emit `SettingsFormPatch` | 字段补丁；Tab 数值转换、autoSave 开关和文本/选择更新 |

## 直接依赖

- `SettingsSectionFrame`
- `@/utils/language` / `editor`
- `@/composables/useSettingsForm`（仅类型）

## 被依赖

- `views/SettingsView.vue`

## 逻辑流程

`fontSize`、`autoSaveIntervalSecs` 保持原始字符串，autoSave=false 只禁用输入框而不豁免校验。`patchNumber` 只用于 Tab 数值字段；其余字段按原类型发补丁。

## 测试

由 `useSettingsForm` 校验/保存测试和 SettingsView 组合根保存测试覆盖。
