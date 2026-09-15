# system.bridge（客户端存储信息 IPC 封装）

> 源文件：`src/bridge/system.bridge.ts`

## 职责

获取客户端存储信息（存储根目录 / 日志文件路径 / 版本号）的 Tauri IPC 薄封装，供设置页「关于」展示。

## 核心类型/函数

| 名称 | 签名 | 对应命令 |
|------|------|----------|
| `getStorageInfo` | `() => Promise<StorageInfo>` | `get_storage_info` |

## 直接依赖

- `@/bridge`（`ipcInvoke`）
- `@/types/system`（仅类型）

## 被依赖

- `services/system.service.ts`（唯一消费方）

## 逻辑流程

纯透传，无参数。
