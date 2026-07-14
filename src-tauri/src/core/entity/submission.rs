use serde::{Deserialize, Serialize};

/// 提交记录与评测状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Submission {
    pub id: String,
    pub problem_id: String,
    pub language: String,
    pub source_code: String,
    pub status: JudgementStatus,
}

/// 评测状态
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
    Unknown,
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
