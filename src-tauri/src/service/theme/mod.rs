// 主题服务：配色方案、字体、圆角等 UI 参数管理。
//
// 主题配置持久化在 ConfigService 中，ThemeService 负责切换逻辑
// 并发布 CoreEvent::ThemeChanged 通知前端更新。
pub mod error;

use std::sync::Arc;

use tracing::{debug, info};

use crate::core::entity::config::ThemeConfig;
use crate::core::event::core_event::CoreEvent;
use crate::core::event::core_event_bus::CoreEventBus;
use crate::core::repository::config_repo::ConfigRepository;
use crate::service::config::ConfigService;

/// 内置主题名称列表。
const BUILTIN_THEMES: &[&str] = &["light", "dark"];

/// 主题管理服务。
pub struct ThemeService<R: ConfigRepository> {
    config: Arc<ConfigService<R>>,
    event_bus: Arc<CoreEventBus>,
}

impl<R: ConfigRepository> ThemeService<R> {
    /// 创建 ThemeService。
    pub fn new(config: Arc<ConfigService<R>>, event_bus: Arc<CoreEventBus>) -> Self {
        Self { config, event_bus }
    }

    /// 获取当前主题名称。
    pub fn current_theme(&self) -> String {
        self.config.get().theme.theme_name
    }

    /// 获取当前编辑器主题名称（Monaco 主题）。
    pub fn current_editor_theme(&self) -> String {
        self.config.get().theme.editor_theme
    }

    /// 获取当前主题配置的完整副本。
    pub fn get_theme_config(&self) -> ThemeConfig {
        self.config.get().theme
    }

    /// 切换主题并发布 `CoreEvent::ThemeChanged`。
    ///
    /// 如果主题名不在内置列表中，仅记录警告但仍允许切换
    /// （为未来自定义主题预留扩展点）。
    ///
    /// 主题落盘由 `ConfigService::update` 显式完成（失败会返回错误且不发事件），
    /// 因此 `ThemeChanged` 是「确已落盘」的事实通知，而不是「请去落盘」的命令。
    pub fn set_theme(&self, theme_name: &str) -> crate::core::error::AppResult<()> {
        if !BUILTIN_THEMES.contains(&theme_name) {
            tracing::warn!(theme = theme_name, "非内置主题，允许切换");
        }

        let name = theme_name.to_string();
        self.config.update(|cfg| {
            cfg.theme.theme_name = name.clone();
            // 自动匹配编辑器主题
            cfg.theme.editor_theme = match theme_name {
                "light" => "vs".into(),
                _ => "vs-dark".into(),
            };
        })?;

        info!(theme = theme_name, "主题已切换");
        self.event_bus.publish(CoreEvent::ThemeChanged);
        debug!("CoreEvent::ThemeChanged 已发布");

        Ok(())
    }

    /// 返回内置主题列表。
    pub fn list_themes(&self) -> &[&str] {
        BUILTIN_THEMES
    }
}
