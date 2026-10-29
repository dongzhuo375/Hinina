//! 进程级 panic 上报：panic 消息、线程、位置与完整堆栈写进现有 logger。
//!
//! GUI 子系统（release 构建，见 main.rs 的 `windows_subsystem`）下 stderr 无处
//! 可去，std 默认 panic 钩子的输出直接蒸发——竞赛客户端最怕「闪退且无迹可寻」。
//! 本模块在 `main()` 最早期安装钩子，让崩溃经 `tracing::error!` 落入
//! `logs/hinina.log`；订阅器尚未就绪时降级为写失败可忽略的 stderr 直写。
//!
//! release 构建默认不带调试符号，堆栈以帧地址为主；需要可读符号时给
//! release profile 开 `debug` 即可（本模块不负责符号化）。

use std::backtrace::Backtrace;
use std::io::Write as _;
use std::panic::PanicHookInfo;

/// 安装进程级 panic 钩子。在 `main()` 最早期调用（早于 Runtime / Builder /
/// Logger）——越早安装，启动期 panic 越有迹可循。
///
/// 与插件 worker 的 `catch_unwind`（`plugin/manager.rs`）互补：钩子先于捕获
/// 运行，崩溃先落日志再走既有恢复路径。钩子自身不做任何可能 panic 的操作
/// （payload 取值用 `payload_as_str`，stderr 写失败忽略）；若钩子内仍发生
/// panic，std 会直接 abort——这是不可恢复的最后防线，立刻崩好过无限递归。
pub fn install() {
    std::panic::set_hook(Box::new(report));
}

fn report(info: &PanicHookInfo<'_>) {
    // force_capture 无视 RUST_BACKTRACE：崩溃取证宁可全量，输出体积不敏感
    let backtrace = Backtrace::force_capture();
    let message = format_report(
        info.payload_as_str(),
        std::thread::current().name(),
        info.location(),
        &backtrace.to_string(),
    );
    // 订阅器就绪后经 error! 双路输出（stderr + logs/hinina.log）；此前 stderr
    // 直写兜底（GUI 子系统下句柄无效，写失败被忽略）。ready() 分流避免就绪后
    // stderr 双写。
    if crate::infra::logger::ready() {
        tracing::error!(target: "panic", "{message}");
    } else {
        let _ = std::io::stderr().write_all(message.as_bytes());
    }
}

/// 纯函数格式化（供单测，`PanicHookInfo` 无公开构造器）：payload 取不到
/// 字符串时如实标注，不猜内容。
fn format_report(
    payload: Option<&str>,
    thread_name: Option<&str>,
    location: Option<&std::panic::Location<'_>>,
    backtrace: &str,
) -> String {
    let payload = payload.unwrap_or("<非字符串 panic payload>");
    let thread = thread_name.unwrap_or("<unnamed>");
    let location = location
        .map(std::string::ToString::to_string)
        .unwrap_or_else(|| "<位置未知>".to_string());
    format!("进程 panic: {payload}\n线程: {thread}\n位置: {location}\n堆栈:\n{backtrace}\n")
}

#[cfg(test)]
#[path = "tests/panic_report_tests.rs"]
mod tests;
