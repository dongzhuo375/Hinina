// Hinina 入口点
//
// 初始化顺序（实际执行在 `AppContext::init`，此处为速查，改动请同步 core/context.md）：
//   1. Logger           — 最先初始化，后续步骤才能记录日志
//   2. Storage          — 文件系统根目录（base_dir），配置与工作区都依赖它
//   3. EventBus         — 纯内存事件总线，ConfigService 与各 Service 都要发布事件
//   4. ConfigService    — 经 FsConfigRepository 读 config.json，决定 OJ 基址等后续行为
//   5. HttpClient       — Reqwest 客户端
//   6. ProviderRegistry — 注册各 OJ Adapter（默认 HOJ，基址取自配置）
//   7. WorkspaceManager — 工作区生命周期（按需创建/恢复，启动时不扫描磁盘）
//   8. Service 层       — theme / auth / contest / problem / submission
//   9. AppContext       — 装配上述所有
//  10. Tauri App        — manage(AppContext) + generate_handler! 注册 Command

use hinina_lib::commands;
use hinina_lib::core::context::AppContext;

/// 工作区落盘事件的前端通道名（与 `src/bridge/workspace.bridge.ts` 的监听一致）。
const WORKSPACE_SAVED_EVENT: &str = "workspace-saved";

/// 新公告事件的前端通道名（与 `src/bridge/announcement.bridge.ts` 的监听一致）。
const ANNOUNCEMENTS_PUBLISHED_EVENT: &str = "announcements-published";

/// 把工作区落盘事件桥接到 webview。
///
/// 前端「已自动备份」指示必须反映**磁盘真值**，而后台 auto-save 由 Rust 触发、
/// 前端无从感知（这是唯一非前端发起的落盘路径），故在此订阅 `EventCategory::Workspace`
/// 并 emit 到 webview。仅转发两种「内容确已落盘」的事件：
/// - `Saved`（显式保存成功）
/// - `AutoSaveTriggered`（auto-save 成功且快照之后无新改动，见 manager 的循环语义）
///
/// `Loaded` / `Switched` 不转发：前端是它们的发起方，无需回环。
fn install_workspace_event_bridge(app: &tauri::AppHandle) {
    use std::sync::Arc;

    use tauri::{Emitter, Manager};

    use hinina_lib::core::event::app_event::{AppEvent, WorkspaceEvent};
    use hinina_lib::core::event::event_bus::EventHandler;
    use hinina_lib::core::event::event_category::EventCategory;

    let ctx = app.state::<AppContext>();
    let handle = app.clone();

    let handler: EventHandler = Arc::new(move |event: &AppEvent| {
        let AppEvent::Workspace(workspace_event) = event else {
            return;
        };
        let (workspace_id, auto) = match workspace_event {
            WorkspaceEvent::Saved { workspace_id } => (workspace_id.clone(), false),
            WorkspaceEvent::AutoSaveTriggered { workspace_id } => (workspace_id.clone(), true),
            _ => return,
        };
        let payload = serde_json::json!({ "workspaceId": workspace_id, "auto": auto });
        if let Err(e) = handle.emit(WORKSPACE_SAVED_EVENT, payload) {
            tracing::warn!(error = %e, "工作区落盘事件下发前端失败");
        }
    });

    ctx.event_bus
        .subscribe(EventCategory::Workspace, handler);
}

/// 把「检测到新公告」事件桥接到 webview。
///
/// 公告红点必须**由事件驱动**：前端虽仍按 60s 节拍拉取公告，但「有新公告」
/// 这一状态变更走 EventBus（项目约定：查询走 Service、状态变更走 EventBus），
/// 由本桥转发后前端即时点亮红点，而不是等下一次列表 diff。
///
/// 只转发 `AnnouncementsPublished`；`ListLoaded` / `Selected` 等前端是发起方，无需回环。
fn install_announcement_event_bridge(app: &tauri::AppHandle) {
    use std::sync::Arc;

    use tauri::{Emitter, Manager};

    use hinina_lib::core::event::app_event::{AppEvent, ContestEvent};
    use hinina_lib::core::event::event_bus::EventHandler;
    use hinina_lib::core::event::event_category::EventCategory;

    let ctx = app.state::<AppContext>();
    let handle = app.clone();

    let handler: EventHandler = Arc::new(move |event: &AppEvent| {
        let AppEvent::Contest(ContestEvent::AnnouncementsPublished {
            contest_id,
            new_ids,
        }) = event
        else {
            return;
        };
        let payload = serde_json::json!({ "contestId": contest_id, "newIds": new_ids });
        if let Err(e) = handle.emit(ANNOUNCEMENTS_PUBLISHED_EVENT, payload) {
            tracing::warn!(error = %e, "新公告事件下发前端失败");
        }
    });

    ctx.event_bus.subscribe(EventCategory::Contest, handler);
}

fn main() {
    // 运行时初始化，阻塞式
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let ctx = rt.block_on(async {
        // TODO: 从 Tauri app_data_dir 获取正式路径（阶段 7 实现后完善）
        let base_dir = std::env::temp_dir().join("hinina");
        AppContext::init(base_dir)
            .await
            .expect("Failed to initialize AppContext")
    });

    tauri::Builder::default()
        .manage(ctx)
        .setup(|app| {
            install_workspace_event_bridge(app.handle());
            install_announcement_event_bridge(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth_cmd::login,
            commands::auth_cmd::logout,
            commands::auth_cmd::get_session,
            commands::auth_cmd::validate_session,
            commands::oj_cmd::switch_oj,
            commands::contest_cmd::list_contests,
            commands::contest_cmd::select_contest,
            commands::contest_cmd::load_configured_contest,
            commands::contest_cmd::get_contest_rank,
            commands::contest_cmd::list_contest_announcements,
            commands::contest_cmd::get_read_announcement_ids,
            commands::contest_cmd::mark_announcements_read,
            commands::problem_cmd::get_problem,
            commands::problem_cmd::list_problems,
            commands::problem_cmd::get_user_problem_status,
            commands::problem_cmd::get_contest_problem_limits,
            commands::submission_cmd::submit_code,
            commands::submission_cmd::get_judgement,
            commands::submission_cmd::list_contest_submissions,
            commands::submission_cmd::get_submission_detail,
            commands::submission_cmd::get_submission_cases,
            commands::workspace_cmd::load_workspace,
            commands::workspace_cmd::save_workspace,
            commands::workspace_cmd::switch_workspace,
            commands::workspace_cmd::current_workspace,
            commands::workspace_cmd::update_workspace_file,
            commands::workspace_cmd::set_workspace_language,
            commands::config_cmd::get_config,
            commands::config_cmd::reload_config,
            commands::config_cmd::update_config,
            commands::config_cmd::get_storage_info,
            commands::cache_cmd::clear_cache,
            commands::theme_cmd::get_theme,
            commands::theme_cmd::set_theme,
        ])
        .run(tauri::generate_context!())
        .expect("Failed to launch Hinina");
}
