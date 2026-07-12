use std::path::PathBuf;
use std::sync::Arc;

use crate::adapter::hoj::HOJAdapter;
use crate::core::error::AppResult;
use crate::core::event::event_bus::EventBus;
use crate::core::provider::oj_type::OJType;
use crate::core::provider::registry::ProviderRegistry;
use crate::infra::fs_config_repo::FsConfigRepository;
use crate::infra::fs_workspace_repo::FsWorkspaceRepository;
use crate::infra::http::HttpClient;
use crate::infra::logger::Logger;
use crate::infra::provider_registry_impl::ProviderRegistryImpl;
use crate::infra::storage::Storage;
use crate::service::auth::AuthService;
use crate::service::config::ConfigService;
use crate::service::contest::ContestService;
use crate::service::problem::ProblemService;
use crate::service::submission::SubmissionService;
use crate::service::theme::ThemeService;
use crate::service::workspace::manager::WorkspaceManager;

/// 统一应用上下文。
///
/// 在 `main.rs` 启动时装配，注入到 Tauri State 中。
/// 所有 Service 通过 AppContext 获取依赖，避免相互直接引用。
pub struct AppContext {
    pub event_bus: Arc<EventBus>,
    pub config: Arc<ConfigService<FsConfigRepository>>,
    pub provider_registry: Arc<dyn ProviderRegistry>,
    pub workspace_manager: Option<Arc<WorkspaceManager>>,
    pub http_client: Arc<HttpClient>,
    pub storage: Arc<Storage>,
    pub logger: Arc<Logger>,
    // ── Service 层 ──
    pub theme: Arc<ThemeService<FsConfigRepository>>,
    pub auth: Arc<AuthService>,
    pub contest: Arc<ContestService>,
    pub problem: Arc<ProblemService>,
    pub submission: Arc<SubmissionService>,
}

impl AppContext {
    /// 按依赖顺序初始化所有基础设施并装配 AppContext。
    ///
    /// # 初始化序列
    ///
    /// 1. Logger — 日志系统
    /// 2. Storage — 文件系统（base_dir 由调用方传入，通常为 Tauri app_data_dir）
    /// 3. EventBus — 事件总线
    /// 4. ConfigService — 配置管理（通过 FsConfigRepository 持久化）
    /// 5. HttpClient — 网络客户端
    /// 6. ProviderRegistry — OJ 适配器注册中心（默认 HOJ，Provider 在阶段 5 注册）
    /// 7. WorkspaceManager — `None`（WorkspaceManager 实现后补全）
    /// 8. 装配 AppContext
    pub async fn init(base_dir: PathBuf) -> AppResult<Self> {
        // 1. 初始化日志
        Logger::init();
        tracing::info!("Hinina 启动中... base_dir={}", base_dir.display());

        // 确保 base_dir 存在
        std::fs::create_dir_all(&base_dir).map_err(|e| {
            crate::core::error::AppError::Io(format!("创建 base_dir 失败: {}", e))
        })?;

        // 2. 初始化文件存储
        let storage = Arc::new(Storage::new(base_dir));

        // 3. 创建事件总线
        let event_bus = Arc::new(EventBus::new());

        // 4. 加载配置（通过 FsConfigRepository）
        let config_repo = Arc::new(FsConfigRepository::new(
            Arc::clone(&storage),
            "config.json",
        ));
        let config = Arc::new(ConfigService::new(config_repo, Arc::clone(&event_bus)));

        // 5. 初始化 HTTP 客户端
        let http_client = Arc::new(HttpClient::new().map_err(|e| {
            crate::core::error::AppError::Network(format!("HttpClient 创建失败: {}", e))
        })?);

        // 6. 创建 Provider 注册中心，默认使用 HOJ
        let provider_registry: Arc<dyn ProviderRegistry> =
            Arc::new(ProviderRegistryImpl::new(OJType::HOJ));

        // 6.5 注册 HOJ Adapter（阶段 5）
        {
            let hoj_base = config.get().oj.hoj_url;
            tracing::info!(base_url = hoj_base, "HOJ Adapter 注册中");
            let hoj = Arc::new(HOJAdapter::new(
                Arc::clone(&http_client),
                hoj_base,
            ));
            provider_registry.register_auth(OJType::HOJ, Arc::clone(&hoj) as Arc<dyn crate::core::provider::auth::AuthProvider>);
            provider_registry.register_contest(OJType::HOJ, Arc::clone(&hoj) as Arc<dyn crate::core::provider::contest::ContestProvider>);
            provider_registry.register_problem(OJType::HOJ, Arc::clone(&hoj) as Arc<dyn crate::core::provider::problem::ProblemProvider>);
            provider_registry.register_submission(OJType::HOJ, Arc::clone(&hoj) as Arc<dyn crate::core::provider::submission::SubmissionProvider>);
        }

        // 7. 创建 WorkspaceManager
        let workspace_repo = Arc::new(FsWorkspaceRepository::new(Arc::clone(&storage)));
        let workspace_manager = Some(Arc::new(WorkspaceManager::new(
            workspace_repo,
            Arc::clone(&event_bus),
        )));

        // 7.5 装配 Service 层
        let theme = Arc::new(ThemeService::new(
            Arc::clone(&config),
            Arc::clone(&event_bus),
        ));
        let auth = Arc::new(AuthService::new(
            Arc::clone(&provider_registry) as Arc<dyn ProviderRegistry>,
            Arc::clone(&storage),
            Arc::clone(&event_bus),
        ));
        let contest = Arc::new(ContestService::new(
            Arc::clone(&provider_registry) as Arc<dyn ProviderRegistry>,
            Arc::clone(&event_bus),
        ));
        let problem = Arc::new(ProblemService::new(
            Arc::clone(&provider_registry) as Arc<dyn ProviderRegistry>,
            Arc::clone(&event_bus),
        ));
        let submission = Arc::new(SubmissionService::new(
            Arc::clone(&provider_registry) as Arc<dyn ProviderRegistry>,
            Arc::clone(&event_bus),
        ));

        // 8. 装配
        Ok(Self {
            event_bus,
            config,
            provider_registry,
            workspace_manager,
            http_client,
            storage,
            logger: Arc::new(Logger),
            theme,
            auth,
            contest,
            problem,
            submission,
        })
    }
}
