# SettingsSwitch（设置开关行）

> 源文件：`src/components/settings/SettingsSwitch.vue`

## 职责

设置页 `role="switch"` 开关按钮（圆钮 + 位移）与「已开启/已关闭」状态文本的组合行。OJ 服务器分组的题面缓存与编辑器分组的自动保存共用。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `label` | prop `string` | 开关的可访问名（设置项名称）——按钮内只有圆钮无可见文本，经 aria-label 提供 |
| `checked` | prop `boolean` | 开关状态（受控：由调用方经 update 事件回写；状态由 aria-checked 表达） |
| `disabled` | prop `boolean?` | 禁用（锁定/域忙碌时） |
| `update` | emit `(checked: boolean)` | 点击请求切换为反转值 |

## 直接依赖

- `vue`（`defineProps` / `defineEmits`）

## 被依赖

- `components/settings/OjServerSection.vue`
- `components/settings/EditorSection.vue`

## 逻辑流程

`settings-switch(-knob)` class 名由 `SettingsSectionFrame` 的 `:deep` 样式消费（圆角/投影），不得改名。受控组件：点击只发事件，状态回写经调用方的 `update:form` patch。

## 测试

`src/components/settings/__tests__/SettingsSwitch.spec.ts` 锁定 role/aria-checked/圆钮 class、update 事件载荷与 disabled 禁用。
