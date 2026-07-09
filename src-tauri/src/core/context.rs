use std::path::PathBuf;
use std::sync::Arc;

use crate::core::error::AppResult;
use crate::core::event::event_bus::EventBus;
use crate::core::provider::oj_type::OJType;
use crate::core::provider::registry::ProviderRegistry;
use crate::infra::http::HttpClient;
use crate::infra::logger::Logger;
use crate::infra::provider_registry_impl::ProviderRegistryImpl;
use crate::infra::storage::Storage;
use crate::service::config::ConfigService;
use crate::service::workspace::manager::WorkspaceManager;

/// 统一应用上下文。
///
/// 在 `main.rs` 启动时装配，注入到 Tauri State 中。
/// 所有 Service 通过 AppContext 获取依赖，避免相互直接引用。
pub struct AppContext {
    pub event_bus: Arc<EventBus>,
    pub config: Arc<ConfigService>,
    pub provider_registry: Arc<dyn ProviderRegistry>,
    pub workspace_manager: Arc<WorkspaceManager>,
    pub http_client: Arc<HttpClient>,
    pub storage: Arc<Storage>,
    pub logger: Arc<Logger>,
}

impl AppContext {
    /// 按顺序初始化所有基础设施并装配 AppContext。
    ///
    /// # 初始化序列
    ///
    /// 1. Logger — 日志系统
    /// 2. ConfigService — 配置管理（当前为空壳，阶段 4 实现）
    /// 3. Storage — 文件系统（base_dir 由调用方传入，通常为 Tauri app_data_dir）
    /// 4. HttpClient — 网络客户端
    /// 5. EventBus — 事件总线
    /// 6. ProviderRegistry — OJ 适配器注册中心（默认 HOJ）
    /// 7. WorkspaceManager —（阶段 2/4 实现，当前占位）
    /// 8. 装配 AppContext
    #[allow(unreachable_code, unused_variables)]
    pub async fn init(base_dir: PathBuf) -> AppResult<Self> {
        // 1. 初始化日志
        Logger::init();
        tracing::info!("Hinina 启动中... base_dir={}", base_dir.display());

        // 2. 加载配置（阶段 4 实现，当前为空壳）
        let config = Arc::new(ConfigService::new());

        // 3. 初始化文件存储
        let storage = Arc::new(Storage::new(base_dir));

        // 4. 初始化 HTTP 客户端
        let http_client = Arc::new(HttpClient::new().map_err(|e| {
            crate::core::error::AppError::Network(format!("HttpClient 创建失败: {}", e))
        })?);

        // 5. 创建事件总线
        let event_bus = Arc::new(EventBus::new());

        // 6. 创建 Provider 注册中心，默认使用 HOJ
        let provider_registry: Arc<dyn ProviderRegistry> =
            Arc::new(ProviderRegistryImpl::new(OJType::HOJ));

        // 7. 创建 Workspace 管理器（阶段 2/4 实现 WorkspaceRepository 后补全）
        let workspace_manager = Arc::new(
            // TODO: 需要 WorkspaceRepository trait 实现（阶段 2）
            // WorkspaceManager::new(repo, event_bus.clone(), workspaces_dir)
            todo!("AppContext::init() — WorkspaceManager 依赖 WorkspaceRepository（阶段 2）")
        );

        // 8. 装配
        Ok(Self {
            event_bus,
            config,
            provider_registry,
            workspace_manager,
            http_client,
            storage,
            logger: Arc::new(Logger),
        })
    }
}
