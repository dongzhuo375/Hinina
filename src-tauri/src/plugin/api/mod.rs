//! @plugin-module
//! @layer plugin/api
//! @version 1
//! @stability stable
//!
//! 插件安全能力契约。具体实现必须绑定宿主授予的 PluginGrant，不接受 JS 指定
//! plugin_id/权限；查询上下文由宿主签发并在切 OJ、换账号或比赛变化后失效。
//! 本轮没有 Service/网络/存储执行器，trait 声明不代表能力已可用。
pub mod contest;
pub mod error;
pub mod event;
pub mod http;
pub mod identity;
pub mod notification;
pub mod problem;
pub mod storage;
pub mod submission;
pub mod ui;
pub mod workspace;

pub use error::{PluginError, PluginResult};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// @plugin-api
/// @since 1.0
/// @stability stable
/// @security redacted
/// 不透明查询句柄；不可用于认证。宿主实现必须验证其绑定而非信任输入值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PluginQueryContext(String);

impl PluginQueryContext {
    /// @plugin-api
    /// @since 1.0
    /// @stability stable
    /// 仅供未来宿主构造；JS 传回的句柄必须重新验证，构造类型不是授权。
    #[must_use]
    pub fn from_host_id(id: String) -> Self {
        Self(id)
    }
}

/// @plugin-api
/// @since 1.0
/// @permission none
/// @stability stable
/// 宿主上下文获取入口；不返回身份、Token 或内部实体。
#[async_trait]
pub trait ContextApi: Send + Sync {
    /// @plugin-api
    /// @since 1.0
    /// @permission none
    /// @stability stable
    /// 签发绑定当前 OJ/用户/比赛/授权代次的句柄。失败为 Unavailable。
    async fn current_context(&self) -> PluginResult<PluginQueryContext>;
}

/// @plugin-api
/// @since 1.0
/// @stability stable
/// 未来 Runtime Bridge 注入的最小能力集合。实现必须逐调用验证权限/上下文，
/// 异步查询前后均验证上下文，避免在途切换后返回另一个账号的数据。
pub trait PluginApi:
    ContextApi
    + submission::SubmissionApi
    + contest::ContestApi
    + identity::IdentityApi
    + storage::PluginStorageApi
    + http::PluginHttpApi
    + notification::NotificationApi
{
}

#[cfg(test)]
#[path = "tests/api_contract_tests.rs"]
mod tests;
