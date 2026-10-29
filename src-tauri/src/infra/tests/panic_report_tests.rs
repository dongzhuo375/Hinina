use super::{format_report, set_bootstrap_path_override, MAX_BOOTSTRAP_FILE_BYTES};
use std::fs;
use std::panic::Location;
use std::path::PathBuf;

/// 构造独立临时目录（用后清理）。
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hinina-test-panic-report-{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

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

/// 追加写 + unix 时间戳头：两次崩溃都落盘且能区分轮次。
#[test]
fn appends_reports_and_separates_runs() {
    let dir = temp_dir("append");
    let path = dir.join("hinina-panic.log");

    super::write_bootstrap_report(&path, "first-probe\n");
    super::write_bootstrap_report(&path, "second-probe\n");

    let content = fs::read_to_string(&path).expect("bootstrap 文件应存在");
    assert!(
        content.contains("first-probe"),
        "第一次崩溃应保留：\n{content}"
    );
    assert!(
        content.contains("second-probe"),
        "第二次崩溃应追加：\n{content}"
    );
    assert_eq!(
        content.matches("unix_epoch_secs:").count(),
        2,
        "每条报告应各带一个时间戳头：\n{content}"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// 超过上限先截断再追加（rotate-by-truncate）：反复早期崩溃不会无限膨胀。
#[test]
fn truncates_oversized_file_before_append() {
    let dir = temp_dir("truncate");
    let path = dir.join("hinina-panic.log");
    let filler = vec![b'x'; (MAX_BOOTSTRAP_FILE_BYTES + 1) as usize];
    fs::write(&path, &filler).expect("预置超限文件失败");

    super::write_bootstrap_report(&path, "after-truncate-probe\n");

    let content = fs::read_to_string(&path).expect("bootstrap 文件应存在");
    assert!(
        content.len() < filler.len(),
        "截断后应远小于超限内容：{} 字节",
        content.len()
    );
    assert!(
        content.contains("after-truncate-probe"),
        "新报告应完整：\n{content}"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// 父目录不存在等写失败一律静默忽略：崩溃路径绝不允许二次 panic。
#[test]
fn silently_ignores_unwritable_path() {
    let dir = temp_dir("unwritable");
    let missing = dir.join("no-such-parent").join("hinina-panic.log");

    super::write_bootstrap_report(&missing, "should-not-panic\n");
    assert!(!missing.exists());

    let _ = fs::remove_dir_all(&dir);
}

/// install + catch_unwind 全链路（单元构建 `logger::ready()` 恒 false，走
/// bootstrap 分支）：崩溃报告应落在注入的临时文件里，含消息、时间戳头与堆栈。
#[test]
fn pre_logger_panic_lands_in_bootstrap_file() {
    let dir = temp_dir("hook-e2e");
    let path = dir.join("hinina-panic.log");
    let _ = fs::remove_file(&path);
    set_bootstrap_path_override(path.clone());

    super::install();
    let result = std::panic::catch_unwind(|| panic!("bootstrap-panic-probe"));
    assert!(result.is_err(), "panic 应照常 unwind，由 catch_unwind 收口");

    let content = fs::read_to_string(&path).expect("bootstrap 崩溃文件应存在");
    assert!(
        content.contains("bootstrap-panic-probe"),
        "消息应落盘：\n{content}"
    );
    assert!(
        content.contains("unix_epoch_secs:"),
        "应有时间戳头：\n{content}"
    );
    assert!(content.contains("堆栈:"), "应有堆栈段：\n{content}");

    let _ = fs::remove_dir_all(&dir);
}
