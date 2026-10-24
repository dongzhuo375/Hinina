# plugin/host

## 职责
宿主侧插件基础设施：纯清单校验、独立错误映射、事件适配和有界交付。运行时生命周期在 [plugin/manager](../manager.md)，文件 I/O 在 Infra。

## 子模块
| 模块 | 职责 | 文档 |
|---|---|---|
| `manifest.rs` | Manifest v2 解析校验、权限/事件枚举、origin 校验 | [manifest.md](manifest.md) |
| `api_error.rs` | AppError → PluginError 脱敏映射 | [api_error.md](api_error.md) |
| `event_adapter.rs` | CoreEvent → PluginEvent 白名单/裁剪投影 | [event_adapter.md](event_adapter.md) |
| `plugin_host.rs` | 唯一 CoreEventBus 插件消费者：过滤与有界投递 | [plugin_host.md](plugin_host.md) |
| `extension.rs` | 远期 UI 扩展点类型草案（当前无调用者） | [extension.md](extension.md) |

## 直接依赖
- `core::error::AppError`（api_error）、`core::event::core_event::CoreEvent`（event_adapter/plugin_host）
- `plugin::api`（错误/事件/权限 DTO）

## 被依赖
- `plugin::manager`（订阅与交付消费）、`plugin::permission`（授权快照）

## 逻辑流程
- Host 不运行 JS、不加载文件、不管理业务动作；事件从 CoreEventBus 经 PluginHost 白名单/权限过滤后进入每插件有界队列
- 参见 [Manifest](manifest.md)、[PluginHost](plugin_host.md)、[总览](../README.md)
