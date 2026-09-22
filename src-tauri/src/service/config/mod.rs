use std::sync::{Arc, RwLock};

use tracing::{debug, info, warn};

use crate::core::entity::config::{normalize_legacy_values, AppConfig};
use crate::core::error::{AppError, AppResult};
use crate::core::event::core_event::CoreEvent;
use crate::core::event::core_event_bus::CoreEventBus;
use crate::core::repository::config_repo::ConfigRepository;

/// 统一配置管理服务。
///
/// 泛型 `R` 为底层持久化实现，当前为 `FsConfigRepository`，
/// 未来可替换为其他存储后端。
pub struct ConfigService<R: ConfigRepository> {
    repo: Arc<R>,
    event_bus: Arc<CoreEventBus>,
    /// 当前内存中的配置副本
    config: RwLock<AppConfig>,
}

impl<R: ConfigRepository> ConfigService<R> {
    /// 创建 ConfigService 并立即从 Repo 加载配置。
    ///
    /// 首次启动时配置文件不存在，自动使用默认值并持久化。
    pub fn new(repo: Arc<R>, event_bus: Arc<CoreEventBus>) -> Self {
        let config = match repo.load_config::<AppConfig>() {
            Ok(mut cfg) => {
                info!("配置加载成功");
                // 历史配置可能仍存着旧默认值（"C++" / dark / 0.45），加载时一次性归一
                normalize_legacy_values(&mut cfg);
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

    /// 通过闭包修改配置，修改后自动保存并发布 `CoreEvent::ConfigChanged`。
    ///
    /// 先调用 updater 闭包修改内存中的配置，然后持久化到磁盘。
    /// 如果持久化失败，内存中的修改会被保留（保持 UI 响应一致），
    /// 返回错误供调用方决定是否回滚 —— **失败时不发布事件**（磁盘与内存已不一致，
    /// 让订阅者按「新配置已生效」行动会掩盖问题）。
    ///
    /// **为什么在这里发布**：`update` 才是生产链路的配置变更入口（`update_config`、
    /// 主题切换、`switch_oj` 的持久化都走它）；只有 `reload` 发布的话，订阅者永远
    /// 等不到事件 —— `reload_config` 命令在前端没有调用方（2026-09-21 复核）。
    ///
    /// **`ConfigChanged` 只是完成事实通知**：需要「配置变更后必须完成的动作」
    /// （如按新间隔同步 auto-save）由命令层在 `update` 返回后**显式调用**，
    /// 不得依赖事件消费者 —— 消费者可能落后、可能不存在。
    pub fn update<F>(&self, updater: F) -> AppResult<AppConfig>
    where
        F: FnOnce(&mut AppConfig),
    {
        let new_config = {
            let mut cfg = self.config.write().unwrap_or_else(|e| e.into_inner());
            updater(&mut cfg);
            cfg.clone()
        };

        // 持久化到磁盘，失败时保留内存修改但上报错误
        if let Err(e) = self.repo.save_config(&new_config) {
            warn!(error = %e, "配置持久化失败");
            return Err(AppError::Config(format!("配置持久化失败: {}", e)));
        }

        debug!("配置已更新并保存");
        self.event_bus.publish(CoreEvent::ConfigChanged);
        Ok(new_config)
    }

    /// 强制从磁盘重新加载配置，覆盖内存中的副本。
    ///
    /// 加载成功后发布 `CoreEvent::ConfigChanged`（与 [`Self::update`] 同款语义：
    /// 任何「配置已变更」的路径都要通知订阅者）。
    pub fn reload(&self) -> AppResult<AppConfig> {
        let new_config = self.repo.load_config::<AppConfig>().map_err(|e| {
            warn!(error = %e, "配置重新加载失败");
            AppError::Config(format!("配置重新加载失败: {}", e))
        })?;
        // 与 new() 同一归一入口：手改磁盘文件后 reload 同样要修正旧值
        let mut new_config = new_config;
        normalize_legacy_values(&mut new_config);

        {
            let mut cfg = self.config.write().unwrap_or_else(|e| e.into_inner());
            *cfg = new_config.clone();
        }

        info!("配置已重新加载");
        self.event_bus.publish(CoreEvent::ConfigChanged);

        debug!("CoreEvent::ConfigChanged 已发布");
        Ok(new_config)
    }

    /// 返回底层 ConfigRepository 引用。
    pub fn repo(&self) -> &Arc<R> {
        &self.repo
    }
}

#[cfg(test)]
#[path = "tests/config_tests.rs"]
mod tests;
