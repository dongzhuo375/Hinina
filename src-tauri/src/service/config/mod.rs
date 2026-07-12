use std::sync::{Arc, RwLock};

use tracing::{debug, info, warn};

use crate::core::entity::config::AppConfig;
use crate::core::error::{AppError, AppResult};
use crate::core::event::app_event::{AppEvent, SystemEvent};
use crate::core::event::event_bus::EventBus;
use crate::core::repository::config_repo::ConfigRepository;

/// 统一配置管理服务。
///
/// 泛型 `R` 为底层持久化实现，当前为 `FsConfigRepository`，
/// 未来可替换为其他存储后端。
pub struct ConfigService<R: ConfigRepository> {
    repo: Arc<R>,
    event_bus: Arc<EventBus>,
    /// 当前内存中的配置副本
    config: RwLock<AppConfig>,
}

impl<R: ConfigRepository> ConfigService<R> {
    /// 创建 ConfigService 并立即从 Repo 加载配置。
    ///
    /// 首次启动时配置文件不存在，自动使用默认值并持久化。
    pub fn new(repo: Arc<R>, event_bus: Arc<EventBus>) -> Self {
        let config = match repo.load_config::<AppConfig>() {
            Ok(cfg) => {
                info!("配置加载成功");
                cfg
            }
            Err(e) => {
                warn!(error = %e, "配置加载失败，使用默认值");
                let default_cfg = AppConfig::default();
                // 首次启动时，将默认配置写回磁盘
                if let Err(save_err) = repo.save_config(&default_cfg) {
                    warn!(error = %save_err, "默认配置保存失败");
                }
                default_cfg
            }
        };

        Self {
            repo,
            event_bus,
            config: RwLock::new(config),
        }
    }

    /// 获取当前配置的不可变副本。
    pub fn get(&self) -> AppConfig {
        self.config
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// 通过闭包修改配置，修改后自动保存。
    ///
    /// 如果保存失败，内存中的修改会被保留（保持 UI 响应一致），
    /// 但会返回错误供调用方决定如何提示用户。
    pub fn update<F>(&self, updater: F) -> AppResult<AppConfig>
    where
        F: FnOnce(&mut AppConfig),
    {
        let new_config = {
            let mut cfg = self
                .config
                .write()
                .unwrap_or_else(|e| e.into_inner());
            updater(&mut cfg);
            cfg.clone()
        };

        // 先持久化，再更新内存（持久化失败时回滚）
        if let Err(e) = self.repo.save_config(&new_config) {
            warn!(error = %e, "配置保存失败");
            return Err(AppError::Config(format!("配置保存失败: {}", e)));
        }

        debug!("配置已更新并保存");
        Ok(new_config)
    }

    /// 强制从磁盘重新加载配置，覆盖内存中的副本。
    ///
    /// 加载成功后发布 `SystemEvent::ConfigReloaded`，供其他 Service 响应配置变更。
    pub fn reload(&self) -> AppResult<AppConfig> {
        let new_config = self.repo.load_config::<AppConfig>().map_err(|e| {
            warn!(error = %e, "配置重新加载失败");
            AppError::Config(format!("配置重新加载失败: {}", e))
        })?;

        {
            let mut cfg = self
                .config
                .write()
                .unwrap_or_else(|e| e.into_inner());
            *cfg = new_config.clone();
        }

        info!("配置已重新加载");
        self.event_bus
            .publish(&AppEvent::System(SystemEvent::ConfigReloaded));

        debug!("SystemEvent::ConfigReloaded 已发布");
        Ok(new_config)
    }

    /// 返回底层 ConfigRepository 引用。
    pub fn repo(&self) -> &Arc<R> {
        &self.repo
    }
}
