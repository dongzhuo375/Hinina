//! `Logger::init` 在**全局订阅器空闲**时的行为（评审 #4 的对称用例）。
//!
//! 与 `logger_try_init_fail_tests.rs` 分属不同测试二进制：全局订阅器是进程级
//! 单例，成败两分支必须隔离验证。本二进制的 lib 以 `cfg(test)`=false 编译，
//! `init` 走生产分支——try_init 应成功接管文件层。

use std::fs;
use std::path::PathBuf;

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "hinina-logger-ok-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 订阅器空闲时 init 成功接管文件层：has_log_file 为 true 且日志确实有写入。
#[test]
fn init_with_free_global_subscriber_takes_over_log_file() {
    let dir = TempDir::new("taken-by-nobody");
    // EnvFilter 优先读 RUST_LOG：宿主环境若设了 warn 及以上，info! 探针会被
    // 过滤，写入断言将在正确代码上失败——显式固定本测试所需级别（评审 #11）
    std::env::set_var("RUST_LOG", "info");
    let logger = hinina_lib::infra::logger::Logger::init(&dir.0);

    assert!(logger.has_log_file(), "订阅器空闲时 init 应成功接管文件层");

    // 真实写入验证：文件层被接管意味着事件会落盘（对称证明 FAIL 分支的
    // 「未接管 = 无写入」不是恒真断言）
    let log_file = dir.0.join("logs/hinina.log");
    assert!(log_file.exists(), "init 后日志文件应已创建");
    tracing::info!("logger integration probe");
    let size = fs::metadata(&log_file).map(|m| m.len()).unwrap_or(0);
    assert!(size > 0, "接管后日志文件应有写入（实测 {} 字节）", size);
}
