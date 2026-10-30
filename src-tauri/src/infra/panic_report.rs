//! 进程级 panic 上报：panic 消息、线程、位置与完整堆栈写入持久文件。
//!
//! **保证**：每次 panic 都**无条件**追加一份报告到 bootstrap 崩溃文件
//! `%TEMP%/hinina-panic.log`——独立持久出口，不经 tracing 过滤器，也不依赖
//! 日志文件是否打开。日志未被过滤器关闭时同一条报告另经 `tracing::error!`
//! 写入 `logs/hinina.log`（同时 stderr）；`RUST_LOG=off`、`panic=off` 会过滤掉
//! `error!`，`logs/` 权限不足/数据目录被占用会让文件层根本不接管——这些
//! 情况下 bootstrap 文件就是唯一痕迹。只有临时目录也不可写时崩溃才真正
//! 不可追查。
//!
//! GUI 子系统（release 构建，见 main.rs 的 `windows_subsystem`）下 stderr 无处
//! 可去，std 默认钩子的输出直接蒸发——竞赛客户端最怕「闪退且无迹可寻」，
//! 本模块保证 panic 至少落一个持久文件。插件 worker 的 `catch_unwind`
//! （`plugin/manager.rs`）只兜插件 panic，主程序崩溃由此兜底；钩子先于捕获
//! 运行，崩溃先落日志再走既有恢复路径。release 构建默认不带调试符号，堆栈
//! 以帧地址为主；需要可读符号时给 release profile 开 `debug` 即可。

use std::backtrace::Backtrace;
use std::io::Write as _;
use std::panic::PanicHookInfo;
use std::path::Path;

/// Bootstrap 崩溃文件名（固定在临时目录根部，不进任何数据目录）。
const BOOTSTRAP_FILE_NAME: &str = "hinina-panic.log";

/// Bootstrap 崩溃文件大小上限：超过即截断重写（rotate-by-truncate，与
/// logger 的启动轮转同源），防止反复早期崩溃无限膨胀。
const MAX_BOOTSTRAP_FILE_BYTES: u64 = 1024 * 1024;

/// Bootstrap 文件路径的测试注入点：单元测试构建把它指到测试临时目录，
/// 避免污染真实 `%TEMP%`（生产构建没有这个静态，钩子直接用固定路径）。
#[cfg(test)]
static BOOTSTRAP_PATH_OVERRIDE: std::sync::Mutex<Option<std::path::PathBuf>> =
    std::sync::Mutex::new(None);

#[cfg(test)]
pub(crate) fn set_bootstrap_path_override(path: std::path::PathBuf) {
    *BOOTSTRAP_PATH_OVERRIDE
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(path);
}

/// Bootstrap 崩溃文件路径：`%TEMP%/hinina-panic.log`。
///
/// 固定用临时目录而不是数据目录：钩子运行时数据目录解析尚未发生（那正是
/// 可能 panic 的环节之一），任何「先解析再写」都会把失败面重新拉进钩子。
fn bootstrap_log_path() -> std::path::PathBuf {
    #[cfg(test)]
    if let Some(path) = BOOTSTRAP_PATH_OVERRIDE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
    {
        return path;
    }
    std::env::temp_dir().join(BOOTSTRAP_FILE_NAME)
}

/// 安装进程级 panic 钩子。在 `main()` 最早期调用（早于参数解析、Runtime /
/// Builder / Logger）——越早安装，启动期 panic 越有迹可循。重复调用只是替换。
///
/// 与插件 worker 的 `catch_unwind` 互补：钩子先于捕获运行，崩溃先落日志再走
/// 既有恢复路径。钩子自身不做任何可能 panic 的操作（payload 取值用
/// `payload_as_str`，stderr 与文件写失败一律忽略）；若钩子内仍发生 panic，
/// std 会直接 abort——这是不可恢复的最后防线，立刻崩好过无限递归。
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
    if crate::infra::logger::ready() {
        // 订阅器已装：error! 输出（stderr；过滤器允许时同时落 hinina.log）。
        tracing::error!(target: "panic", "{message}");
    } else {
        // Logger 尚未就绪：stderr 兜底。钩子经 tracing 文件层写日志存在
        // SharedFileWriter 重入自死锁的理论窗口（见 logger.rs），
        // 就绪期之前根本没有文件层，无此窗口。
        let _ = std::io::stderr().write_all(message.as_bytes());
    }
    // 独立持久出口，**不经 tracing 过滤器**：RUST_LOG=off / panic=off 会过滤
    // error!（订阅器在、文件层也在，却什么都不写），日志文件打不开时也只有
    // stderr 一路——两种情况都靠这里兜底。每次 panic 都追加，不做「文件层在
    // 就跳过」的优化：是否被过滤器拦下无法在钩子里判定，宁可重复不可丢失。
    write_bootstrap_report(&bootstrap_log_path(), &message);
}

/// 把崩溃报告追加到 bootstrap 文件（**独立持久出口**）。
///
/// 每次 panic 都写，不经 tracing 过滤器——`RUST_LOG=off` 会过滤 `error!`，
/// 日志文件打不开时订阅器也只有 stderr，这两类情况下它是唯一的持久痕迹。
/// 所有错误静默忽略：这是崩溃路径，写不进去也绝不允许二次 panic。
/// 每条报告以 unix 时间戳行开头，区分多次运行的崩溃记录。
fn write_bootstrap_report(path: &Path, message: &str) {
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() > MAX_BOOTSTRAP_FILE_BYTES {
            let _ = std::fs::write(path, []);
        }
    }
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let entry = format!("--- unix_epoch_secs: {secs} ---\n{message}");
    let _ = file.write_all(entry.as_bytes());
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
