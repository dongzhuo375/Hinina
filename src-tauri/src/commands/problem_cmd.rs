use std::collections::HashMap;

use tauri::State;
use tracing::info;

use crate::core::context::AppContext;
use crate::core::entity::problem::Problem;
use crate::core::entity::rank::ProblemLimits;
use crate::core::error::AppResult;

/// 获取题目详情。
///
/// 前端 invoke 签名: `get_problem`({ contestId, problemId })
///
/// 调用 `ProblemService::open_problem`，获取题目描述/样例/限制等完整信息，
/// 并发布 `ProblemEvent::Opened` 供前端 Workspace 切换。
/// 题面缓存开关（`oj.cache_problem_statement`）由本层读取后传入 —— 与
/// `contest_cmd` 传 `cache_ttl_secs` 同款约定：配置读取归命令层，Service 只接参数。
#[tauri::command]
pub async fn get_problem(
    ctx: State<'_, AppContext>,
    contest_id: String,
    problem_id: String,
) -> AppResult<Problem> {
    info!(contest_id = %contest_id, problem_id = %problem_id, "Command: 打开题目");
    let cache_enabled = ctx.config.get().oj.cache_problem_statement;
    ctx.problem
        .open_problem(&contest_id, &problem_id, cache_enabled)
        .await
}

/// 获取比赛下所有题目列表。
///
/// 前端 invoke 签名: `list_problems`({ contestId })
///
/// 返回题目摘要列表（不含完整题面描述）。获取详情请用 `get_problem`。
#[tauri::command]
pub async fn list_problems(
    ctx: State<'_, AppContext>,
    contest_id: String,
) -> AppResult<Vec<Problem>> {
    info!(contest_id = %contest_id, "Command: 获取题目列表");
    ctx.problem.list_problems(&contest_id).await
}

/// 批量获取当前用户对指定题目的提交状态。
///
/// 前端 invoke 签名: `get_user_problem_status`({ contestId, problemIds })
///
/// 返回 `{ pid: 0|1|2 }`（0=未提交，1=已AC，2=尝试过）；未出现的 pid 视为未提交。
/// 用于题目卡片的状态标记与「解题进度」统计。
#[tauri::command]
pub async fn get_user_problem_status(
    ctx: State<'_, AppContext>,
    contest_id: String,
    problem_ids: Vec<String>,
) -> AppResult<HashMap<String, i32>> {
    info!(contest_id = %contest_id, count = problem_ids.len(), "Command: 获取用户题目状态");
    ctx.problem
        .get_user_problem_status(&contest_id, &problem_ids)
        .await
}

/// 批量获取比赛题目的 limits（时间 ms / 内存 MB）。
///
/// 前端 invoke 签名: `get_contest_problem_limits`({ contestId, displayIds })
///
/// 比赛题目列表接口不返回 limits，只能按题拉详情，因此服务端做了内存 + 磁盘双层缓存
/// （`cache/problem_limits/{cid}.json`）并限制并发扇出。返回顺序与入参一致，
/// **获取失败的题目不会出现在结果里**（前端应显示占位而非假默认值）。
#[tauri::command]
pub async fn get_contest_problem_limits(
    ctx: State<'_, AppContext>,
    contest_id: String,
    display_ids: Vec<String>,
) -> AppResult<Vec<ProblemLimits>> {
    info!(contest_id = %contest_id, count = display_ids.len(), "Command: 获取题目 limits");
    ctx.problem
        .load_problem_limits(&contest_id, &display_ids)
        .await
}
