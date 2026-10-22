//! @plugin-module
//! @layer plugin/api
//! @version 1
//! @stability stable
//! 插件错误白名单；无内部错误文本、栈、路径、请求或响应字段。

use crate::plugin::host::manifest::PluginPermission;
use serde::Serialize;
use thiserror::Error;

/// @plugin-api
/// @since 1.0
/// @stability stable
/// @security redacted
/// JS Promise 拒绝时返回的稳定错误对象。
#[derive(Debug, Clone, Error, PartialEq, Eq, Serialize)]
#[serde(
    tag = "code",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum PluginError {
    #[error("permission denied")]
    PermissionDenied { permission: PluginPermission },
    #[error("invalid input")]
    InvalidInput,
    #[error("query context expired")]
    ContextExpired,
    #[error("not authenticated")]
    Unauthenticated,
    #[error("resource not found")]
    NotFound,
    #[error("provider query failed")]
    ProviderError,
    #[error("limit exceeded")]
    LimitExceeded,
    #[error("operation timed out")]
    Timeout,
    #[error("capability unavailable")]
    Unavailable,
    #[error("script execution failed")]
    ScriptError,
    #[error("capability is not implemented")]
    Unsupported,
}

/// @plugin-api
/// @since 1.0
/// @stability stable
/// 插件专用 Result，内部 AppResult 不能直接越过桥。
pub type PluginResult<T> = Result<T, PluginError>;
