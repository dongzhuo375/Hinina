//! @plugin-module
//! @layer plugin/api
//! @version 1
//! @stability stable
//! 纯文本用户通知契约。接收端只能按文本渲染，不能执行 HTML/脚本。
use super::{PluginError, PluginResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// @plugin-api
/// @since 1.0
/// @stability stable
/// 单条通知最大 UTF-8 字节数。
pub const MAX_NOTIFICATION_BYTES: usize = 512;

/// @plugin-api
/// @since 1.0
/// @permission notification
/// @stability stable
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PluginNotificationLevel {
    Info,
    Success,
    Warn,
    Error,
}

/// @plugin-api
/// @since 1.0
/// @stability stable
/// 验证文本长度与控制字符；保留字面 HTML 文本，不将其解释为标记。
pub fn validate_message(message: &str) -> PluginResult<()> {
    if message.trim().is_empty()
        || message
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(PluginError::InvalidInput);
    }
    if message.len() > MAX_NOTIFICATION_BYTES {
        return Err(PluginError::LimitExceeded);
    }
    Ok(())
}

/// @plugin-api
/// @since 1.0
/// @permission notification
/// @stability stable
#[async_trait]
pub trait NotificationApi: Send + Sync {
    /// @plugin-api
    /// @since 1.0
    /// @permission notification
    /// @stability stable
    /// 实现先验证权限和消息边界；没有前端出口时 Unavailable。
    async fn notify(&self, level: PluginNotificationLevel, message: &str) -> PluginResult<()>;
}
