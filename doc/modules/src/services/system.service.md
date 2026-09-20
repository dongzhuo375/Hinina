# system.service（系统服务）

> 源文件：`src/services/system.service.ts`

## 职责

客户端自身运行信息与维护操作（存储目录 / 日志路径 / 版本号 / 重置客户端 / 清理本地数据）的入口：透传 `system.bridge` 调用。不做缓存 —— 版本号构建期固定，但存储目录可能随用户数据迁移变化，设置页每次挂载都取实时值。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `SystemService.getStorageInfo` | `() => Promise<StorageInfo>` | 存储信息（baseDir / logPath / version）；失败由调用方降级展示（设置页「关于」区块显示「获取失败」，不影响其余分组） |
| `SystemService.resetClient` | `() => Promise<void>` | 重置客户端（三层缓存 + 公告基线 + 公告已读状态）；**不重拉**，补拉由调用方编排（设置页重置后立刻重拉当前比赛数据） |
| `SystemService.localDataUsage` | `() => Promise<LocalDataUsage>` | 可清理项的体积预览（日志字节数 / 留档总与过期条数字节） |
| `SystemService.purgeLocalData` | `(logs: boolean, staleSnapshots: boolean) => Promise<PurgeReport>` | 清理本地数据（**不可逆**）；范围由调用方显式传入，服务层不设默认值 |
| `SystemService.getDataDir` | `() => Promise<DataDirInfo>` | 当前数据目录 / 默认目录 / 来源 / 是否待重启 |
| `SystemService.setDataDir` | `(path: string, migrate: boolean) => Promise<DataDirChange>` | 更改数据目录（**重启后生效**）；只「校验 + 记下改动」，搬运由下次启动完成 |
| `SystemService.resetDataDir` | `(migrate: boolean) => Promise<DataDirChange>` | 恢复默认数据目录（**重启后生效**） |
| `SystemService.pickDataDir` | `() => Promise<string \| null>` | 原生目录选择器（用户取消返回 null） |
| `systemService` | 单例 | 全局唯一实例 |

## 直接依赖

- `@/bridge/system.bridge`（`getStorageInfo` / `resetClient` / `localDataUsage` / `purgeLocalData` / `getDataDir` / `setDataDir` / `resetDataDir` / `pickDataDir`）
- `@/types/system`（`StorageInfo` / `LocalDataUsage` / `PurgeReport` / `DataDirInfo` / `DataDirChange`）

## 被依赖

- `views/SettingsView.vue` — 「数据目录」（当前目录 / 来源 / 更改 / 恢复默认）、「重置与清理」（重置客户端 + 清理本地数据，均二次确认）与「关于」（版本 / 存储目录 / 日志路径 + 逐项复制）

## 逻辑流程

```
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
