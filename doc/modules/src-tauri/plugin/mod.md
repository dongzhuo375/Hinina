# plugin 模块

见[总览](README.md)。依赖方向为JS Runtime → 安全API Adapter → 只读Service/专用Infra；Core/Service不依赖plugin。PluginHost仅消费唯一CoreEventBus，Manager持有独立worker控制器。
