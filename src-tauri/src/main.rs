// Hinina 入口点
//
// 初始化顺序（实际执行在 `AppContext::init`，此处为速查，改动请同步 core/context.md）：
//   0. 数据目录         — `infra::data_dir::prepare_startup`：解析 base_dir + 一次性迁移
//                         （**必须最先**，日志一旦打开就会占住旧目录）
//   1. Logger           — 最先初始化，后续步骤才能记录日志
//   2. Storage          — 文件系统根目录（base_dir），配置、会话与工作区都依赖它
//   3. CoreEventBus     — 进程内事实总线（`tokio::sync::broadcast` 封装）
//   4. SessionRepository— 会话持久化（AuthService 与 Provider 凭证轮换共用）
//   5. ConfigService    — 经 FsConfigRepository 读 config.json，决定 OJ 基址等后续行为
//   6. HttpClient       — Reqwest 客户端
//   7. ProviderRegistry — 注册各 OJ Adapter（默认 HOJ，基址取自配置）
//   8. WorkspaceManager — 工作区生命周期（按需创建/恢复，启动时不扫描磁盘）
//   9. Service 层       — theme / auth / contest / problem / submission
//  10. PluginHost       — 插件事件边界（只持总线句柄，消费者在下面启动）
//  11. AppContext       — 装配上述所有
//  12. Tauri App        — manage(AppContext) + 启动消费者 + generate_handler! 注册 Command
//
// 为什么整个初始化在 `.setup()` 里而不是 `main()`：只有 setup 能拿到 AppHandle，
// 也就只有那里能解析 `app_local_data_dir()`（数据根目录）。

use std::sync::Arc;

use tauri::{Emitter, Manager};

use hinina_lib::commands;
use hinina_lib::core::context::AppContext;
use hinina_lib::core::event::consumer::{spawn_consumer, ResyncReason};
use hinina_lib::core::event::core_event::CoreEvent;
use hinina_lib::core::event::core_event_bus::CoreEventBus;
use hinina_lib::infra::audit::spawn_audit_consumer;
use hinina_lib::infra::data_dir::{self, DataDirPlan, DataDirSource, MigrateOutcome};

/// 工作区落盘事件的前端通道名（与 `src/bridge/workspace.bridge.ts` 的监听一致）。
const WORKSPACE_SAVED_EVENT: &str = "workspace-saved";

/// 新公告事件的前端通道名（与 `src/bridge/announcement.bridge.ts` 的监听一致）。
const ANNOUNCEMENTS_PUBLISHED_EVENT: &str = "announcements-published";

/// 前端事件桥的消费者名（日志字段）。
const FRONTEND_BRIDGE_CONSUMER: &str = "tauri-frontend-bridge";

/// 启动全部事件消费者。
///
/// # 三类消费者，一条底层事件流
///
/// ```text
/// CoreEventBus（broadcast）
///   ├── tauri-frontend-bridge → emit 到 webview（前端 Store 只做 UI 刷新）
///   ├── audit                 → 只读审计日志
///   └── plugin-host           → CoreEvent → PluginEvent（白名单/脱敏/版本）
/// ```
///
/// **内部 Service 之间没有任何事件订阅**：工作区落盘、OJ 切换清缓存、配置落盘、
/// 会话持久化、登出清理全部由 Service / Command 显式完成。事件只承载「已经发生的
/// 事实」，因此消费者落后、崩溃或缺失都不会影响任何业务动作。
///
/// # 为什么整段包在 `tauri::async_runtime::spawn` 里
///
/// `spawn_consumer` 内部用 `tokio::spawn`，必须有 tokio runtime 上下文；
/// 而 `.setup()` 回调运行在 Tauri 的主线程上，不在运行时里。命令层（异步命令）
/// 天然在运行时内，组合根则需要显式投递一次。
fn spawn_event_consumers(app: &tauri::AppHandle) {
    let ctx = app.state::<AppContext>();
    let bus = Arc::clone(&ctx.event_bus);
    let plugin_host = Arc::clone(&ctx.plugin_host);
    let handle = app.clone();

    tauri::async_runtime::spawn(async move {
        spawn_frontend_event_bridge(Arc::clone(&bus), handle);
        spawn_audit_consumer(&bus);
        plugin_host.start();
        tracing::info!("事件消费者已启动（前端桥 / 审计 / 插件宿主）");
    });
}

/// 把核心事件桥接到 webview。
///
/// 只转发**前端需要即时感知**的事实：
/// - `WorkspaceSaved` → `workspace-saved`：后台 auto-save 由 Rust 触发，前端无从感知
///   （这是唯一非前端发起的落盘路径），故用它清除「编辑中…」指示；
/// - `AnnouncementChanged` → `announcements-published`：红点即时点亮，不必等下一轮 diff。
///
/// 其余事件（OJ 切换、配置变更、提交创建/判定……）前端都能从 IPC 返回值直接得到，
/// 转发只会制造「前端靠事件拿状态」的错误预期 —— 前端刷新触发之外的一切真实状态
/// 仍必须经 IPC 查询（评测状态、榜单、公告内容、工作区真值）。
///
/// **事件丢失可恢复**：桥落后时只记日志 —— 前端本来就有轮询（公告 60s、榜单 10s、
/// 题目总览 30s、评测状态），下一周期即补齐。
fn spawn_frontend_event_bridge(bus: Arc<CoreEventBus>, app: tauri::AppHandle) {
    spawn_consumer(
        &bus,
        FRONTEND_BRIDGE_CONSUMER,
        move |event: CoreEvent| {
            let app = app.clone();
            async move {
                forward_to_webview(&app, &event);
            }
        },
        |reason: ResyncReason| {
            if let ResyncReason::Lagged(lost) = reason {
                tracing::warn!(
                    lost,
                    "前端事件桥落后，已丢弃不可恢复的通知；前端轮询将在下一周期补齐真实状态"
                );
            }
        },
    );
}

/// 单条事件的转发逻辑。
fn forward_to_webview(app: &tauri::AppHandle, event: &CoreEvent) {
    match event {
        CoreEvent::WorkspaceSaved {
            workspace_id,
            revision,
            automatic,
        } => {
            let payload = serde_json::json!({
                "workspaceId": workspace_id,
                "revision": revision,
                "auto": automatic,
            });
            emit_or_warn(app, WORKSPACE_SAVED_EVENT, payload, "工作区落盘事件");
        }
        CoreEvent::AnnouncementChanged {
            contest_id,
            new_ids,
        } => {
            let payload = serde_json::json!({ "contestId": contest_id, "newIds": new_ids });
            emit_or_warn(app, ANNOUNCEMENTS_PUBLISHED_EVENT, payload, "新公告事件");
        }
        // 其余事件不桥接：前端经 IPC 返回值即可得到，转发只会造成状态来源分叉
        _ => {}
    }
}

/// `emit` 失败只告警：前端事件是刷新触发，丢了不影响正确性（轮询会补齐）。
fn emit_or_warn(app: &tauri::AppHandle, channel: &str, payload: serde_json::Value, what: &str) {
    if let Err(e) = app.emit(channel, payload) {
        tracing::warn!(channel = channel, error = %e, "{what}下发前端失败");
    }
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
    // 注意：事件消费者**不**跑在这个运行时上（它在 setup 结束时随 `rt` 一起销毁），
    // 而是由 `spawn_event_consumers` 投到 Tauri 的运行时 —— 与异步命令同一个。
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    tauri::Builder::default()
        // 目录选择器（设置页「数据目录」）。本项目此前不注册任何 Tauri 插件
        // （fs/http 的依赖与 capability 存在但从未注册，全部 I/O 都走 Rust），
        // 故这里是**第一个真正注册的插件**。
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            // AppContext 必须在这里初始化而不是 main()：只有 setup 能拿到 AppHandle，
            // 也就只有这里能解析 `app_local_data_dir()`。
            let default_dir = app.path().app_local_data_dir().unwrap_or_else(|e| {
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
            // ③ 消费者在 AppContext 就绪后启动：它们只观察事实，不参与任何业务动作
            spawn_event_consumers(app.handle());
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
            commands::workspace_cmd::delete_workspace_file,
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
