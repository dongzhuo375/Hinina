use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("登录失败: {0}")]
    LoginFailed(String),
    #[error("会话已过期")]
    SessionExpired,
}
