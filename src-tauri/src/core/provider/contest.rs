use async_trait::async_trait;

use crate::core::entity::contest::{Contest, ContestProblem};
use crate::core::entity::rank::{ContestRankPage, RankQuery};
use crate::core::error::AppResult;

/// 比赛 Provider：获取比赛列表与详情
#[async_trait]
pub trait ContestProvider: Send + Sync {
    /// 获取比赛列表
    async fn list_contests(&self) -> AppResult<Vec<Contest>>;

    /// 获取比赛详情
    async fn get_contest(&self, contest_id: &str) -> AppResult<Contest>;

    /// 获取比赛题目列表（仅摘要，不含完整题面）
    async fn list_contest_problems(&self, contest_id: &str) -> AppResult<Vec<ContestProblem>>;

    /// 获取比赛排行榜（分页）。
    ///
    /// 实现约定：
    /// - 返回的行可能包含服务端前置的「当前用户/关注用户」副本，调用方渲染前需按 `uid` 去重；
    /// - `total` 含这些前置条目，**不能**当作真实参赛人数；
    /// - 榜单为服务端实时计算，本方法不做缓存，轮询节奏由调用方控制（建议 ≥10s 且加抖动错峰）。
    async fn get_contest_rank(
        &self,
        contest_id: &str,
        query: &RankQuery,
    ) -> AppResult<ContestRankPage>;
}
