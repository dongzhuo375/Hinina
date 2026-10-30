//! `RUST_LOG` 不得让 panic 记录消失（评审 P2 回归）：`RUST_LOG=off` 会过滤
//! `tracing::error!`，即使订阅器与文件层都正常也什么都不写。钩子必须无条件
//! 追加 bootstrap 文件，作为不受普通过滤配置影响的独立持久出口。
//!
//! 独占测试二进制：EnvFilter 在 init 时读取 RUST_LOG，panic 钩子与全局
//! 订阅器都是进程级单例。临时目录经 `tests/common` 隔离。

mod common;

use std::fs;

#[test]
fn rust_log_off_still_persists_panic_in_bootstrap_file() {
    let sandbox = common::SandboxTemp::new("rust-log-off");
    let base_dir = sandbox.join("data");
    fs::create_dir_all(&base_dir).unwrap();
    // "off" 过滤掉所有事件，包括 error!
    std::env::set_var("RUST_LOG", "off");

    let logger = hinina_lib::infra::logger::Logger::init(&base_dir);
    assert!(logger.has_log_file(), "文件层应正常接管");

    hinina_lib::infra::panic_report::install();
    let result = std::panic::catch_unwind(|| panic!("rust-log-off-probe"));
    assert!(result.is_err(), "panic 应照常 unwind，由 catch_unwind 收口");

    let bootstrap = sandbox.join("hinina-panic.log");
    let content = fs::read_to_string(&bootstrap).expect("RUST_LOG=off 时 bootstrap 仍应落盘");
    assert!(
        content.contains("rust-log-off-probe"),
        "消息应落盘：\n{content}"
    );
    assert!(
        content.contains("unix_epoch_secs:"),
        "应有时间戳头：\n{content}"
    );
    assert!(content.contains("堆栈:"), "应有堆栈段：\n{content}");

    // 反向证明过滤器确实拦下了 error!：日志文件存在但没有这条 panic
    let log = fs::read_to_string(base_dir.join("logs/hinina.log")).expect("日志文件应存在");
    assert!(
        !log.contains("rust-log-off-probe"),
        "RUST_LOG=off 时 error! 应被过滤（这正是需要 bootstrap 的原因）：\n{log}"
    );
}
