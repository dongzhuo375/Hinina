//! 审计 / 日志消费者 —— 核心事件的**只读**观察者。
//!
//! 存在的意义有两个：
//!
//! 1. 排障与审计：所有核心事实集中在一处按事件名结构化落日志，不必到各 Service
//!    里翻日志拼时间线；
//! 2. 结构验证：它是「同一底层事件流可挂多个互不相干的消费者」的**在产证据** ——
//!    与 Tauri 前端桥、PluginHost 平级，谁也不依赖谁。
//!
//! **只读**：不做任何持久化、不调用 Service、不改变任何状态。因此它落后、
//! 崩溃或缺失都不影响任何业务动作（`CoreEventBus::publish` 在无消费者时也只是
//! 记一条 `debug`）。

use std::sync::Arc;

use tracing::{debug, info};

use crate::core::event::consumer::{spawn_consumer, ConsumerExit, ResyncReason};
use crate::core::event::core_event::CoreEvent;
use crate::core::event::core_event_bus::CoreEventBus;

/// 审计消费者的稳定名（日志字段）。
pub const AUDIT_CONSUMER_NAME: &str = "audit";

/// 启动审计消费者。
///
/// 返回的 `JoinHandle` 产出 [`ConsumerExit`]，供组合根在停机时观察。
pub fn spawn_audit_consumer(bus: &Arc<CoreEventBus>) -> tokio::task::JoinHandle<ConsumerExit> {
    spawn_consumer(
        bus,
        AUDIT_CONSUMER_NAME,
        |event: CoreEvent| {
            log_event(&event);
            std::future::ready(())
        },
        // 审计只读，且不承担任何状态同步职责：落后无需重新查询任何东西。
        |reason: ResyncReason| {
            if let ResyncReason::Lagged(lost) = reason {
                debug!(lost, "审计消费者落后（只读消费者，无需重新同步）");
            }
        },
    )
}

/// 按事件类型记录审计日志。
///
/// 分级依据「排障价值 / 频率」：
/// - `info`：低频且对复盘有意义的事实（登录登出、会话失效、OJ 切换、配置与主题变更、
///   工作区落盘、提交创建与判定、新公告）；
/// - `debug`：可能高频的纯状态提醒。
///
/// 只记录事件名与 ID / 状态摘要 —— `CoreEvent` 的载荷本身不含任何敏感字段
/// （见 `core::event::core_event` 的载荷约束），这里也不额外拼接领域实体。
fn log_event(event: &CoreEvent) {
    match event {
        CoreEvent::LoggedIn { oj_id, user_id } => {
            info!(event = event.kind(), oj_id = %oj_id, user_id = %user_id, "审计");
        }
        CoreEvent::LoggedOut { oj_id } => {
            info!(event = event.kind(), oj_id = %oj_id, "审计");
        }
        CoreEvent::SessionExpired { oj_id } => {
            info!(event = event.kind(), oj_id = %oj_id, "审计");
        }
        CoreEvent::TokenRotated { oj_id } => {
            // 只记录「已轮换」这一事实，不记录凭证内容
            info!(event = event.kind(), oj_id = %oj_id, "审计");
        }
        CoreEvent::OjSwitched { oj_id } => {
            info!(event = event.kind(), oj_id = %oj_id, "审计");
        }
        CoreEvent::WorkspaceSaved {
            workspace_id,
            revision,
            automatic,
        } => {
            info!(
                event = event.kind(),
                workspace_id = %workspace_id,
                revision,
                automatic,
                "审计"
            );
        }
        CoreEvent::SubmissionCreated { submission_id } => {
            info!(event = event.kind(), submission_id = %submission_id, "审计");
        }
        CoreEvent::SubmissionJudged {
            submission_id,
            status,
        } => {
            info!(
                event = event.kind(),
                submission_id = %submission_id,
                status = %status,
                "审计"
            );
        }
        CoreEvent::AnnouncementChanged {
            contest_id,
            new_ids,
        } => {
            info!(
                event = event.kind(),
                contest_id = %contest_id,
                new_count = new_ids.len(),
                "审计"
            );
        }
        CoreEvent::ContestSelected { contest_id } => {
            info!(event = event.kind(), contest_id = %contest_id, "审计");
        }
        CoreEvent::ProblemOpened {
            contest_id,
            problem_id,
        } => {
            debug!(
                event = event.kind(),
                contest_id = %contest_id,
                problem_id = %problem_id,
                "审计"
            );
        }
        CoreEvent::ConfigChanged => {
            info!(event = event.kind(), "审计");
        }
        CoreEvent::ThemeChanged => {
            debug!(event = event.kind(), "审计");
        }
    }
}

#[cfg(test)]
#[path = "tests/audit_tests.rs"]
mod tests;
