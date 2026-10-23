//! @plugin-module
//! @layer plugin/api
//! @version 2
//! @stability stable
//! 宿主日志出口契约。日志只写入宿主自己的日志文件，不读取任何宿主数据，
//! 因此不需要权限；不注入 console 对象，复杂数据由插件作者自行 JSON.stringify。
use super::PluginResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::plugin::limits::PLUGIN_LOG_BYTES;

/// @plugin-api
/// @since 2.0
/// @stability stable
/// 单条日志最大 UTF-8 字节数（与 `plugin/limits::PLUGIN_LOG_BYTES` 同源）。
pub const MAX_LOG_BYTES: usize = PLUGIN_LOG_BYTES;

/// @plugin-api
/// @since 2.0
/// @permission none
/// @stability stable
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PluginLogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// @plugin-api
/// @since 2.0
/// @permission none
/// @stability stable
/// 验证长度与控制字符；`\n`/`\t` 放行，但写入前会被替换为空格
/// （见 [`sanitize_log_message`]），保证日志按行可 grep。
pub fn validate_log_message(message: &str) -> PluginResult<()> {
    super::validate_literal_text(message, MAX_LOG_BYTES)
}

/// @plugin-api
/// @since 2.0
/// @permission none
/// @stability stable
/// 写入前把 `\n`/`\t` 拍平为空格；配合桥侧 Debug 转义双保险，日志行不可被伪造。
/// 无需拍平时借用原串，不额外分配。
pub fn sanitize_log_message(message: &str) -> Cow<'_, str> {
    if message.contains(['\n', '\t']) {
        Cow::Owned(message.replace(['\n', '\t'], " "))
    } else {
        Cow::Borrowed(message)
    }
}

/// @plugin-api
/// @since 2.0
/// @permission none
/// @stability stable
#[async_trait]
pub trait LogApi: Send + Sync {
    /// @plugin-api
    /// @since 2.0
    /// @permission none
    /// @stability stable
    /// 写入宿主日志文件；每激活 30 条/分钟，超限返回 LimitExceeded 而非静默丢弃。
    async fn write(&self, level: PluginLogLevel, message: &str) -> PluginResult<()>;
}
