# systemStore（系统服务的前端消费入口）

> 源文件：`src/stores/systemStore.ts`（2026-11-04，分层口径统一为严格
> View → Store → Service 后引入）

## 职责

View / component / composable 访问**系统服务**（窗口控制、启动标志、存储信息、
维护清理、数据目录）的唯一入口。`services/system.service` 只面向本 store、
守卫与组合根；视图不再直接 import service（与 `system.service.ts` 内注释的旧
口径「允许 View 经服务」相矛盾的部分已统一）。

**本 store 不复制状态**：启动标志快照仍由 `systemService.launchFlags` 持有
（守卫首航填充，早于任何组件挂载）。`isKeyed` 转发单一判据
`systemService.isKeyed()`；Pinia getter 是 computed，求值期间方法内部对
响应式快照的读取会被依赖追踪，快照填充/变化后自动重新求值（测试用真实
响应式 ref 打桩，锁定同一 store 实例在快照变化后返回新值）。

**为什么是逐个显式转发**（而非把 service 对象挂在 store 上或做成动态门面）：
① View 只能看到 store 的**有类型方法**，拿不到 service 实例——否则 `store.service.x()`
又绕回了 View→Service，严格口径失效；② 每个方法都可被 spec 直接断言（转发目标与
回传值），新增系统接口的「三处改动」（bridge / service / store）是分层代价：
bridge 管 IPC、service 管编排、store 管 UI 入口，各层职责不同，合并会重新制造
「service 兼顾视图调用」的旧问题。

## 核心类型/函数

| 名称 | 签名 | 说明 |
|---|---|---|
| `isKeyed` | getter `() => boolean` | 进程级钥匙判据（读 `systemService.launchFlags.value.settings`） |
| `minimizeWindow` / `toggleMaximizeWindow` / `closeWindow` | `() => Promise<void>` | 无边框窗口控制（转发 service → bridge/window.bridge） |
| `destroyWindow` | `() => Promise<void>` | close-guard 落盘后的收尾 |
| `onCloseRequested` | `(handler) => Promise<() => void>` | 关窗握手事件源 |
| `getStorageInfo` | `() => Promise<StorageInfo>` | 设置页「关于」 |
| `resetClient` / `localDataUsage` / `purgeLocalData` | 见 service | 设置页「重置与清理」 |
| `getDataDir` / `pickDataDir` / `setDataDir` / `resetDataDir` | 见 service | 设置页「数据目录」 |

## 直接依赖

- `@/services/system.service`（全部动作转发；窗口部分再经 `bridge/window.bridge`）
- `@/types/system`（`DataDirChange` / `DataDirInfo` / `LocalDataUsage` / `PurgeReport` / `StorageInfo`）

## 被依赖

- `src/App.vue`（登录页兜底窗口条）
- `components/layout/WindowControls.vue`（TopBar/极简壳共用窗口按钮）
- `components/layout/ActivityBar.vue`、`views/LoginView.vue`、`views/SettingsView.vue`（`isKeyed` 判据）
- `composables/useAboutSettings.ts` / `useMaintenanceSettings.ts` / `useDataDirSettings.ts`（设置页三个区块的控制器）

## 逻辑流程

```
View/composable → useSystemStore()
  → isKeyed（读启动标志快照）
  → 动作：转发 systemService.* → bridge（window/system）→ IPC
```

## 测试

`src/stores/__tests__/systemStore.spec.ts`：`isKeyed` 读快照（非方法调用）、
窗口动作与设置动作转发并原样回传结果。
