//! Bootstrap 崩溃文件的端到端证明（logger 未就绪期的「闪退可追查」）。
//!
//! 与 `logger_try_init_*` / `panic_report_writes_log_tests` 同理独占测试
//! 二进制：panic 钩子是进程级单例。本二进制**不初始化 logger**（lib 以
//! `cfg(test)`=false 编译，无订阅器，`logger::ready()` 为 false），钩子应把
//! 崩溃写入固定落点 `%TEMP%/hinina-panic.log`。测试前清掉该文件、断言后删除，
//! 不向真实环境遗留测试内容。

use std::fs;
use std::path::PathBuf;

fn bootstrap_path() -> PathBuf {
    std::env::temp_dir().join("hinina-panic.log")
}

#[test]
fn pre_logger_panic_writes_fixed_bootstrap_file() {
    let path = bootstrap_path();
    let _ = fs::remove_file(&path);

    hinina_lib::infra::panic_report::install();
    let result = std::panic::catch_unwind(|| panic!("bootstrap-e2e-probe"));
    assert!(result.is_err(), "panic 应照常 unwind，由 catch_unwind 收口");

    let content = fs::read_to_string(&path).expect("bootstrap 崩溃文件应存在");
    assert!(
        content.contains("bootstrap-e2e-probe"),
        "消息应落盘：\n{content}"
    );
    assert!(
        content.contains("unix_epoch_secs:"),
        "应有时间戳头：\n{content}"
    );
    assert!(content.contains("堆栈:"), "应有堆栈段：\n{content}");

    let _ = fs::remove_file(&path);
}
