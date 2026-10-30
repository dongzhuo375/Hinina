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
//! **写入顺序同样是保证的一部分**：先 bootstrap、后 `tracing` / stderr。后者
//! 可能因 `SharedFileWriter` 重入而阻塞（见 `logger.rs`），把持久出口排在它
//! 之后会让「每次 panic 都有记录」依赖那条路径不挂起。
//!
//! 日志开启时同一报告在 `logs/hinina.log` 与 bootstrap 文件中各有一份，
//! **这是有意重复**：钩子内无法判断事件是否会被 `EnvFilter` 丢弃，重复可接受、
//! 丢失不可接受。被插件 worker 的 `catch_unwind` 捕获的 panic 同样会记录
//! （钩子按 Rust 语义先于捕获运行），这是有意的可观测性——它仍是一次真实 panic。
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
/// logger 的启动轮转共用 `logger::truncate_if_oversized`），防止反复早期崩溃
/// 无限膨胀。
const MAX_BOOTSTRAP_FILE_BYTES: u64 = 1024 * 1024;

/// 进程内 bootstrap 写入串行化。
///
/// 「检查大小 → 截断 → 追加」必须是一个整体：并发 panic（多线程、插件 worker）
/// 各自检查后都截断，会把对方刚写入的记录抹掉。
///
/// 用**阻塞 `lock()`** 而不是 `try_lock`：崩溃记录不允许因争用被静默丢弃 ——
/// `try_lock` 失败即返回，两个 panic 同时发生时后到那条整条消失，即使文件远未
/// 到上限（这是从「无锁追加」改成加锁时的行为回归）。
///
/// 阻塞不会自死锁：临界区内没有任何会 panic 的调用（符号链接检查、截断、打开、
/// 追加全部返回 `Result`），且钩子内再 panic 时 std 直接 abort，不走 unwind、
/// 不会重新取这把锁。代价是临时目录 I/O 挂起（安全软件扫描等）时 panic 线程会
/// 等待——以等待换「每次 panic 都有记录」。中毒按仓库约定 `into_inner` 取回，
/// 避免一次理论上的中毒让此后所有写入被永久跳过。跨进程竞争（多开应用实例）
/// 仍不处理。
static BOOTSTRAP_WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Bootstrap 崩溃文件路径：`%TEMP%/hinina-panic.log`。
///
/// 固定用临时目录而不是数据目录：钩子运行时数据目录解析尚未发生（那正是
/// 可能 panic 的环节之一），任何「先解析再写」都会把失败面重新拉进钩子。
fn bootstrap_log_path() -> std::path::PathBuf {
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
    // **顺序是保证的一部分：先写 bootstrap，再走 tracing / stderr。**
    // tracing 文件层存在 SharedFileWriter 重入自死锁的理论窗口（见 logger.rs）：
    // 若本线程正持有那把锁（panic 发生在日志字段的 Display 里），`error!` 会阻塞，
    // 钩子永远走不到后面的语句。持久出口必须排在它之前——那条路径挂住最多丢掉
    // 日志副本，不会让唯一记录消失。各层写失败一律静默（崩溃路径不许二次 panic）。
    write_bootstrap_report(&bootstrap_log_path(), &message);
    if crate::infra::logger::ready() {
        // 订阅器已装：error! 输出（stderr；过滤器允许时同时落 hinina.log）。
        // 上面已落盘，因此这里被 RUST_LOG 过滤或在重入窗口挂起，都不影响
        // 「每次 panic 都有持久记录」。
        tracing::error!(target: "panic", "{message}");
    } else {
        // Logger 尚未就绪：stderr 兜底。尚无文件层，不存在上述重入窗口。
        let _ = std::io::stderr().write_all(message.as_bytes());
    }
}

/// 把崩溃报告追加到 bootstrap 文件（**独立持久出口**）。
///
/// 每次 panic 都写，不经 tracing 过滤器——`RUST_LOG=off` 会过滤 `error!`，
/// 日志文件打不开时订阅器也只有 stderr，这两类情况下它是唯一的持久痕迹。
/// 所有错误静默忽略：这是崩溃路径，写不进去也绝不允许二次 panic。
/// 每条报告以 unix 时间戳行开头，区分多次运行的崩溃记录。
///
/// 调用位置：`report` 的**第一个**副作用（先于 tracing / stderr），顺序理由见
/// [`report`]——不排在可能因重入而阻塞的路径之后。
///
/// 安全与并发：
/// - 进程内用 [`BOOTSTRAP_WRITE_LOCK`] 串行化「检查-截断-追加」（阻塞等锁，
///   争用时排队而不是跳过 —— 每条 panic 都必须留下记录）；
/// - 拒绝符号链接目标（Unix 加固）：临时目录可被同机用户写入时不跟随预置
///   链接改写别人的文件；首次创建用 `create_new` 抢占，已有文件再校验一次。
///   检查与打开之间仍有极窄 TOCTOU 窗口，产品只发 Windows，Unix 风险主要
///   存在于开发/CI 环境；
/// - Unix 上新文件以 `0600` 创建、已有文件收敛到 `0600`。
fn write_bootstrap_report(path: &Path, message: &str) {
    // 阻塞等锁而非 try_lock：争用时跳过等于丢一条崩溃记录。临界区无 panic
    // 路径故不会自死锁（钩子内再 panic 会 abort，不会重新取锁）；中毒取回
    // 内部值继续写，避免一次中毒让后续所有记录被永久跳过。
    let _guard = BOOTSTRAP_WRITE_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if is_symlink(path) {
        return;
    }
    // 超限先轮转（与 logger 共用同一判据与手法）；失败静默
    let _ = crate::infra::logger::truncate_if_oversized(path, MAX_BOOTSTRAP_FILE_BYTES);

    let mut options = std::fs::OpenOptions::new();
    options.append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    // 首建用 create_new：若符号链接检查之后有人放入链接，创建会失败，
    // 且不再回退——宁可丢一条记录也不跟随链接写。
    let opened = match options.clone().create_new(true).open(path) {
        Ok(file) => Ok(file),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            if is_symlink(path) {
                return;
            }
            options.open(path)
        }
        Err(e) => Err(e),
    };
    let Ok(mut file) = opened else {
        return;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = file.set_permissions(std::fs::Permissions::from_mode(0o600));
    }

    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let entry = format!("--- unix_epoch_secs: {secs} ---\n{message}");
    let _ = file.write_all(entry.as_bytes());
}

/// 路径本身是否是符号链接（不跟随；不存在或读取失败一律视为「不是」）。
fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
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
