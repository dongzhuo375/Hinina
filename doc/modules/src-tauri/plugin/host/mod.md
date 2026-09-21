# mod

## 职责
插件宿主（Host）子模块的入口，声明并公开四个子模块：`manifest`（manifest 与权限模型）、`extension`（扩展点契约）、`event_adapter`（`CoreEvent` → `PluginEvent` 的唯一转换点）、`plugin_host`（`PluginHost`：订阅生命周期、权限校验、投递与引用环约束）。

## 核心类型/函数
- 无（仅模块声明）

## 直接依赖
- 无

## 被依赖
- `plugin/mod.rs` — 通过 `pub mod host` 声明子模块

## 逻辑流程
无（仅模块声明）
