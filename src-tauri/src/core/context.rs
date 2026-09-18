use std::path::PathBuf;
use std::sync::Arc;

use crate::adapter::hoj::HOJAdapter;
use crate::core::error::AppResult;
use crate::core::event::event_bus::EventBus;
use crate::core::provider::oj_id::OjId;
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
        // 1. 初始化日志（stderr + {base_dir}/logs/hinina.log 双路输出）
        Logger::init(&base_dir);
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

        // 5. 初始化 HTTP 客户端（超时取自配置 oj.timeout_secs，不再硬编码）
        let timeout_secs = config.get().oj.timeout_secs;
        let http_client = Arc::new(
            HttpClient::with_timeout(std::time::Duration::from_secs(timeout_secs.max(1))).map_err(
                |e| {
                    crate::core::error::AppError::Network(format!("HttpClient 创建失败: {}", e))
                },
            )?,
        );

        // 6. 创建 Provider 注册中心 + 注册全部内建 OJ（工厂数据化：注册侧聚合）。
        //
        // 当前 OJ 取自配置的 `user.lastOjType`（身份是数据：字符串 id，不再是
        // 编译期枚举）。未注册的 id（配置手改/拼写错误）在注册完成后回退 HOJ
        // 并告警 —— 兜底语义与旧版 `unwrap_or(HOJ)` 一致。
        let configured_oj = OjId::new(&config.get().user.last_oj_type);
        let provider_registry: Arc<dyn ProviderRegistry> =
            Arc::new(ProviderRegistryImpl::new(configured_oj.clone()));

        let adapter_deps = crate::adapter::AdapterDeps {
            http_client: Arc::clone(&http_client),
            event_bus: Arc::clone(&event_bus),
            storage: Arc::clone(&storage),
        };
        for factory in crate::adapter::factories() {
            let id = OjId::new(factory.id());
            // 按 OJ 取地址的能力随 instances 配置（下一改造）到达；当前 HOJ 是
            // 唯一内建 OJ，地址取自 oj.hoj_url
            let base_url = config.get().oj.hoj_url.clone();
            let set = factory.build(&adapter_deps, &base_url);
            provider_registry.register(id.clone(), set);
            tracing::info!(oj_id = %id, base_url = base_url, "OJ 适配器已注册");
        }

        // 身份字符串化后失去编译期穷尽检查，此为第一道防线：启动时校验当前
        // OJ 已注册（另两道：查询未命中 ProviderNotFound、factories 测试）。
        if !provider_registry.list_available().contains(&configured_oj) {
            tracing::warn!(configured = %configured_oj, "配置的当前 OJ 未注册，回退 HOJ");
            provider_registry.set_current(OjId::new(HOJAdapter::ID));
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
            Arc::clone(&storage),
        ));
        let problem = Arc::new(ProblemService::new(
            Arc::clone(&provider_registry) as Arc<dyn ProviderRegistry>,
            Arc::clone(&event_bus),
            Arc::clone(&storage),
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
