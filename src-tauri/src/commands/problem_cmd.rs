use tauri::State;

use crate::core::context::AppContext;
use crate::core::entity::problem::Problem;
use crate::core::error::AppResult;

/// 获取题目详情
/// invoke('problem:get', { contest_id, problem_id })
#[tauri::command]
pub async fn get_problem(
    ctx: State<'_, AppContext>,
    contest_id: String,
    problem_id: String,
) -> AppResult<Problem> {
    let _ = (ctx, contest_id, problem_id);
    todo!("problem_cmd::get_problem()")
}

/// 获取比赛下所有题目
/// invoke('problem:list', { contest_id })
#[tauri::command]
pub async fn list_problems(
    ctx: State<'_, AppContext>,
    contest_id: String,
) -> AppResult<Vec<Problem>> {
    let _ = (ctx, contest_id);
    todo!("problem_cmd::list_problems()")
}
