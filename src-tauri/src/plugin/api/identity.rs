//! @plugin-module
//! @layer plugin/api
//! @version 1
//! @stability stable
//! 当前身份的最小投影；不返回 Session、Token、用户名或内部 User。
use super::{PluginQueryContext, PluginResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// @plugin-api
/// @since 1.0
/// @permission identity.read
/// @stability stable
/// @security redacted
/// userKey 为 OJ 内稳定不透明标识，不直接序列化内部 User。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginIdentity {
    pub oj_id: String,
    pub user_key: String,
}

/// @plugin-api
/// @since 1.0
/// @permission identity.read
/// @stability stable
#[async_trait]
pub trait IdentityApi: Send + Sync {
    /// @plugin-api
    /// @since 1.0
    /// @permission identity.read
    /// @stability stable
    /// 未登录返回 None；旧查询上下文返回 ContextExpired。
    async fn current(&self, context: &PluginQueryContext) -> PluginResult<Option<PluginIdentity>>;
}
