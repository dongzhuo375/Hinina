# plugin 模块

## 职责
外围 JavaScript 插件宿主的模块入口与依赖方向声明：把 QuickJS 隔离执行、安全查询、私有存储/受控 HTTP/通知/日志与插件生命周期装配为一个子系统。依赖方向为 **JS Runtime → 安全 API Adapter → 只读 Service/专用 Infra**；Core/Service 不依赖 plugin。PluginHost 仅消费唯一 CoreEventBus，Manager 持有独立 worker 控制器。

## 子模块
| 模块 | 职责 | 文档 |
|---|---|---|
| `api/` | 插件可见 DTO、能力 trait、错误白名单 | [api/mod.md](api/mod.md) |
| `permission/` | 宿主批准 ∩ 清单声明的不可变授权快照 | [permission/mod.md](permission/mod.md) |
| `query.rs` | 作用域签发、只读查询、游标分页、Provider 预算 | [query.md](query.md) |
| `execution_api.rs` | 能力装配与有限 JSON 桥 | [execution_api.md](execution_api.md) |
| `bounded_json.rs` | 流式有界 JSON 编码 | [bounded_json.md](bounded_json.md) |
| `host/` | 清单校验、错误映射、事件适配、有界交付 | [host/mod.md](host/mod.md) |
| `registry/` | 配置/清单/源码加载端口 | [registry/mod.md](registry/mod.md) |
| `runtime/` | 引擎端口与 QuickJS 适配 | [runtime/mod.md](runtime/mod.md) |
| `manager.rs` | 启动/停止/重载/Resync 生命周期 | [manager.md](manager.md) |
| `limits.rs` | 全宿主共用的期限与字节预算常量 | [limits.md](limits.md) |

无源文件的专题内容统一放在 [doc/plugin/](../../../plugin/README.md)：安全模型（[architecture.md §6](../../../plugin/architecture.md)）、开发教程（[development.md §1](../../../plugin/development.md)）、验证记录（[history.md](../../../plugin/history.md)）、路线图（[roadmap.md](../../../plugin/roadmap.md)）；模块索引见 [README.md](README.md)。

## 直接依赖
- `core::access`（AccessRevision 撤销栅栏）
- `core::event::core_event_bus`（唯一事件源，PluginHost 为消费者之一）
- 只读 Service 读路径（经 `QuerySource` 端口，生产实现为 `ServiceQueries`）
- infra 专用端口实现（存储/HTTP/文件仓库/通知）——plugin 层只依赖端口、不引用具体 infra，专用 Infra 反向依赖 plugin 契约（**infra → plugin 单向**），见 [Architecture](../../../Architecture.md)

## 被依赖
- `commands/plugin_cmd.rs`（start/stop/reload/resync 开发命令）
- `core::context`（AppContext 持有 Send 控制器与工厂装配）

## 逻辑流程
- 内部 [limits](limits.md) 统一执行期限与响应/游标预算，[bounded_json](bounded_json.md) 以流式编码实施字节上限
- 测试挂载：manager/query 各自在所属模块用 `#[cfg(test)]` + `#[path = "tests/{module}_tests.rs"]` 引入，不经 tests/mod.rs 聚合；`tests/support.rs` 由 plugin/mod.rs 直接挂载为仅测试可用的 `test_support`，供跨模块夹具复用
- 业务规则（首 AC 判断、幂等重试、外部去重）不在宿主：参考实现见 [sdk/examples/balloon](sdk/examples/balloon/README.md)
