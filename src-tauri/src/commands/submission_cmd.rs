use tauri::State;
use tracing::info;

use crate::core::context::AppContext;
use crate::core::entity::submission::{
    JudgementResult, SubmissionCases, SubmissionDetail, SubmissionPage, SubmissionQuery,
};
use crate::core::error::AppResult;

/// 提交列表默认分页大小
const DEFAULT_SUBMISSION_LIMIT: i64 = 20;

/// 提交代码到 OJ。
///
/// 前端 invoke 签名: `submit_code`({ contestId, problemId, language, sourceCode })
///
/// 返回 `submissionId` 字符串，前端可用 `get_judgement` 轮询结果。
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

/// 单次查询评测结果。
///
/// 前端 invoke 签名: `get_judgement`({ submissionId })
///
/// 轮询节拍 / 总超时 / 终态停止全部由前端 submissionStore 编排
/// （createPoller，抖动 ±20% 封顶 500ms）；`oj.poll_interval_secs` /
/// `poll_timeout_secs` 由前端经 get_config 消费，本命令不再读取。
/// 终态发布 `SubmissionEvent::Judged`；非终态原样透传、不发事件。
#[tauri::command]
pub async fn get_judgement(
    ctx: State<'_, AppContext>,
    submission_id: String,
) -> AppResult<JudgementResult> {
    info!(submission_id = %submission_id, "Command: 查询评测结果");
    ctx.submission.get_judgement(&submission_id).await
}

/// 获取比赛提交列表（分页）。
///
/// 前端 invoke 签名: `list_contest_submissions`({ contestId, currentPage?, limit?, problemDisplayId?, status? })
///
/// 默认第 1 页、每页 20 条。不做缓存，刷新节奏由前端控制。
#[tauri::command]
pub async fn list_contest_submissions(
    ctx: State<'_, AppContext>,
    contest_id: String,
    current_page: Option<i64>,
    limit: Option<i64>,
    problem_display_id: Option<String>,
    status: Option<i32>,
) -> AppResult<SubmissionPage> {
    let query = SubmissionQuery {
        contest_id: contest_id.clone(),
        current_page: current_page.unwrap_or(1).max(1),
        limit: limit.unwrap_or(DEFAULT_SUBMISSION_LIMIT).max(1),
        // 产品决策「提交记录只显示本人」，后端强制，前端不可绕过
        only_mine: true,
        problem_display_id: problem_display_id.filter(|s| !s.trim().is_empty()),
        status,
    };

    info!(
        contest_id = %contest_id,
        page = query.current_page,
        limit = query.limit,
        "Command: 获取比赛提交列表"
    );
    ctx.submission.list_contest_submissions(&query).await
}

/// 获取提交详情（含源代码与错误信息）。
///
/// 前端 invoke 签名: `get_submission_detail`({ submissionId })
#[tauri::command]
pub async fn get_submission_detail(
    ctx: State<'_, AppContext>,
    submission_id: String,
) -> AppResult<SubmissionDetail> {
    info!(submission_id = %submission_id, "Command: 获取提交详情");
    ctx.submission.get_submission_detail(&submission_id).await
}

/// 获取提交的全部测试点结果。
///
/// 前端 invoke 签名: `get_submission_cases`({ submissionId })
#[tauri::command]
pub async fn get_submission_cases(
    ctx: State<'_, AppContext>,
    submission_id: String,
) -> AppResult<SubmissionCases> {
    info!(submission_id = %submission_id, "Command: 获取测试点结果");
    ctx.submission.get_submission_cases(&submission_id).await
}
