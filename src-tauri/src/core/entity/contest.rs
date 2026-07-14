use serde::{Deserialize, Serialize};

/// 比赛信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contest {
    pub id: String,
    pub title: String,
    /// 比赛开始时间（UTC 秒级时间戳），各 Adapter 负责统一转换
    pub start_time: i64,
    /// 比赛结束时间（UTC 秒级时间戳），各 Adapter 负责统一转换
    pub end_time: i64,
    /// 比赛描述（HTML/Markdown）
    #[serde(default)]
    pub description: String,
    /// 赛制：0=ACM，1=OI
    #[serde(default)]
    pub contest_type: i32,
    /// 比赛状态：-1=未开始，0=进行中，1=已结束
    #[serde(default)]
    pub status: i32,
    /// 权限：0=公开，1=私有（需密码），2=保护
    #[serde(default)]
    pub auth: i32,
    /// 题目标题列表（按 displayId 顺序排列）
    pub problems: Vec<String>,
}

/// 比赛题目摘要（问题列表用）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContestProblem {
    /// 内部 ID
    pub id: i64,
    /// 比赛中展示 ID（如 "A", "B", "C"）
    pub display_id: String,
    /// 比赛 ID
    pub cid: i64,
    /// 题目真实 ID（HOJ 的 pid）
    pub problem_id: String,
    /// 比赛中显示标题
    #[serde(default)]
    pub display_title: String,
    /// AC 数
    #[serde(default)]
    pub ac: i64,
    /// 总提交数
    #[serde(default)]
    pub total: i64,
}
