# AboutSection（存储信息分组）

> 源文件：`src/components/settings/AboutSection.vue`

## 职责

展示客户端版本、存储目录与日志路径，并提供逐项复制。组件只呈现 `useAboutSettings` 模型，复制反馈由共享 `useTransientValue` 管理。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `model` | prop `ReturnType<typeof useAboutSettings>` | `storage/storageFailed/loadStorage/copiedKey/copyText` |

## 直接依赖

- `SettingsSectionFrame`
- `@/composables/useAboutSettings`（模型类型）

## 被依赖

- `views/SettingsView.vue`

## 逻辑流程

父级挂载调用 `loadStorage()`；读取失败只在本组显示“获取失败”。点击复制调用 `copyText(key, value)`，成功才显示对应 key 的“已复制”，2 秒后回弹；剪贴板失败保持静默。

## 测试

`useAboutSettings.spec.ts` 覆盖存储读取、复制 key 与失败静默；`SettingsView.behavior.spec.ts` 验证真实 DOM 复制链和回弹。
