//! @plugin-module
//! @layer plugin/api
//! @version 1
//! @stability stable
//! 只读提交投影。所有查询强制本人，禁止源码、评测点和原始响应。

use super::{PluginError, PluginQueryContext, PluginResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// @plugin-api
/// @since 1.0
/// @stability stable
/// 单次列表查询最大条数。
pub const MAX_SUBMISSION_PAGE_SIZE: u32 = 100;

/// @plugin-api
/// @since 1.0
/// @permission submission.read
/// @stability stable
/// @security redacted
/// 提交最小摘要；submittedAt 为 UTC 毫秒，status 为稳定状态名。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginSubmission {
    pub submission_id: String,
    pub contest_id: String,
    pub problem_id: String,
    pub problem_display_id: String,
    pub status: String,
    pub submitted_at: i64,
}

/// @plugin-api
/// @since 1.0
/// @permission submission.read
/// @stability stable
/// 按比赛分页；problemId 为真实题号，可省略以进行 Resync 全比赛扫描。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginSubmissionQuery {
    pub contest_id: String,
    pub problem_id: Option<String>,
    pub page: u32,
    pub page_size: u32,
}

impl PluginSubmissionQuery {
    /// @plugin-api
    /// @since 1.0
    /// @permission submission.read
    /// @stability stable
    /// 非空 ID <= 256 UTF-8 字节，page 从 1 开始，pageSize 1..=100。
    /// # Failure
    /// InvalidInput 或 LimitExceeded；不发起查询。
    pub fn validate(&self) -> PluginResult<()> {
        if self.contest_id.trim().is_empty()
            || self.contest_id.len() > 256
            || self.page == 0
            || self
                .problem_id
                .as_ref()
                .is_some_and(|s| s.trim().is_empty() || s.len() > 256)
        {
            return Err(PluginError::InvalidInput);
        }
        if !(1..=MAX_SUBMISSION_PAGE_SIZE).contains(&self.page_size) {
            return Err(PluginError::LimitExceeded);
        }
        Ok(())
    }
}

/// @plugin-api
/// @since 1.0
/// @permission submission.read
/// @stability stable
/// 单页结果不保证历史完整性或跨页快照一致性；按 submittedAt/id 稳定排序。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginSubmissionPage {
    pub records: Vec<PluginSubmission>,
    pub page: u32,
    pub page_size: u32,
    pub total: u64,
    pub has_more: bool,
}

/// @plugin-api
/// @since 1.0
/// @permission submission.read
/// @stability stable
/// 绑定授权实例的接口；实现必须验证当前用户和 contestId 所属作用域。
#[async_trait]
pub trait SubmissionApi: Send + Sync {
    /// @plugin-api
    /// @since 1.0
    /// @permission submission.read
    /// @stability stable
    /// 不可见资源与不存在资源均返回 None；错误只返回 PluginError。
    async fn get(
        &self,
        context: &PluginQueryContext,
        contest_id: &str,
        submission_id: &str,
    ) -> PluginResult<Option<PluginSubmission>>;
    /// @plugin-api
    /// @since 1.0
    /// @permission submission.read
    /// @stability stable
    /// 强制本人分页；不得把 Command 的 only_mine 策略遗漏到此接口。
    async fn list(
        &self,
        context: &PluginQueryContext,
        query: PluginSubmissionQuery,
    ) -> PluginResult<PluginSubmissionPage>;
}
