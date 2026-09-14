use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 榜单单元格（ACM 与 OI 归一后的 OJ 无关结构）。
///
/// ACM 赛制使用 `error_num` / `try_num` / `is_ac` / `is_first_ac` / `ac_time` / `is_after_contest`；
/// OI 赛制使用 `score`（该题得分）。Adapter 负责把两种 VO 归一到本结构，
/// 上层（Service / Command / 前端）不感知赛制差异。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RankCell {
    /// 未通过次数（含罚时）。注意：该题 AC 时，HOJ 前端的既有语义是显示 `error_num + 1`
    #[serde(default)]
    pub error_num: i32,
    /// 封榜时段内的提交次数；非封榜为 `None`（封榜期间不写入 is_ac/ac_time）
    #[serde(default)]
    pub try_num: Option<i32>,
    #[serde(default)]
    pub is_ac: bool,
    /// 是否一血（相同提交时间也算一血）
    #[serde(default)]
    pub is_first_ac: bool,
    /// AC 时的比赛进度（**秒**，相对 startTime）
    #[serde(default)]
    pub ac_time: Option<i64>,
    /// 赛后提交且 `containsEnd=true` 时为 true（展示时时间前加 `*`）
    #[serde(default)]
    pub is_after_contest: bool,
    /// OI 赛制该题得分
    #[serde(default)]
    pub score: Option<i32>,
}

/// 榜单行。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContestRankRow {
    /// 排名；**`-1` 表示打星队伍**（不参与排名）
    pub rank: i32,
    pub uid: String,
    pub username: String,
    #[serde(default)]
    pub realname: String,
    #[serde(default)]
    pub nickname: String,
    #[serde(default)]
    pub school: String,
    /// `female` 时前端给用户单元格加背景色
    #[serde(default)]
    pub gender: String,
    #[serde(default)]
    pub avatar: String,
    /// ACM：AC 题数
    #[serde(default)]
    pub ac: i64,
    /// ACM：该用户比赛内总提交数
    #[serde(default)]
    pub total: i64,
    /// ACM：总罚时（**秒**）；OI：总耗时（**毫秒**）
    #[serde(default)]
    pub total_time: i64,
    /// OI：总得分
    #[serde(default)]
    pub total_score: Option<i64>,
    /// key = 题目 `displayId`（如 "A"）；未出现的题目表示无任何提交记录
    #[serde(default)]
    pub submission_info: HashMap<String, RankCell>,
    /// OI：key = `displayId`，value = 该题 AC 提交的最优耗时（毫秒）
    #[serde(default)]
    pub time_info: HashMap<String, i64>,
}

/// 分页榜单（对应 MyBatis-Plus `IPage`）。
///
/// 注意：HOJ 会把「当前登录用户」与「关注列表用户」的排名**复制一份插到 records 最前面**，
/// 因此 `total` 略大于真实参赛人数，且第 1 页可能出现重复行 —— 渲染前必须按 `uid` 去重，
/// 真实人数应以去重后非打星行的最大 `rank` 为准。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContestRankPage {
    pub records: Vec<ContestRankRow>,
    pub total: i64,
    pub size: i64,
    pub current: i64,
    pub pages: i64,
}

/// 榜单查询参数（对应 HOJ `ContestRankDTO` 的客户端可控部分）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RankQuery {
    /// 页码，从 1 开始
    pub current_page: i64,
    /// 每页大小；HOJ 建议 50（榜单为全量计算后分页，limit 越大单次越慢）
    pub limit: i64,
    /// 搜索关键词，服务端只匹配**学校**或**榜单显示名**
    #[serde(default)]
    pub keyword: Option<String>,
    /// 是否移除打星队伍
    #[serde(default)]
    pub remove_star: bool,
    /// 是否展示赛后提交结果（仅比赛 `allowEndSubmit=true` 时生效）
    #[serde(default)]
    pub contains_end: bool,
}

impl Default for RankQuery {
    fn default() -> Self {
        Self {
            current_page: 1,
            limit: 50,
            keyword: None,
            remove_star: false,
            contains_end: false,
        }
    }
}

/// 题目限制（时间 ms / 内存 MB）。
///
/// 只能从题目详情类接口取得（比赛题目列表接口不返回 limits），
/// 且对同一题基本不变，因此由 `ProblemService` 做内存 + 磁盘缓存。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemLimits {
    /// 比赛中题目序号（如 "A"）
    pub display_id: String,
    /// 时间限制（毫秒，C/C++ 基准；其它语言判题时 ×2）
    pub time_limit: u32,
    /// 内存限制（MB，C/C++ 基准；其它语言判题时 ×2）
    pub memory_limit: u32,
}
