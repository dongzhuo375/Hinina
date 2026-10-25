//! `Logger::init` 在**全局订阅器已被占用**时的行为（评审 #4 的直接回归测试）。
//!
//! 这是集成测试而非单元测试：`cfg(test)` 下 lib 不安装订阅器（测试进程隔离
//! 约定，见 `infra/logger.rs`），`try_init` 的失败分支在单元测试中不可达；
//! 集成测试二进制链接 lib 时 `cfg(test)` 为 false，`init` 走**生产分支**，
//! 从而可以真实地制造「订阅器已被占用 → try_init 失败」并锁定其后果。
//!
//! 本文件必须与 `logger_try_init_ok_tests.rs` 保持分离：全局订阅器是进程级
//! 单例，两个分支的测试放同一个二进制会互相污染（先跑者决定成败）。

use std::fs;
use std::path::PathBuf;

/// 每用例独立的临时目录（集成测试二进制不共享 lib 的 test_support）。
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "hinina-logger-fail-{tag}-{}-{}",
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

/// 先占用进程级全局订阅器，再调 `Logger::init`：try_init 必然失败。
#[test]
fn init_with_global_subscriber_taken_reports_no_log_file() {
    // 占位：进程级订阅器只能设置一次，此后的 try_init 一律失败
    tracing_subscriber::fmt().try_init().ok();

    let dir = TempDir::new("taken");
    let logger = hinina_lib::infra::logger::Logger::init(&dir.0);

    assert!(
        !logger.has_log_file(),
        "订阅器安装失败 = 本次没有任何输出目的地，has_log_file 必须为 false"
    );
}
