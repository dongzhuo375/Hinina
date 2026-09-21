//! `CoreEvent` → `PluginEvent` 适配器 —— 插件事件边界的**唯一**转换点。
//!
//! 职责（`PluginHost` 只负责驱动它，不重复实现其中任何一项）：
//!
//! - **白名单**：不在协议内的事件直接丢弃（当前唯一被排除的是
//!   `CoreEvent::TokenRotated`）；
//! - **字段裁剪 / 脱敏**：只搬运协议声明的字段（`AnnouncementChanged` 只带数量、
//!   `SessionLoggedIn` 不带用户 UUID），因此敏感内容**在类型层面**就无法外泄；
//! - **协议版本**：每条 envelope 都带 `PLUGIN_EVENT_PROTOCOL_VERSION`；
//! - **单调序号**：每个插件订阅持有一个适配器实例，序号连续，插件可据此自查缺口。
//!
//! 本模块**不**创建任何 channel、不持有总线、不做分发 —— 那些是 `PluginHost`
//! 与 `core::event` 的职责。

use crate::core::event::core_event::CoreEvent;
use crate::plugin::api::event::{PluginEvent, PluginEventEnvelope, PLUGIN_EVENT_PROTOCOL_VERSION};

/// 单插件订阅的事件适配器。
///
/// 每个插件一个实例：序号是**每订阅**的连续计数，插件侧据此检测缺口。
#[derive(Debug, Default)]
pub struct PluginEventAdapter {
    sequence: u64,
}

impl PluginEventAdapter {
    /// 创建适配器（序号从 0 起，首个事件为 1）。
    #[must_use]
    pub fn new() -> Self {
        Self { sequence: 0 }
    }

    /// 把核心事件适配为插件事件信封。
    ///
    /// 白名单外的事件返回 `None`，且**不消耗序号** —— 否则插件会看到
    /// 「序号跳号但没收到任何事件」的假缺口。
    pub fn adapt(&mut self, event: &CoreEvent) -> Option<PluginEventEnvelope> {
        let plugin_event = to_plugin_event(event)?;
        Some(self.wrap(plugin_event))
    }

    /// 生成一条协议级重新同步通知（`PluginHost` 在 `Lagged` / 订阅建立时下发）。
    pub fn resync_notice(&mut self, lost: u64) -> PluginEventEnvelope {
        self.wrap(PluginEvent::ResyncRequired { lost })
    }

    /// 当前序号（已下发的最后一条）。
    #[must_use]
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// 包装为信封并推进序号。
    fn wrap(&mut self, event: PluginEvent) -> PluginEventEnvelope {
        self.sequence += 1;
        PluginEventEnvelope {
            version: PLUGIN_EVENT_PROTOCOL_VERSION,
            sequence: self.sequence,
            occurred_at: utc_now_ms(),
            event,
        }
    }
}

/// 白名单映射：返回 `None` 表示该核心事件不对插件开放。
///
/// **脱敏要点**：
/// - `LoggedIn` 只取 `oj_id`（用户 UUID 属内部标识，插件经插件 API 按权限查询）；
/// - `AnnouncementChanged` 只取公告数量（正文可能很长且含未读语义，经 API 查询）；
/// - `WorkspaceSaved` 只取工作区 id / 修订号 / 是否自动（**不含**文件内容与路径）；
/// - `SubmissionJudged` 只取状态摘要（不含耗时、内存、测试点明细）。
fn to_plugin_event(event: &CoreEvent) -> Option<PluginEvent> {
    let plugin_event = match event {
        CoreEvent::OjSwitched { oj_id } => PluginEvent::OjSwitched {
            oj_id: oj_id.clone(),
        },
        CoreEvent::ContestSelected { contest_id } => PluginEvent::ContestSelected {
            contest_id: contest_id.clone(),
        },
        CoreEvent::AnnouncementChanged {
            contest_id,
            new_ids,
        } => PluginEvent::AnnouncementChanged {
            contest_id: contest_id.clone(),
            new_count: new_ids.len(),
        },
        CoreEvent::ProblemOpened {
            contest_id,
            problem_id,
        } => PluginEvent::ProblemOpened {
            contest_id: contest_id.clone(),
            problem_id: problem_id.clone(),
        },
        CoreEvent::SubmissionCreated { submission_id } => PluginEvent::SubmissionCreated {
            submission_id: submission_id.clone(),
        },
        CoreEvent::SubmissionJudged {
            submission_id,
            status,
        } => PluginEvent::SubmissionJudged {
            submission_id: submission_id.clone(),
            status: status.clone(),
        },
        CoreEvent::WorkspaceSaved {
            workspace_id,
            revision,
            automatic,
        } => PluginEvent::WorkspaceSaved {
            workspace_id: workspace_id.clone(),
            revision: *revision,
            automatic: *automatic,
        },
        CoreEvent::ConfigChanged => PluginEvent::ConfigChanged,
        CoreEvent::ThemeChanged => PluginEvent::ThemeChanged,
        CoreEvent::LoggedIn { oj_id, .. } => PluginEvent::SessionLoggedIn {
            oj_id: oj_id.clone(),
        },
        CoreEvent::LoggedOut { oj_id } => PluginEvent::SessionLoggedOut {
            oj_id: oj_id.clone(),
        },
        CoreEvent::SessionExpired { oj_id } => PluginEvent::SessionExpired {
            oj_id: oj_id.clone(),
        },
        // 白名单外：凭证轮换是内部安全生命周期事实，对插件无价值且扩大攻击面。
        CoreEvent::TokenRotated { .. } => return None,
    };
    Some(plugin_event)
}

/// 当前 UTC 毫秒时间戳。
fn utc_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
#[path = "tests/event_adapter_tests.rs"]
mod tests;
