# 插件宿主模块文档索引

> 本目录是 `src-tauri/src/plugin/` 的**逐文件模块文档**（五节格式，见 [模块文档约定](../../README.md)）；插件系统的设计、契约、教程与路线图在 [doc/plugin/](../../../plugin/README.md)，不在本目录重复。

| 文档 | 对应源文件 | 职责一句话 |
|---|---|---|
| [mod.md](mod.md) | `plugin/mod.rs` | 模块入口与依赖方向 |
| [api/mod.md](api/mod.md) | `plugin/api/` | 插件可见 DTO、能力 trait、错误白名单 |
| [permission/mod.md](permission/mod.md) | `plugin/permission/mod.rs` | 宿主批准 ∩ 清单声明的授权快照 |
| [query.md](query.md) | `plugin/query.rs` | 作用域签发、只读查询、游标分页、Provider 预算 |
| [execution_api.md](execution_api.md) | `plugin/execution_api.rs` | 能力装配与有限 JSON 桥 |
| [bounded_json.md](bounded_json.md) | `plugin/bounded_json.rs` | 流式有界 JSON 编码 |
| [host/](host/mod.md) | `plugin/host/` | 清单校验、错误映射、事件适配、有界交付 |
| [registry/mod.md](registry/mod.md) | `plugin/registry/mod.rs` | 配置/清单/源码加载端口 |
| [runtime/](runtime/mod.md) | `plugin/runtime/` | 引擎端口与 QuickJS 适配 |
| [manager.md](manager.md) | `plugin/manager.rs` | 启动/停止/重载/Resync 生命周期 |
| [limits.md](limits.md) | `plugin/limits.rs` | 全宿主共用的期限与字节预算常量 |

专题内容（无对应源文件，统一放在 [doc/plugin/](../../../plugin/README.md)）：

- 安全模型 → [architecture.md §6](../../../plugin/architecture.md)
- 开发教程（作者快速开始）→ [development.md §1](../../../plugin/development.md)
- 验证记录 → [history.md](../../../plugin/history.md)
- 路线图与已知缺口 → [roadmap.md](../../../plugin/roadmap.md)

## 直接依赖（Cargo.toml）

`rquickjs`（JS 引擎，构建需 C 编译器）、`atomicwrites` + `fs4`（原子写与跨实例锁）、`governor`（令牌桶限流）、`semver`（清单与最低版本校验）、`sha2`（存储命名空间与身份键哈希）、`uuid`（句柄/游标不透明 ID）、`url` + `ipnet` + `ip_network`（精确 origin 与地址策略）、`tokio-util`（取消令牌）；dev-dependency 为 tokio `test-util`（虚拟时间）。

## 参考业务插件

[sdk/examples/balloon](sdk/examples/balloon/README.md)——首次 AC 气球派发参考实现（文档侧代码，不构建进应用），含逻辑测试与验收指南。
