use async_trait::async_trait;

use crate::core::entity::submission::{
    JudgementResult, SubmissionCases, SubmissionDetail, SubmissionPage, SubmissionQuery,
};
use crate::core::error::AppResult;

/// 提交 Provider：提交代码与查询评测结果
#[async_trait]
pub trait SubmissionProvider: Send + Sync {
    /// 提交代码，返回提交 ID。
    ///
    /// `problem_id` 与 `display_id` 是**同一道题的两个标识**，各 OJ 认的不是同一个：
    /// - `problem_id`：题目真实 ID（HOJ 数字 pid / Hydro ObjectId），
    ///   也是工作区隔离与 `get_user_problem_status` 的键；
    /// - `display_id`：比赛内展示题号（如 `"A"`）。
    ///
    /// HOJ 的 `POST /submit-problem-judge` 收的 `pid` 是**比赛内展示题号**：
    /// 服务端拿它去查 `contest_problem.display_id`，查不到就直接 NPE 返回 HTTP 500
    /// （实测传数字 pid 必 500）。Hydro 的 `/p/{id}/submit` 收的则是真实 ID。
    /// 故两个都传入，由 Adapter 各取所需 —— 不要在某一家里「猜」另一家的语义。
    async fn submit(
        &self,
        contest_id: &str,
        problem_id: &str,
        display_id: &str,
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
