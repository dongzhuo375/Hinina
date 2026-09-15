// 提交服务：代码提交、评测结果轮询、超时处理。
//
// 提交从当前 Workspace 获取代码，轮询间隔和超时由 Config 控制。
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
use crate::core::error::{AppError, AppResult};
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

    /// 轮询评测结果，支持超时控制。
    ///
    /// `poll_interval_secs` — 轮询间隔
    /// `poll_timeout_secs` — 最大等待时间
    ///
    /// 如果在超时前获取到结果，发布 `SubmissionEvent::Judged`；
    /// 否则发布 `SubmissionEvent::PollTimeout`。
    pub async fn poll_judgement(
        &self,
        submission_id: &str,
        poll_interval_secs: u64,
        poll_timeout_secs: u64,
    ) -> AppResult<JudgementResult> {
        let oj_type = self.registry.current_oj();
        let provider = self.registry.get_submission(&oj_type)?;

        let deadline = tokio::time::Instant::now()
            + std::time::Duration::from_secs(poll_timeout_secs);
        let mut attempt = 0u64;

        loop {
            attempt += 1;

            if tokio::time::Instant::now() > deadline {
                warn!(
                    submission_id = submission_id,
                    attempts = attempt,
                    "评测轮询超时"
                );
                self.event_bus.publish(&AppEvent::Submission(
                    SubmissionEvent::PollTimeout {
                        submission_id: submission_id.to_string(),
                    },
                ));
                return Err(AppError::Submission(format!(
                    "评测超时: {}（{} 次轮询后仍未完成）",
                    submission_id, attempt
                )));
            }

            match provider.get_judgement(submission_id).await {
                Ok(result) => {
                    // 非终态（Running）：继续轮询
                    if matches!(result.status, JudgementStatus::Running) {
                        debug!(
                            submission_id = submission_id,
                            attempt = attempt,
                            "评测进行中，继续轮询"
                        );
                    } else {
                        debug!(
                            submission_id = submission_id,
                            status = ?result.status,
                            attempts = attempt,
                            "评测完成"
                        );
                        self.event_bus.publish(&AppEvent::Submission(
                            SubmissionEvent::Judged {
                                submission_id: submission_id.to_string(),
                                result: result.clone(),
                            },
                        ));
                        return Ok(result);
                    }
                }
                Err(e) => {
                    // 认证类错误立即上抛，不重试：token 已失效，继续轮询只会在
                    // poll_timeout_secs（默认 300s）后报「评测超时」—— 既让选手干等五分钟，
                    // 又把会话失效伪装成评测问题（Submission 变体不会触发前端 sessionGuard）
                    if matches!(e, AppError::Auth(_)) {
                        warn!(
                            submission_id = submission_id,
                            error = %e,
                            "评测查询遇到认证失败，停止轮询"
                        );
                        return Err(e.context("评测查询失败"));
                    }
                    warn!(
                        submission_id = submission_id,
                        attempt = attempt,
                        error = %e,
                        "评测查询失败，等待重试"
                    );
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(poll_interval_secs)).await;
        }
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
