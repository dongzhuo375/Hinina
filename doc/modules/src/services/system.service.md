# system.service（系统服务）

> 源文件：`src/services/system.service.ts`

## 职责

客户端自身运行信息与维护操作（存储目录 / 日志路径 / 版本号 / 重置客户端 / 清理本地数据 / 启动标志）的入口：透传 `system.bridge` 调用。不做缓存 —— 版本号构建期固定，但存储目录可能随用户数据迁移变化，设置页每次挂载都取实时值。**唯一例外是启动标志**：一次进程生命周期内不可变，且守卫与组件多处消费必须读到同一份，故 memoize 并维护**响应式同步快照**（`launchFlags`）——组件同步读取（异步判据会把外壳/锁态闪帧反转到钥匙启动的首帧）。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `SystemService.launchFlags` | `readonly Ref<LaunchFlags>` | 启动标志**同步快照**（响应式，`readonly()` 包装——「进程内不可变」由类型强制，评审十 #6）：守卫首航 `await getLaunchFlags()` 时填充（早于任何组件挂载），组件/computed 直接读。默认 `{settings:false}` = 安全侧 |
| `SystemService.getLaunchFlags` | `() => Promise<LaunchFlags>` | 启动标志的 Promise 入口（**仅守卫消费**）：进程内 memoize，解析成功填充快照；读取失败（旧二进制、桥异常）保持快照默认「无参数」—— 安全侧默认值，绝不让钥匙通道故障卡死启动或设置页；**失败不缓存**，下次调用重试（评审十九 #7：一次瞬时不等于整个进程失去钥匙）；守卫另有 try/catch 二道防线 |
| `SystemService.isKeyed` | `() => boolean` | 本次启动是否为 `--settings` 管理方钥匙（进程级判据，无路由维度）。组件一律经它判断钥匙模式，不直接读 `launchFlags.value.settings`（评审二十一 #4）。**keyed 判定的唯一入口**：原 `guards/keyed-settings-policy` 纯转发模块已归并到此（2026-11-04，G2）——它曾是 guards/ 里唯一的非装配成员，且造成 config.service → guards 反向边 |
| `SystemService.minimizeWindow` / `toggleMaximizeWindow` / `closeWindow` | `() => Promise<void>` | 无边框窗口控制（View 经 Service 调用，Service 转发 `bridge/window.bridge`——View 不直接 import bridge，评审二十一 #6） |
| `SystemService.destroyWindow` | `() => Promise<void>` | 销毁窗口（close-guard 落盘完成后的收尾，2026-11-04 G3） |
| `SystemService.onCloseRequested` | `(handler) => Promise<() => void>` | 订阅关闭请求（关窗握手事件源，返回取消订阅函数） |
| `SystemService.isKeyedSettings` | `(routeName) => boolean` | **钥匙设置页的唯一判据**（评审十 #5）：消费方为阶段闸门/最小壳（ContestLayout）、sessionGuard 旁路、OJ 切换停留（useSettingsForm）、**router 的三处**（beforeEach 条件豁免、fromKeyedSettings、afterEach 落地不变量）——都走这里，避免各自内联漂移 |
| `SystemService.getStorageInfo` | `() => Promise<StorageInfo>` | 存储信息（baseDir / logPath / version）；失败由调用方降级展示（设置页「关于」区块显示「获取失败」，不影响其余分组） |
| `SystemService.resetClient` | `() => Promise<void>` | 重置客户端（三层缓存 + 公告基线 + 公告已读状态）；**不重拉**，补拉由调用方编排（设置页重置后立刻重拉当前比赛数据） |
| `SystemService.localDataUsage` | `() => Promise<LocalDataUsage>` | 可清理项的体积预览（日志与崩溃记录字节数 / 留档总与过期条数字节） |
| `SystemService.purgeLocalData` | `(logs: boolean, staleSnapshots: boolean) => Promise<PurgeReport>` | 清理本地数据（**不可逆**）：日志内容 + 崩溃记录文件 + 过期留档；范围由调用方显式传入，服务层不设默认值 |
| `SystemService.getDataDir` | `() => Promise<DataDirInfo>` | 当前数据目录 / 默认目录 / 来源 / 是否待重启 / 指针是否在临时目录 |
| `SystemService.setDataDir` | `(path: string, migrate: boolean) => Promise<DataDirChange>` | 更改数据目录（**重启后生效**）；只「校验 + 记下改动」，搬运由下次启动完成 |
| `SystemService.resetDataDir` | `(migrate: boolean) => Promise<DataDirChange>` | 恢复默认数据目录（**重启后生效**） |
| `SystemService.pickDataDir` | `() => Promise<string \| null>` | 原生目录选择器（用户取消返回 null） |
| `systemService` | 单例 | 全局唯一实例 |

## 直接依赖

- `@/bridge/system.bridge`（`getLaunchFlags` / `getStorageInfo` / `resetClient` / `localDataUsage` / `purgeLocalData` / `getDataDir` / `setDataDir` / `resetDataDir` / `pickDataDir`）
- `@/types/system`（`LaunchFlags` / `StorageInfo` / `LocalDataUsage` / `PurgeReport` / `DataDirInfo` / `DataDirChange`）

## 被依赖

- `composables/useDataDirSettings.ts` / `useMaintenanceSettings.ts` / `useAboutSettings.ts` — 数据目录生命周期、重置清理与 About 存储信息；对应 section 组件只消费模型，View 不直接调用 service
- `router/index.ts`（守卫消费 `getLaunchFlags`：`--settings` 初始直落判据；并消费 `isKeyedSettings` 三处：`beforeEach` 条件豁免 / `fromKeyedSettings` / `afterEach` 落地不变量）
- `views/SettingsView`（分层锁）、`views/ContestLayout`（最小壳）、`guards/sessionGuard`、`composables/useSettingsForm`、`views/LoginView`、`components/layout/ActivityBar`——均直接取 `isKeyed()` / `isKeyedSettings()` 判据，不直接读 `launchFlags`

## 逻辑流程

```
getLaunchFlags() → [memoize] systemBridge.getLaunchFlags() → ipcInvoke('launch_flags')
                   → Rust commands::app_cmd::launch_flags
                   → 成功：填充 launchFlags 快照；失败：保持默认（安全侧 false）
getStorageInfo() → systemBridge.getStorageInfo() → ipcInvoke('get_storage_info')
                   → Rust commands::config_cmd::get_storage_info
resetClient()    → systemBridge.resetClient()    → ipcInvoke('reset_client')
                   → Rust commands::maintenance_cmd::reset_client
localDataUsage() → systemBridge.localDataUsage() → ipcInvoke('local_data_usage')
purgeLocalData(logs, staleSnapshots)
                 → systemBridge.purgeLocalData(...) → ipcInvoke('purge_local_data')
                   → Rust commands::maintenance_cmd::purge_local_data
getDataDir() / setDataDir() / resetDataDir() / pickDataDir()
                 → ipcInvoke('get_data_dir' | 'set_data_dir' | 'reset_data_dir' | 'pick_data_dir')
                   → Rust commands::data_dir_cmd
```

设计要点：分层约定 View / Store 不得直接调用 `system.bridge`，统一经本服务消费；服务本身无状态、无兜底值（与 config.service 的「读取失败回退默认」不同，存储信息缺失时降级责任在调用方 UI）。`resetClient` / `purgeLocalData` / `setDataDir` / `resetDataDir` 失败必须上抛 —— 「已重置」「已清理」「已记录」都是断言，不能让失败静默通过。
