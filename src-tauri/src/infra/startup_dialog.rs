//! 启动期致命错误的系统错误对话框（panic 钩子的 UI 出口）。
//!
//! **只在启动期弹一次**：`main()` 装好 panic 钩子后立即 [`arm`]；Tauri 发出
//! `RunEvent::Ready`（窗口创建与 setup 均已完成）时 [`mark_started`]。两者之间
//! 主线程上的 panic 视为启动失败——三处启动 `expect`（Tokio runtime / Tauri
//! Builder / AppContext）与 Tauri 自身创建窗口失败（WebView2 缺失等）都走同一条路。
//! 运行期 panic 只落日志。
//!
//! **顺序是保证的一部分**：钩子先写 `%TEMP%/hinina-panic.log`（见
//! `panic_report.rs`），再调用 [`present_startup_panic`]。对话框是模态的、
//! 可能无限阻塞；持久记录必须先落盘。
//!
//! **为什么用独立线程**：Tauri 的 `.setup()` 与窗口创建都跑在事件循环回调里，
//! 若在 panic 线程上直接开 MessageBox，它自带的模态消息泵会重入事件循环。
//! 这里把对话框丢到独立线程并 `join`，panic 线程只等待、不泵消息。
//!
//! **捕获假设**：启动路径没有吞掉 panic 的 `catch_unwind`。tao 0.35 在 Windows
//! 窗口回调处 `catch_unwind` 后会在消息循环里 `resume_unwind`（避免 panic 穿过
//! Win32 FFI），不是恢复运行；Tauri/wry 没有 `catch_unwind`。因此启动期主线程
//! panic 都按致命处理。若未来引入真正吞掉 panic 的捕获，必须在捕获点之后调用
//! [`mark_started`] 或增加抑制入口。
//!
//! **测试与 CI 不弹窗**：只有 `main()` 会 [`arm`]；测试二进制（含
//! `panic_report_*` 集成测试）从不 arm，钩子里的调用直接返回。
//!
//! 非 Windows 是 no-op：产品只发 Windows，开发/CI 的 stderr 已能看到 panic。

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::thread::ThreadId;

/// 对话框标题（Windows 窗口标题，也是本地冒烟时 `FindWindowW` 的查找依据）。
/// 只在 Windows 的 `show` 里使用；非 Windows 的 no-op 构建不需要它。
#[cfg(windows)]
const DIALOG_TITLE: &str = "Hinina 启动失败";

/// 启动装配已完成；此后 panic 只落日志，不再弹窗。
static STARTED: AtomicBool = AtomicBool::new(false);
/// 本进程是否已经弹过（并发 panic 只弹第一张）。
static PRESENTED: AtomicBool = AtomicBool::new(false);
/// `arm()` 时的线程，只有它的 panic 才算启动失败。
static MAIN_THREAD: OnceLock<ThreadId> = OnceLock::new();

/// 启用启动期对话框：`main()` 在 `panic_report::install()` 之后调用。
///
/// 记录主线程 ID：后台线程（Tauri 运行时 worker、插件任务）的 panic 可能被
/// `catch_unwind` 或任务边界收住，不该弹「启动失败」。主线程未记录即未启用——
/// 测试二进制不会调用本函数，钩子里的调用因此直接返回。
pub fn arm() {
    let _ = MAIN_THREAD.set(std::thread::current().id());
}

/// 标记启动完成：`main.rs` 的 `run` 回调收到 `RunEvent::Ready` 时调用
/// （窗口创建与 setup 均已完成），此后 panic 不再弹窗。
pub fn mark_started() {
    STARTED.store(true, Ordering::SeqCst);
}

/// panic 钩子的调用点：启动期、主线程、每进程只弹一次。
///
/// 必须在崩溃记录写完**之后**调用（见模块文档）：对话框可能阻塞到用户点击，
/// 记录不能等它。线程创建或 FFI 失败静默跳过——弹窗是尽力而为，唯一保证仍是
/// `panic_report` 的持久文件。
///
/// `crash_log` 为 `None` 表示崩溃记录**没有写入成功**：此时正文明确告知
/// 「未能写入」，不再把用户指向一个不存在的文件。
pub fn present_startup_panic(payload: &str, crash_log: Option<&Path>) {
    let on_main = MAIN_THREAD.get() == Some(&std::thread::current().id());
    if !should_present(STARTED.load(Ordering::SeqCst), on_main) {
        return;
    }
    if !claim_presentation(&PRESENTED) {
        return;
    }
    show_blocking(message(payload, crash_log));
}

/// 纯判定（供单测）：启动未完成且仍在主线程。未 `arm` 时主线程 ID 为 `None`，
/// `on_main` 必为 false，因此不需要单独的 armed 标志。
fn should_present(started: bool, on_main: bool) -> bool {
    !started && on_main
}

/// 一次性领取弹窗权（并发 panic 只有一个线程拿到）。
fn claim_presentation(presented: &AtomicBool) -> bool {
    !presented.swap(true, Ordering::SeqCst)
}

/// 对话框正文：直接展示 panic payload（三处 expect 已改成中文上下文）。
///
/// `crash_log` 只在崩溃记录真正写入成功时传入；`None` 时明确说「未能写入」，
/// 避免把用户指向一个因临时目录不可写而并不存在的文件。
fn message(payload: &str, crash_log: Option<&Path>) -> String {
    let details = match crash_log {
        Some(path) => format!(
            "崩溃详情：{}\n如需现场排查，请从终端带 --console 参数重新启动。",
            path.display()
        ),
        None => "崩溃记录未能写入（临时目录不可写）。\n\
                 如需现场排查，请从终端带 --console 参数重新启动。"
            .to_string(),
    };
    format!(
        "Hinina 启动失败，程序无法继续启动。\n\n\
         {payload}\n\n\
         关闭此对话框后程序将退出。\n\
         {details}"
    )
}

#[cfg(windows)]
fn show_blocking(text: String) {
    let spawned = std::thread::Builder::new()
        .name("startup-dialog".into())
        .spawn(move || show(&text));
    if let Ok(handle) = spawned {
        let _ = handle.join();
    }
}

/// 非 Windows 没有产品面：开发/CI 的 stderr 已经能看到 panic 报告。
#[cfg(not(windows))]
fn show_blocking(_text: String) {}

#[cfg(windows)]
fn show(text: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MB_TOPMOST,
    };
    let title = crate::infra::console::wide(DIALOG_TITLE);
    let body = crate::infra::console::wide(text);
    // SAFETY: 两个 UTF-16 缓冲都以 NUL 结尾且在本调用期间存活；hwnd 为 null
    // 表示无父窗口（启动失败时可能还没有窗口）。
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            body.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR | MB_SETFOREGROUND | MB_TOPMOST,
        );
    }
}

#[cfg(test)]
#[path = "tests/startup_dialog_tests.rs"]
mod tests;
