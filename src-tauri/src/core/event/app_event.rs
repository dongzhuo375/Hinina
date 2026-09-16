use crate::core::entity::contest::Contest;
use crate::core::entity::submission::JudgementResult;
use crate::core::entity::user::User;
use crate::core::event::event_category::EventCategory;
use crate::core::provider::oj_type::OJType;

/// 应用全局事件枚举。
/// 按领域分类，便于订阅者按类别过滤。
#[derive(Debug, Clone)]
pub enum AppEvent {
    Auth(AuthEvent),
    Contest(ContestEvent),
    Problem(ProblemEvent),
    Submission(SubmissionEvent),
    Workspace(WorkspaceEvent),
    System(SystemEvent),
}

impl AppEvent {
    /// 返回事件对应的 EventCategory，用于 EventBus 按类别分发。
    #[must_use]
    pub fn category(&self) -> EventCategory {
        match self {
            AppEvent::Auth(_) => EventCategory::Auth,
            AppEvent::Contest(_) => EventCategory::Contest,
            AppEvent::Problem(_) => EventCategory::Problem,
            AppEvent::Submission(_) => EventCategory::Submission,
            AppEvent::Workspace(_) => EventCategory::Workspace,
            AppEvent::System(_) => EventCategory::System,
        }
    }
}

#[derive(Debug, Clone)]
pub enum AuthEvent {
    LoginSuccess { user: User },
    Logout,
    SessionExpired,
    /// Provider 侧会话凭证已轮换（如 HOJ 的 Refresh-Token 机制）。
    ///
    /// 事件本身是认证域通用概念，仅携带新凭证字符串，不含任何 OJ 私有语义；
    /// 各 Provider 自行决定何时发布（qduoj/hustoj 当前不发布）。
    /// AuthService 订阅后将新凭证回写磁盘会话，避免重启后回注过期 token。
    TokenRefreshed { token: String },
}

#[derive(Debug, Clone)]
pub enum ContestEvent {
    ListLoaded { contests: Vec<Contest> },
    Selected { contest_id: String },
    CountdownTick { remaining_seconds: i64 },
}

#[derive(Debug, Clone)]
pub enum ProblemEvent {
    Opened { contest_id: String, problem_id: String },
    CodeChanged { problem_id: String, file_name: String },
}

#[derive(Debug, Clone)]
pub enum SubmissionEvent {
    Created { submission_id: String },
    Judged { submission_id: String, result: JudgementResult },
}

#[derive(Debug, Clone)]
pub enum WorkspaceEvent {
    Loaded { workspace_id: String },
    Saved { workspace_id: String },
    AutoSaveTriggered { workspace_id: String },
    Switched { from: String, to: String },
}

#[derive(Debug, Clone)]
pub enum SystemEvent {
    ConfigReloaded,
    ThemeChanged,
    OJSwitched { oj_type: OJType },
    WindowClosing,
}
