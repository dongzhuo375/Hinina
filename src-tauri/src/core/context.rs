use std::sync::Arc;

use crate::core::event::event_bus::EventBus;
use crate::core::provider::registry::ProviderRegistry;
use crate::infra::http::HttpClient;
use crate::infra::logger::Logger;
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
    pub provider_registry: Arc<ProviderRegistry>,
    pub workspace_manager: Arc<WorkspaceManager>,
    pub http_client: Arc<HttpClient>,
    pub storage: Arc<Storage>,
    pub logger: Arc<Logger>,
}

impl AppContext {
    /// 按顺序初始化所有基础设施并装配 AppContext
    pub async fn init() -> Result<Self, Box<dyn std::error::Error>> {
        // TODO: 实现初始化序列
        // 1. Logger
        // 2. ConfigService → 加载配置
        // 3. Storage → 文件系统根目录
        // 4. HttpClient → Reqwest 客户端
        // 5. EventBus
        // 6. ProviderRegistry → 注册各 OJ Adapter
        // 7. WorkspaceManager → 扫描并恢复工作区
        // 8. 装配 AppContext
        todo!("AppContext::init()")
    }
}
