//! 核心事件（`CoreEvent`）—— 进程内**事实通知**的统一载荷。
//!
//! # 语义边界（硬约束）
//!
//! `CoreEvent` 表示「某个事实**已经发生**」，**不是**「请某个消费者执行某个动作」。
//! 因此：
//!
//! - 必须等待结果、否则会导致数据错误或状态冲突的逻辑（工作区落盘、OJ 切换清缓存、
//!   配置落盘、Session 持久化、登出清理）**由 Service / Command 显式完成**，
//!   不得依赖事件消费者；
//! - 事件**允许丢失**（`broadcast` 是有限容量、可 Lagged 的异步通道），
//!   消费者必须能通过 Service / IPC 查询重新同步；
//! - 消费者**不能回滚**已经完成的核心业务动作。
//!
//! # 载荷约束
//!
//! 载荷只放 ID、修订号与状态摘要。**禁止**出现：
//! Token、密码、完整 Session、认证头、源代码、内部文件路径、完整领域实体、
//! 内部缓存对象、未脱敏的用户实体、内部错误栈。
//!
//! 需要大数据时，消费者收到事件后经 Service / IPC 查询（事件只负责「提醒去看」）。

/// 核心事件。
///
/// `Clone` + 轻量：`broadcast` 会为每个 receiver 克隆一份，载荷过大会放大开销。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreEvent {
    // ── 认证 ──
    /// 登录成功（**不含** token 与完整 User 实体）。
    LoggedIn { oj_id: String, user_id: String },
    /// 登出完成（会话已删除、用户域缓存已清理 —— 清理是显式动作，本事件只是通知）。
    LoggedOut { oj_id: String },
    /// 会话被服务端判定失效（本地会话已清除）。
    SessionExpired { oj_id: String },
    /// Provider 侧凭证已轮换，且**新凭证已显式落盘成功**。
    ///
    /// 只带 OJ 标识：token 本身绝不进入事件流。
    TokenRotated { oj_id: String },

    // ── 系统 ──
    /// 配置已变更并**已成功落盘**。
    ConfigChanged,
    /// 主题已切换。
    ThemeChanged,
    /// 当前 OJ 已切换（缓存清理是显式动作，本事件只是通知）。
    OjSwitched { oj_id: String },

    // ── 比赛 / 题目 ──
    /// 已选中比赛。
    ContestSelected { contest_id: String },
    /// 检测到新公告（按公告 ID 基线比对得出，首次拉取不发）。
    ///
    /// 公告本身仍由前端轮询经 IPC 获取；本事件只是「有新内容」的刷新提醒，
    /// 丢失后下一次轮询必须能恢复。
    AnnouncementChanged {
        contest_id: String,
        new_ids: Vec<String>,
    },
    /// 已打开题目（题面内容不进入事件，由 IPC 返回值承载）。
    ProblemOpened {
        contest_id: String,
        problem_id: String,
    },

    // ── 提交 ──
    /// 提交已创建。
    SubmissionCreated { submission_id: String },
    /// 评测到达终态（`status` 为 `JudgementStatus` 的稳定字符串名）。
    ///
    /// **不替代轮询**：前端仍按自己的节拍经 IPC 查询评测状态，
    /// 本事件只用于其他页面 / 插件按需响应。
    SubmissionJudged {
        submission_id: String,
        status: String,
    },

    // ── 工作区 ──
    /// 工作区内容**确已落盘**（`automatic` = 后台 auto-save 触发）。
    ///
    /// 写盘失败或快照之后又有新改动时**不发布**本事件 —— 它等价于
    /// 「最新内容已在磁盘上」，前端据此清除「编辑中…」指示。
    WorkspaceSaved {
        workspace_id: String,
        /// 内容修订号：消费者据此丢弃过期事件（切题前保存旧工作区的事件
        /// 可能在新工作区已加载之后才送达）。
        revision: u64,
        automatic: bool,
    },
}

impl CoreEvent {
    /// 事件的稳定短名（结构化日志 / 插件事件名用）。
    ///
    /// 不用 `EventCategory` 之类的分类枚举：单一 broadcast 通道下，
    /// 消费者直接 `match` 事件变体，分类表只会成为第二套需要同步维护的映射。
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::LoggedIn { .. } => "auth.logged_in",
            Self::LoggedOut { .. } => "auth.logged_out",
            Self::SessionExpired { .. } => "auth.session_expired",
            Self::TokenRotated { .. } => "auth.token_rotated",
            Self::ConfigChanged => "system.config_changed",
            Self::ThemeChanged => "system.theme_changed",
            Self::OjSwitched { .. } => "system.oj_switched",
            Self::ContestSelected { .. } => "contest.selected",
            Self::AnnouncementChanged { .. } => "contest.announcement_changed",
            Self::ProblemOpened { .. } => "problem.opened",
            Self::SubmissionCreated { .. } => "submission.created",
            Self::SubmissionJudged { .. } => "submission.judged",
            Self::WorkspaceSaved { .. } => "workspace.saved",
        }
    }
}

#[cfg(test)]
#[path = "tests/core_event_tests.rs"]
mod tests;
