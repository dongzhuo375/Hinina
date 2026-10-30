//! Bootstrap 崩溃文件的端到端证明（logger 未就绪期的「闪退可追查」）。
//!
//! 与 `logger_try_init_*` / 其它 panic 测试同理独占测试二进制：panic 钩子是
//! 进程级单例。本二进制**不初始化 logger**（lib 以 `cfg(test)`=false 编译，
//! 无订阅器，`logger::ready()` 为 false），钩子应把崩溃写入
//! `std::env::temp_dir()/hinina-panic.log`。临时目录已被重定向到用例沙箱，
//! 真实 `%TEMP%/hinina-panic.log` 不会被读写或删除（见 `tests/common`）。

mod common;

use std::fs;

#[test]
fn pre_logger_panic_writes_bootstrap_file_in_sandbox() {
    let sandbox = common::SandboxTemp::new("bootstrap");
    let path = sandbox.join("hinina-panic.log");

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
}
