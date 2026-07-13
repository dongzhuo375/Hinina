// HOJ mod.rs 单元测试：parse_time / parse_samples

use super::*;

// ── parse_time ──

#[test]
fn parse_time_iso_format() {
    // "2024-01-01T08:00:00" = 2024-01-01 08:00:00 UTC
    let ts = HOJAdapter::parse_time("2024-01-01T08:00:00");
    // (2024-1970)*365 + leap(13) + MONTH_DAYS[0] + 0 = 19710 + 13 = 19723 days
    // 19723 * 86400 + 8 * 3600 = 1704096000
    assert_eq!(ts, 1_704_096_000);
}

#[test]
fn parse_time_space_separated() {
    let ts = HOJAdapter::parse_time("2024-01-01 08:00:00");
    assert_eq!(ts, 1_704_096_000);
}

#[test]
fn parse_time_leap_year_march() {
    // 2024 is leap year, month=3 > 2 → +1 day
    // (2024-1970)*365 + 13 + 59 + 0 + 1 = 19710 + 13 + 59 + 1 = 19783 days
    // 19783 * 86400 + 12 * 3600 = 1709294400
    let ts = HOJAdapter::parse_time("2024-03-01T12:00:00");
    assert_eq!(ts, 1_709_294_400);
}

#[test]
fn parse_time_non_leap_year_march() {
    // 2023 is not leap year, month=3 no adjustment
    // (2023-1970)*365 + 13 + 59 = 19345 + 13 + 59 = 19417 days
    // 19417 * 86400 + 12 * 3600 = 1677672000
    let ts = HOJAdapter::parse_time("2023-03-01T12:00:00");
    assert_eq!(ts, 1_677_672_000);
}

#[test]
fn parse_time_epoch_relative() {
    // 2000-01-01 00:00:00
    // (2000-1970)*365 + 7 = 10950 + 7 = 10957 days
    // 10957 * 86400 = 946684800
    let ts = HOJAdapter::parse_time("2000-01-01T00:00:00");
    assert_eq!(ts, 946_684_800);
}

#[test]
fn parse_time_empty() {
    assert_eq!(HOJAdapter::parse_time(""), 0);
}

#[test]
fn parse_time_too_short() {
    assert_eq!(HOJAdapter::parse_time("2024"), 0);
}

// ── parse_samples ──

#[test]
fn parse_samples_empty() {
    let samples = HOJAdapter::parse_samples("");
    assert!(samples.is_empty());
}

#[test]
fn parse_samples_single_pre() {
    let html = "<pre>1 2</pre>";
    let samples = HOJAdapter::parse_samples(html);
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].input, "1 2");
    assert_eq!(samples[0].output, "");
}

#[test]
fn parse_samples_one_pair() {
    let html = "<pre>3 4</pre><pre>7</pre>";
    let samples = HOJAdapter::parse_samples(html);
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].input, "3 4");
    assert_eq!(samples[0].output, "7");
}

#[test]
fn parse_samples_two_pairs() {
    let html = "<pre>A</pre><pre>B</pre><pre>C</pre><pre>D</pre>";
    let samples = HOJAdapter::parse_samples(html);
    assert_eq!(samples.len(), 2);
    assert_eq!(samples[0].input, "A");
    assert_eq!(samples[0].output, "B");
    assert_eq!(samples[1].input, "C");
    assert_eq!(samples[1].output, "D");
}

#[test]
fn parse_samples_with_attributes() {
    let html = r#"<pre class="input">5</pre><pre class="output">6</pre>"#;
    let samples = HOJAdapter::parse_samples(html);
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].input, "5");
    assert_eq!(samples[0].output, "6");
}

#[test]
fn parse_samples_html_entities() {
    let html = "<pre>&lt;int&gt; &amp; &quot;str&quot;</pre><pre>&nbsp;ok</pre>";
    let samples = HOJAdapter::parse_samples(html);
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].input, r#"<int> & "str""#);
    assert_eq!(samples[0].output, " ok");
}

#[test]
fn parse_samples_br_tags() {
    let html = "<pre>line1<br>line2<br/>line3</pre><pre>out</pre>";
    let samples = HOJAdapter::parse_samples(html);
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].input, "line1\nline2\nline3");
    assert_eq!(samples[0].output, "out");
}

#[test]
fn parse_samples_trim_whitespace() {
    let html = "<pre>\n  hello  \n</pre><pre>  world  </pre>";
    let samples = HOJAdapter::parse_samples(html);
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].input, "hello");
    assert_eq!(samples[0].output, "world");
}
