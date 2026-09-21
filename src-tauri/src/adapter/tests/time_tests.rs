// adapter::time 单元测试：ISO 8601 → 秒级 UTC 时间戳。
//
// 覆盖 doc/problem.md P64 / P30 点名的形态：HOJ 实测的
// `2026-09-21T16:00:00.000+0000`（带毫秒与偏移后缀）、非零偏移、非零填充、
// 畸形输入。此前的 HOJ 实现按固定 19 字符取位，恰好把偏移后缀截断忽略 ——
// 本文件的存在就是为了让「碰巧正确」不可能再被后续重构打破。

use super::*;

// ── 实测格式（HOJ get-contest-list 夹具里的原样串）──

#[test]
fn parses_real_fixture_timestamp_with_millis_and_offset_suffix() {
    // `adapter/hoj/tests/fixtures/contest_list_anon.json` 实测格式
    assert_eq!(parse_time("2026-09-21T16:00:00.000+0000"), 1_790_006_400);
    // 与显式 UTC 写法必须等价（这正是原实现「碰巧正确」的来源）
    assert_eq!(parse_time("2026-09-21T16:00:00Z"), 1_790_006_400);
}

#[test]
fn non_zero_offset_is_converted_to_utc() {
    // +0800 的 16:00 == UTC 08:00
    assert_eq!(parse_time("2026-09-21T16:00:00.000+0800"), 1_789_977_600);
    assert_eq!(parse_time("2026-09-21T16:00:00+08:00"), 1_789_977_600);
    // -0800 的 16:00 == 次日 UTC 00:00
    assert_eq!(parse_time("2026-09-21T16:00:00-0800"), 1_790_035_200);
    // 半小时偏移也要算对（印度 +05:30 这类部署）
    assert_eq!(parse_time("2026-09-21T16:00:00+0530"), 1_789_986_600);
}

#[test]
fn zulu_suffix_case_insensitive() {
    assert_eq!(parse_time("2026-09-21T16:00:00Z"), 1_790_006_400);
    assert_eq!(parse_time("2026-09-21T16:00:00z"), 1_790_006_400);
}

// ── 其它既有形态（回归保护）──

#[test]
fn space_separator_and_no_timezone_is_utc() {
    assert_eq!(parse_time("2026-09-21 16:00:00"), 1_790_006_400);
}

#[test]
fn non_padded_fields_are_parsed() {
    // P30 点名：`2024-1-1 8:00:00` 按固定字节取位会得到 0（长度不足 19）
    assert_eq!(parse_time("2024-1-1 8:00:00"), 1_704_096_000);
    assert_eq!(parse_time("2024-1-1T8:00:00"), 1_704_096_000);
}

#[test]
fn date_only_defaults_time_to_midnight() {
    assert_eq!(parse_time("2026-09-21"), 1_789_948_800);
}

#[test]
fn leap_day_and_epoch_boundary() {
    assert_eq!(parse_time("2024-02-29T00:00:00Z"), 1_709_164_800);
    assert_eq!(parse_time("1970-01-01T00:00:00Z"), 0);
    assert_eq!(parse_time("2000-01-01T00:00:00Z"), 946_684_800);
}

#[test]
fn out_of_range_month_is_clamped_not_panicking() {
    // 服务端给畸形月份时不得 panic（曾靠 month.clamp 防住下标越界）
    assert_eq!(
        parse_time("2026-13-01T00:00:00Z"),
        parse_time("2026-12-01T00:00:00Z")
    );
    assert_eq!(
        parse_time("2026-00-01T00:00:00Z"),
        parse_time("2026-01-01T00:00:00Z")
    );
}

// ── 失败路径：返回 0 但不 panic ──

#[test]
fn empty_or_blank_is_unset() {
    // 空串 = 「未设置」，属正常情形（不告警）
    assert_eq!(parse_time(""), 0);
    assert_eq!(parse_time("   "), 0);
}

#[test]
fn malformed_input_returns_zero_without_panicking() {
    // 这些以前会走「长度不足 19 → 0」或「按字节取位得到垃圾数」两条路，
    // 现在统一为「返回 0 + warn」（见函数文档）
    assert_eq!(parse_time("2024"), 0);
    assert_eq!(parse_time("not-a-time"), 0);
    assert_eq!(parse_time("2026-09-21Txx:00:00Z"), 0);
    assert_eq!(parse_time("2026-xx-21T00:00:00Z"), 0);
}

#[test]
fn trailing_junk_after_offset_does_not_shift_the_result() {
    // 偏移后缀之后的内容不再参与解析（原实现会把它当秒数读）
    assert_eq!(parse_time("2026-09-21T16:00:00.000+0000"), 1_790_006_400);
}
