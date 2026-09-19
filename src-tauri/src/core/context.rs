use std::path::PathBuf;
use std::sync::Arc;

use crate::core::entity::config::{AppConfig, OjInstance};
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

        // 6. 创建 Provider 注册中心 + 按配置实例注册全部内建 OJ（工厂数据化）。
        //
        // 当前 OJ 取自 `oj.active`；实例清单来自 `oj.instances`（enabled 的才注册），
        // 与工厂按 id 匹配 —— 接一个新 OJ = 配置加一条实例 + 工厂清单加一行。
        // 未注册的 active（配置手改/拼写错误）在注册完成后回退 HOJ 并告警。
        let configured_oj = OjId::new(&config.get().oj.active);
        let provider_registry: Arc<dyn ProviderRegistry> =
            Arc::new(ProviderRegistryImpl::new(configured_oj.clone()));

        let adapter_deps = crate::adapter::AdapterDeps {
            http_client: Arc::clone(&http_client),
            event_bus: Arc::clone(&event_bus),
            storage: Arc::clone(&storage),
        };
        let configured = config.get();
        for instance in enabled_instances(&configured) {
            register_instance(provider_registry.as_ref(), &adapter_deps, instance);
        }

        // 身份字符串化后失去编译期穷尽检查，此为第一道防线：启动时校验当前
        // OJ 已注册（另两道：查询未命中 ProviderNotFound、factories 测试）。
        // 回退目标取**首个已注册 OJ**而非硬编码 HOJ —— HOJ 实例可能被禁用或
        // 移除，回退到一个同样未注册的 id 是死路（所有查询 ProviderNotFound）。
        if !provider_registry.list_available().contains(&configured_oj) {
            match provider_registry.list_available().into_iter().next() {
                Some(fallback) => {
                    tracing::warn!(
                        configured = %configured_oj,
                        fallback = %fallback,
                        "配置的当前 OJ 未注册，回退首个已注册 OJ"
                    );
                    provider_registry.set_current(fallback);
                }
                None => {
                    // instances 全部禁用或无匹配工厂：无 OJ 可用，只能告警；
                    // 后续首次查询会以 ProviderNotFound 如实暴露
                    tracing::warn!(
                        configured = %configured_oj,
                        "没有任何已注册的 OJ 适配器（instances 全部禁用或无匹配工厂）"
                    );
                }
            }
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

    /// 确保某个**已配置且启用**的 OJ 实例已完成注册（幂等）。返回是否本次新注册。
    ///
    /// 为什么需要「按需注册」：注册只发生在启动时（`init`），而设置页允许用户从
    /// 枚举里挑一个尚未配置的 OJ、填地址保存后**立即切换** —— 若切换时只查注册表，
    /// 用户会撞上「OJ 未注册」，只能重启客户端才能用上新 OJ。
    ///
    /// 只认配置里已启用且 id 匹配的实例（与 `init` 同源：都用 [`enabled_instance`]
    /// 与 [`register_instance`]）：不能凭空激活一个未配置的 OJ，否则 `switch_oj`
    /// 会绕过配置成为后门。
    pub fn ensure_oj_registered(&self, oj_id: &str) -> bool {
        let id = OjId::new(oj_id);
        if id.as_str().is_empty() || self.provider_registry.list_available().contains(&id) {
            return false;
        }

        let config = self.config.get();
        let Some(instance) = enabled_instance(&config, id.as_str()) else {
            return false;
        };
        let deps = crate::adapter::AdapterDeps {
            http_client: Arc::clone(&self.http_client),
            event_bus: Arc::clone(&self.event_bus),
            storage: Arc::clone(&self.storage),
        };
        let registered = register_instance(self.provider_registry.as_ref(), &deps, instance);
        if registered {
            tracing::debug!(oj_id = %id, "配置的 OJ 尚未注册，已按需补注册");
        }
        registered
    }
}

/// 已启用的 OJ 实例 —— **「启用」条件的唯一来源**：`init` 的全量注册与
/// [`enabled_instance`] 的单实例查找都经此，避免两处判定漂移。
fn enabled_instances(config: &AppConfig) -> impl Iterator<Item = &OjInstance> {
    config.oj.instances.iter().filter(|instance| instance.enabled)
}

/// 从配置里挑出指定 id 的**已启用**实例（纯函数，便于单测锁定判定）。
///
/// `oj_id` 须是已归一（trim）的 id：调用方经 `OjId::new` 归一，
/// 配置里的实例 id 也由 `AppConfig::sanitize` trim 过。
fn enabled_instance<'a>(config: &'a AppConfig, oj_id: &str) -> Option<&'a OjInstance> {
    enabled_instances(config).find(|instance| instance.id == oj_id)
}

/// 按配置实例构造 `ProviderSet` 并注册 —— **`init` 与按需注册共用同一实现**。
///
/// 抽成自由函数而非 `AppContext` 方法：`init` 执行时 `AppContext` 尚未装配完成，
/// 手上只有局部的注册表与依赖。两处必须同源 —— 否则「启动时注册」与「切换时补注册」
/// 会漂移（例如一边认 `enabled`、一边不认），漂移的后果是「重启后能用、切换时不能用」
/// 这类最难排查的不一致。
///
/// 返回是否注册成功；配置了未知 OJ（无匹配工厂）时 `warn` 并返回 `false`，不 panic。
fn register_instance(
    registry: &dyn ProviderRegistry,
    deps: &crate::adapter::AdapterDeps,
    instance: &OjInstance,
) -> bool {
    let Some(factory) = crate::adapter::factories()
        .into_iter()
        .find(|f| f.id() == instance.id)
    else {
        tracing::warn!(oj_id = %instance.id, "配置了未知 OJ（无对应适配器），跳过");
        return false;
    };
    let id = OjId::new(factory.id());
    registry.register(id.clone(), factory.build(deps, &instance.base_url));
    tracing::info!(oj_id = %id, base_url = %instance.base_url, "OJ 适配器已注册");
    true
}

#[cfg(test)]
#[path = "tests/context_tests.rs"]
mod tests;
