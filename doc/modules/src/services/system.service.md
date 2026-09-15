# system.service（系统服务）

> 源文件：`src/services/system.service.ts`

## 职责

客户端自身运行信息（存储目录 / 日志路径 / 版本号）的读取入口：透传 `system.bridge` 调用。不做缓存 —— 版本号构建期固定，但存储目录可能随用户数据迁移变化，设置页每次挂载都取实时值。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `SystemService.getStorageInfo` | `() => Promise<StorageInfo>` | 存储信息（baseDir / logPath / version）；失败由调用方降级展示（设置页「关于」区块显示「获取失败」，不影响其余分组） |
| `systemService` | 单例 | 全局唯一实例 |

## 直接依赖

- `@/bridge/system.bridge`（`getStorageInfo` → IPC `get_storage_info`）
- `@/types/system`（仅类型）

## 被依赖

- `views/SettingsView.vue` — 「关于」区块（版本 / 存储目录 / 日志路径 + 逐项复制）

## 逻辑流程

```
getStorageInfo() → systemBridge.getStorageInfo() → ipcInvoke('get_storage_info')
                   → Rust commands::system_cmd::StorageInfo
```

设计要点：分层约定 View / Store 不得直接调用 `system.bridge`，统一经本服务消费；服务本身无状态、无兜底值（与 config.service 的「读取失败回退默认」不同，存储信息缺失时降级责任在调用方 UI）。
