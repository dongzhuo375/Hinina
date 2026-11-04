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
| `isKeyedSettingsRoute` | 参数化 getter `() => (routeName: string \| symbol \| null \| undefined) => boolean` | **路由级钥匙判据**（钥匙 × `routeName === 'Settings'`，缺失/symbol 名恒假）。2026-11-04 H3：原 `guards/keyed-settings-route`（18 行）收敛到此单一入口，消除「store 读进程级 / guard 读路由级」双路径；消费方 router（beforeEach 豁免 / fromKeyedSettings / afterEach 不变量）、sessionGuard（401 旁路）、ContestLayout（keyedSettings）、useSettingsForm（切换后停留判据）。guards/ 回归 sessionGuard/globalErrorGuard 两成员 |
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
- `views/ContestLayout.vue`（`isKeyedSettingsRoute` —— 最小壳/阶段闸门豁免判据）
- `composables/useSettingsForm.ts`（`isKeyedSettingsRoute` —— 切换后「停留设置页」判据）
- `router/index.ts`、`guards/sessionGuard.ts`（守卫回调内 `useSystemStore()` 取 `isKeyedSettingsRoute`；2026-11-04 H3 起守卫也经 store）
- `composables/useAboutSettings.ts` / `useMaintenanceSettings.ts` / `useDataDirSettings.ts`（设置页三个区块的控制器）

## 逻辑流程

```
View/composable → useSystemStore()
  → isKeyed（读启动标志快照）
  → 动作：转发 systemService.* → bridge（window/system）→ IPC
```

## 测试

`src/stores/__tests__/systemStore.spec.ts`：`isKeyed` 读快照（非方法调用）、
`isKeyedSettingsRoute` 参数化 getter（钥匙 × Settings 才真；非钥匙 / 缺失 /
symbol 名恒假；同一函数随快照变化重新求值——原 keyed-settings-route.spec 的
3 用例随模块删除迁入此处）、窗口动作与设置动作转发并原样回传结果。
