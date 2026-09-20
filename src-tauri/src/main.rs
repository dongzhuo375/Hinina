// Hinina 入口点
//
// 初始化顺序（实际执行在 `AppContext::init`，此处为速查，改动请同步 core/context.md）：
//   0. 数据目录         — `infra::data_dir::prepare_startup`：解析 base_dir + 一次性迁移
//                         （**必须最先**，日志一旦打开就会占住旧目录）
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
//
// 为什么整个初始化在 `.setup()` 里而不是 `main()`：只有 setup 能拿到 AppHandle，
// 也就只有那里能解析 `app_local_data_dir()`（数据根目录）。

use hinina_lib::commands;
use hinina_lib::core::context::AppContext;
use hinina_lib::infra::data_dir::{self, DataDirPlan, DataDirSource, MigrateOutcome};

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

/// 把「配置已重载」事件接到 auto-save 的启停同步上。
///
/// 设置页改「自动保存开关/间隔」原先要重启客户端才生效（旧实现用一次性
/// `static AtomicBool` 懒启动，关掉后再也起不来，间隔也只读首次配置）。
/// 现在配置变更即生效：`update_config` 落盘后发布 `SystemEvent::ConfigReloaded`，
/// 本订阅者按新配置同步 auto-save（判据见 `commands::workspace_cmd::auto_save_action`）。
///
/// **为什么放在组合根**：与工作区落盘事件桥同理 —— 事件是应用级关注点，
/// 且这里才拿得到 `WorkspaceManager` 与配置服务的实例。
fn install_auto_save_config_sync(app: &tauri::AppHandle) {
    use std::sync::Arc;

    use tauri::Manager;

    use hinina_lib::commands::workspace_cmd::sync_auto_save_with_config;
    use hinina_lib::core::event::app_event::{AppEvent, SystemEvent};
    use hinina_lib::core::event::event_bus::EventHandler;
    use hinina_lib::core::event::event_category::EventCategory;

    let ctx = app.state::<AppContext>();
    let event_bus = Arc::clone(&ctx.event_bus);
    let config = Arc::clone(&ctx.config);
    let workspace_manager = ctx.workspace_manager.clone();

    let handler: EventHandler = Arc::new(move |event: &AppEvent| {
        if !matches!(event, AppEvent::System(SystemEvent::ConfigReloaded)) {
            return;
        }
        let Some(wm) = workspace_manager.as_ref() else {
            return;
        };
        // 事件在发布方（update_config 命令）的栈内同步执行，处于 tokio 上下文，
        // `start_auto_save` 内部的 tokio::spawn 可用。
        let editor = config.get().editor;
        sync_auto_save_with_config(&editor, wm);
    });

    event_bus.subscribe(EventCategory::System, handler);
}

/// 汇报数据目录方案与迁移结果（在日志就绪后调用）。
///
/// 三件事必须说清楚，因为它们都影响「我的数据到底在哪、会不会丢」：
/// - **回退到临时目录** → `error` 级告警（数据随时可能被系统清理，这是本次修复要消除的状态）；
/// - **迁移有失败项** → `error` 级告警并列出条目（下次启动会自动重试）；
/// - **迁移成功** → `info` 记录去向。
fn report_data_dir(plan: &DataDirPlan, migration: &MigrateOutcome) {
    match plan.source {
        DataDirSource::Default => {
            tracing::info!(dir = %plan.base_dir.display(), "数据目录（默认）");
        }
        DataDirSource::Custom => {
            tracing::info!(dir = %plan.base_dir.display(), "数据目录（用户指定）");
        }
        DataDirSource::FallbackTemp => {
            tracing::error!(
                dir = %plan.base_dir.display(),
                default_dir = %plan.default_dir.display(),
                "数据目录回退到临时目录：数据随时可能被系统清理！请在设置页更改数据目录"
            );
        }
    }

    if migration.is_noop() {
        return;
    }
    if migration.is_ok() {
        tracing::info!(
            moved = ?migration.moved,
            skipped = ?migration.skipped,
            legacy_removed = migration.legacy_removed,
            "数据已迁移到新的数据目录"
        );
    } else {
        // 不阻断启动：数据仍在原处，客户端能正常工作；但必须让用户与排障者看见
        tracing::error!(
            moved = ?migration.moved,
            failed = ?migration.failed,
            "数据迁移未全部完成（下次启动会自动重试）；失败项仍留在原目录"
        );
    }
}

fn main() {
    // 运行时在 setup 之前建好：AppContext::init 是 async，而 setup 是同步回调。
    // 在 setup 里 block_on 是安全的 —— 那时事件循环还没跑起来，不存在阻塞它的风险。
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    tauri::Builder::default()
        // 目录选择器（设置页「数据目录」）。本项目此前不注册任何 Tauri 插件
        // （fs/http 的依赖与 capability 存在但从未注册，全部 I/O 都走 Rust），
        // 故这里是**第一个真正注册的插件**。
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            use tauri::Manager;

            // AppContext 必须在这里初始化而不是 main()：只有 setup 能拿到 AppHandle，
            // 也就只有这里能解析 `app_local_data_dir()`。
            let default_dir = app
                .path()
                .app_local_data_dir()
                .unwrap_or_else(|e| {
                    // 拿不到默认目录时退到临时目录，让 resolve 继续走它的回退链
                    eprintln!("解析 app_local_data_dir 失败，回退临时目录: {e}");
                    data_dir::legacy_dir()
                });
            let legacy = data_dir::legacy_dir();

            // ① 一次性数据迁移必须在 `AppContext::init` **之前**：init 会打开日志文件，
            //    之后旧目录里的 logs/ 就被占住（虽然迁移清单不含 logs，但把顺序固定下来
            //    能让「谁在什么时候动文件」保持可推理）。
            let (plan, migration) = data_dir::prepare_startup(&default_dir, &legacy);

            let ctx = rt
                .block_on(AppContext::init(
                    plan.base_dir.clone(),
                    plan.default_dir.clone(),
                    plan.source,
                ))
                .expect("Failed to initialize AppContext");

            // ② 迁移结果与数据目录来源必须**在日志就绪后**立刻汇报
            report_data_dir(&plan, &migration);

            app.manage(ctx);
            install_workspace_event_bridge(app.handle());
            install_announcement_event_bridge(app.handle());
            install_auto_save_config_sync(app.handle());
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
            commands::maintenance_cmd::reset_client,
            commands::maintenance_cmd::local_data_usage,
            commands::maintenance_cmd::purge_local_data,
            commands::data_dir_cmd::get_data_dir,
            commands::data_dir_cmd::set_data_dir,
            commands::data_dir_cmd::reset_data_dir,
            commands::data_dir_cmd::pick_data_dir,
            commands::theme_cmd::get_theme,
            commands::theme_cmd::set_theme,
        ])
        .run(tauri::generate_context!())
        .expect("Failed to launch Hinina");
}
