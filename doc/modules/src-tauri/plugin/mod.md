# plugin 模块

见[总览](README.md)。依赖方向为JS Runtime → 安全API Adapter → 只读Service/专用Infra；Core/Service不依赖plugin。PluginHost仅消费唯一CoreEventBus，Manager持有独立worker控制器。

内部 [limits](limits.md) 统一执行期限与响应/游标预算，[bounded_json](bounded_json.md) 以流式编码实施字节上限。专用Infra调用ScopedQueries授权的依赖例外见 [Architecture](../../../Architecture.md)。
