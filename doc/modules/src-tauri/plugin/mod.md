# plugin 模块

见[总览](README.md)。依赖方向为JS Runtime → 安全API Adapter → 只读Service/专用Infra；Core/Service不依赖plugin。PluginHost仅消费唯一CoreEventBus，Manager持有独立worker控制器。

内部 [limits](limits.md) 统一执行期限与响应/游标预算，[bounded_json](bounded_json.md) 以流式编码实施字节上限。plugin 层只依赖存储/HTTP/配置端口，不引用具体 infra；专用 Infra 实现这些端口并依赖 plugin 契约（infra → plugin 单向），详见 [Architecture](../../../Architecture.md)。

manager/query各自在所属模块中用#[cfg(test)]及#[path = "tests/{module}_tests.rs"]挂载测试，不再通过tests/mod.rs聚合。tests/support.rs由plugin/mod.rs直接挂载为仅测试可用的test_support，供跨模块夹具复用。
