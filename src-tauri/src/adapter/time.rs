// OJ 响应中的时间串解析（各 Adapter 共用）。
//
// **为什么单独成模块**：HOJ 与 Hydro 都要把服务端时间串折算成秒级 UTC 时间戳，
// 而两者的格式差异（毫秒、`Z`、`±HHMM` / `±HH:MM` 偏移、空格分隔、非零填充）
// 属于同族问题。此前两个 Adapter 各写一份：Hydro 那份按字段切分，能处理偏移与
// 毫秒；HOJ 那份按**固定 19 字符取位**，恰好把 `.000+0000` 截断忽略 —— 在本机
// UTC 部署下「碰巧正确」，一旦目标部署时区非 UTC 就会整体偏移 8 小时且**静默
// 无告警**（P64，已修复归档）。两份实现并存也让「修好一个」不等于「修好
// 全部」，故合并到此处。

use tracing::warn;

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
    if let Some(stripped) = trimmed
        .strip_suffix('Z')
        .or_else(|| trimmed.strip_suffix('z'))
    {
        return (stripped, 0);
    }
    // 从第 1 个字符之后找符号，避免把 "12:00" 里的内容当偏移。
    // 用 `get(1..)` 而非 `[1..]`：只有日期的串（如 `2026-01-01`）传进来的
    // `rest` 是空串，直接切片会 panic —— 原 Hydro 实现有这个未覆盖的边界。
    if let Some(idx) = trimmed
        .get(1..)
        .and_then(|tail| tail.find(['+', '-']))
        .map(|i| i + 1)
    {
        let (time, tz) = trimmed.split_at(idx);
        let sign = if tz.starts_with('-') { -1 } else { 1 };
        let digits: String = tz.chars().filter(|c| c.is_ascii_digit()).collect();
        let (h, m) = match digits.len() {
            4 => (
                digits[..2].parse::<i64>().unwrap_or(0),
                digits[2..].parse::<i64>().unwrap_or(0),
            ),
            _ => (0, 0),
        };
        return (time, sign * (h * 3_600 + m * 60));
    }
    (trimmed, 0)
}

fn parse_num(raw: &str) -> Option<i64> {
    raw.trim().parse::<i64>().ok()
}

/// ISO 8601 时间串 → 秒级 UTC 时间戳。
///
/// 支持（各 OJ 实测与文档出现的全部形态）：
/// - `2026-01-01T01:00:00.000Z`（Hydro 榜单/比赛文档）
/// - `2026-09-21T16:00:00.000+0000`（HOJ 实测：**带毫秒与偏移后缀**）
/// - `2026-01-01T09:00:00+08:00` / `+0800`（自建部署常见）
/// - `2024-01-01 08:00:00`（空格分隔、无时区 → 按 UTC 解释）
/// - `2024-1-1 8:00:00`（**非零填充**：按字段切分而非固定字节取位）
/// - `2026-01-01`（只有日期，时刻补 0）
///
/// 日期部分要求「年-月-日」齐全：`2024` 这类残缺值按无法解析处理（见下）。
///
/// **无法解析时返回 0 并 `warn!`，绝不 panic** —— 时间解析失败不该让整场比赛的
/// 数据加载失败，0 由调用方按「未设置」处理。但**不静默**：畸形输入是服务端格式
/// 变化的第一现场信号，必须留日志（此前 HOJ 那份静默归零，只能靠现场倒计时不对
/// 才发现）。
///
/// 空串/纯空白视为「未设置」，返回 0 且**不告警**（`Option` 字段的常见取值）。
///
/// 带**非零**时区偏移时额外 `warn!` 一次：本项目已知 HOJ 自建部署可能把本地时间
/// 序列化进来，这是部署时区配置异常的第一现场信号。
pub fn parse_time(raw: &str) -> i64 {
    let s = raw.trim();
    if s.is_empty() {
        return 0;
    }
    match try_parse(s) {
        Some((secs, offset)) => {
            if offset != 0 {
                warn!(
                    raw = %s,
                    offset_secs = offset,
                    "时间串带非零时区偏移（服务端可能按本地时间序列化），已换算为 UTC"
                );
            }
            secs
        }
        None => {
            warn!(raw = %s, "时间串无法解析，按「未设置」(0) 处理");
            0
        }
    }
}

/// 解析成功返回 `(UTC 秒, 原始时区偏移秒)`；失败返回 `None`。
fn try_parse(s: &str) -> Option<(i64, i64)> {
    let (date, rest) = match s.find(['T', 't', ' ']) {
        Some(idx) => (&s[..idx], &s[idx + 1..]),
        None => (s, ""),
    };
    let (time, offset) = split_timezone(rest);

    let mut date_parts = date.split('-');
    let year = parse_num(date_parts.next()?)?;
    // 要求「年-月-日」齐全：时间戳字段出现 `2024` 这种残缺值几乎必然是坏数据，
    // 静默折算成 2024-01-01 会把问题藏起来（返回 0 + warn 更好排障）
    let month = parse_num(date_parts.next()?)?;
    let day = parse_num(date_parts.next()?)?;

    let (hour, minute, second) = if time.trim().is_empty() {
        (0, 0, 0)
    } else {
        let mut time_parts = time.split(':');
        let hour = parse_num(time_parts.next()?)?;
        let minute = match time_parts.next() {
            Some(v) => parse_num(v)?,
            None => 0,
        };
        // 秒可能带小数（`.000`），取整数部分
        let second = match time_parts.next() {
            Some(v) => parse_num(v.split('.').next()?)?,
            None => 0,
        };
        (hour, minute, second)
    };

    Some((epoch_secs(year, month, day, hour, minute, second) - offset, offset))
}

#[cfg(test)]
#[path = "tests/time_tests.rs"]
mod tests;
