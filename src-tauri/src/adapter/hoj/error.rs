use thiserror::Error;

use crate::core::error::AppError;

/// HOJ 适配器专用错误。
#[derive(Debug, Error)]
pub enum HOJError {
    /// HOJ API 返回非 200 状态码
    #[error("HOJ API 错误: status={0}, msg={1}")]
    ApiError(i32, String),

    /// HTTP 请求失败
    #[error("HOJ 网络请求失败: {0}")]
    HttpError(String),

    /// JSON 解析失败
    #[error("HOJ 响应解析失败: {0}")]
    JsonError(String),

    /// Token 缺失（未登录调用需认证接口）
    #[error("HOJ 未授权: {0}")]
    Unauthorized(String),

    /// 评测状态未知
    #[error("HOJ 未知评测状态码: {0}")]
    UnknownStatus(i32),
}

impl From<HOJError> for AppError {
    fn from(e: HOJError) -> Self {
        match e {
            HOJError::ApiError(status, msg) => {
                AppError::Network(format!("HOJ API status={} msg={}", status, msg))
            }
            HOJError::HttpError(msg) => AppError::Network(format!("HOJ: {}", msg)),
            HOJError::JsonError(msg) => AppError::Serialization(format!("HOJ: {}", msg)),
            HOJError::Unauthorized(msg) => AppError::Auth(format!("HOJ: {}", msg)),
            HOJError::UnknownStatus(code) => {
                AppError::Unknown(format!("HOJ 未知状态码: {}", code))
            }
        }
    }
}
