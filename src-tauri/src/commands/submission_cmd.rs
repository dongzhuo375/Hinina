use tauri::State;

use crate::core::context::AppContext;
use crate::core::entity::submission::JudgementResult;
use crate::core::error::AppResult;

/// 提交代码
/// invoke('submission:submit', { contest_id, problem_id, language, source_code })
#[tauri::command]
pub async fn submit_code(
    ctx: State<'_, AppContext>,
    contest_id: String,
    problem_id: String,
    language: String,
    source_code: String,
) -> AppResult<String> {
    let _ = (ctx, contest_id, problem_id, language, source_code);
    todo!("submission_cmd::submit_code()")
}

/// 查询评测结果
/// invoke('submission:get_judgement', { submission_id })
#[tauri::command]
pub async fn get_judgement(
    ctx: State<'_, AppContext>,
    submission_id: String,
) -> AppResult<JudgementResult> {
    let _ = (ctx, submission_id);
    todo!("submission_cmd::get_judgement()")
}
