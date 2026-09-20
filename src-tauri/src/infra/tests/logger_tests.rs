// infra/logger.rs 单元测试：日志文件打开 / 截断式轮转。
//
// 只测 `open_log_file` 纯文件行为，不测 `init`：全局 subscriber 每进程只能设置一次，
// 在测试运行时里 init 会与 cargo test 的输出捕获及其他测试冲突。

use super::*;

use std::io::Write;

/// 构造独立临时目录（用后清理）。
fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("hinina-test-logger-{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

#[test]
fn open_log_file_creates_dir_and_file() {
    let dir = temp_dir("create");
    let file = Logger::open_log_file(&dir).expect("应能创建并打开日志文件");
    drop(file);

    let log_path = dir.join(LOG_RELATIVE_PATH);
    assert!(log_path.exists(), "logs/hinina.log 应被自动创建");
    assert_eq!(fs::metadata(&log_path).unwrap().len(), 0);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn open_log_file_appends_without_truncating_small_file() {
    let dir = temp_dir("append");
    let log_path = dir.join(LOG_RELATIVE_PATH);
    fs::create_dir_all(log_path.parent().unwrap()).unwrap();
    fs::write(&log_path, b"old-line\n").unwrap();

    let mut file = Logger::open_log_file(&dir).expect("应能打开日志文件");
    file.write_all(b"new-line\n").unwrap();
    drop(file);

    let content = fs::read_to_string(&log_path).unwrap();
    assert_eq!(content, "old-line\nnew-line\n", "未超限的既有日志应追加而不是清空");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn open_log_file_truncates_oversized_file() {
    // rotate-by-truncate：客户端无长期日志留存需求，超过 5MB 直接清空防膨胀
    let dir = temp_dir("truncate");
    let log_path = dir.join(LOG_RELATIVE_PATH);
    fs::create_dir_all(log_path.parent().unwrap()).unwrap();
    let oversized = vec![b'x'; (MAX_LOG_FILE_BYTES + 1) as usize];
    fs::write(&log_path, &oversized).unwrap();

    let file = Logger::open_log_file(&dir).expect("应能打开日志文件");
    drop(file);

    assert_eq!(
        fs::metadata(&log_path).unwrap().len(),
        0,
        "超过上限的日志文件应在启动时被截断"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn open_log_file_keeps_file_at_exact_limit() {
    // 边界：恰好等于上限不截断（判据是 > 而不是 >=）
    let dir = temp_dir("exact-limit");
    let log_path = dir.join(LOG_RELATIVE_PATH);
    fs::create_dir_all(log_path.parent().unwrap()).unwrap();
    fs::write(&log_path, vec![b'y'; MAX_LOG_FILE_BYTES as usize]).unwrap();

    let file = Logger::open_log_file(&dir).expect("应能打开日志文件");
    drop(file);

    assert_eq!(fs::metadata(&log_path).unwrap().len(), MAX_LOG_FILE_BYTES);
    let _ = fs::remove_dir_all(&dir);
}

// ── 运行期清理（设置页「清理本地数据」） ──

#[test]
fn truncate_log_file_empties_file_while_append_handle_is_open() {
    // 真实场景：tracing 文件层持有一个**追加模式**的句柄时清理日志。
    // 这正是不能用 `set_len` 的原因（追加模式只有 FILE_APPEND_DATA，Windows 拒绝截断），
    // 故本用例必须真的先打开句柄再清理，否则测不到那条约束。
    let dir = temp_dir("clear");
    let log_path = dir.join(LOG_RELATIVE_PATH);
    fs::create_dir_all(log_path.parent().unwrap()).unwrap();
    fs::write(&log_path, b"line-1\nline-2\n").unwrap();

    let mut live = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .unwrap();
    let freed = truncate_log_file(&log_path).expect("持有追加句柄时也应能清理");

    assert_eq!(freed, 14, "应返回被释放的字节数");
    assert_eq!(fs::metadata(&log_path).unwrap().len(), 0, "文件应被清空");

    // 后续日志必须从 0 开始，而不是在旧长度处写出稀疏文件
    live.write_all(b"after\n").unwrap();
    drop(live);
    assert_eq!(fs::read_to_string(&log_path).unwrap(), "after\n");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn truncate_log_file_on_empty_file_reports_zero() {
    let dir = temp_dir("clear-empty");
    let log_path = dir.join(LOG_RELATIVE_PATH);
    fs::create_dir_all(log_path.parent().unwrap()).unwrap();
    fs::write(&log_path, b"").unwrap();

    assert_eq!(truncate_log_file(&log_path).expect("空文件清理也应成功"), 0);
    let _ = fs::remove_dir_all(&dir);
}
