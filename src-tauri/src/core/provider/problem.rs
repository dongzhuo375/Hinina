use async_trait::async_trait;

use crate::core::entity::problem::Problem;
use crate::core::error::AppResult;

/// 题目 Provider：获取题目描述与样例
#[async_trait]
pub trait ProblemProvider: Send + Sync {
    /// 获取题目详情（含题面与样例）
    async fn get_problem(&self, contest_id: &str, problem_id: &str) -> AppResult<Problem>;

    /// 获取比赛下所有题目列表
    async fn list_problems(&self, contest_id: &str) -> AppResult<Vec<Problem>>;
}
