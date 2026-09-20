use std::collections::HashMap;

use async_trait::async_trait;

use crate::core::entity::problem::Problem;
use crate::core::error::AppResult;

/// 题目 Provider：获取题目描述与样例
#[async_trait]
pub trait ProblemProvider: Send + Sync {
    /// 获取题目详情（含题面与样例）
    async fn get_problem(&self, contest_id: &str, problem_id: &str) -> AppResult<Problem>;

    /// 批量获取当前用户对指定题目的提交状态。
    ///
    /// 返回 map 的 key 为题目真实 ID（pid）字符串，value 为 `0=未提交 / 1=已AC / 2=尝试过`；
    /// 未出现在 map 中的题目视为未提交。
    async fn get_user_problem_status(
        &self,
        contest_id: &str,
        problem_ids: &[String],
    ) -> AppResult<HashMap<String, i32>>;
}
