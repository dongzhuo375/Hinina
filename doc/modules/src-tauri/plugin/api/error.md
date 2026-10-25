# api/error

## 职责
插件专用稳定错误：`PluginError` / `PluginResult` 是 JS 侧唯一可见的错误形态，无内部文本或栈；内部 AppError 的映射留在 Host（见 [api_error](../host/api_error.md)）。

## 核心类型/函数
- `pub enum PluginError` — `PermissionDenied { permission }` 与 `InvalidInput / ContextExpired / Unauthenticated / NotFound / ProviderError / LimitExceeded / Timeout / Unavailable / Unsupported / ScriptError`（serde 序列化为稳定 snake_case code）
- `pub type PluginResult<T> = Result<T, PluginError>`

## 直接依赖
- `plugin::host::manifest::PluginPermission`（PermissionDenied 载荷）
- `serde`、`thiserror`

## 被依赖
- `plugin::api` 全部能力 trait 与实现
- `plugin::host::api_error`（AppError → PluginError 映射）

## 逻辑流程
- 错误载荷仅允许 `code` 和可选字符串 `permission`；Rust 穷尽序列化测试与前端形态测试共同锁定该契约
- 完整权限、字段与错误语义见 [Plugin API 规范](mod.md)
