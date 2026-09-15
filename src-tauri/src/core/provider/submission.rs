use async_trait::async_trait;

use crate::core::entity::submission::{
    JudgementResult, SubmissionCases, SubmissionDetail, SubmissionPage, SubmissionQuery,
};
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

    /// 查询比赛提交列表（分页）
    async fn list_contest_submissions(
        &self,
        query: &SubmissionQuery,
    ) -> AppResult<SubmissionPage>;

    /// 查询提交详情（含源代码与错误信息）
    async fn get_submission_detail(&self, submit_id: &str) -> AppResult<SubmissionDetail>;

    /// 查询提交的全部测试点结果
    async fn get_submission_cases(&self, submit_id: &str) -> AppResult<SubmissionCases>;
}
