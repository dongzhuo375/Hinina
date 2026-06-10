use async_trait::async_trait;

use crate::core::entity::user::User;
use crate::core::error::AppResult;

/// 认证 Provider：登录、登出、会话管理
#[async_trait]
pub trait AuthProvider: Send + Sync {
    /// 登录，返回用户信息与会话 token
    async fn login(&self, username: &str, password: &str) -> AppResult<User>;

    /// 登出
    async fn logout(&self) -> AppResult<()>;

    /// 校验当前会话是否有效
    async fn validate_session(&self) -> AppResult<bool>;
}
