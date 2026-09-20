# system（客户端存储信息与本地数据占用类型）

> 源文件：`src/types/system.ts`

## 职责

设置页「关于」「重置与清理」展示用的跨端契约：客户端存储信息（`StorageInfo`）与本地数据占用 / 清理结果（`LocalDataUsage` / `PurgeReport`），对应 Rust `commands::config_cmd::StorageInfo` 与 `commands::maintenance_cmd` 的 VO。

## 核心类型/函数

| 名称 | 结构 | 说明 |
|------|------|------|
| `StorageInfo` | `{ baseDir, logPath, version }` | 本地存储根目录 / 日志文件完整路径 / 客户端版本号（构建期注入） |
| `LocalDataUsage` | `{ logBytes, logPath, snapshotTotalCount, snapshotTotalBytes, snapshotStaleCount, snapshotStaleBytes, keepDays }` | 可清理项的体积预览。`keepDays` 是留档保留窗口，早于它的留档计入 `snapshotStale*` |
| `PurgeReport` | `{ freedBytes, removedSnapshots, logCleared }` | 清理结果。`logCleared` 为 `false` 有三种成因：文件层不可用（磁盘只读）、截断失败、或本次没勾选日志 —— 界面据此提示「日志未清理」并以警告色呈现，而不是笼统报「已清理」 |

## 直接依赖

无（纯类型模块）。

## 被依赖

- `bridge/system.bridge.ts`、`services/system.service.ts`、`views/SettingsView.vue`
