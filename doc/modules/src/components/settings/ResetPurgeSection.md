# ResetPurgeSection（重置与清理分组）

> 源文件：`src/components/settings/ResetPurgeSection.vue`

## 职责

展示重置客户端、占用预览、清理勾选与二次确认。组件只呈现 `useMaintenanceSettings` 模型，不直接调用维护服务，也不参与中央 dirty/save。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `model` | prop `ReturnType<typeof useMaintenanceSettings>` | 重置/清理状态、方法、占用预览与 `formatBytes` |

模型包含 `resetting/resetConfirmOpen/resetNotice/resetError`、`usage/usageFailed`、`purgeLogs/purgeSnapshots/purging/purgeConfirmOpen/purgeNotice/purgeError`、`resetClient/loadUsage/purgeLocalData/formatBytes`。

## 直接依赖

- `SettingsSectionFrame`
- `@/composables/useMaintenanceSettings`（模型类型）

## 被依赖

- `views/SettingsView.vue`

## 逻辑流程

按钮调用模型方法：重置由 controller 执行服务与补拉；清理先做双空拦截，再由 controller 调用服务、重读占用和显示真实反馈。通知时长与公告 `void refresh` 语义不变。

## 测试

`useMaintenanceSettings.spec.ts` 覆盖 controller 负向路径；`SettingsView.spec.ts` 的三条真实组合根测试继续锁定双空拦截、日志未清和补拉失败提示。
