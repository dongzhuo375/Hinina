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
  7. ProviderRegistry — 初始当前 OJ 取 `OjId::new(&oj.active)`；随后按配置实例注册全部内建 OJ（工厂数据化，约 8 行 for 循环）：组装 `AdapterDeps`（http / event_bus / storage，仅 infra）→ 遍历 `oj.instances` 中 enabled 的实例并与 `adapter::factories()` 按 id 匹配（无对应适配器 `warn!` 跳过）→ `factory.build(&deps, &instance.base_url)` 得 `ProviderSet` 后 `register`（HOJAdapter 仍经 deps 注入 `Arc<EventBus>`，token 轮换时发布 `AuthEvent::TokenRefreshed`）。注册完成后校验当前 OJ 已注册，未注册（配置手改/拼写错误）`warn!` 并**回退首个已注册 OJ**（不硬编码 HOJ —— HOJ 实例可能被禁用/移除，那是死路；一个都没注册时仅告警，首次查询以 `ProviderNotFound` 如实暴露）
  8. WorkspaceManager — 通过 `FsWorkspaceRepository` 创建，包装为 `Some(Arc<...>)`
  9. 装配 5 个 Service：ThemeService → AuthService → ContestService（注入 `Arc<Storage>`，供公告已读状态持久化 `announcements_read/`）→ ProblemService（注入 `Arc<Storage>`，供题目 limits 磁盘缓存 `cache/problem_limits/`）→ SubmissionService
  10. 装配 AppContext 并返回

- **`AppContext::ensure_oj_registered(oj_id: &str) -> bool`** — **按需注册**某个已配置且启用的 OJ 实例（幂等，返回是否本次新注册）。存在的理由：注册只发生在 `init`，而设置页允许用户从 OJ 枚举里挑一个尚未配置的类型、填地址保存后立即切换 —— 不补注册用户就得重启客户端（`commands/oj_cmd::switch_oj` 在校验前调用它）。判定与注册**与 `init` 同源**：共用 [`enabled_instance`] 与 [`register_instance`]，不能凭空激活未配置的 OJ，否则 `switch_oj` 会绕过配置成为后门
- **`enabled_instances(config) -> impl Iterator<Item = &OjInstance>`**（私有）— 已启用的实例；**「启用」条件的唯一来源**（`init` 的全量注册与 `enabled_instance` 的单实例查找都经此，避免两处判定漂移）
- **`enabled_instance(config, oj_id) -> Option<&OjInstance>`**（私有纯函数）— 从已启用实例里按 id 精确查找
- **`register_instance(registry, deps, instance) -> bool`**（私有）— 按配置实例经 `adapter::factories()` 匹配工厂、`factory.build(deps, base_url)` 构造 `ProviderSet` 并注册；**`init` 与按需注册共用同一实现**（抽成自由函数而非方法：`init` 执行时 `AppContext` 尚未装配完成）。未知 OJ（无匹配工厂）→ `warn` + `false`，不 panic；重复注册幂等

## 直接依赖

- `adapter::hoj::HOJAdapter` 及 `adapter::{AdapterDeps, factories}`（实例注册循环；未注册 active 的回退目标取自 `list_available()` 首项，不再引用 HOJAdapter::ID）
- `core::event::event_bus::EventBus`
- `core::provider::registry::ProviderRegistry`
- `core::provider::oj_id::OjId`
- `core::entity::config::{AppConfig, OjInstance}`（`ensure_oj_registered` 的实例判定）
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
- `commands::oj_cmd`
- `commands::problem_cmd`
- `commands::submission_cmd`
- `commands::theme_cmd`
- `commands::workspace_cmd`

## 逻辑流程
`AppContext::init(base_dir)` 按依赖顺序初始化：Logger（双路输出，日志文件落在 base_dir 下，故最先拿到 base_dir）→ Storage → EventBus → ConfigService → HttpClient（超时由配置注入）→ ProviderRegistry（当前 OJ 取 `oj.active`，按 `oj.instances` 的 enabled 实例匹配工厂注册全部内建 OJ，active 未注册回退**首个已注册 OJ**并告警）→ WorkspaceManager → 逐个装配 Service（theme → auth → contest → problem → submission）→ 装配 AppContext。所有 Service 通过 Arc 共享 EventBus、ConfigService、ProviderRegistry 和 Storage（AuthService 用于会话持久化，ContestService 用于公告已读状态，ProblemService 用于 limits 磁盘缓存）。WorkspaceManager 在 Phase 4 已补全，不再是 `None`。

**按需注册路径**（设置页新建实例后立即切换）：
```
switch_oj(id) → AppContext::ensure_oj_registered(id)
                  ├─ 已注册 ────────────────────► false（幂等短路）
                  ├─ 配置里无该 id / 实例被禁用 ► false（不凭空激活）
                  ├─ 无匹配工厂 ────────────────► false + warn
                  └─ 命中 ──► factories().find(id).build(&AdapterDeps, base_url)
                              → registry.register(OjId, ProviderSet) → true
                → 常规校验（未注册则 ProviderNotFound）→ 切换 → 发布 OJSwitched → 持久化 active
```

## 测试
`src-tauri/src/core/tests/context_tests.rs`（12 例）分两组：

**实例判定**（`enabled_instance` / `enabled_instances`）：命中已启用实例、**跳过禁用实例**（否则会绕开「禁用」开关）、未配置 id / 大小写不符 / 空串一律 `None`（id 是精确匹配的键，不做模糊归一）、空实例清单、默认配置恰有一个启用实例（开箱即用的 HOJ）。

**注册路径**（`register_instance`，用真实注册表 + 真实工厂）：已知 OJ 注册后出现在 `list_available()`、未知 OJ → `false` 且不注册任何东西（不 panic）、**重复注册幂等**（不产生重复项）、工厂按 id **精确匹配**（`hoj` 不是 `HOJ`）。这组用例的价值在于锁定「启动注册与按需注册共用同一实现」—— 若两处各自实现而漂移，会退化成「重启后能用、切换时不能用」这类最难排查的不一致。

`ensure_oj_registered` 本身需要完整 `AppContext`（http / 事件总线 / 存储 / 注册表），构造代价大于收益；其正确性由「只调 `enabled_instance` + `register_instance`」这条结构约束 + 上两组用例保证。
