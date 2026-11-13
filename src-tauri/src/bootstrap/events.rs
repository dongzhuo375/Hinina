// CoreEvent → webview 的前端事件桥与消费者启动。
//
// # 三类消费者，一条底层事件流
//
// ```text
// CoreEventBus（broadcast）
//   ├── tauri-frontend-bridge → emit 到 webview（前端 Store 只做 UI 刷新）
//   ├── audit                 → 只读审计日志
//   └── plugin-host           → CoreEvent → PluginEvent（白名单/脱敏/版本）
// ```
//
// **内部 Service 之间没有任何事件订阅**：工作区落盘、OJ 切换清缓存、配置落盘、
// 会话持久化、登出清理全部由 Service / Command 显式完成。事件只承载「已经发生的
// 事实」，因此消费者落后、崩溃或缺失都不会影响任何业务动作。

use std::sync::Arc;

use tauri::{Emitter, Manager};

use crate::core::context::AppContext;
use crate::core::event::consumer::{spawn_consumer, ResyncReason};
use crate::core::event::core_event::CoreEvent;
use crate::core::event::core_event_bus::CoreEventBus;
use crate::infra::audit::spawn_audit_consumer;

/// 工作区落盘事件的前端通道名（与 `src/bridge/workspace.bridge.ts` 的监听一致）。
const WORKSPACE_SAVED_EVENT: &str = "workspace-saved";

/// 新公告事件的前端通道名（与 `src/bridge/announcement.bridge.ts` 的监听一致）。
const ANNOUNCEMENTS_PUBLISHED_EVENT: &str = "announcements-published";

/// 前端事件桥的消费者名（日志字段）。
const FRONTEND_BRIDGE_CONSUMER: &str = "tauri-frontend-bridge";

/// 启动全部事件消费者。
///
/// # 为什么整段包在 `tauri::async_runtime::spawn` 里
///
/// `spawn_consumer` 内部用 `tokio::spawn`，必须有 tokio runtime 上下文；
/// 而 `.setup()` 回调运行在 Tauri 的主线程上，不在运行时里。命令层（异步命令）
/// 天然在运行时内，组合根则需要显式投递一次。
pub(crate) fn spawn_event_consumers(app: &tauri::AppHandle) {
    let ctx = app.state::<AppContext>();
    let bus = Arc::clone(&ctx.event_bus);
    let plugin_host = Arc::clone(&ctx.plugin_host);
    let handle = app.clone();

    tauri::async_runtime::spawn(async move {
        spawn_frontend_event_bridge(Arc::clone(&bus), handle);
        spawn_audit_consumer(&bus);
        if let Err(error) = plugin_host.start() {
            tracing::warn!(plugin_id="", plugin_version="", runtime="js", event="",
                operation="host.start", permission="", request_id="", error=%error,
                "插件宿主消费者未启动");
        }
        tracing::info!("事件消费者已启动（前端桥 / 审计 / 插件宿主）");
    });
}

/// 把核心事件桥接到 webview。
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

/// 单条事件到前端通道的映射（`None` = 不桥接）。
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
/// match **刻意穷尽**（无 `_` 通配臂）：给 `CoreEvent` 新增变体时这里编译失败，
/// 迫使作者显式决定「桥接与否」——通配臂会让新变体静默落入「不桥接」而无人察觉。
fn payload_for(event: &CoreEvent) -> Option<(&'static str, serde_json::Value, &'static str)> {
    match event {
        CoreEvent::WorkspaceSaved {
            workspace_id,
            revision,
            automatic,
        } => Some((
            WORKSPACE_SAVED_EVENT,
            serde_json::json!({
                "workspaceId": workspace_id,
                "revision": revision,
                "auto": automatic,
            }),
            "工作区落盘事件",
        )),
        CoreEvent::AnnouncementChanged {
            contest_id,
            new_ids,
        } => Some((
            ANNOUNCEMENTS_PUBLISHED_EVENT,
            serde_json::json!({ "contestId": contest_id, "newIds": new_ids }),
            "新公告事件",
        )),
        // 其余事件不桥接：前端经 IPC 返回值即可得到，转发只会造成状态来源分叉
        CoreEvent::LoggedIn { .. } => None,
        CoreEvent::LoggedOut { .. } => None,
        CoreEvent::SessionExpired { .. } => None,
        CoreEvent::TokenRotated { .. } => None,
        CoreEvent::ConfigChanged => None,
        CoreEvent::ThemeChanged => None,
        CoreEvent::OjSwitched { .. } => None,
        CoreEvent::ContestSelected { .. } => None,
        CoreEvent::ProblemOpened { .. } => None,
        CoreEvent::SubmissionCreated { .. } => None,
        CoreEvent::SubmissionJudged { .. } => None,
    }
}

/// 单条事件的转发逻辑。
fn forward_to_webview(app: &tauri::AppHandle, event: &CoreEvent) {
    if let Some((channel, payload, what)) = payload_for(event) {
        emit_or_warn(app, channel, payload, what);
    }
}

/// `emit` 失败只告警：前端事件是刷新触发，丢了不影响正确性（轮询会补齐）。
fn emit_or_warn(app: &tauri::AppHandle, channel: &str, payload: serde_json::Value, what: &str) {
    if let Err(e) = app.emit(channel, payload) {
        tracing::warn!(channel = channel, error = %e, "{what}下发前端失败");
    }
}

#[cfg(test)]
#[path = "tests/events_tests.rs"]
mod tests;
