# config.bridge（配置 IPC 桥接）

> 源文件：`src/bridge/config.bridge.ts`

## 职责

配置读写 Tauri IPC 的薄封装：`get_config` / `update_config` / `reload_config` 三个 Command 的透传。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `getConfig` | `() => Promise<AppConfig>` | invoke `get_config`，返回完整应用配置（user/oj/editor/theme/layout 五组） |
| `updateConfig` | `(config: AppConfig) => Promise<void>` | invoke `update_config`，**整体替换**语义（Rust 端持久化到 config.json 并更新内存值）——调用方必须先读当前值再改（由 `config.service.updateConfig` 编排） |
| `reloadConfig` | `() => Promise<void>` | invoke `reload_config`，触发后端从磁盘重载（发布 ConfigReloaded 事件） |

## 直接依赖

- `@/bridge`（`ipcInvoke`）
- `@/types/config`（仅 `AppConfig` 类型）

## 被依赖

- `services/config.service.ts` — 唯一调用方（进程内缓存 Promise，并发共享同一次 IPC）

## 逻辑流程

```
config.service.getConfig（缓存未命中时）→ ipcInvoke('get_config') → Rust commands::config_cmd
config.service.updateConfig（设置页保存）→ 读当前 → 副本变更 → ipcInvoke('update_config') → invalidate
```

设计要点：

- 分层约定：View/Store 不得直接调本桥接，配置读写一律经 `config.service`
  （见 `services/config.service.md`）。
