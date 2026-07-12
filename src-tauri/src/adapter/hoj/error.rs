use thiserror::Error;

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
