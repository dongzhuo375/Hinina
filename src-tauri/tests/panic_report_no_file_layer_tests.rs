//! 无文件层时的 panic 持久落点（评审缺口回归）：日志文件打不开 → 订阅器
//! 只有 stderr → GUI 构建下 panic 无落点。钩子必须把报告写进 bootstrap 文件。
//!
//! 独占测试二进制：panic 钩子与全局订阅器都是进程级单例。本二进制让
//! `open_log_file` 失败（在 base_dir 下放一个**同名文件** `logs`，目录创建
//! 必然失败），断言订阅器已装但文件层未接管，且 panic 报告落在沙箱内的
//! `hinina-panic.log`。真实 `%TEMP%` 由 `tests/common` 隔离，不会被触碰。

mod common;

use std::fs;

#[test]
fn panic_without_log_file_lands_in_bootstrap_file() {
    let sandbox = common::SandboxTemp::new("no-file-layer");
    let base_dir = sandbox.join("data");
    fs::create_dir_all(&base_dir).unwrap();
    // 用同名文件占住 `logs` 路径：open_log_file 的 create_dir_all 必然失败
    fs::write(base_dir.join("logs"), b"not a directory").unwrap();
    std::env::set_var("RUST_LOG", "info");

    let logger = hinina_lib::infra::logger::Logger::init(&base_dir);
    assert!(
        !logger.has_log_file(),
        "logs/ 建不出来时文件层必须如实为不可用"
    );
    assert!(
        hinina_lib::infra::logger::ready(),
        "stderr-only 订阅器应已安装"
    );

    hinina_lib::infra::panic_report::install();
    let result = std::panic::catch_unwind(|| panic!("no-file-layer-probe"));
    assert!(result.is_err(), "panic 应照常 unwind，由 catch_unwind 收口");

    let bootstrap = sandbox.join("hinina-panic.log");
    let content = fs::read_to_string(&bootstrap).expect("无文件层时 bootstrap 崩溃文件应存在");
    assert!(
        content.contains("no-file-layer-probe"),
        "消息应落盘：\n{content}"
    );
    assert!(
        content.contains("unix_epoch_secs:"),
        "应有时间戳头：\n{content}"
    );
    assert!(content.contains("堆栈:"), "应有堆栈段：\n{content}");
}
