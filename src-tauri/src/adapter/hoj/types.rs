// HOJ API 请求/响应类型定义。
//
// 基于 doc/HOJ/HOJ-API-Documentation.md 提取的关键 DTO。
// 统一响应格式: { "status": 200, "msg": "success", "data": {} }

use serde::{Deserialize, Serialize};

/// 递归剔除 JSON 中值为 `null` 的对象成员与数组元素。
///
/// 为什么需要：HOJ 对未设置的字段返回 `null` 而不是省略（实测
/// `GET /api/get-contest-list` 的 `sealRank` / `rankShowName` / `count` / `now` /
/// `openPrint` 全为 `null`），而 serde 的 `#[serde(default)]` **只在字段缺失时生效**，
/// 遇到显式 `null` 仍会报 `invalid type: null, expected a boolean`，
/// 导致一个可选字段为 null 就让整个响应解析失败。
///
/// 剔除后 `null` 与「字段缺失」等价：非 Option 字段落到 `#[serde(default)]` 的默认值，
/// Option 字段落到 `None` —— 与 HOJ 的语义一致（null 就是「没有值」）。
/// 在解析入口统一处理，新增 DTO 字段无需逐个标注，也不会再犯同类错误。
pub fn strip_nulls(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            map.retain(|_, v| !v.is_null());
            for v in map.values_mut() {
                strip_nulls(v);
            }
        }
        serde_json::Value::Array(items) => {
            items.retain(|v| !v.is_null());
            for v in items.iter_mut() {
                strip_nulls(v);
            }
        }
        _ => {}
    }
}

/// HOJ 统一响应包装。
/// `data` 在空响应时为 `null`，在错误时为 `Option::None`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    /// 成功恒为 200（不是 0）；缺失时按失败处理，由 `into_data` 给出消息
    #[serde(default)]
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
///
/// 全部字段可缺失：HOJ 的分页对象在不同接口上返回的键并不一致
/// （`get-contest-list` 实测含 `records/total/size/current/orders/searchCount/pages`，
/// 文档只承诺 `records/total`），缺任何一个都不应让整页数据解析失败。
///
/// `bound` 显式声明是必需的：`records` 上的 `#[serde(default)]` 会让 serde 自动
/// 给 `T` 加上 `Default` 约束，而各 VO 并没有（也不该有）`Default` 实现。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound(deserialize = "T: serde::Deserialize<'de>"))]
pub struct PageResult<T> {
    #[serde(default)]
    pub records: Vec<T>,
    #[serde(default)]
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
    #[serde(default)]
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
    /// 榜单显示名规则：username / realname / nickname
    #[serde(default)]
    pub rank_show_name: Option<String>,
    /// 是否封榜
    #[serde(default)]
    pub seal_rank: bool,
    /// 封榜起始时间（ISO 字符串，可为 null）
    #[serde(default)]
    pub seal_rank_time: Option<String>,
    /// 是否允许赛后提交
    #[serde(default)]
    pub allow_end_submit: bool,
    /// OI 榜单计分规则："Recent" / "Highest"（非 OI 赛为 null）
    #[serde(default)]
    pub oi_rank_score_type: Option<String>,
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

// ── 榜单 ──

/// `POST /api/get-contest-rank` 请求体（ContestRankDTO）。
///
/// `force_refresh` 恒为 false：非比赛创建者/超管传 true 会被服务端忽略，
/// 封榜状态应由 `Contest::seal_rank` + `seal_rank_time` 自行判断。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContestRankDTO {
    pub cid: i64,
    pub current_page: i64,
    pub limit: i64,
    pub force_refresh: bool,
    pub remove_star: bool,
    /// 只匹配学校或榜单显示名
    pub keyword: Option<String>,
    pub contains_end: bool,
    /// 关注用户 uid 列表（客户端本地维护），本项目暂不使用
    pub concerned_list: Vec<String>,
    /// 联赛合并榜单用，单场比赛恒为 null
    pub external_cid_list: Option<Vec<i64>>,
}

/// 榜单记录（ACM `ACMContestRankVO` 与 OI `OIContestRankVO` 共用的宽松 DTO）。
///
/// 两种赛制的 `submissionInfo` 值类型不同（ACM 为对象、OI 为得分整数），
/// 故保留为 `serde_json::Value` 后由 `into_rank_row` 归一 —— 这样即使赛制判断失误
/// 或 HOJ 调整字段，也只是单元格降级为默认值，不会让整页解析失败。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContestRankVO {
    /// -1 表示打星队伍
    #[serde(default)]
    pub rank: i32,
    #[serde(default)]
    pub uid: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub realname: Option<String>,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub school: Option<String>,
    #[serde(default)]
    pub gender: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    /// ACM：总罚时（秒）
    #[serde(default)]
    pub total_time: Option<i64>,
    /// ACM：总提交数
    #[serde(default)]
    pub total: Option<i64>,
    /// ACM：AC 题数
    #[serde(default)]
    pub ac: Option<i64>,
    /// OI：总得分
    #[serde(default)]
    pub total_score: Option<i64>,
    /// key = displayId
    #[serde(default)]
    pub submission_info: std::collections::HashMap<String, serde_json::Value>,
    /// OI：key = displayId，value = 最优耗时（ms）
    #[serde(default)]
    pub time_info: std::collections::HashMap<String, i64>,
}

/// ACM 榜单单元格明细（`submissionInfo` 的对象形态）。
///
/// 字段名按 HOJ 原始 JSON 显式 rename（`isAC` / `isFirstAC` / `ACTime` 大小写不规则，
/// 不能依赖 `rename_all = "camelCase"`）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AcmSubmissionInfo {
    #[serde(default, rename = "errorNum")]
    pub error_num: i32,
    #[serde(default, rename = "tryNum")]
    pub try_num: Option<i32>,
    #[serde(default, rename = "isAC")]
    pub is_ac: bool,
    #[serde(default, rename = "isFirstAC")]
    pub is_first_ac: bool,
    #[serde(default, rename = "ACTime")]
    pub ac_time: Option<i64>,
    #[serde(default, rename = "isAfterContest")]
    pub is_after_contest: bool,
}

/// `POST /api/get-user-problem-status` 请求体。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProblemStatusDTO {
    pub pid_list: Vec<String>,
    pub is_contest_problem_list: bool,
    pub cid: i64,
    pub gid: Option<i64>,
    pub contains_end: bool,
}

impl ContestRankVO {
    /// 归一为 OJ 无关的榜单行（ACM 与 OI 共用同一入口）。
    pub fn into_rank_row(self) -> crate::core::entity::rank::ContestRankRow {
        use crate::core::entity::rank::ContestRankRow;
        ContestRankRow {
            rank: self.rank,
            uid: self.uid,
            username: self.username,
            realname: self.realname.unwrap_or_default(),
            nickname: self.nickname.unwrap_or_default(),
            school: self.school.unwrap_or_default(),
            gender: self.gender.unwrap_or_default(),
            avatar: self.avatar.unwrap_or_default(),
            ac: self.ac.unwrap_or(0),
            total: self.total.unwrap_or(0),
            total_time: self.total_time.unwrap_or(0),
            total_score: self.total_score,
            submission_info: self
                .submission_info
                .into_iter()
                .map(|(display_id, value)| (display_id, cell_from_value(&value)))
                .collect(),
            time_info: self.time_info,
        }
    }
}

/// 把 `submissionInfo` 的值归一为 `RankCell`。
///
/// OI 赛制的值是整数（该题得分），ACM 赛制的值是明细对象。
/// 对象解析失败时降级为默认单元格，而不是让整行/整页解析失败 ——
/// 榜单在赛场上是高频只读数据，局部字段异常不应导致整页不可用。
fn cell_from_value(value: &serde_json::Value) -> crate::core::entity::rank::RankCell {
    use crate::core::entity::rank::RankCell;

    if let Some(score) = value.as_i64() {
        return RankCell {
            score: Some(score as i32),
            ..Default::default()
        };
    }

    match serde_json::from_value::<AcmSubmissionInfo>(value.clone()) {
        Ok(info) => RankCell {
            error_num: info.error_num,
            try_num: info.try_num,
            is_ac: info.is_ac,
            is_first_ac: info.is_first_ac,
            ac_time: info.ac_time,
            is_after_contest: info.is_after_contest,
            score: None,
        },
        Err(_) => RankCell::default(),
    }
}

/// 从 `get-user-problem-status` 的响应条目里取出 **HOJ 原始评测状态码**。
///
/// 服务端返回的每个条目是 `{"status": <Constants.Judge 码>, "score": …}`；
/// 文档标注值类型为 `Object`，故对数字/布尔/对象三种形态都做容错。
/// 无法识别时返回 `None`，由调用方按「未提交」处理（保守：不会把未做的题标成已通过）。
///
/// 取到的是**原始码**（可能是 -10 / -1 / 0 …），不是前端契约值 ——
/// 必须再经 `normalize_problem_status` 归一。
pub fn extract_problem_status_code(value: &serde_json::Value) -> Option<i32> {
    if let Some(n) = value.as_i64() {
        return Some(n as i32);
    }
    if let Some(b) = value.as_bool() {
        // 历史形态：布尔直出（true=已 AC）
        return Some(if b { 0 } else { -10 });
    }
    value.get("status").and_then(|v| v.as_i64()).map(|n| n as i32)
}

/// HOJ 原始状态码 → 前端「我的题目状态」契约 `0=未提交 / 1=已AC / 2=尝试过`。
///
/// HOJ 这里返回的是**评测状态码**（与提交状态同一张 `Constants.Judge` 码表），
/// 而不是「0/1/2」三态。实测（比赛 1012，本人已 AC 该题）：
/// - `isContestProblemList=false` → `{"1000":{"status":-10}}`（-10 = Not Submitted，**错**）
/// - `isContestProblemList=true`  → `{"1000":{"status":0}}`（0 = Accepted，**对**）
///
/// 故调用方必须同时满足两点：请求体 `is_contest_problem_list = true`，
/// 且把原始码经本函数归一。
///
/// 归一规则：
/// - `0`（Accepted）→ `1` 已通过
/// - `-10`（Not Submitted）/ 无法识别 → `0` 未提交
/// - 其余（-4/-3/-2/-1/1..15）→ `2` 尝试过（未通过）
pub fn normalize_problem_status(code: i32) -> i32 {
    match code {
        0 => 1,    // AC
        -10 => 0,  // Not Submitted
        _ => 2,    // 已尝试未通过
    }
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
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub color: String,
}

// ── 提交与评测 ──

/// 提交请求体。`pid` 的取值必须经 [`submit_pid`] 计算，不要直接塞 `problem_id`。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitRequest {
    /// 题目标识 —— **比赛提交时是比赛内展示题号（"A"）**，非比赛提交时是题目展示 ID。
    /// 详见 [`submit_pid`]。
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

/// 计算提交请求体的 `pid`。
///
/// **HOJ 的 `pid` 语义随 `cid` 变化，这是本项目踩过的坑**：
/// - `cid != 0`（比赛提交）→ 服务端 `BeforeDispatchInitManager.initContestSubmission(cid, displayId, …)`
///   拿它查 `contest_problem.display_id`，查不到会直接 `contestProblem.getId()` **NPE → HTTP 500**。
///   故比赛提交必须传**比赛内展示题号**（`"A"`），传数字 pid（`"1000"`）必 500。
/// - `cid == 0`（非比赛提交）→ 走 `initCommonSubmission`，按 `problem_id` 查，此时传题目展示 ID。
///
/// `display_id` 为空/纯空白时退回 `problem_id`：非比赛场景两者同源，
/// 且这条回退保证「没有比赛上下文」的调用不会退化成提交空 pid。
///
/// 抽成纯函数是为了让这条判据可被单测锁定 —— 它一旦写错，现场表现是**所有比赛提交都 500**，
/// 而服务端不会留下任何记录，排查成本极高。
pub fn submit_pid(problem_id: &str, display_id: &str) -> String {
    let display = display_id.trim();
    if display.is_empty() {
        problem_id.trim().to_string()
    } else {
        display.to_string()
    }
}

/// 提交后返回的 Judge 对象，同时作为提交列表（`contest-submissions`）条目的宽松 DTO。
///
/// 列表场景比提交响应多出 title / displayId / time / memory / score 等字段，
/// 全部按可缺失处理：两个接口共用一个 DTO，缺哪个都落到默认值。
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
    /// 提交者 uid（列表接口返回）
    #[serde(default)]
    pub uid: String,
    /// 题目标题（列表接口返回）
    #[serde(default)]
    pub title: String,
    /// 比赛中题目序号（如 "A"，列表接口返回，可为 null）
    #[serde(default)]
    pub display_id: String,
    /// 评测状态码（0-15，见 map_status）
    #[serde(default)]
    pub status: i32,
    #[serde(default)]
    pub submit_time: String,
    /// 运行耗时（ms，评测未完成时为 null）
    #[serde(default)]
    pub time: i64,
    /// 运行内存（KB，评测未完成时为 null）
    #[serde(default)]
    pub memory: i64,
    /// OI 题目得分（ACM 题为 null）
    #[serde(default)]
    pub score: Option<f64>,
    /// OI 榜单计入分数（可为 null）
    #[serde(default)]
    pub oi_rank_score: Option<i32>,
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
    /// 判题机标识（可为 null）
    #[serde(default)]
    pub judger: Option<String>,
    /// OI 榜单计入分数（可为 null）
    #[serde(default)]
    pub oi_rank_score: Option<i32>,
}

// ── 公告 ──

/// 比赛公告条目（AnnouncementVO）。
///
/// **时间字段是 `gmtCreate` / `gmtModified`**，不是 `createTime` / `updateTime` ——
/// 实测 `/api/get-contest-announcement` 返回的键就是 gmt* 形态；写成 createTime
/// 会让 `createdAt` 恒为 0（界面显示 1970）。字段名不改，只改 serde rename。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnouncementVO {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub title: String,
    /// 公告正文（HTML，可为 null）
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub uid: String,
    #[serde(default)]
    pub username: String,
    #[serde(default, rename = "gmtCreate")]
    pub create_time: String,
    #[serde(default, rename = "gmtModified")]
    pub update_time: String,
}

// ── 测试点结果 ──

/// `GET /api/get-all-case-result` 响应（JudgeCaseVO）。
///
/// 两个列表都保留为原始 `Value` 逐条转换：SubTask 形态在文档中不完整，
/// 单条测试点字段类型异常时只跳过该条，绝不让整个响应解析失败。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JudgeCaseVO {
    #[serde(default)]
    pub judge_case_list: Vec<serde_json::Value>,
    #[serde(default)]
    pub sub_task_judge_case_vo_list: Vec<serde_json::Value>,
    #[serde(default)]
    pub judge_case_mode: Option<String>,
}

/// 单个测试点（JudgeCaseDTO），全字段宽松。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JudgeCaseDTO {
    #[serde(default)]
    pub submit_id: Option<i64>,
    #[serde(default)]
    pub case_id: Option<i64>,
    /// 评测状态码（与提交状态同一张表）
    #[serde(default)]
    pub status: Option<i32>,
    #[serde(default)]
    pub time: Option<i64>,
    #[serde(default)]
    pub memory: Option<i64>,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub group_num: Option<i64>,
    #[serde(default)]
    pub seq: Option<i64>,
    #[serde(default)]
    pub mode: Option<String>,
}

/// 子任务分组（SubTaskDTO），文档未完整给出形态，按宽松结构解析。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubTaskDTO {
    #[serde(default)]
    pub group_num: Option<i64>,
    #[serde(default)]
    pub judge_case_list: Vec<serde_json::Value>,
}

/// 逐条宽松转换测试点列表：类型异常的条目直接跳过（warn 由调用方记录）。
pub fn lenient_case_list(values: &[serde_json::Value]) -> Vec<JudgeCaseDTO> {
    values
        .iter()
        .filter_map(|v| serde_json::from_value::<JudgeCaseDTO>(v.clone()).ok())
        .collect()
}

// ── 状态码映射 ──
//
// HOJ status → JudgementStatus

/// HOJ 服务端在「错误信息不可查看」时回填的占位文案。
///
/// 出处：HOJ `JudgeManager.getSubmissionInfo` —— 只要状态不是 CE / SE / SF，
/// 就把 `errorMessage` 覆写成这句话（即 AC/WA/TLE… 全部都会带上它）。
/// 它是「本状态没有错误信息」的标记而非错误内容，必须过滤，
/// 否则提交详情页会对每一份 AC 代码弹出一块红色的「错误信息」面板。
pub const ERROR_MESSAGE_PLACEHOLDER: &str = "The error message does not support viewing.";

/// 过滤 HOJ 的占位错误文案：占位串与空串一律视为「无错误信息」。
pub fn normalize_error_message(raw: Option<String>) -> Option<String> {
    raw.filter(|msg| !msg.trim().is_empty() && msg.trim() != ERROR_MESSAGE_PLACEHOLDER)
}

/// 将 HOJ 评测状态码映射为 JudgementStatus。
///
/// 码表出自 HOJ `Constants.Judge`（`hoj-springboot/JudgeServer/.../util/Constants.java`），
/// **注意其取值域含负数** —— 这不是「0 起顺排」的码表：
///
/// | 码 | 含义 | 变体 |
/// |----|------|------|
/// | -10 | Not Submitted | `NotSubmitted` |
/// | -4 | Cancelled | `Cancelled` |
/// | -3 | Presentation Error | `PresentationError` |
/// | -2 | Compile Error | `CompilationError` |
/// | -1 | Wrong Answer | `WrongAnswer` |
/// | 0 | Accepted | `Accepted` |
/// | 1 | Time Limit Exceeded | `TimeLimitExceeded` |
/// | 2 | Memory Limit Exceeded | `MemoryLimitExceeded` |
/// | 3 | Runtime Error | `RuntimeError` |
/// | 4 | System Error | `SystemError` |
/// | 5 | Pending | `Pending` |
/// | 6 | Compiling | `Compiling` |
/// | 7 | Judging | `Running` |
/// | 8 | Partial Accepted | `PartiallyAccepted` |
/// | 9 | Submitting | `Pending`（同属「尚未开跑」，必须是非终态） |
/// | 10 | Submitted Failed | `SubmitFailed` |
/// | 15 | No Status | `Unknown` |
///
/// 码表之外的值一律归入 `Unknown`。本表曾整体错位（把 `0` 当 Pending、`5` 当 AC），
/// 后果是**所有 AC 提交被显示为 Pending 并无限轮询** —— 修改时务必对照 HOJ 源码，
/// 不要凭「0 开头即排队中」的直觉推排。
pub fn map_status(status: i32) -> crate::core::entity::submission::JudgementStatus {
    use crate::core::entity::submission::JudgementStatus;
    match status {
        -10 => JudgementStatus::NotSubmitted,      // Not Submitted
        -4 => JudgementStatus::Cancelled,          // Cancelled
        -3 => JudgementStatus::PresentationError,  // PE
        -2 => JudgementStatus::CompilationError,   // CE
        -1 => JudgementStatus::WrongAnswer,        // WA
        0 => JudgementStatus::Accepted,            // AC
        1 => JudgementStatus::TimeLimitExceeded,   // TLE
        2 => JudgementStatus::MemoryLimitExceeded, // MLE
        3 => JudgementStatus::RuntimeError,        // RE
        4 => JudgementStatus::SystemError,         // SE
        5 => JudgementStatus::Pending,             // Pending
        6 => JudgementStatus::Compiling,           // Compiling
        7 => JudgementStatus::Running,             // Judging
        8 => JudgementStatus::PartiallyAccepted,   // PA（部分通过，独立变体，不再折算 AC）
        9 => JudgementStatus::Pending,             // Submitting（等待判题机接手）
        10 => JudgementStatus::SubmitFailed,       // SF
        15 => JudgementStatus::Unknown,            // No Status
        _ => JudgementStatus::Unknown,
    }
}

/// 判断是否终态（需要停止轮询）。
///
/// 非终态 = `{5 Pending, 6 Compiling, 7 Judging, 9 Submitting}`。
/// 与 `map_status` 的输出必须等价：`map_status(code).is_terminal() == is_terminal_status(code)`。
pub fn is_terminal_status(status: i32) -> bool {
    !matches!(status, 5 | 6 | 7 | 9)
}

#[cfg(test)]
#[path = "tests/types_tests.rs"]
mod tests;
