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
    /// 榜单显示名规则：`username` / `realname` / `nickname`（为空时前端回退 username）
    #[serde(default)]
    pub rank_show_name: String,
    /// 是否封榜（封榜期间榜单只显示尝试次数，不显示通过状态）
    #[serde(default)]
    pub seal_rank: bool,
    /// 封榜起始时间（UTC 秒级时间戳）；未封榜或未设置时为 `None`
    #[serde(default)]
    pub seal_rank_time: Option<i64>,
    /// 是否允许赛后提交（决定榜单查询的 `containsEnd` 是否真正生效）
    #[serde(default)]
    pub allow_end_submit: bool,
    /// OI 榜单计分规则："Recent"（最近一次）/ "Highest"（最高分）；非 OI 赛或未设置时为 `None`
    #[serde(default)]
    pub oi_rank_score_type: Option<String>,
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
    /// 气球颜色（如 "#FF0000"）；驱动题目卡片字母徽章与榜单列头配色，可能为空
    #[serde(default)]
    pub color: String,
}

/// 配置比赛加载结果：比赛详情 + 题目列表。
///
/// 作为 `load_configured_contest` Command 的返回值，
/// 序列化为 `{ "contest": ..., "problems": [...] }`（对象而非元组数组），
/// 供前端 `loadConfiguredContest` 直接解构使用。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContestBundle {
    pub contest: Contest,
    pub problems: Vec<ContestProblem>,
}
