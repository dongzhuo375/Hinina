//! @plugin-module
//! @layer plugin/api
//! @version 1
//! @stability stable
//! 比赛安全查询契约，无选择/切换比赛能力。
use super::{PluginQueryContext, PluginResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// @plugin-api
/// @since 1.0
/// @permission contest.read
/// @stability stable
/// @security redacted
/// 时间均为 UTC 毫秒，不返回比赛密码或 HTML 描述。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginContest {
    pub contest_id: String,
    pub title: String,
    pub starts_at: i64,
    pub ends_at: i64,
}

/// @plugin-api
/// @since 1.0
/// @permission contest.read
/// @stability stable
#[async_trait]
pub trait ContestApi: Send + Sync {
    /// @plugin-api
    /// @since 1.0
    /// @permission contest.read
    /// @stability stable
    /// 无选中比赛返回 None；上下文失效返回 ContextExpired。
    async fn current(&self, context: &PluginQueryContext) -> PluginResult<Option<PluginContest>>;
    /// @plugin-api
    /// @since 1.0
    /// @permission contest.read
    /// @stability stable
    /// 仅可访问宿主当前上下文允许的比赛，否则 None。
    async fn get(
        &self,
        context: &PluginQueryContext,
        contest_id: &str,
    ) -> PluginResult<Option<PluginContest>>;
}
