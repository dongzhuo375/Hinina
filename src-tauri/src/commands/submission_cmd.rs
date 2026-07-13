use tauri::State;
use tracing::info;

use crate::core::context::AppContext;
use crate::core::entity::submission::JudgementResult;
use crate::core::error::AppResult;

/// 提交代码到 OJ。
///
/// 前端 invoke 签名: `submission:submit`({ contest_id, problem_id, language, source_code })
///
/// 返回 `submission_id` 字符串，前端可用 `submission:get_judgement` 轮询结果。
/// 发布 `SubmissionEvent::Created`。
#[tauri::command]
pub async fn submit_code(
    ctx: State<'_, AppContext>,
    contest_id: String,
    problem_id: String,
    language: String,
    source_code: String,
) -> AppResult<String> {
    info!(contest_id = %contest_id, problem_id = %problem_id, language = %language, "Command: 提交代码");
    ctx.submission
        .submit(&contest_id, &problem_id, &language, &source_code)
        .await
}

/// 轮询评测结果。
///
/// 前端 invoke 签名: `submission:get_judgement`({ submission_id })
///
/// 轮询间隔和超时从 Config 读取（`oj.poll_interval_secs` / `oj.poll_timeout_secs`）。
/// 评测完成时返回 `JudgementResult`，超时或查询失败时返回错误。
/// 发布 `SubmissionEvent::Judged` 或 `SubmissionEvent::PollTimeout`。
#[tauri::command]
pub async fn get_judgement(
    ctx: State<'_, AppContext>,
    submission_id: String,
) -> AppResult<JudgementResult> {
    info!(submission_id = %submission_id, "Command: 查询评测结果");
    let cfg = ctx.config.get().oj;
    ctx.submission
        .poll_judgement(&submission_id, cfg.poll_interval_secs, cfg.poll_timeout_secs)
        .await
}
