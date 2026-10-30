use super::{format_report, MAX_BOOTSTRAP_FILE_BYTES};
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
    assert_eq!(
        content.matches("pid: ").count(),
        2,
        "每条报告应各带 pid（多实例归因）：\n{content}"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// 并发争用不得丢弃记录：后到的写入阻塞等锁，锁释放后照常落盘。
///
/// 回归 `try_lock` 实现：争用失败即 `return`，两个 panic 同时发生时后到那条
/// 整条消失（即使文件远未到上限）。用例刻意由测试线程持锁，使旧实现必然留下
/// 缺口、新实现必然补齐 —— 不是概率性并发用例。
#[test]
fn waits_for_the_lock_instead_of_skipping_the_record() {
    let dir = temp_dir("contended");
    let path = dir.join("hinina-panic.log");

    let held = super::BOOTSTRAP_WRITE_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let (done, received) = std::sync::mpsc::channel();
    let writer_path = path.clone();
    let writer = std::thread::spawn(move || {
        super::write_bootstrap_report(&writer_path, "contended-probe\n");
        let _ = done.send(());
    });

    // 锁仍被持有：写入必须阻塞在此，不得提前返回（旧实现会立刻发信号并跳过记录）
    assert!(
        received
            .recv_timeout(std::time::Duration::from_millis(150))
            .is_err(),
        "写入线程应在等锁，而不是跳过这条记录"
    );
    drop(held);
    received
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("锁释放后写入应完成");
    writer.join().expect("写入线程不应 panic");

    let content = fs::read_to_string(&path).expect("bootstrap 文件应存在");
    assert!(
        content.contains("contended-probe"),
        "争用不得丢弃记录：\n{content}"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// 顺序契约：`report` 必须**先**写 bootstrap、**再**输出 tracing/stderr。
///
/// 无法在单测里制造 `SharedFileWriter` 的重入阻塞（该锁属非测试构建），故按
/// 源码顺序锁定这条契约——与插件侧「比对 `api/http.rs` 请求头黑名单」同款源码
/// 契约测试。回归的是「持久出口排在可阻塞路径之后」这一写法：worse case 下
/// 钩子挂在 `error!` 里，bootstrap 永远不写，「每次 panic 都有记录」失效。
#[test]
fn report_persists_bootstrap_before_tracing_output() {
    let source = fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/infra/panic_report.rs"),
    )
    .expect("应能读取 panic_report.rs 源码");
    let start = source
        .find("fn report(info: &PanicHookInfo")
        .expect("应能找到 report");
    let end = source[start..]
        .find("\n/// 把崩溃报告追加到 bootstrap 文件")
        .map(|offset| start + offset)
        .unwrap_or(source.len());
    let body = &source[start..end];

    let persist = body
        .find("write_bootstrap_report(")
        .expect("report 应调用 write_bootstrap_report");
    let trace = body
        .find("tracing::error!")
        .expect("report 应包含 tracing 输出");
    assert!(
        persist < trace,
        "bootstrap 必须先于 tracing 输出：持久出口不能排在可能因重入而阻塞的路径之后"
    );
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

/// Unix 加固：bootstrap 路径是符号链接时拒绝写入，不改写链接目标。
/// 临时目录可被同机用户写入时，避免预置链接把崩溃报告追加进别人的文件。
#[cfg(unix)]
#[test]
fn refuses_symlink_target() {
    let dir = temp_dir("symlink");
    let victim = dir.join("victim.txt");
    fs::write(&victim, b"original-content").expect("预置目标文件失败");
    let link = dir.join("hinina-panic.log");
    std::os::unix::fs::symlink(&victim, &link).expect("创建符号链接失败");

    super::write_bootstrap_report(&link, "symlink-probe\n");

    assert_eq!(
        fs::read_to_string(&victim).expect("目标文件应可读"),
        "original-content",
        "符号链接目标不得被追加写入"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// 清理崩溃文件：返回删除前字节数；不存在时返回 0 且不报错。
/// 走 `remove_file_and_size` 而不是 `clear_bootstrap_log`：后者指向真实 `%TEMP%`，
/// 测试不得删除开发机上的真实崩溃记录。
#[test]
fn remove_file_and_size_reports_freed_bytes_and_tolerates_missing() {
    let dir = temp_dir("clear");
    let path = dir.join("hinina-panic.log");
    fs::write(&path, b"12345").expect("预置文件失败");

    assert_eq!(
        super::remove_file_and_size(&path).expect("删除应成功"),
        5,
        "应返回删除前的字节数"
    );
    assert!(!path.exists(), "文件应被删除");
    assert_eq!(
        super::remove_file_and_size(&path).expect("文件不存在不应报错"),
        0,
        "不存在时应报 0"
    );

    let _ = fs::remove_dir_all(&dir);
}
