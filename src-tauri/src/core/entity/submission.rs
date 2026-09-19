use serde::{Deserialize, Serialize};

/// 评测状态。
///
/// 变体名即 IPC 序列化值（前端按这些确切名称做文案与配色映射），
/// 覆盖 HOJ 全部状态码（0-15，见 `adapter/hoj/types.rs` 的 `map_status`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JudgementStatus {
    Pending,
    Compiling,
    Running,
    Accepted,
    WrongAnswer,
    TimeLimitExceeded,
    MemoryLimitExceeded,
    RuntimeError,
    CompilationError,
    PresentationError,
    OutputLimitExceeded,
    SystemError,
    RemoteJudgeError,
    SubmitFailed,
    PartiallyAccepted,
    FrequentLimit,
    UnknownError,
    Unknown,
}

impl JudgementStatus {
    /// 是否已到达**终态**（评测不再变化，可停止轮询）。
    ///
    /// 非终态仅 `Pending` / `Compiling` / `Running` 三个 —— 其余（含 `Unknown`
    /// 与各类系统错误）一律视为终态：把无法识别的状态当非终态会让轮询无限进行。
    ///
    /// 三处判据必须保持一致（新增状态时同步）：
    /// - 本方法（核心层，OI/ACM 无关）
    /// - `adapter::hoj::types::is_terminal_status`（HOJ 原始状态码 → 终态，0/1 之外皆终态）
    /// - `adapter::hydro::types::is_terminal_status`（Hydro 原始状态码 → 终态，
    ///   0/20/21/22 之外皆终态；**22 FETCHED 特意折入 `Pending`** 而非 `Unknown`，
    ///   否则轮询会在评测开始前就停住，并把在途结果写进终态缓存）
    /// - 前端 `utils/submission.isTerminalStatus`（同一非终态集合）
    ///
    /// 用途：提交详情/测试点**只有终态结果才可缓存** —— 评测中的状态随时会变，
    /// 缓存它等于让界面停在「评测中」。
    pub fn is_terminal(&self) -> bool {
        !matches!(self, Self::Pending | Self::Compiling | Self::Running)
    }
}

/// 评测结果详情
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JudgementResult {
    pub status: JudgementStatus,
    pub score: f64,
    pub time_ms: u64,
    pub memory_kb: u64,
}

/// 提交列表条目（比赛提交记录页用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmissionRecord {
    pub submit_id: String,
    /// 题目真实 ID
    pub pid: String,
    /// 题目展示 ID（如 "HOJ-1061"）
    pub display_pid: String,
    pub title: String,
    /// 比赛中题目序号（如 "A"）
    pub display_id: String,
    pub username: String,
    /// 提交时间（UTC 秒级时间戳）
    pub submit_time: i64,
    pub status: JudgementStatus,
    /// 运行耗时（毫秒）
    pub time_ms: u64,
    /// 运行内存（KB）
    pub memory_kb: u64,
    /// OI 题目得分（ACM 题为 None）
    pub score: Option<f64>,
    /// 代码长度（字节）
    pub length: u64,
    pub language: String,
}

/// 提交列表分页结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmissionPage {
    pub records: Vec<SubmissionRecord>,
    pub total: i64,
    pub size: i64,
    pub current: i64,
    pub pages: i64,
}

/// 提交列表查询参数（内部类型，不跨 IPC 序列化）。
#[derive(Debug, Clone)]
pub struct SubmissionQuery {
    pub contest_id: String,
    pub current_page: i64,
    pub limit: i64,
    /// 只看本人提交（产品决策：后端强制为 true，见 commands/submission_cmd.rs）
    pub only_mine: bool,
    /// 按题目展示 ID 筛选（如 "A"）
    pub problem_display_id: Option<String>,
    /// 按 HOJ 评测状态码筛选
    pub status: Option<i32>,
}

/// 提交详情（含源代码与错误信息）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmissionDetail {
    pub submit_id: String,
    pub pid: String,
    pub display_pid: String,
    pub username: String,
    /// 提交时间（UTC 秒级时间戳）
    pub submit_time: i64,
    pub status: JudgementStatus,
    pub time_ms: u64,
    pub memory_kb: u64,
    pub score: Option<f64>,
    pub length: u64,
    pub language: String,
    pub code: String,
    /// 编译错误信息（CE 时非空）
    pub error_message: Option<String>,
    /// 判题机标识
    pub judger: Option<String>,
    /// OI 榜单计入分数
    pub oi_rank_score: Option<i32>,
}

/// 单个测试点的评测结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JudgeCase {
    pub case_id: i64,
    /// 测试点序号
    pub seq: i64,
    pub status: JudgementStatus,
    pub time_ms: u64,
    pub memory_kb: u64,
    pub score: Option<f64>,
    /// 子任务分组号（非子任务题为 None）
    pub group_num: Option<i64>,
}

/// 子任务分组（subtask 模式）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubTaskCases {
    pub group_num: i64,
    pub cases: Vec<JudgeCase>,
}

/// 提交的全部测试点结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmissionCases {
    /// 默认模式下的测试点列表
    pub cases: Vec<JudgeCase>,
    /// 子任务模式下的分组列表
    pub sub_tasks: Vec<SubTaskCases>,
    /// 判题模式："default" / "subtask_lowest" / "subtask_lowest_all" / "ergodic_without_skipped" 等
    pub mode: String,
}

#[cfg(test)]
#[path = "tests/submission_tests.rs"]
mod tests;
