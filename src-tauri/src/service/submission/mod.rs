// 提交服务：代码提交、评测结果单次查询、提交列表 / 详情 / 测试点查询。
//
// 提交从当前 Workspace 获取代码。评测轮询的节拍与超时由前端 `submissionStore`
// 的 createPoller 拥有（配置 `oj.pollIntervalSecs` / `oj.pollTimeoutSecs` 经
// get_config 由前端消费）；后端只做单次查询与终态事件发布。
//
// **错误处理约定**：传播 Provider 错误一律用 `AppError::context()` 补环节名，
// 不得重新包装成 `AppError::Submission` —— 变体是前端 `sessionGuard` 判定会话失效的依据
// （见 `core/error.rs`）。token 过期时提交若被改写成 Submission 变体，
// 选手只会看到一条错误文案而不会被带回登录页，反复重试也全部失败。
pub mod error;

use std::sync::Arc;

use tracing::{debug, info, warn};

use crate::core::entity::submission::{
    JudgementResult, JudgementStatus, SubmissionCases, SubmissionDetail, SubmissionPage,
    SubmissionQuery,
};
use crate::core::error::AppResult;
use crate::core::event::app_event::{AppEvent, SubmissionEvent};
use crate::core::event::event_bus::EventBus;
use crate::core::provider::registry::ProviderRegistry;

/// 提交服务。
pub struct SubmissionService {
    registry: Arc<dyn ProviderRegistry>,
    event_bus: Arc<EventBus>,
}

impl SubmissionService {
    /// 创建 SubmissionService。
    pub fn new(registry: Arc<dyn ProviderRegistry>, event_bus: Arc<EventBus>) -> Self {
        Self {
            registry,
            event_bus,
        }
    }

    /// 提交代码到 OJ。
    ///
    /// 返回 submission_id 供后续轮询使用。
    /// 发布 `SubmissionEvent::Created`。
    pub async fn submit(
        &self,
        contest_id: &str,
        problem_id: &str,
        language: &str,
        source_code: &str,
    ) -> AppResult<String> {
        let oj_type = self.registry.current_oj();
        let provider = self.registry.get_submission(&oj_type)?;

        info!(contest_id = contest_id, problem_id = problem_id, language = language, "提交代码");
        let submission_id = provider
            .submit(contest_id, problem_id, language, source_code)
            .await
            .map_err(|e| {
                warn!(contest_id = contest_id, problem_id = problem_id, error = %e, "提交失败");
                e.context("提交失败")
            })?;

        debug!(submission_id = submission_id, "代码已提交");

        self.event_bus.publish(&AppEvent::Submission(
            SubmissionEvent::Created {
                submission_id: submission_id.clone(),
            },
        ));

        Ok(submission_id)
    }

    /// 单次查询评测结果。
    ///
    /// 轮询节拍与总超时由前端 `submissionStore` 的 createPoller 拥有，后端不循环、
    /// 不睡眠、不设 deadline：每次调用只发一次 `provider.get_judgement`。
    /// 终态发布 `SubmissionEvent::Judged`；非终态（Pending/Compiling/Running）
    /// 原样透传、不发事件，是否继续轮询由前端决定。
    pub async fn get_judgement(&self, submission_id: &str) -> AppResult<JudgementResult> {
        let oj_type = self.registry.current_oj();
        let provider = self.registry.get_submission(&oj_type)?;

        // 错误一律 context() 补环节名、变体穿透（Auth 变体是前端 sessionGuard 的判据）；
        // 瞬时抖动的容忍与重试同样由前端 poller 编排
        let result = provider
            .get_judgement(submission_id)
            .await
            .map_err(|e| {
                warn!(submission_id = submission_id, error = %e, "评测查询失败");
                e.context("评测查询失败")
            })?;

        // 非终态判据与原轮询循环一致：Pending/Compiling/Running 三态之外即终态
        if matches!(
            result.status,
            JudgementStatus::Pending | JudgementStatus::Compiling | JudgementStatus::Running
        ) {
            debug!(
                submission_id = submission_id,
                status = ?result.status,
                "评测进行中"
            );
        } else {
            debug!(
                submission_id = submission_id,
                status = ?result.status,
                "评测完成"
            );
            self.event_bus.publish(&AppEvent::Submission(
                SubmissionEvent::Judged {
                    submission_id: submission_id.to_string(),
                    result: result.clone(),
                },
            ));
        }

        Ok(result)
    }

    /// 查询比赛提交列表（分页）。
    ///
    /// **不做缓存**：提交状态随时在变（评测中 → 终态），必须由前端控制刷新节奏。
    pub async fn list_contest_submissions(
        &self,
        query: &SubmissionQuery,
    ) -> AppResult<SubmissionPage> {
        let oj_type = self.registry.current_oj();
        let provider = self.registry.get_submission(&oj_type)?;

        let page = provider
            .list_contest_submissions(query)
            .await
            .map_err(|e| {
                warn!(contest_id = query.contest_id, error = %e, "获取提交列表失败");
                e.context("获取提交列表失败")
            })?;

        debug!(contest_id = query.contest_id, count = page.records.len(), "提交列表已获取");
        Ok(page)
    }

    /// 查询提交详情（含源代码与错误信息）。
    pub async fn get_submission_detail(&self, submit_id: &str) -> AppResult<SubmissionDetail> {
        let oj_type = self.registry.current_oj();
        let provider = self.registry.get_submission(&oj_type)?;

        provider
            .get_submission_detail(submit_id)
            .await
            .map_err(|e| {
                warn!(submit_id = submit_id, error = %e, "获取提交详情失败");
                e.context("获取提交详情失败")
            })
    }

    /// 查询提交的全部测试点结果。
    pub async fn get_submission_cases(&self, submit_id: &str) -> AppResult<SubmissionCases> {
        let oj_type = self.registry.current_oj();
        let provider = self.registry.get_submission(&oj_type)?;

        provider
            .get_submission_cases(submit_id)
            .await
            .map_err(|e| {
                warn!(submit_id = submit_id, error = %e, "获取测试点结果失败");
                e.context("获取测试点结果失败")
            })
    }
}

#[cfg(test)]
#[path = "tests/submission_tests.rs"]
mod tests;
