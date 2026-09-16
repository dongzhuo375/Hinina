# context

## 职责
定义统一应用上下文 `AppContext`，在启动时按依赖顺序装配所有基础设施和 7 个 Service 层实例，注入到 Tauri State 中供所有 Command 通过依赖注入使用。

## 核心类型/函数
- **`AppContext`** — 统一应用上下文 struct，持有所有基础设施和 Service 的 `Arc` 引用。
  字段：`event_bus`, `config: Arc<ConfigService<FsConfigRepository>>`, `provider_registry: Arc<dyn ProviderRegistry>`, `workspace_manager: Option<Arc<WorkspaceManager>>`, `http_client`, `storage`, `logger`, `theme: Arc<ThemeService<FsConfigRepository>>`, `auth: Arc<AuthService>`, `contest: Arc<ContestService>`, `problem: Arc<ProblemService>`, `submission: Arc<SubmissionService>`
- **`AppContext::init(base_dir: PathBuf) -> AppResult<Self>`** — 异步初始化序列：
  1. Logger — 日志系统初始化（`Logger::init(&base_dir)`，stderr + `{base_dir}/logs/hinina.log` 双路输出）
  2. `create_dir_all` — 确保 base_dir 存在
  3. Storage — 文件系统（base_dir 传入）
  4. EventBus — 事件总线
  5. ConfigService — 通过 `FsConfigRepository` 加载配置（首次启动使用默认值并持久化；加载路径做旧值归一）
  6. HttpClient — 网络客户端，**超时取自配置 `oj.timeout_secs`**（`HttpClient::with_timeout`，`.max(1)` 防 0 值，不再硬编码 30s）
  7. ProviderRegistry — 默认 HOJ（HOJAdapter 注入 `Arc<EventBus>`，token 轮换时发布 `AuthEvent::TokenRefreshed`）
  8. WorkspaceManager — 通过 `FsWorkspaceRepository` 创建，包装为 `Some(Arc<...>)`
  9. 装配 5 个 Service：ThemeService → AuthService → ContestService（注入 `Arc<Storage>`，供公告已读状态持久化 `announcements_read/`）→ ProblemService（注入 `Arc<Storage>`，供题目 limits 磁盘缓存 `cache/problem_limits/`）→ SubmissionService
  10. 装配 AppContext 并返回

## 直接依赖
- `core::event::event_bus::EventBus`
- `core::provider::registry::ProviderRegistry`
- `core::provider::oj_type::OJType`
- `core::error::AppResult`
- `infra::http::HttpClient`
- `infra::logger::Logger`
- `infra::storage::Storage`
- `infra::fs_config_repo::FsConfigRepository`
- `infra::fs_workspace_repo::FsWorkspaceRepository`
- `infra::provider_registry_impl::ProviderRegistryImpl`
- `service::config::ConfigService`
- `service::theme::ThemeService`
- `service::auth::AuthService`
- `service::contest::ContestService`
- `service::problem::ProblemService`
- `service::submission::SubmissionService`
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
`AppContext::init(base_dir)` 按依赖顺序初始化：Logger（双路输出，日志文件落在 base_dir 下，故最先拿到 base_dir）→ Storage → EventBus → ConfigService → HttpClient（超时由配置注入）→ ProviderRegistry → WorkspaceManager → 逐个装配 Service（theme → auth → contest → problem → submission）→ 装配 AppContext。所有 Service 通过 Arc 共享 EventBus、ConfigService、ProviderRegistry 和 Storage（AuthService 用于会话持久化，ContestService 用于公告已读状态，ProblemService 用于 limits 磁盘缓存）。WorkspaceManager 在 Phase 4 已补全，不再是 `None`。
