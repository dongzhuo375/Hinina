use tauri::State;

use crate::core::context::AppContext;
use crate::core::entity::workspace::Workspace;
use crate::core::error::AppResult;

/// 加载或创建工作区
/// invoke('workspace:load', { contest_id, problem_id })
#[tauri::command]
pub async fn load_workspace(
    ctx: State<'_, AppContext>,
    contest_id: String,
    problem_id: String,
) -> AppResult<Workspace> {
    let _ = (ctx, contest_id, problem_id);
    todo!("workspace_cmd::load_workspace()")
}

/// 保存当前工作区文件
/// invoke('workspace:save')
#[tauri::command]
pub async fn save_workspace(ctx: State<'_, AppContext>) -> AppResult<()> {
    let _ = ctx;
    todo!("workspace_cmd::save_workspace()")
}

/// 切换工作区
/// invoke('workspace:switch', { workspace_id })
#[tauri::command]
pub async fn switch_workspace(
    ctx: State<'_, AppContext>,
    workspace_id: String,
) -> AppResult<Workspace> {
    let _ = (ctx, workspace_id);
    todo!("workspace_cmd::switch_workspace()")
}

/// 获取当前工作区
/// invoke('workspace:current')
#[tauri::command]
pub async fn current_workspace(ctx: State<'_, AppContext>) -> AppResult<Option<Workspace>> {
    let _ = ctx;
    todo!("workspace_cmd::current_workspace()")
}
