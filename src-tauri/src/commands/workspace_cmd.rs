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
/// 加载前按当前配置同步 auto-save 状态（修复 P39：在 Tauri 的 tokio runtime 上运行；
/// 修复 P48：不再用一次性 static 标记，配置改动可生效）。
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

    sync_auto_save_with_config(&ctx.config.get().editor, wm);

    wm.find_or_create(&contest_id, &problem_id, "")
}

/// auto-save 的期望动作（由配置与当前运行状态决定）。
///
/// 抽成纯函数是为了让判据可被单测锁定 —— 它是「配置改动能否生效」的全部逻辑，
/// 而这条链路此前出过两次问题（P39 的 runtime 上下文、P48 的一次性 static 标记）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AutoSaveAction {
    /// 维持现状（已在按同一间隔运行，或本就该停）
    Keep,
    /// 启动/以新间隔重启
    Start(u64),
    /// 停止
    Stop,
}

/// 判据：`auto_save` 打开且间隔 > 0 → 运行；否则停止。
///
/// 已在运行的间隔与目标一致时返回 `Keep`，避免每次 `load_workspace` 都重置计时器。
pub(crate) fn auto_save_action(
    auto_save: bool,
    interval_secs: u64,
    current_interval: Option<u64>,
) -> AutoSaveAction {
    let desired = (auto_save && interval_secs > 0).then_some(interval_secs);
    match (desired, current_interval) {
        (None, None) => AutoSaveAction::Keep,
        (None, Some(_)) => AutoSaveAction::Stop,
        (Some(target), Some(running)) if target == running => AutoSaveAction::Keep,
        (Some(target), _) => AutoSaveAction::Start(target),
    }
}

/// 按当前配置同步 auto-save 的启停与间隔（**幂等**）。
///
/// 调用点两处：`load_workspace`（首次进入解题页）与 `SystemEvent::ConfigReloaded`
/// （配置变更后立即生效 —— 事件由 `ConfigService::update` / `reload` 发布，见
/// `service/config/mod.md`；**只靠 `reload_config` 命令是不够的，它在前端无调用方**）。
///
/// 必须在 tokio runtime 上下文里调用（`WorkspaceManager::start_auto_save` 内部
/// 用 `tokio::spawn` 起后台循环）。两个调用点都满足：前者是 Tauri 异步命令，
/// 后者由 `update_config` 命令同步发布事件时在发布方栈内执行。
pub fn sync_auto_save_with_config(
    editor: &crate::core::entity::config::EditorConfig,
    wm: &std::sync::Arc<crate::service::workspace::manager::WorkspaceManager>,
) {
    match auto_save_action(editor.auto_save, editor.auto_save_interval_secs, wm.auto_save_interval_secs())
    {
        AutoSaveAction::Keep => {}
        AutoSaveAction::Start(interval_secs) => {
            wm.start_auto_save(interval_secs);
            info!(interval_secs, "auto-save 已按配置启动");
        }
        AutoSaveAction::Stop => {
            wm.stop_auto_save();
            info!("auto-save 已按配置停止");
        }
    }
}

/// 持久化当前工作区到磁盘（脏时全量写入文件 + 元数据）。
///
/// 前端 invoke 签名: `save_workspace`
///
/// 未脏时直接返回且不发布事件；成功落盘时发布 `WorkspaceEvent::Saved`
/// （经 `main.rs` 的事件桥转为前端的 `workspace-saved`，驱动「已自动备份」指示）。
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
/// **只更新内存中的文件内容，不落盘**（前端 2 秒防抖的落点）：磁盘写入由
/// auto-save 周期与显式 `save_workspace`（切题 / 失焦 / 关窗时前端编排）负责，
/// 「自动保存间隔」因此真正决定落盘频率。
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
