//! @plugin-module
//! @layer plugin/host
//! @version 1
//! @stability stable
//! 内部 AppError 向 PluginError 的安全转换；丢弃所有内部文本。
use crate::core::error::AppError;
use crate::plugin::api::PluginError;

/// @plugin-api
/// @since 1.0
/// @stability stable
/// @security redacted
/// 供 API Adapter 使用；不得将原错误 Display/Debug 文本传入插件。
pub fn redact_app_error(error: &AppError) -> PluginError {
    match error {
        AppError::Auth(_) => PluginError::Unauthenticated,
        AppError::Contest(_) | AppError::Problem(_) | AppError::Submission(_) => {
            PluginError::ProviderError
        }
        _ => PluginError::Unavailable,
    }
}
