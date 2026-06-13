use async_trait::async_trait;

use crate::core::entity::submission::JudgementResult;
use crate::core::error::AppResult;

/// 提交 Provider：提交代码与查询评测结果
#[async_trait]
pub trait SubmissionProvider: Send + Sync {
    /// 提交代码，返回提交 ID
    async fn submit(
        &self,
        contest_id: &str,
        problem_id: &str,
        language: &str,
        source_code: &str,
    ) -> AppResult<String>;

    /// 查询评测结果
    async fn get_judgement(&self, submission_id: &str) -> AppResult<JudgementResult>;
}
