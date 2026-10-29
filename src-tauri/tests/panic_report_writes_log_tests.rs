//! panic 钩子把崩溃写进日志的端到端证明（「闪退可追查」的回归闸门）。
//!
//! 与 `logger_try_init_ok/fail_tests` 同理独占测试二进制：panic 钩子与全局
//! 订阅器都是进程级单例，必须隔离验证。本二进制的 lib 以 `cfg(test)`=false
//! 编译，`Logger::init` 走生产分支装真实订阅器；`install` 后触发 panic，
//! 断言 `logs/hinina.log` 含 panic 消息、真实触发位置与真实堆栈段。

use std::fs;
use std::path::PathBuf;

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "hinina-panic-report-{tag}-{}-{}",
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

/// panic 经钩子 → tracing error! → 文件层落盘：消息、位置、堆栈三要素齐全。
#[test]
fn panic_message_lands_in_log_file_with_backtrace() {
    let dir = TempDir::new("end-to-end");
    // EnvFilter 优先读 RUST_LOG：显式固定级别，宿主环境不得干扰（评审 #11 同款）
    std::env::set_var("RUST_LOG", "info");
    let logger = hinina_lib::infra::logger::Logger::init(&dir.0);
    assert!(logger.has_log_file(), "订阅器空闲时应接管文件层");

    hinina_lib::infra::panic_report::install();
    let result = std::panic::catch_unwind(|| panic!("崩溃探针-boom-probe"));
    assert!(result.is_err(), "panic 应照常 unwind，由 catch_unwind 收口");

    let log = fs::read_to_string(dir.0.join("logs/hinina.log")).expect("日志文件应存在");
    assert!(
        log.contains("崩溃探针-boom-probe"),
        "panic 消息应落盘，实际日志：\n{log}"
    );
    // panic! 的触发位置就是本测试文件：证明 location 记录的是真实调用点
    assert!(
        log.contains("panic_report_writes_log_tests"),
        "panic 位置应落盘，实际日志：\n{log}"
    );
    // std `Backtrace` 的 Display 恒带编号帧（帧符号可随符号信息缺失而退化，
    // 但编号与缩进恒在）：证明是真捕获而非空占位
    assert!(
        log.contains("\n   0: "),
        "应含真实堆栈帧，实际日志：\n{log}"
    );
}
