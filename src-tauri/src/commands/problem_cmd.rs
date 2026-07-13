use tauri::State;
use tracing::info;

use crate::core::context::AppContext;
use crate::core::entity::problem::Problem;
use crate::core::error::AppResult;

/// 获取题目详情。
///
/// 前端 invoke 签名: `problem:get`({ contest_id, problem_id })
///
/// 调用 `ProblemService::open_problem`，获取题目描述/样例/限制等完整信息，
/// 并发布 `ProblemEvent::Opened` 供前端 Workspace 切换。
#[tauri::command]
pub async fn get_problem(
    ctx: State<'_, AppContext>,
    contest_id: String,
    problem_id: String,
) -> AppResult<Problem> {
    info!(contest_id = %contest_id, problem_id = %problem_id, "Command: 打开题目");
    ctx.problem.open_problem(&contest_id, &problem_id).await
}

/// 获取比赛下所有题目列表。
///
/// 前端 invoke 签名: `problem:list`({ contest_id })
///
/// 返回题目摘要列表（不含完整题面描述）。获取详情请用 `problem:get`。
#[tauri::command]
pub async fn list_problems(
    ctx: State<'_, AppContext>,
    contest_id: String,
) -> AppResult<Vec<Problem>> {
    info!(contest_id = %contest_id, "Command: 获取题目列表");
    ctx.problem.list_problems(&contest_id).await
}
