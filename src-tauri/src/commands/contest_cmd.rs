use tauri::State;

use crate::core::context::AppContext;
use crate::core::entity::contest::Contest;
use crate::core::error::AppResult;

/// 获取比赛列表
/// invoke('contest:list')
#[tauri::command]
pub async fn list_contests(ctx: State<'_, AppContext>) -> AppResult<Vec<Contest>> {
    let _ = ctx;
    todo!("contest_cmd::list_contests()")
}

/// 选中比赛
/// invoke('contest:select', { contest_id })
#[tauri::command]
pub async fn select_contest(ctx: State<'_, AppContext>, contest_id: String) -> AppResult<Contest> {
    let _ = (ctx, contest_id);
    todo!("contest_cmd::select_contest()")
}
