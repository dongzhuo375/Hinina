# useMaintenanceSettings（重置与本地清理）

> 源文件：`src/composables/useMaintenanceSettings.ts`

## 职责

持有“重置客户端”和“清理本地数据”两个维护动作的状态、服务调用和提示 timer。它们不属于中央配置表单，也不受底部保存/放弃控制。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `resetClient` | `() => Promise<void>` | 重置后依次补拉比赛、失效我的题目状态并 `void` 发出公告刷新；补拉失败如实 warning |
| `loadUsage` | `() => Promise<void>` | 读取日志/留档占用；进入页面及每次清理后调用 |
| `purgeLocalData` | `() => Promise<void>` | 双空勾选前端拦截；按勾选调用清理、重读占用并生成真实报告 |
| `formatBytes` | `(bytes) => string` | 自适应 B/KB/MB 展示 |
| `reset*` / `purge*` / `usage*` | refs | 确认框、执行态、错误与 3/4 秒通知状态 |

## 直接依赖

- `vue`（`ref`）
- `@/composables/useTransientValue`（3s 重置提示 / 4s 清理提示）
- `@/services/system.service`
- `@/stores/contestStore` / `problemStore` / `announcementStore`
- `@/utils/error`
- `@/types/system`

## 被依赖

- `views/SettingsView.vue`
- `components/settings/ResetPurgeSection.vue`
- `composables/__tests__/useMaintenanceSettings.spec.ts`

## 逻辑流程

重置服务成功后只等待 `contestStore.loadContest()` 的结果来决定提示是否降级；`announcementStore.refresh()` 保持 `void` 发出，不纳入成功判定。清理只在实际勾选日志且 `logCleared=false` 时追加“日志或崩溃记录未清理”，随后必须重读占用（预览含临时目录的崩溃记录体积）。reset/purge 提示复用 `useTransientValue`，由 scope dispose 清理。

## 测试

`src/composables/__tests__/useMaintenanceSettings.spec.ts` 覆盖重置成功补拉、重置失败不补拉、双空拦截及日志未清 warning。`SettingsView.spec.ts` 继续提供原有三条真实组合根回归。
