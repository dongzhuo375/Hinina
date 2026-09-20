// Hydro 响应 DTO 与归一化纯函数。
//
// Hydro 与传统 REST 的两处协议事实决定了本模块的形态：
//   1. **没有统一响应包络**：成功直接是 `this.response.body` 的 JSON；
//      失败是 `{"error":{"name","params","code"}}`（无 message、无 stack），
//      且「未登录」在 JSON 模式下可能是 HTTP 200 + `{"url":"/login?redirect=..."}`。
//   2. **未设置字段返回 `null`**（`judger`/`judgeAt`/`progress`/`tsdoc` …），
//      而 serde 的 `#[serde(default)]` 只在**字段缺失**时生效 —— 显式 null 会让
//      整个响应解析失败。故所有响应统一先 `strip_nulls` 再类型化解析。
//
// 因此本模块与 `adapter/hoj/types.rs` 同构：DTO + 纯函数（可单测穷尽锁定），
// 不含任何网络调用。

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;
use tracing::warn;

use crate::core::entity::rank::{ContestRankPage, ContestRankRow, RankCell};
use crate::core::entity::submission::JudgementStatus;
use crate::core::error::AppError;

use super::error::HydroError;

// ── 响应归一化 ──

/// 递归剔除 JSON 中的 `null` 成员。
///
/// Hydro 对未设置的字段返回 `null` 而非省略（实测榜单/记录/题目文档均如此），
/// 而 serde 的 `#[serde(default)]` 只管字段缺失，显式 null 会报
/// `invalid type: null, expected ...` 并让**整个响应**解析失败。
///
/// 放在解析入口统一处理而不是逐字段标注 `Option`：新增 DTO 字段无需记得处理，
/// 也不会再犯同类错误。注意 `false` / `0` / `""` **不是** null，必须保留 ——
/// 否则封榜、打星、零分语义会被抹掉（与 HOJ 侧同一约定）。
pub fn strip_nulls(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|_, v| !v.is_null());
            for (_, v) in map.iter_mut() {
                strip_nulls(v);
            }
        }
        Value::Array(items) => {
            items.retain(|v| !v.is_null());
            for item in items.iter_mut() {
                strip_nulls(item);
            }
        }
        _ => {}
    }
}

/// 截取响应体前 200 字符用于错误诊断。
///
/// 按**字符**而非字节截断：响应含中文题面/用户名，按字节切会落在 UTF-8 序列中间。
pub fn preview(body: &str) -> String {
    const MAX_CHARS: usize = 200;
    if body.chars().count() <= MAX_CHARS {
        return body.to_string();
    }
    body.chars().take(MAX_CHARS).collect::<String>() + "…"
}

// ── 错误包络 ──

/// Hydro 错误对象：`{"error":{"name":"LoginError","params":["alice"],"code":403}}`
#[derive(Debug, Clone, Deserialize)]
pub struct HydroErrorBody {
    pub name: String,
    #[serde(default)]
    pub params: Vec<Value>,
    #[serde(default)]
    pub code: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HydroErrorEnvelope {
    pub error: HydroErrorBody,
}

/// 把 Hydro 错误包络翻译成 `AppError`；不是错误包络时返回 `None`。
///
/// 变体判定全部委托给 `HydroError` 的 `From` 实现（会话失效与业务错误的边界
/// 在那里集中收敛，见 `error::is_auth_error_name`）。
pub fn parse_hydro_error(value: &Value) -> Option<AppError> {
    let envelope: HydroErrorEnvelope = serde_json::from_value(value.clone()).ok()?;
    Some(
        HydroError::ApiError {
            code: envelope.error.code,
            name: envelope.error.name,
            params: envelope.error.params,
        }
        .into(),
    )
}

/// 识别 JSON 化的「未登录重定向」：`{"url":"/login?redirect=..."}`。
///
/// Hydro 在 `onerror` 里对未登录的 `PermissionError`/`PrivilegeError` 直接
/// 重定向到 `/login`；在 `Accept: application/json` 下重定向被序列化成
/// **HTTP 200 + `{"url":"/login?..."}`**。不识别它的话，会话过期会被当成
/// 「成功但数据为空」，前端永远回不到登录页。
///
/// 判定保守：只认指向 `/login` 的 url，其它 `{"url":...}`（如登录成功后的
/// `{"url":"/"}`、题目文件下载的签名链接）一律不视为会话问题。
pub fn login_redirect_url(value: &Value) -> Option<String> {
    let url = value.get("url")?.as_str()?;
    if url.starts_with("/login") || url.contains("/login?") {
        return Some(url.to_string());
    }
    None
}

/// 识别「域相关」的 JSON 化重定向：`{"url":"/d/..."}`。
///
/// Hydro 用重定向表达三类「这次请求不该在这里处理」：
/// 1. 未登录 → `/login?redirect=...`（已由 [`login_redirect_url`] 翻成 `Auth`）；
/// 2. **域不匹配** → 按 `Host` 反查出的域与路径里指定的域不一致时，重定向到
///    「把路径域换成推断域」的地址（文档 §1.1 补充行为）；
/// 3. **未加入域** → 重定向到 `/d/<域>/domain/join?...`。
///
/// 带 `Accept: application/json` 时它们一律表现为 **HTTP 200 + `{"url":"..."}`**。
/// 不识别的话，这个对象会漏进 DTO 解析并报成「响应字段不匹配」—— 把「base_url 里的
/// 域前缀不对」这种**配置问题**伪装成 DTO 问题，排障方向完全错。
///
/// 判定同样保守：只认**相对路径**且以 `/d/` 开头者。绝对 URL（文件下载签名链接）与
/// `{"url":"/"}`（登出）都不在此列。变体取 `Unknown` 而非 `Auth`：这不是会话失效，
/// 不该触发登出（前端 `sessionGuard` 只认 `Auth`）。
pub fn domain_redirect_error(value: &Value) -> Option<AppError> {
    let url = value.get("url")?.as_str()?;
    if !url.starts_with("/d/") {
        return None;
    }
    warn!(redirect = %url, "Hydro 返回域相关重定向");
    Some(AppError::Unknown(format!(
        "Hydro 重定向到 {}：base_url 里的域前缀可能与该部署不匹配（Host 已被绑定到另一个域），\
         或当前账号需要先加入该域",
        url
    )))
}

/// 从响应头提取 `Set-Cookie: sid=<32 位随机串>` 中的会话 ID。
///
/// Hydro 的登录响应体里**没有** token（只有 `{"url":"/"}`），会话完全靠
/// `Set-Cookie` 下发，因此这是唯一的取 sid 途径。取第一段 `name=value`，
/// 忽略 `Expires`/`Path`/`SameSite` 等属性（属性段不以 `sid=` 开头）。
pub fn extract_sid(headers: &reqwest::header::HeaderMap) -> Option<String> {
    for value in headers.get_all(reqwest::header::SET_COOKIE).iter() {
        let Ok(raw) = value.to_str() else { continue };
        for part in raw.split(';') {
            let part = part.trim();
            if let Some(rest) = part.strip_prefix("sid=") {
                let sid = rest.trim();
                if !sid.is_empty() {
                    return Some(sid.to_string());
                }
            }
        }
    }
    None
}

// ── 时间 ──

/// 月份累计天数（非闰年，下标 = 月份）
static MONTH_DAYS: [i64; 13] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334, 365];

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// 统计 `[from, to)` 区间内的闰年数（`to` 不参与）。
fn leap_years_between(from: i64, to: i64) -> i64 {
    (from..to).filter(|y| is_leap_year(*y)).count() as i64
}

/// 把「日期 + 时刻」折算为 1970-01-01 起的秒数（纯 std，不引入时间库）。
fn epoch_secs(year: i64, month: i64, day: i64, hour: i64, minute: i64, second: i64) -> i64 {
    let month = month.clamp(1, 12);
    let mut days = (year - 1970) * 365 + leap_years_between(1970, year);
    // 表是「该月之前的天数」，故 1 月取下标 0
    days += MONTH_DAYS[(month - 1) as usize];
    if month > 2 && is_leap_year(year) {
        days += 1;
    }
    days += day - 1;
    days * 86_400 + hour * 3_600 + minute * 60 + second
}

/// 从时刻串里切出时区偏移（秒），返回 `(去掉偏移的串, 偏移秒数)`。
///
/// 支持 `Z` / `z`（UTC）与 `±HH:MM` / `±HHMM`。**只扫时间部分**：
/// 日期部分用 `-` 分隔，若连日期一起扫会把 `2026-01-01` 误判成负偏移。
fn split_timezone(rest: &str) -> (&str, i64) {
    let trimmed = rest.trim_end();
    if let Some(stripped) = trimmed.strip_suffix('Z').or_else(|| trimmed.strip_suffix('z')) {
        return (stripped, 0);
    }
    // 从第 1 个字符之后找符号，避免把 "12:00" 里的内容当偏移
    if let Some(idx) = trimmed[1..].find(['+', '-']).map(|i| i + 1) {
        let (time, tz) = trimmed.split_at(idx);
        let sign = if tz.starts_with('-') { -1 } else { 1 };
        let digits: String = tz.chars().filter(|c| c.is_ascii_digit()).collect();
        let (h, m) = match digits.len() {
            4 => (digits[..2].parse::<i64>().unwrap_or(0), digits[2..].parse::<i64>().unwrap_or(0)),
            _ => (0, 0),
        };
        return (time, sign * (h * 3_600 + m * 60));
    }
    (trimmed, 0)
}

/// Hydro 的 ISO 时间串 → UTC 秒级时间戳。
///
/// 形如 `2026-01-01T01:00:00.000Z`（榜单/比赛文档）或 `2026-01-01 08:00:00`
/// （无时区，按 UTC 解释）。无法解析时返回 0（调用方按「未设置」处理），
/// 绝不 panic —— 时间解析失败不该让整场比赛的数据加载失败。
pub fn parse_time(raw: &str) -> i64 {
    let s = raw.trim();
    if s.is_empty() {
        return 0;
    }
    let (date, rest) = match s.find(['T', 't', ' ']) {
        Some(idx) => (&s[..idx], &s[idx + 1..]),
        None => (s, ""),
    };
    let (time, offset) = split_timezone(rest);

    let mut date_parts = date.split('-');
    let year = date_parts
        .next()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(1970);
    let month = date_parts
        .next()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(1);
    let day = date_parts
        .next()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(1);

    let mut time_parts = time.split(':');
    let hour = time_parts
        .next()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);
    let minute = time_parts
        .next()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);
    // 秒可能带小数（`.000`），按整数部分取值
    let second = time_parts
        .next()
        .map(|v| v.split('.').next().unwrap_or("0"))
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);

    epoch_secs(year, month, day, hour, minute, second) - offset
}

/// 从 Hydro 记录的 `_id`（时间型 ObjectId）反推**提交时刻**（UTC 秒）。
///
/// 为什么需要它：Hydro 的记录投影只给 `judgeAt`（评测完成时刻），**没有提交时间**，
/// 而 `SubmissionRecord.submit_time` 必须填。ObjectId 的前 4 字节就是创建时刻的
/// 秒级时间戳（Hydro 自身也依赖该性质：跨域查询用
/// `_id: { $gt: Time.getObjectID(now - 10周) }` 做时间过滤）。
///
/// 判定严格，避免把非 ObjectId 的字符串读成天文数字时间：
/// 必须是 24 位十六进制，且折算结果落在 [2010, 2100) 之间；否则返回 `None`
/// 由调用方回退到 `judgeAt`。
pub fn objectid_seconds(id: &str) -> Option<i64> {
    let id = id.trim();
    if id.len() != 24 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let secs = i64::from_str_radix(&id[..8], 16).ok()?;
    const MIN: i64 = 1_262_304_000; // 2010-01-01
    const MAX: i64 = 4_102_444_800; // 2100-01-01
    (MIN..MAX).contains(&secs).then_some(secs)
}

/// 把 `formatSeconds` 的输出反解为秒（`"1:23:45"` / `"23:45"` / `"45"`）。
///
/// Hydro 榜单单元格只给**格式化后的文本**（`value` 为 `"2\n1:23:45"` 这类），
/// 而 `RankCell.ac_time` / `total_time` 需要秒数，故按 `:` 分段累加。
/// 非法输入返回 `None`，由调用方决定回退（通常是 0）。
pub fn parse_duration_seconds(raw: &str) -> Option<i64> {
    let text = raw.trim();
    if text.is_empty() {
        return None;
    }
    let mut total: i64 = 0;
    let mut segments = 0;
    for part in text.split(':') {
        let part = part.trim();
        if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        total = total * 60 + part.parse::<i64>().ok()?;
        segments += 1;
    }
    (segments > 0).then_some(total)
}

// ── 评测状态 ──

/// Hydro 状态码 → 领域状态变体。
///
/// 码表出自 `packages/common/status.ts`（文档「评测状态码」节）：
/// 0 WAITING / 1 ACCEPTED / 2 WA / 3 TLE / 4 MLE / 5 OLE / 6 RE / 7 CE / 8 SE /
/// 9 CANCELED / 10 ETC / 11 HACKED / 20 JUDGING / 21 COMPILING / 22 FETCHED /
/// 30 IGNORED / 31 FORMAT_ERROR / 32 HACK_SUCCESSFUL / 33 HACK_UNSUCCESSFUL。
///
/// `JudgementStatus` 的值域是 HOJ 的（entity 层不可改），Hydro 有几个状态在 HOJ 侧
/// 没有对应变体，一律按「不猜」原则折入语义最近的变体：
/// - 9 CANCELED → `Cancelled`（HOJ `-4`，语义精确对应，无文案落差）
/// - 11 HACKED / 30 IGNORED / 32 HACK_SUCCESSFUL / 33 HACK_UNSUCCESSFUL → `Unknown`
///   （HOJ 值域里确实没有对应项，前端显示 "Unknown"，与 Hydro 网页端的 "Hacked"
///   等有文案落差，见设计缺口报告 D9）
/// - 31 FORMAT_ERROR → `PresentationError`（语义最近）
/// - 22 FETCHED → `Pending`（见下）
///
/// **22 FETCHED 必须折入非终态**（`Pending`）：它是「评测机已取件、尚未开跑」，
/// 前端终态判据是「非 Pending/Compiling/Running 即终态」，若折成 `Unknown`
/// 会让轮询在评测开始前就停住，选手永远看不到结果。
pub fn map_status(status: i64) -> JudgementStatus {
    match status {
        0 => JudgementStatus::Pending,             // WAITING
        1 => JudgementStatus::Accepted,            // ACCEPTED
        2 => JudgementStatus::WrongAnswer,         // WRONG_ANSWER
        3 => JudgementStatus::TimeLimitExceeded,   // TIME_LIMIT_EXCEEDED
        4 => JudgementStatus::MemoryLimitExceeded, // MEMORY_LIMIT_EXCEEDED
        5 => JudgementStatus::OutputLimitExceeded, // OUTPUT_LIMIT_EXCEEDED
        6 => JudgementStatus::RuntimeError,        // RUNTIME_ERROR
        7 => JudgementStatus::CompilationError,    // COMPILE_ERROR
        8 => JudgementStatus::SystemError,         // SYSTEM_ERROR
        9 => JudgementStatus::Cancelled,           // CANCELED → HOJ -4 语义精确对应
        10 => JudgementStatus::UnknownError,       // ETC
        11 => JudgementStatus::Unknown,            // HACKED（HOJ 无对应变体）
        20 => JudgementStatus::Running,            // JUDGING
        21 => JudgementStatus::Compiling,          // COMPILING
        22 => JudgementStatus::Pending,            // FETCHED：非终态，见文档注释
        30 => JudgementStatus::Unknown,            // IGNORED
        31 => JudgementStatus::PresentationError,  // FORMAT_ERROR
        32 => JudgementStatus::Unknown,            // HACK_SUCCESSFUL
        33 => JudgementStatus::Unknown,            // HACK_UNSUCCESSFUL
        _ => JudgementStatus::Unknown,
    }
}

/// 是否为非终态（前端据此决定是否继续轮询评测结果）。
///
/// Hydro 的非终态集合是 `{0 WAITING, 20 JUDGING, 21 COMPILING, 22 FETCHED}`
/// （`record.ts` 判定 + 详情 WebSocket 的关闭条件）。
pub fn is_terminal_status(status: i64) -> bool {
    !matches!(status, 0 | 20 | 21 | 22)
}

/// HOJ 状态码 → Hydro 状态码（评测页「状态筛选」参数翻译）。
///
/// 背景：前端状态下拉的取值域是 **HOJ 码表**（`utils/submission.ts` 的
/// `STATUS_OPTIONS`，出自 HOJ `Constants.Judge`，含负数码），
/// `SubmissionQuery.status` 原样把 HOJ 码透传到 Adapter。
/// Hydro 的 `/record?status=` 收的是自己的码，故必须翻译。
///
/// 返回 `None` 表示 Hydro **没有**该语义的状态：HOJ 的 `-10` Not Submitted /
/// `8` PA 在 Hydro 码表中不存在。调用方据此给出明确错误
/// 而不是静默忽略筛选条件 —— 静默忽略会让选手以为「筛出来的就是全部」。
pub fn hoj_status_to_hydro(code: i32) -> Option<i64> {
    match code {
        5 => Some(0),   // Pending → WAITING
        6 => Some(21),  // Compiling → COMPILING
        7 => Some(20),  // Judging → JUDGING
        9 => Some(0),   // Submitting → WAITING（Hydro 无独立变体，同为「未开跑」）
        0 => Some(1),   // AC → ACCEPTED
        -1 => Some(2),  // WA → WRONG_ANSWER
        1 => Some(3),   // TLE → TIME_LIMIT_EXCEEDED
        2 => Some(4),   // MLE → MEMORY_LIMIT_EXCEEDED
        3 => Some(6),   // RE → RUNTIME_ERROR
        -2 => Some(7),  // CE → COMPILE_ERROR
        -3 => Some(31), // PE → FORMAT_ERROR（Hydro 语义最近者）
        4 => Some(8),   // SE → SYSTEM_ERROR
        -4 => Some(9),  // Cancelled → CANCELED
        15 => Some(10), // No Status → ETC
        _ => None,
    }
}

// ── 语言 ──

/// Hydro 语言 key → 提交/展示用语言名（**权威值仍是 HOJ 显示名**）。
///
/// 背景（架构约束「语言权威值 = HOJ 显示名」）：Hinina 全链路（工作区元数据、
/// 配置 `defaultLanguage`、提交参数、题目 `languages` 允许列表）都存显示名，
/// 而 Hydro 提交契约收的是 key（`cc.cc17`）。翻译必须落在 Adapter 层，
/// 且必须是**双射**：展示名由本表生成、提交时按同一张表反查，往返恒等。
///
/// 名字刻意选用前端 `utils/language` 能识别的前缀写法（"C++17" → cpp 高亮、
/// `main.cpp` 源文件名；"Python 3" → python；"PyPy 3" → python），
/// 使编辑器高亮、源文件名与 limits 倍率判定全部照常工作。
const LANG_TABLE: &[(&str, &str)] = &[
    ("bash", "Bash"),
    ("c", "C"),
    ("cc", "C++"),
    ("cc.cc98", "C++98"),
    ("cc.cc98o2", "C++98 (O2)"),
    ("cc.cc11", "C++11"),
    ("cc.cc11o2", "C++11 (O2)"),
    ("cc.cc14", "C++14"),
    ("cc.cc14o2", "C++14 (O2)"),
    ("cc.cc17", "C++17"),
    ("cc.cc17o2", "C++17 (O2)"),
    ("cc.cc20", "C++20"),
    ("cc.cc20o2", "C++20 (O2)"),
    ("cs", "C#"),
    ("go", "Go"),
    ("hs", "Haskell"),
    ("java", "Java"),
    ("js", "JavaScript"),
    ("kt", "Kotlin"),
    ("kt.jvm", "Kotlin (JVM)"),
    ("pas", "Pascal"),
    ("php", "PHP"),
    ("py", "Python"),
    ("py.py2", "Python 2"),
    ("py.py3", "Python 3"),
    ("py.pypy3", "PyPy 3"),
    ("r", "R"),
    ("rb", "Ruby"),
    ("rs", "Rust"),
];

/// Hydro 语言 key → 展示名。未知 key 原样返回。
///
/// 原样返回是刻意的：Hydro 允许部署自定义 `setting.langs`，未知 key 至少能
/// 在下拉框里显示出来并**原样提交回去**（往返仍恒等），比强行归到 C++ 安全
/// —— 判题端按 key 选编译器，猜错等于用错语言评测。
pub fn lang_display(key: &str) -> String {
    let key = key.trim();
    LANG_TABLE
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, name)| (*name).to_string())
        .unwrap_or_else(|| key.to_string())
}

/// 展示名 → Hydro 语言 key（提交时反查）。未命中返回 `None`。
pub fn lang_key(display: &str) -> Option<String> {
    let display = display.trim();
    LANG_TABLE
        .iter()
        .find(|(_, name)| *name == display)
        .map(|(k, _)| (*k).to_string())
}

/// 比赛题目序号：下标 → 展示字母（0→A … 25→Z）。
///
/// 依据 Hydro 自身的映射：`record_main` 用 `tdoc.pids[parseInt(pid, 36) - 10]`
/// 把单字母 pid 解析为题目，即 `'A'` = 下标 0 … `'Z'` = 下标 25。
/// 超过 26 题时退回十进制序号（Hydro 网页端同样无字母可用）。
pub fn display_letter(index: usize) -> String {
    if index < 26 {
        char::from(b'A' + index as u8).to_string()
    } else {
        (index + 1).to_string()
    }
}

// ── 值归一 ──

/// 把可能是数字或字符串的 ID 归一为字符串（Hydro 的 `_id`/`docId`/`pid` 混用两种）。
pub fn coerce_id(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

/// 把可能是数字或字符串的值归一为 i64。
pub fn coerce_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

/// 把单元格 `value` 归一为字符串（文档声明是 string，实测可能是数字）。
pub fn cell_text(cell: &CellVO) -> String {
    match cell.value.as_ref() {
        Some(Value::String(s)) => s.clone(),
        Some(other) => coerce_id(other),
        None => String::new(),
    }
}

/// 去掉 HTML 标签，保留文本（榜单单元格的 `value` 含图标 span）。
///
/// **刻意不做整体 trim**：榜单单元格的换行是**结构**而非空白 ——
/// ACM 通过格的 `value` 是 `<span class="icon icon-check"></span>\n0:12:00`，
/// 首行的图标被剥离后是空串、第二行才是 AC 用时；整体 trim 会把两行并成一行，
/// 让「失败次数」与「AC 用时」的解析全部错位。需要展示文本的调用点自行 trim。
pub fn strip_html(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_tag = false;
    for ch in raw.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// 提取封榜期间「+N 次待判提交」中的 N。
///
/// Hydro 的 ACM/OI 行在封榜时会往单元格追加
/// `<span style="color:orange">+{npending}</span>`，映射到 `RankCell.try_num`。
///
/// **必须锚定 `color:orange`**：AC 单元格的第一行本身就是 `+{失败次数}` 文本，
/// 只找 `+数字` 会把「AC 前的失败次数」误当成封榜待判次数。
pub fn pending_count(raw: &str) -> Option<i32> {
    let marker = raw.find("color:orange")?;
    let tail = &raw[marker..];
    let plus = tail.find('+')?;
    let digits: String = tail[plus + 1..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse::<i32>().ok()
}

/// Hydro 的题目状态（`psdoc` / `psdict` 的值）→ Hinina 三态
/// （`0` 未提交 / `1` 已 AC / `2` 尝试过）。
///
/// 依据 `ProblemStatusDoc{score,status,star,rid}`：`status` 是该题最优/末次提交的
/// 评测状态码，`rid` 存在即表示有提交。无提交信息一律归 0（未提交）。
pub fn problem_status_code(value: &Value) -> i32 {
    let Some(status) = value.get("status").and_then(coerce_i64) else {
        // 没有 status 字段：有 rid 说明提交过，按「尝试过」处理
        return if value.get("rid").is_some_and(|v| !v.is_null()) {
            2
        } else {
            0
        };
    };
    if status == 1 {
        1
    } else {
        2
    }
}

// ── DTO ──

/// 用户简要信息（`udict` 的值 / `UserContext`）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserBriefVO {
    #[serde(rename = "_id")]
    pub id: Option<Value>,
    #[serde(default)]
    pub uname: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
}

impl UserBriefVO {
    /// 展示名回退顺序：`displayName` → `uname`。
    pub fn display(&self) -> String {
        self.display_name
            .clone()
            .filter(|s| !s.is_empty())
            .or_else(|| self.uname.clone())
            .unwrap_or_default()
    }
}

/// 题目文档（`pdoc`）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdocVO {
    #[serde(rename = "_id")]
    pub id: Option<Value>,
    #[serde(default)]
    pub doc_id: Option<Value>,
    #[serde(default)]
    pub pid: Option<Value>,
    #[serde(default)]
    pub title: Option<String>,
    /// 原始 Markdown 题面（`html` 为 true 时是 HTML）
    #[serde(default)]
    pub content: Option<String>,
    /// 题面是否为 HTML（**Hinina 的 Problem 无此判别位**，见缺口 D5）
    #[serde(default)]
    pub html: Option<bool>,
    #[serde(default)]
    pub n_submit: Option<i64>,
    #[serde(default)]
    pub n_accept: Option<i64>,
    /// 解析后的 `config.yaml` 对象；服务端解析失败时会是**错误字符串**，故用 Value
    #[serde(default)]
    pub config: Option<Value>,
    #[serde(default)]
    pub tag: Option<Vec<String>>,
}

impl PdocVO {
    /// 题目真实 ID 字符串（优先 `pid`，回退 `docId`）。
    pub fn problem_id(&self) -> String {
        self.pid
            .as_ref()
            .map(coerce_id)
            .filter(|s| !s.is_empty())
            .or_else(|| self.doc_id.as_ref().map(coerce_id))
            .unwrap_or_default()
    }

    /// 题目数字主键（`docId`）；无法解析返回 0。
    pub fn doc_id_num(&self) -> i64 {
        self.doc_id.as_ref().and_then(coerce_i64).unwrap_or(0)
    }

    /// 解析 `config`（宽松：非对象/解析失败一律视为缺失）。
    pub fn problem_config(&self) -> Option<ProblemConfigVO> {
        let raw = self.config.clone()?;
        if !raw.is_object() {
            // 服务端把 config.yaml 的解析错误以字符串形式塞在这里
            return None;
        }
        serde_json::from_value(raw).ok()
    }
}

/// 题目配置（`pdoc.config` 的子集）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemConfigVO {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub count: Option<i64>,
    #[serde(default)]
    pub time_min: Option<f64>,
    #[serde(default)]
    pub time_max: Option<f64>,
    #[serde(default)]
    pub memory_min: Option<f64>,
    #[serde(default)]
    pub memory_max: Option<f64>,
    /// 本题允许的提交语言 key 列表
    #[serde(default)]
    pub langs: Option<Vec<String>>,
}

impl ProblemConfigVO {
    /// 时间限制（毫秒）：取 `timeMax`（多测试点不同限时的最严者），回退 `timeMin`。
    pub fn time_limit_ms(&self) -> u32 {
        self.time_max
            .or(self.time_min)
            .filter(|v| v.is_finite() && *v > 0.0)
            .map(|v| v as u32)
            .unwrap_or(0)
    }

    /// 内存限制（MB）：取 `memoryMax`，回退 `memoryMin`。
    pub fn memory_limit_mb(&self) -> u32 {
        self.memory_max
            .or(self.memory_min)
            .filter(|v| v.is_finite() && *v > 0.0)
            .map(|v| v as u32)
            .unwrap_or(0)
    }
}

/// 比赛/作业文档（`tdoc`）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TdocVO {
    #[serde(rename = "_id")]
    pub id: Option<Value>,
    #[serde(default)]
    pub doc_id: Option<Value>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    /// 赛制：`acm` / `oi` / `ioi` / `strictioi` / `ledo` / `homework`
    #[serde(default)]
    pub rule: Option<String>,
    #[serde(default)]
    pub begin_at: Option<String>,
    #[serde(default)]
    pub end_at: Option<String>,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub rated: Option<bool>,
    #[serde(default)]
    pub attend: Option<i64>,
    /// 题目 docId 列表（**展示字母的唯一权威顺序来源**）
    #[serde(default)]
    pub pids: Option<Vec<Value>>,
    /// 封榜时刻（`lockAt`）；与 `unlocked` 共同决定是否处于封榜期
    #[serde(default)]
    pub lock_at: Option<String>,
    #[serde(default)]
    pub unlocked: Option<bool>,
    #[serde(default)]
    pub allow_team: Option<bool>,
}

impl TdocVO {
    /// 比赛 ID 字符串（`_id` 是 24 位 hex ObjectId）。
    pub fn contest_id(&self) -> String {
        self.id
            .as_ref()
            .map(coerce_id)
            .filter(|s| !s.is_empty())
            .or_else(|| self.doc_id.as_ref().map(coerce_id))
            .unwrap_or_default()
    }

    /// 题目 docId 列表（字符串形态，供展示字母派生与字母→pid 解析）。
    pub fn pid_list(&self) -> Vec<String> {
        self.pids
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(coerce_id)
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// 是否处于封榜期（已设 `lockAt` 且未 `unlocked`）。
    pub fn is_locked(&self) -> bool {
        self.lock_at.is_some() && !self.unlocked.unwrap_or(false)
    }
}

/// `GET /contest` 响应
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContestListVO {
    #[serde(default)]
    pub page: Option<i64>,
    /// 总页数（`<prefix>pcount`，Hydro 的分页命名）
    #[serde(default)]
    pub tpcount: Option<i64>,
    #[serde(default)]
    pub tdocs: Option<Vec<TdocVO>>,
}

/// `GET /contest/:tid` 响应
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContestDetailVO {
    #[serde(default)]
    pub tdoc: Option<TdocVO>,
    #[serde(default)]
    pub tsdoc: Option<Value>,
}

/// `GET /contest/:tid/problems` 响应
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContestProblemListVO {
    #[serde(default)]
    pub tdoc: Option<TdocVO>,
    /// key = docId
    #[serde(default)]
    pub pdict: Option<HashMap<String, PdocVO>>,
    /// key = docId（**不是 pid**），值为 `ProblemStatusDoc`
    #[serde(default)]
    pub psdict: Option<HashMap<String, Value>>,
}

/// `GET /p/:pid` 响应
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemDetailVO {
    #[serde(default)]
    pub pdoc: Option<PdocVO>,
    /// 当前用户在该题的状态；**比赛模式下恒为 null**
    #[serde(default)]
    pub psdoc: Option<Value>,
    #[serde(default)]
    pub mode: Option<String>,
}

/// `POST /p/:pid/submit` 响应
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitVO {
    /// 评测记录 ID；比赛隐藏本人记录时服务端改返回 `tid`
    #[serde(default)]
    pub rid: Option<Value>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub tid: Option<Value>,
}

/// 单个测试点（`rdoc.testCases[]`）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestCaseVO {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub subtask_id: Option<i64>,
    #[serde(default)]
    pub status: Option<i64>,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub time: Option<i64>,
    #[serde(default)]
    pub memory: Option<i64>,
    #[serde(default)]
    pub message: Option<String>,
}

/// 评测记录（`rdoc`）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RdocVO {
    #[serde(rename = "_id")]
    pub id: Option<Value>,
    #[serde(default)]
    pub pid: Option<Value>,
    #[serde(default)]
    pub uid: Option<Value>,
    #[serde(default)]
    pub lang: Option<String>,
    /// 源代码；无权限或未开放分享时被服务端清空
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub score: Option<f64>,
    /// 运行耗时（毫秒）
    #[serde(default)]
    pub time: Option<i64>,
    /// 运行内存（KiB）
    #[serde(default)]
    pub memory: Option<i64>,
    #[serde(default)]
    pub status: Option<i64>,
    /// 编译错误信息
    #[serde(default)]
    pub compiler_texts: Option<Vec<String>>,
    #[serde(default)]
    pub test_cases: Option<Vec<TestCaseVO>>,
    #[serde(default)]
    pub subtasks: Option<Vec<Value>>,
    #[serde(default)]
    pub judger: Option<Value>,
    /// 评测完成时刻（**不是提交时刻**，见 `objectid_seconds`）
    #[serde(default)]
    pub judge_at: Option<String>,
}

impl RdocVO {
    /// 记录 ID 字符串
    pub fn record_id(&self) -> String {
        self.id.as_ref().map(coerce_id).unwrap_or_default()
    }

    /// 提交时刻（UTC 秒）：优先 ObjectId 前缀，回退 `judgeAt`。
    ///
    /// Hydro 的记录投影没有提交时间字段，`judgeAt` 只是评测完成时刻
    /// （比提交晚若干秒），故优先用 ObjectId 前缀；两者都拿不到返回 0。
    pub fn submit_time(&self) -> i64 {
        let from_id = objectid_seconds(&self.record_id());
        from_id.unwrap_or_else(|| {
            self.judge_at
                .as_deref()
                .map(parse_time)
                .unwrap_or_default()
        })
    }

    /// 源代码长度（字节）；代码被清空时为 0
    pub fn code_length(&self) -> u64 {
        self.code.as_ref().map(|c| c.len() as u64).unwrap_or(0)
    }

    /// 编译错误文本（多行拼接）
    pub fn compiler_message(&self) -> Option<String> {
        let texts = self.compiler_texts.as_ref()?;
        let joined = texts
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        (!joined.is_empty()).then_some(joined)
    }

    /// 状态码（缺失按 WAITING=0 处理）
    pub fn status_code(&self) -> i64 {
        self.status.unwrap_or(0)
    }

    /// 非终态时清零的指标（与 HOJ 侧 `into_judgement_result` 同一约定：
    /// 未评测完的 time/memory/score 没有意义，避免前端展示 0 以外的脏值）
    pub fn terminal_metrics(&self) -> (u64, u64, f64) {
        if is_terminal_status(self.status_code()) {
            (
                self.time.unwrap_or(0).max(0) as u64,
                self.memory.unwrap_or(0).max(0) as u64,
                self.score.unwrap_or(0.0),
            )
        } else {
            (0, 0, 0.0)
        }
    }
}

/// `GET /record/:rid` 响应
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordDetailVO {
    #[serde(default)]
    pub rdoc: Option<RdocVO>,
    #[serde(default)]
    pub pdoc: Option<PdocVO>,
    #[serde(default)]
    pub udoc: Option<UserBriefVO>,
    #[serde(default)]
    pub tdoc: Option<TdocVO>,
}

/// `GET /record` 响应
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordListVO {
    #[serde(default)]
    pub page: Option<i64>,
    #[serde(default)]
    pub rdocs: Option<Vec<RdocVO>>,
    /// key = pid（字符串形态的数字）
    #[serde(default)]
    pub pdict: Option<HashMap<String, PdocVO>>,
    #[serde(default)]
    pub udict: Option<HashMap<String, UserBriefVO>>,
    #[serde(default)]
    pub tdoc: Option<TdocVO>,
}

/// 榜单单元格（`ScoreboardNode`）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellVO {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(default)]
    pub raw: Option<Value>,
    #[serde(default)]
    pub score: Option<f64>,
    /// 首 A 高亮等内联样式
    #[serde(default)]
    pub style: Option<String>,
    #[serde(default)]
    pub hover: Option<String>,
}

/// `GET /contest/:tid/scoreboard` 响应
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreboardVO {
    #[serde(default)]
    pub tdoc: Option<TdocVO>,
    /// 第 0 行是表头，其余是数据行
    #[serde(default)]
    pub rows: Option<Vec<Vec<CellVO>>>,
    #[serde(default)]
    pub udict: Option<HashMap<String, UserBriefVO>>,
    #[serde(default)]
    pub pdict: Option<HashMap<String, PdocVO>>,
}

// ── 榜单归一 ──

/// 榜单表头的列位置（按 `type` 定位，**不依赖列顺序**）。
///
/// Hydro 的列顺序随赛制变化（`acm` = rank→user→solved→每题；`oi` 家族 =
/// rank→user→total_score→每题；`homework` 还多一列 time），且文档内部示例
/// 与速查表不一致，故一律以 `rows[0]` 的 `type` 为准。
#[derive(Debug, Default)]
struct ScoreboardHeader {
    rank: Option<usize>,
    user: Option<usize>,
    solved: Option<usize>,
    time: Option<usize>,
    total_score: Option<usize>,
    /// (列下标, 展示字母, 题目 pid)
    problems: Vec<(usize, String, Option<String>)>,
}

fn find_column(header: &[CellVO], kind: &str) -> Option<usize> {
    header
        .iter()
        .position(|cell| cell.kind.as_deref() == Some(kind))
}

fn parse_header(header: &[CellVO]) -> ScoreboardHeader {
    let mut parsed = ScoreboardHeader {
        rank: find_column(header, "rank"),
        user: find_column(header, "user"),
        solved: find_column(header, "solved"),
        time: find_column(header, "time"),
        total_score: find_column(header, "total_score"),
        problems: Vec::new(),
    };
    for (index, cell) in header.iter().enumerate() {
        if cell.kind.as_deref() != Some("problem") {
            continue;
        }
        let text = strip_html(&cell_text(cell)).trim().to_string();
        let letter = if text.is_empty() {
            // 表头没给字母时按题目列出现顺序派生（与 `display_letter` 同规则）
            display_letter(parsed.problems.len())
        } else {
            text
        };
        let pid = cell.raw.as_ref().map(coerce_id).filter(|s| !s.is_empty());
        parsed.problems.push((index, letter, pid));
    }
    parsed
}

/// 榜单单元格 → `RankCell`（ACM 赛制）。
///
/// 依据 Hydro `acm` 的 `scoreboardRow`（文档「比赛排行榜详解」）：
/// - 通过：`value = "{+失败次数 或 ✓}\n{AC 用时}"`、`score = 100`、首 A 时带高亮 `style`；
/// - 未通过但有提交：`value = "-{失败次数}"`；
/// - 封榜期间额外追加 `<span style="color:orange">+{待判次数}</span>`。
///
/// `is_ac` 以 `score == 100` 为准（Hydro 只在通过时写入该字段）；`error_num`
/// 取文本首行的失败次数（`+n` / `-n` / `✓`），语义与 HOJ 的 `errorNum`
/// 一致（**不含**本次 AC），前端展示时的 `+1` 是 HOJ 侧的既有约定。
fn acm_rank_cell(cell: Option<&CellVO>) -> RankCell {
    let Some(cell) = cell else {
        return RankCell::default();
    };
    let is_ac = cell.score == Some(100.0);
    let text = strip_html(&cell_text(cell));
    let mut lines = text.lines();
    let first = lines.next().unwrap_or("").trim();
    let second = lines.next().unwrap_or("").trim().to_string();

    let error_num = if let Some(rest) = first.strip_prefix(['+', '-']) {
        rest.trim().parse::<i32>().unwrap_or(0)
    } else {
        0
    };

    RankCell {
        error_num,
        try_num: pending_count(&cell_text(cell)),
        is_ac,
        // 首 A 高亮：Hydro 以 style 表达（`background-color: rgb(217, 240, 199);`）
        is_first_ac: cell.style.as_deref().is_some_and(|s| !s.trim().is_empty()),
        ac_time: if is_ac {
            parse_duration_seconds(&second)
        } else {
            None
        },
        is_after_contest: false,
        score: None,
    }
}

/// 榜单单元格 → `RankCell`（OI 家族：oi / ioi / strictioi / ledo / homework）。
///
/// 这些赛制的每题单元格只给分数（`score` = 该题原始分 0-100，
/// `value` = 按题目权重缩放后的展示分），没有尝试次数与用时，
/// 故 `error_num` / `ac_time` / `time_info` 留空（见缺口 D10）。
fn oi_rank_cell(cell: Option<&CellVO>) -> RankCell {
    let Some(cell) = cell else {
        return RankCell::default();
    };
    let score = cell.score.map(|s| s.round() as i32);
    RankCell {
        error_num: 0,
        try_num: pending_count(&cell_text(cell)),
        is_ac: score == Some(100),
        is_first_ac: cell.style.as_deref().is_some_and(|s| !s.trim().is_empty()),
        ac_time: None,
        is_after_contest: false,
        score,
    }
}

/// 解析单元格里的整数（`solved` 列首行 = 通过题数）。
fn cell_int(cell: Option<&CellVO>) -> Option<i64> {
    let cell = cell?;
    let text = strip_html(&cell_text(cell));
    let first = text.lines().next().unwrap_or("").trim();
    first.parse::<i64>().ok()
}

/// 解析单元格里的时长（秒）：优先数字型 `raw`，否则反解格式化文本。
fn cell_duration_seconds(cell: Option<&CellVO>) -> Option<i64> {
    let cell = cell?;
    if let Some(raw) = cell.raw.as_ref().and_then(coerce_i64) {
        return Some(raw);
    }
    let text = strip_html(&cell_text(cell));
    // `solved` 列是 "通过数\n总罚时"，`time` 列直接是时长
    let candidate = match text.lines().count() {
        0 | 1 => text.clone(),
        _ => text.lines().nth(1).unwrap_or("").to_string(),
    };
    parse_duration_seconds(&candidate)
}

/// Hydro 榜单单元格矩阵 → `ContestRankPage`。
///
/// **分页语义**：Hydro 榜单是「整榜算完后一次性返回」，服务端**不支持**
/// 分页/搜索/打星/赛后提交等参数（`RankQuery` 的字段除页码外全部被忽略），
/// 故本函数把整榜放进单页（`current = 1`、`pages = 1`、`total = size = 行数`）。
///
/// 打星：Hydro 用 `rank.value == "0"` 表示不计名次（`db.ranked` 对
/// `doc.unrank` 直接给 0），映射为 Hinina 的 `rank = -1`。
pub fn scoreboard_rank_page(vo: &ScoreboardVO) -> ContestRankPage {
    let rows = vo.rows.as_deref().unwrap_or_default();
    let empty = ContestRankPage {
        records: Vec::new(),
        total: 0,
        size: 0,
        current: 1,
        pages: 1,
    };
    if rows.is_empty() {
        return empty;
    }

    let header = parse_header(&rows[0]);
    let udict = vo.udict.as_ref();
    // 赛制：OI 家族的单元格只给分数，ACM 给尝试次数与用时
    let is_acm = vo
        .tdoc
        .as_ref()
        .and_then(|t| t.rule.as_deref())
        .map(|rule| rule == "acm")
        .unwrap_or(false);

    let mut records = Vec::with_capacity(rows.len().saturating_sub(1));
    for row in rows.iter().skip(1) {
        let rank_cell = header.rank.and_then(|i| row.get(i));
        // `rank.value == "0"` 是打星（不参与排名）→ -1
        let rank = cell_int(rank_cell)
            .map(|r| if r == 0 { -1 } else { r as i32 })
            .unwrap_or(0);

        let user_cell = header.user.and_then(|i| row.get(i));
        let uid = user_cell
            .and_then(|c| c.raw.as_ref())
            .map(coerce_id)
            .unwrap_or_default();
        let brief = udict.and_then(|map| map.get(&uid));
        let username = brief
            .map(UserBriefVO::display)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                user_cell
                    .map(|c| strip_html(&cell_text(c)).trim().to_string())
                    .unwrap_or_default()
            });

        let mut submission_info: HashMap<String, RankCell> = HashMap::new();
        for (index, letter, _pid) in &header.problems {
            let cell = row.get(*index);
            let mapped = if is_acm {
                acm_rank_cell(cell)
            } else {
                oi_rank_cell(cell)
            };
            submission_info.insert(letter.clone(), mapped);
        }

        // ACM 总提交数：服务端不提供，按各题「尝试次数」求和反推
        // （AC 题 = 失败次数 + 本次通过；未通过题 = 失败次数）
        let total = if is_acm {
            submission_info
                .values()
                .map(|cell| cell.error_num as i64 + i64::from(cell.is_ac))
                .sum()
        } else {
            0
        };
        let ac = cell_int(header.solved.and_then(|i| row.get(i)))
            .unwrap_or_else(|| submission_info.values().filter(|c| c.is_ac).count() as i64);

        // 单位契约（entity 注释）：ACM 的 `total_time` 是秒、OI 是毫秒。
        // Hydro 的时长一律是秒（`time(jdoc) = floor((rid - beginAt) / 1000)`），
        // 故非 ACM 赛制换算为毫秒以满足上层契约。
        let time_seconds = cell_duration_seconds(
            header
                .time
                .or(header.solved)
                .and_then(|i| row.get(i)),
        )
        .unwrap_or(0);
        let total_time = if is_acm {
            time_seconds
        } else {
            time_seconds.saturating_mul(1000)
        };

        records.push(ContestRankRow {
            rank,
            uid: uid.clone(),
            username,
            realname: brief
                .and_then(|b| b.display_name.clone())
                .unwrap_or_default(),
            nickname: String::new(),
            school: String::new(),
            // Hydro 榜单投影没有性别字段 → 前端「女生队」高亮不可用（缺口 D10）
            gender: String::new(),
            avatar: brief
                .and_then(|b| b.avatar_url.clone().or_else(|| b.avatar.clone()))
                .unwrap_or_default(),
            ac,
            total,
            total_time,
            total_score: cell_int(header.total_score.and_then(|i| row.get(i))),
            submission_info,
            // Hydro 每题单元格不含用时 → OI 的「最优耗时」不可得（缺口 D10）
            time_info: HashMap::new(),
        });
    }

    let count = records.len() as i64;
    ContestRankPage {
        records,
        total: count,
        size: count,
        current: 1,
        pages: 1,
    }
}

/// `GET /login` / `POST /logout` 的响应（只有重定向目标）
#[derive(Debug, Clone, Deserialize)]
pub struct LoginVO {
    #[serde(default)]
    pub url: Option<String>,
}

#[cfg(test)]
#[path = "tests/types_tests.rs"]
mod tests;
