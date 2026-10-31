# system.bridge（客户端存储信息与维护 IPC 封装）

> 源文件：`src/bridge/system.bridge.ts`

## 职责

客户端存储信息读取（存储根目录 / 日志文件路径 / 版本号）、本地维护操作（重置客户端 / 清理本地数据）与启动标志读取（`--settings` 管理方钥匙）的 Tauri IPC 薄封装，供设置页「关于」「重置与清理」展示与启动路由/分层锁判定。

## 核心类型/函数

| 名称 | 签名 | 对应命令 |
|------|------|----------|
| `getLaunchFlags` | `() => Promise<LaunchFlags>` | `launch_flags`（Rust `commands::app_cmd`；消费方经 system.service 的 memoize） |
| `getStorageInfo` | `() => Promise<StorageInfo>` | `get_storage_info` |
| `resetClient` | `() => Promise<void>` | `reset_client` |
| `localDataUsage` | `() => Promise<LocalDataUsage>` | `local_data_usage` |
| `purgeLocalData` | `(logs: boolean, staleSnapshots: boolean) => Promise<PurgeReport>` | `purge_local_data` |
| `getDataDir` | `() => Promise<DataDirInfo>` | `get_data_dir` |
| `setDataDir` | `(path: string, migrate: boolean) => Promise<DataDirChange>` | `set_data_dir` |
| `resetDataDir` | `(migrate: boolean) => Promise<DataDirChange>` | `reset_data_dir` |
| `pickDataDir` | `() => Promise<string \| null>` | `pick_data_dir`（原生目录选择器） |

## 直接依赖

- `@/bridge`（`ipcInvoke`）
- `@/types/system`（`LaunchFlags` / `StorageInfo` / `LocalDataUsage` / `PurgeReport` / `DataDirInfo` / `DataDirChange`）

## 被依赖

- `services/system.service.ts`（唯一消费方）

## 逻辑流程

纯透传。`launch_flags` / `reset_client` 无参数；`purge_local_data` 的两个开关**必填**（不可逆动作不接受隐式范围，不在 bridge 层设默认值）；`set_data_dir` / `reset_data_dir` 的 `migrate` 同样必填。启动标志语义见 `doc/modules/src-tauri/infra/launch_args.md`，清理范围见 `doc/modules/src-tauri/commands/maintenance_cmd.md`，数据目录语义见 `commands/data_dir_cmd.md`。
