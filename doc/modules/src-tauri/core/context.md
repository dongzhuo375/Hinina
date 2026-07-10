# context

## 职责
定义统一应用上下文 `AppContext`，在启动时装配所有基础设施（EventBus、ConfigService、ProviderRegistry、WorkspaceManager、HttpClient、Storage、Logger），注入到 Tauri State 中供所有 Service 通过依赖注入使用。

## 核心类型/函数
- **`AppContext`** — 统一应用上下文 struct，持有所有基础设施的 `Arc` 引用。
  `workspace_manager` 为 `Option<Arc<WorkspaceManager>>`，当前为 `None`（阶段 2/4 补全）。
- **`AppContext::init(base_dir: PathBuf) -> AppResult<Self>`** — 按 8 步初始化序列装配 AppContext：
  1. Logger — 日志系统初始化
  2. `create_dir_all` — 确保 base_dir 存在
  3. ConfigService — 空壳实例（阶段 4 实现）
  4. Storage — 文件系统（base_dir 由调用方传入）
  5. HttpClient — 网络客户端
  6. EventBus — 事件总线
  7. ProviderRegistry — 默认 HOJ（Provider 在阶段 5 注册）
  8. WorkspaceManager — `None`（阶段 2/4 补全）

## 直接依赖
- `core::event::event_bus::EventBus`
- `core::provider::registry::ProviderRegistry`
- `core::provider::oj_type::OJType`
- `core::error::AppResult`
- `infra::http::HttpClient`
- `infra::logger::Logger`
- `infra::storage::Storage`
- `infra::provider_registry_impl::ProviderRegistryImpl`
- `service::config::ConfigService`
- `service::workspace::manager::WorkspaceManager`
- `tracing`（启动日志）

## 被依赖
- `commands::auth_cmd`
- `commands::config_cmd`
- `commands::contest_cmd`
- `commands::problem_cmd`
- `commands::submission_cmd`
- `commands::theme_cmd`
- `commands::workspace_cmd`

## 逻辑流程
`AppContext::init(base_dir)` 按序初始化：Logger → ConfigService → Storage → HttpClient → EventBus → ProviderRegistry(HOJ) → WorkspaceManager(todo!) → 装配。当前 WorkspaceManager 为运行时占位，待阶段 2（Repository）和阶段 4（Service）补全。
