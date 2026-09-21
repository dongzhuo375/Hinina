//! 插件事件协议 —— 插件侧**唯一**允许依赖的事件接口。
//!
//! # 边界（硬约束）
//!
//! 插件**不得**直接依赖：
//!
//! - `CoreEvent`（内部事件载荷）
//! - 内部 Service 类型、领域实体、内部缓存结构
//! - `tokio::sync::broadcast::Receiver<CoreEvent>`
//!
//! 插件只能收到经 `PluginEventAdapter` 转换、白名单过滤、脱敏与版本化之后的
//! [`PluginEventEnvelope`]。转换与过滤发生在 `plugin::host::event_adapter`，
//! 由 `PluginHost` 统一驱动 —— **不存在第二套业务总线**。
//!
//! # 为什么是独立类型而不是复用 `CoreEvent`
//!
//! 1. **稳定性**：`CoreEvent` 是内部实现，随重构演进；插件协议必须能独立版本化。
//! 2. **权限与脱敏**：协议层只暴露「插件确实需要」的字段，内部字段（如用户 UUID、
//!    凭证轮换事实）不进协议。
//! 3. **可跨进程**：`Serialize` + `Deserialize`，将来换 JS/WASM 运行时或跨进程
//!    投递时不需要改协议形状。

use serde::{Deserialize, Serialize};

/// 插件事件协议版本。
///
/// 任何**破坏性**变更（删除变体、改字段语义、改必填性）都必须自增，
/// 插件据 `envelope.version` 决定是否继续处理。
pub const PLUGIN_EVENT_PROTOCOL_VERSION: u32 = 1;

/// 插件可见的事件。
///
/// # 白名单
///
/// 只有本枚举中的变体会被投递给插件（由 `PluginEventAdapter` 决定映射）。
/// 内部事件 `CoreEvent::TokenRotated` **刻意不在白名单内**：凭证轮换是内部
/// 安全生命周期事件，对插件没有任何价值，暴露它只会扩大攻击面。
///
/// # 禁止出现的内容
///
/// Token、密码、完整 Session、认证头、源代码、内部文件路径、内部缓存对象、
/// 未脱敏的用户实体、内部错误栈、内部实现细节。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum PluginEvent {
    /// 当前 OJ 已切换。
    OjSwitched { oj_id: String },

    /// 已选中比赛。
    ContestSelected { contest_id: String },

    /// 检测到新公告（只给数量，不给正文 —— 正文经插件 API 按权限查询）。
    AnnouncementChanged {
        contest_id: String,
        new_count: usize,
    },

    /// 已打开题目（不含题面内容）。
    ProblemOpened {
        contest_id: String,
        problem_id: String,
    },

    /// 提交已创建。
    SubmissionCreated { submission_id: String },

    /// 评测到达终态（状态摘要，不含评测明细）。
    SubmissionJudged {
        submission_id: String,
        status: String,
    },

    /// 工作区内容已落盘（不含文件内容，工作区路径也不暴露）。
    WorkspaceSaved {
        workspace_id: String,
        revision: u64,
        automatic: bool,
    },

    /// 配置已变更并落盘（不含具体配置项 —— 配置内容经插件 API 按权限查询）。
    ConfigChanged,

    /// 主题已切换。
    ThemeChanged,

    /// 用户已登录（只给 OJ 标识；用户身份经插件 API 查询，避免协议层外泄标识）。
    SessionLoggedIn { oj_id: String },

    /// 用户已登出。
    SessionLoggedOut { oj_id: String },

    /// 会话已失效。
    SessionExpired { oj_id: String },

    /// **协议级通知**：插件错过了 `lost` 条事件，必须重新查询当前状态。
    ///
    /// 由 `PluginHost` 在底层消费者 `Lagged` 时下发（也用于插件刚订阅时告知
    /// 「请先同步一次当前状态」）。插件收到后应调用插件 API 拉取真值，
    /// 而不是假设事件会自动补发。
    ResyncRequired { lost: u64 },
}

/// 插件事件信封 —— 版本、序号与发生时间随事件一起下发。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginEventEnvelope {
    /// 协议版本（见 [`PLUGIN_EVENT_PROTOCOL_VERSION`]）
    pub version: u32,
    /// 该插件订阅上的单调递增序号（从 1 开始）。
    ///
    /// 插件可据此**自行检测缺口**（序号不连续 → 主动重新同步），
    /// 与 `ResyncRequired` 互为补充：前者是插件侧的兜底，后者是宿主侧的主动通知。
    pub sequence: u64,
    /// 事件发生时间（UTC 毫秒时间戳）
    pub occurred_at: i64,
    /// 事件本体
    pub event: PluginEvent,
}

#[cfg(test)]
#[path = "tests/event_tests.rs"]
mod tests;
