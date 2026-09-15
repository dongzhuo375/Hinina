# config.bridge（配置 IPC 桥接）

> 源文件：`src/bridge/config.bridge.ts`

## 职责

配置读取 Tauri IPC 的薄封装：`get_config` Command 的透传（后端另有 reload/update Command，前端本轮只读）。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `getConfig` | `() => Promise<AppConfig>` | invoke `get_config`，返回完整应用配置（user/oj/editor/theme/layout 五组） |

## 直接依赖

- `@/bridge`（`ipcInvoke`）
- `@/types/config`（仅 `AppConfig` 类型）

## 被依赖

- `services/config.service.ts` — 唯一调用方（进程内缓存 Promise，并发共享同一次 IPC）

## 逻辑流程

```
config.service.getConfig（缓存未命中时）→ ipcInvoke('get_config') → Rust commands::config_cmd
```

设计要点：

- 分层约定：View/Store 不得直接调本桥接，配置读取一律经 `config.service`
  （见 `services/config.service.md`）。
