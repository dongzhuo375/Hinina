// HOJ API 请求/响应类型定义。
//
// 基于 doc/HOJ/HOJ-API-Documentation.md 提取的关键 DTO。
// 统一响应格式: { "status": 200, "msg": "success", "data": {} }

use serde::{Deserialize, Serialize};

/// HOJ 统一响应包装。
/// `data` 在空响应时为 `null`，在错误时为 `Option::None`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub status: i32,
    pub msg: Option<String>,
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    /// 检查响应是否成功。
    pub fn is_success(&self) -> bool {
        self.status == 200
    }

    /// 提取 data 字段，若 status 非 200 返回错误消息。
    pub fn into_data(self) -> Result<T, String> {
        if self.is_success() {
            self.data.ok_or_else(|| "响应数据为空".to_string())
        } else {
            Err(self.msg.unwrap_or_else(|| format!("HOJ 错误 status={}", self.status)))
        }
    }
}

/// 分页响应（用于比赛列表等）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageResult<T> {
    pub records: Vec<T>,
    pub total: i64,
    #[serde(default)]
    pub size: i64,
    #[serde(default)]
    pub current: i64,
    #[serde(default)]
    pub pages: i64,
}

// ── 认证 ──

/// 登录请求（密码为明文，服务端自行 MD5 比对）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// HOJ 返回的用户信息（UserInfoVO）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfoVO {
    pub uid: String,
    pub username: String,
    /// 昵称（可为 null）
    #[serde(default)]
    pub nickname: Option<String>,
    /// 头像 URL（可为 null）
    #[serde(default)]
    pub avatar: Option<String>,
    /// 角色列表（如 ["root", "admin"]）
    #[serde(default)]
    pub role_list: Vec<String>,
}

// ── 比赛 ──

/// 比赛列表条目（ContestVO）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContestVO {
    pub id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    /// 赛制：0=ACM, 1=OI
    #[serde(default)]
    pub r#type: i32,
    /// 状态：-1=未开始, 0=进行中, 1=已结束
    #[serde(default)]
    pub status: i32,
    #[serde(default)]
    pub start_time: String,
    #[serde(default)]
    pub end_time: String,
    /// 时长（秒）
    #[serde(default)]
    pub duration: i64,
    /// 权限：0=公开, 1=私有（需密码）, 2=保护
    #[serde(default)]
    pub auth: i32,
    #[serde(default)]
    pub author: String,
}

/// 比赛题目列表条目（ContestProblemVO）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContestProblemVO {
    #[serde(default)]
    pub id: i64,
    /// 比赛中题目序号（如 "A"）
    #[serde(default)]
    pub display_id: String,
    /// 比赛ID
    #[serde(default)]
    pub cid: i64,
    /// 题目真实ID
    #[serde(default)]
    pub pid: i64,
    #[serde(default)]
    pub display_title: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub ac: i64,
    #[serde(default)]
    pub total: i64,
}

// ── 题目 ──

/// 题目详情原始字段（ProblemVO）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemVO {
    pub id: i64,
    /// 题目展示ID（如 "HOJ-1001"）
    #[serde(default)]
    pub problem_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub r#type: i32,
    /// 时间限制（ms）
    #[serde(default)]
    pub time_limit: i64,
    /// 内存限制（MB）
    #[serde(default)]
    pub memory_limit: i64,
    #[serde(default)]
    pub stack_limit: i64,
    #[serde(default)]
    pub description: Option<String>,
    /// 输入描述
    #[serde(default)]
    pub input: Option<String>,
    /// 输出描述
    #[serde(default)]
    pub output: Option<String>,
    /// 样例（HTML 格式）
    #[serde(default)]
    pub examples: Option<String>,
    #[serde(default)]
    pub hint: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub auth: i32,
    #[serde(default)]
    pub difficulty: i32,
    #[serde(default)]
    pub judge_mode: String,
}

/// get-problem-detail 和 get-contest-problem-details 的完整响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemInfoVO {
    pub problem: ProblemVO,
    #[serde(default)]
    pub tags: Vec<TagVO>,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub code_template: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagVO {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub color: String,
}

// ── 提交与评测 ──

/// 提交请求体。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitRequest {
    /// 题目展示ID
    pub pid: String,
    /// 编程语言
    pub language: String,
    /// 源代码
    pub code: String,
    /// 比赛ID（非比赛提交填 0）
    #[serde(default)]
    pub cid: i64,
    #[serde(default)]
    pub tid: Option<i64>,
    #[serde(default)]
    pub gid: Option<i64>,
    #[serde(default)]
    pub is_remote: bool,
}

/// 提交后返回的 Judge 对象。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JudgeVO {
    #[serde(default)]
    pub submit_id: i64,
    #[serde(default)]
    pub pid: i64,
    #[serde(default)]
    pub display_pid: String,
    #[serde(default)]
    pub username: String,
    /// 评测状态码（0-15，见 map_status）
    #[serde(default)]
    pub status: i32,
    #[serde(default)]
    pub submit_time: String,
    #[serde(default)]
    pub length: i64,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub cid: i64,
}

/// 提交详情响应（SubmissionInfoVO）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionInfoVO {
    pub submission: SubmissionDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmissionDetail {
    #[serde(default)]
    pub submit_id: i64,
    #[serde(default)]
    pub pid: i64,
    #[serde(default)]
    pub display_pid: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub submit_time: String,
    /// 评测状态码
    #[serde(default)]
    pub status: i32,
    /// 编译错误信息（CE 时非空）
    #[serde(default)]
    pub error_message: Option<String>,
    /// 运行时间（ms）
    #[serde(default)]
    pub time: i64,
    /// 运行内存（KB）
    #[serde(default)]
    pub memory: i64,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub length: i64,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub cid: i64,
}

// ── 状态码映射 ──
//
// HOJ status → JudgementStatus

/// 将 HOJ 评测状态码（0-15）映射为 JudgementStatus。
///
/// 非终态（0=Pending, 1=Judging）映射为 Running 供上层轮询。
/// JudgementStatus 无 PE/OLE/SE/RJE/FREQ 枚举，统一归入 WrongAnswer/Unknown。
pub fn map_status(status: i32) -> crate::core::entity::submission::JudgementStatus {
    use crate::core::entity::submission::JudgementStatus;
    match status {
        0 | 1 => JudgementStatus::Running,          // Pending / Judging
        2 => JudgementStatus::CompilationError,      // CE
        3 => JudgementStatus::WrongAnswer,           // PE（无对应枚举）
        4 => JudgementStatus::WrongAnswer,           // WA
        5 => JudgementStatus::Accepted,              // AC
        6 => JudgementStatus::TimeLimitExceeded,     // TLE
        7 => JudgementStatus::MemoryLimitExceeded,   // MLE
        8 => JudgementStatus::Unknown,               // OLE（无对应枚举）
        9 => JudgementStatus::RuntimeError,           // RE
        10 => JudgementStatus::Unknown,               // SE
        11 => JudgementStatus::Unknown,               // RJE
        12 => JudgementStatus::WrongAnswer,           // SF
        13 => JudgementStatus::Accepted,              // PA（Partial AC，保守映射为 AC）
        14 => JudgementStatus::Unknown,               // FREQ
        15 => JudgementStatus::Unknown,               // UE
        _ => JudgementStatus::Unknown,
    }
}

/// 判断是否终态（需要停止轮询）。
pub fn is_terminal_status(status: i32) -> bool {
    !matches!(status, 0 | 1)
}

#[cfg(test)]
#[path = "tests/types_tests.rs"]
mod tests;
