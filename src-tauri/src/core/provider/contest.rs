use async_trait::async_trait;

use crate::core::entity::contest::{Contest, ContestProblem};
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
}
