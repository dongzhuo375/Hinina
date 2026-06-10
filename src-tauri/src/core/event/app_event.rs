use crate::core::entity::contest::Contest;
use crate::core::entity::submission::JudgementResult;
use crate::core::entity::user::User;
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

#[derive(Debug, Clone)]
pub enum AuthEvent {
    LoginSuccess { user: User },
    Logout,
    SessionExpired,
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
    PollTimeout { submission_id: String },
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
