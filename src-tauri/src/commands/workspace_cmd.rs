use tauri::State;
use tracing::info;

use crate::core::context::AppContext;
use crate::core::entity::workspace::Workspace;
use crate::core::error::{AppError, AppResult};

/// 加载或创建工作区。
///
/// 前端 invoke 签名: `workspace:load`({ contest_id, problem_id })
///
/// 始终为新打开的题目创建独立 Workspace，实现比赛隔离。
/// `root_path` 从 Storage base_dir 推导（`workspaces/{workspace_id}`）。
#[tauri::command]
pub async fn load_workspace(
    ctx: State<'_, AppContext>,
    contest_id: String,
    problem_id: String,
) -> AppResult<Workspace> {
    info!(contest_id = %contest_id, problem_id = %problem_id, "Command: 加载工作区");

    let wm = ctx.workspace_manager.as_ref().ok_or_else(|| {
        AppError::Workspace("WorkspaceManager 未初始化".into())
    })?;

    let root_path = format!("workspaces/{{id}}"); // 占位，实际路径由 create 内部填充
    wm.create(&contest_id, &problem_id, &root_path)
}

/// 持久化当前工作区的脏文件到磁盘。
///
/// 前端 invoke 签名: `workspace:save`
///
/// 仅保存已修改（dirty）的文件，发布 `WorkspaceEvent::Saved`。
#[tauri::command]
pub async fn save_workspace(ctx: State<'_, AppContext>) -> AppResult<()> {
    let wm = ctx.workspace_manager.as_ref().ok_or_else(|| {
        AppError::Workspace("WorkspaceManager 未初始化".into())
    })?;

    wm.save()
}

/// 切换活动工作区。
///
/// 前端 invoke 签名: `workspace:switch`({ workspace_id })
///
/// 保存当前工作区 → 加载目标工作区 → 返回新 Workspace。
/// 发布 `WorkspaceEvent::Switched`。
#[tauri::command]
pub async fn switch_workspace(
    ctx: State<'_, AppContext>,
    workspace_id: String,
) -> AppResult<Workspace> {
    info!(workspace_id = %workspace_id, "Command: 切换工作区");

    let wm = ctx.workspace_manager.as_ref().ok_or_else(|| {
        AppError::Workspace("WorkspaceManager 未初始化".into())
    })?;

    let root_path = format!("workspaces/{{id}}");
    wm.switch(&workspace_id, &root_path)
}

/// 获取当前活动工作区。
///
/// 前端 invoke 签名: `workspace:current`
///
/// 返回 `None` 表示当前无活动工作区。
#[tauri::command]
pub async fn current_workspace(ctx: State<'_, AppContext>) -> AppResult<Option<Workspace>> {
    let wm = match ctx.workspace_manager.as_ref() {
        Some(wm) => wm,
        None => return Ok(None),
    };

    Ok(wm.current())
}
