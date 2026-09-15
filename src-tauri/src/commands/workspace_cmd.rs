use tauri::State;
use tracing::info;

use crate::core::context::AppContext;
use crate::core::entity::workspace::Workspace;
use crate::core::error::{AppError, AppResult};

/// 加载或创建工作区（修复 P36：find_or_create）。
///
/// 前端 invoke 签名: `load_workspace`({ contestId, problemId })
///
/// 优先查找已有工作区（按 contest_id + problem_id 匹配），
/// 找到则恢复之前保存的代码，否则创建新工作区。
///
/// 首次调用时自动启动 auto-save（修复 P39：确保在 Tauri 的 tokio runtime 上运行）。
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

    // P39: 首次加载工作区时，在 Tauri Command 的 tokio 上下文中懒启动 auto-save
    start_auto_save_if_needed(&ctx, wm);

    wm.find_or_create(&contest_id, &problem_id, "")
}

/// 首次调用时启动 auto-save，确保在 Tauri 的 tokio runtime 上运行。
fn start_auto_save_if_needed(
    ctx: &AppContext,
    wm: &std::sync::Arc<crate::service::workspace::manager::WorkspaceManager>,
) {
    use std::sync::atomic::Ordering;
    use std::sync::atomic::AtomicBool;
    static STARTED: AtomicBool = AtomicBool::new(false);

    if STARTED.swap(true, Ordering::SeqCst) {
        return; // 已启动
    }

    let cfg = ctx.config.get().editor;
    if cfg.auto_save && cfg.auto_save_interval_secs > 0 {
        let wm = std::sync::Arc::clone(wm);
        // 此时在 Tauri Command 的 async 上下文中，tokio::spawn 可用
        tokio::spawn(async move {
            wm.start_auto_save(cfg.auto_save_interval_secs);
        });
        info!(interval_secs = cfg.auto_save_interval_secs, "auto-save 已启动");
    }
}

/// 持久化当前工作区的脏文件到磁盘。
///
/// 前端 invoke 签名: `save_workspace`
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
/// 前端 invoke 签名: `switch_workspace`({ workspaceId })
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

    wm.switch(&workspace_id, "")
}

/// 获取当前活动工作区。
///
/// 前端 invoke 签名: `current_workspace`
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

/// 更新工作区中的文件内容（前端的 Monaco 编辑器同步到后端）。
///
/// 前端 invoke 签名: `update_workspace_file`({ fileName, content })
///
/// 仅更新内存中的文件内容，不立即持久化到磁盘。
/// 持久化由 auto-save 或显式 save_workspace 负责。
#[tauri::command]
pub async fn update_workspace_file(
    ctx: State<'_, AppContext>,
    file_name: String,
    content: String,
) -> AppResult<()> {
    let wm = ctx.workspace_manager.as_ref().ok_or_else(|| {
        AppError::Workspace("WorkspaceManager 未初始化".into())
    })?;
    wm.update_file(&file_name, &content)
}

/// 设置当前工作区的编程语言并立即落盘。
///
/// 前端 invoke 签名: `set_workspace_language`({ language })
///
/// 语言不属于任何代码文件，`update_workspace_file` 带不上它；
/// 若不单独持久化，切题或重启后会退回默认语言，导致用错语言提交。
#[tauri::command]
pub async fn set_workspace_language(
    ctx: State<'_, AppContext>,
    language: String,
) -> AppResult<Workspace> {
    let wm = ctx.workspace_manager.as_ref().ok_or_else(|| {
        AppError::Workspace("WorkspaceManager 未初始化".into())
    })?;
    wm.set_language(&language)
}
