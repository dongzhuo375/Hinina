//! @plugin-module
//! @layer plugin/api
//! @version 2
//! @stability stable
//! 比赛安全查询契约，无选择/切换比赛能力。
use super::{PluginError, PluginQueryContext, PluginResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// @plugin-api
/// @since 2.0
/// @permission contest.read
/// @stability stable
/// 赛制；服务端未知值归一到 acm。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PluginContestType {
    Acm,
    Oi,
}

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
    /// @plugin-api
    /// @since 2.0
    /// @permission contest.read
    /// @stability stable
    pub contest_type: PluginContestType,
    /// 封榜窗口内为 true（`seal_rank && seal_rank_time <= now`，与 RankView 同规则）。
    /// 解释性标志，不是安全闸门：真正的闸门是服务端封榜期间不下发通过状态。
    pub sealed: bool,
    /// 封榜**开始**时刻（`seal_rank_time`，UTC 毫秒）；仅封榜时非 null。
    /// 服务端不下发封榜结束时刻（比赛结束后也不自动解封），故不提供 sealedUntil。
    pub sealed_since: Option<i64>,
}

/// @plugin-api
/// @since 2.0
/// @permission contest.read
/// @stability stable
/// @security redacted
/// 比赛题目摘要：题号 ↔ 气球颜色映射；不含题面、限制或统计之外的内部字段。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginContestProblem {
    /// 真实题号
    pub problem_id: String,
    /// 比赛内题号（"A"）
    pub display_id: String,
    pub title: String,
    /// 气球颜色；Hydro 无此字段，可能为空串
    pub color: String,
}

/// @plugin-api
/// @since 2.0
/// @permission contest.rank
/// @stability stable
/// 榜单分页查询；不带 contestId（作用域由上下文绑定）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginRankQuery {
    /// 页码，1..=100
    pub page: i64,
    /// 每页行数，1..=200
    pub limit: i64,
}

impl PluginRankQuery {
    /// @plugin-api
    /// @since 2.0
    /// @permission contest.rank
    /// @stability stable
    /// 越界统一返回 LimitExceeded，先于任何取数发生。
    pub fn validate(&self) -> PluginResult<()> {
        let max_rows = crate::plugin::limits::MAX_RANK_ROWS as i64;
        if self.page < 1
            || self.page > crate::plugin::limits::MAX_RANK_PAGE
            || self.limit < 1
            || self.limit > max_rows
        {
            return Err(PluginError::LimitExceeded);
        }
        Ok(())
    }
}

/// @plugin-api
/// @since 2.0
/// @permission contest.rank
/// @stability stable
/// @security redacted
/// 榜单单元格。封榜期间服务端不下发通过状态：`solved`/`firstAc` 恒 false、
/// `acTime` 恒 null，需结合 `contest.current` 的 `sealed` 解释。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginRankCell {
    pub display_id: String,
    pub solved: bool,
    pub first_ac: bool,
    /// 比赛开始后的秒数
    pub ac_time: Option<i64>,
    pub error_num: i32,
    /// 封榜期间的提交次数
    pub try_num: Option<i32>,
    /// OI 赛制该题得分
    pub score: Option<i32>,
    pub after_contest: bool,
}

/// @plugin-api
/// @since 2.0
/// @permission contest.rank
/// @stability stable
/// @security redacted
/// 榜单行；不返回 uid 字段、原始 username、realname、nickname、avatar、gender、totalTime。
/// 唯一例外是 `displayName` 的最终回退：全空时以 uid 文本作显示名（与前端
/// resolveDisplayName 同规则，保证榜单永远有可辨识文本）。
/// `teamKey` 与 `identity.userKey` 同算法，可直接比对自身。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginRankRow {
    /// -1 表示打星队
    pub rank: i32,
    pub team_key: String,
    /// 服务端榜单显示名（rankShowName 规则的结果）
    pub display_name: String,
    pub school: String,
    pub ac: i64,
    pub total_score: Option<i64>,
    /// key = displayId
    pub cells: HashMap<String, PluginRankCell>,
}

/// @plugin-api
/// @since 2.0
/// @permission contest.rank
/// @stability stable
/// @security redacted
/// 榜单分页；刻意不带 `sealed`/`contestType`（来自比赛元信息，见 `contest.current`），
/// 也不带 `total`（HOJ 的 total 含服务端前置条目且偏大，行数以去重后非打星行为准）。
/// 跨页无事务快照，需按 `teamKey` 去重。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginRankPage {
    pub rows: Vec<PluginRankRow>,
    /// 服务端实际页（Hydro 恒为 1），非请求页。
    pub page: i64,
    pub has_more: bool,
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
    /// @plugin-api
    /// @since 2.0
    /// @permission contest.read
    /// @stability stable
    /// 当前比赛的题目列表（题号 ↔ 气球颜色）；未选中比赛返回 ContextExpired。
    async fn problems(
        &self,
        context: &PluginQueryContext,
    ) -> PluginResult<Vec<PluginContestProblem>>;
    /// @plugin-api
    /// @since 2.0
    /// @permission contest.rank
    /// @stability stable
    /// 全场榜单分页；需 `contest.rank` ∩ `contest.read`，缺任一返回 PermissionDenied。
    async fn rank(
        &self,
        context: &PluginQueryContext,
        query: PluginRankQuery,
    ) -> PluginResult<PluginRankPage>;
}
