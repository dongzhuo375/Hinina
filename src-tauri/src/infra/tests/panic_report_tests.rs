use super::format_report;
use std::panic::Location;

/// 字符串 payload：消息、线程、位置、堆栈各段齐全，报告以换行结尾。
#[test]
fn formats_string_payload_with_thread_location_and_backtrace() {
    let location = Location::caller();
    let report = format_report(
        Some("boom-probe"),
        Some("main"),
        Some(location),
        "stack backtrace:\n   0: frame_zero",
    );

    assert!(report.contains("进程 panic: boom-probe"));
    assert!(report.contains("线程: main"));
    assert!(report.contains(&format!("位置: {location}")));
    assert!(report.contains("堆栈:\nstack backtrace:\n   0: frame_zero"));
    assert!(report.ends_with('\n'), "报告应以换行结尾，便于日志文件续写");
}

/// 非字符串 payload / 无线程名 / 无位置：如实标注占位，不猜内容。
#[test]
fn marks_missing_parts_instead_of_guessing() {
    let report = format_report(None, None, None, "");

    assert!(report.contains("进程 panic: <非字符串 panic payload>"));
    assert!(report.contains("线程: <unnamed>"));
    assert!(report.contains("位置: <位置未知>"));
    assert!(report.contains("堆栈:"));
}

/// install + catch_unwind 全链路不 abort（单元测试构建无全局订阅器，
/// 走 stderr 兜底分支，输出由 libtest 捕获）。
#[test]
fn installed_hook_reports_through_catch_unwind_without_aborting() {
    super::install();
    let result = std::panic::catch_unwind(|| panic!("panic-report-unit-probe"));
    assert!(result.is_err(), "panic 应照常 unwind，由 catch_unwind 收口");
}
