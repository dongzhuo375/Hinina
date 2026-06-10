# context

## 职责
定义统一应用上下文 `AppContext`，在启动时装配所有基础设施（EventBus、ConfigService、ProviderRegistry、WorkspaceManager、HttpClient、Storage、Logger），注入到 Tauri State 中供所有 Service 通过依赖注入使用。

## 核心类型/函数
- **`AppContext`** — 统一应用上下文 struct，持有所有基础设施的 `Arc` 引用
- **`AppContext::init()`** — 按顺序初始化所有基础设施并装配 `AppContext`（TODO）

## 直接依赖
- `core::event::event_bus::EventBus`
- `core::provider::registry::ProviderRegistry`
- `infra::http::HttpClient`
- `infra::logger::Logger`
- `infra::storage::Storage`
- `service::config::ConfigService`
- `service::workspace::manager::WorkspaceManager`

## 被依赖
- `commands::auth_cmd`
- `commands::config_cmd`
- `commands::contest_cmd`
- `commands::problem_cmd`
- `commands::submission_cmd`
- `commands::theme_cmd`
- `commands::workspace_cmd`

## 逻辑流程
`AppContext::init()` 按序初始化：Logger → ConfigService → Storage → HttpClient → EventBus → ProviderRegistry → WorkspaceManager → 装配 AppContext（当前为 TODO 占位）。
