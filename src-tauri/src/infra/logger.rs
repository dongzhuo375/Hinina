use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;
#[cfg(not(test))]
use std::sync::{Arc, Mutex};

use crate::core::error::{AppError, AppResult};

// 订阅器装配只存在于非测试构建（见 `Logger::init` 的 cfg 拆分）：
// 测试构建不装全局 tracing 订阅器，这些导入若不加门控会在 `cargo clippy
// --all-targets -- -D warnings` 下以 unused-import 阻塞 CI（评审 #6）。
#[cfg(not(test))]
use tracing_subscriber::fmt::MakeWriter;
#[cfg(not(test))]
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

/// 日志文件相对 base_dir 的路径。
///
/// 落盘与展示的唯一事实来源：`get_storage_info` Command 直接引用此常量
/// 拼接展示路径，避免两处硬编码漂移。
pub const LOG_RELATIVE_PATH: &str = "logs/hinina.log";

/// 日志文件大小上限：超过即在启动时截断重开。
///
/// 采用「截断式轮转」而不是按天/按份数滚动：客户端无长期日志留存需求，
/// 日志只服务于近期排障，单文件 + 上限截断足以防止无限膨胀占满选手磁盘。
const MAX_LOG_FILE_BYTES: u64 = 5 * 1024 * 1024;

/// 进程内**实际接管文件层的日志路径**（评审第三轮 #12）。
///
/// 全局订阅器是进程级单例：第二次 `init` 的 `try_init` 必然失败，但第一次
/// 安装的文件层仍在向**它自己的路径**写入。`log_path`/`has_log_file` 以
/// 实际生效的路径为准——同路径二次 init 时沿用已安装路径（文件层确实还在
/// 写），进程内从未成功安装过则如实为 `None`。仅非测试构建使用。
#[cfg(not(test))]
static ACTIVE_LOG_PATH: std::sync::Mutex<Option<std::path::PathBuf>> = std::sync::Mutex::new(None);

/// 日志追踪（基于 Tracing）。
///
/// 双路输出：stderr（开发调试）+ 文件（现场排障，选手机器上没有控制台）。
/// 支持 debug/release 自适应级别与 RUST_LOG 覆盖。
pub struct Logger {
    /// 日志文件绝对路径（**运行期清理日志**用；文件层不可用时为 `None`）。
    ///
    /// 为什么存路径而不是文件句柄：Windows 上 `set_len` 需要 `FILE_WRITE_DATA`，
    /// 而日志句柄是**追加模式**打开的（只有 `FILE_APPEND_DATA`），截断会被拒
    /// （实测 `Os code 5, PermissionDenied`）。改用「重开 + 截断」：新句柄带
    /// `GENERIC_WRITE`，而两个句柄的共享模式都允许对方写入，故互不冲突。
    /// 这与启动时的截断式轮转是同一手法，行为一致。
    log_path: Option<std::path::PathBuf>,
}

/// 多线程共享的日志文件写入器。
///
/// tracing 的 `MakeWriter` 每次事件都会取一个 `io::Write`，
/// 而 `File` 不是 `Sync`，故用 `Arc<Mutex<File>>` 共享句柄并逐条加锁写入。
/// 仅非测试构建使用（测试构建不装订阅器，见 `Logger::init`）。
#[cfg(not(test))]
struct SharedFileWriter(Arc<Mutex<File>>);

#[cfg(not(test))]
impl io::Write for SharedFileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut file = self.0.lock().unwrap_or_else(|e| e.into_inner());
        io::Write::write(&mut *file, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::Write::flush(&mut *self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

#[cfg(not(test))]
impl<'a> MakeWriter<'a> for SharedFileWriter {
    type Writer = SharedFileWriter;

    fn make_writer(&'a self) -> Self::Writer {
        SharedFileWriter(Arc::clone(&self.0))
    }
}

impl Logger {
    /// 初始化日志系统（stderr + `{base_dir}/logs/hinina.log` 双路输出）。
    ///
    /// - debug 构建：默认 `debug` 级别
    /// - release 构建：默认 `info` 级别
    /// - 可通过 `RUST_LOG` 环境变量覆盖（同时作用于两路输出）
    /// - 日志中包含 span 事件的 enter/exit 信息
    /// - 文件层失败（磁盘只读等）时降级为仅 stderr，不阻断启动
    ///
    /// 返回持有文件句柄的实例（供运行期截断日志，见 [`Logger::clear_log_file`]）。
    ///
    /// **测试构建不装全局订阅器**（`cfg(test)` 分支只探测文件层是否可用）：
    /// 全局订阅器是进程级单例，并行测试各自 `AppContext::init` 会争抢全局位；
    /// 且一旦装上，整个测试进程所有用例的调试日志都会被格式化输出——
    /// 输出体积暴涨，与 libtest 的捕获机制叠加在 Windows 上曾引发进程级崩溃
    /// （STATUS_STACK_BUFFER_OVERRUN）。`clear_log_file` / `has_log_file` 是
    /// 纯文件层操作，不依赖订阅器，测试语义不受影响。
    pub fn init(base_dir: &Path) -> Self {
        #[cfg(not(test))]
        {
            let default_level = if cfg!(debug_assertions) {
                "debug"
            } else {
                "info"
            };

            let filter =
                EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));

            let stderr_layer = fmt::layer()
                .with_span_events(FmtSpan::ENTER | FmtSpan::EXIT)
                .with_target(true)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true)
                .with_writer(io::stderr);

            match Self::open_log_file(base_dir) {
                Some(file) => {
                    let file_layer = fmt::layer()
                        .with_span_events(FmtSpan::ENTER | FmtSpan::EXIT)
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_file(true)
                        .with_line_number(true)
                        // 文件里的 ANSI 转义序列不可读，必须关闭着色
                        .with_ansi(false)
                        .with_writer(SharedFileWriter(Arc::new(Mutex::new(file))));
                    // try_init 而非 init：全局 subscriber 每进程只能设一次，重复
                    // init 时沿用已装好的订阅器而不是 panic（防御性；生产只跑一次）。
                    // 安装失败时本次没有任何订阅器生效，文件层不会有写入 ——
                    // log_path 必须如实为 None（评审 #4：has_log_file 不得虚报）
                    let installed = tracing_subscriber::registry()
                        .with(filter)
                        .with(stderr_layer)
                        .with(file_layer)
                        .try_init();
                    let path = if installed.is_ok() {
                        let p = base_dir.join(LOG_RELATIVE_PATH);
                        *ACTIVE_LOG_PATH.lock().unwrap_or_else(|e| e.into_inner()) =
                            Some(p.clone());
                        Some(p)
                    } else {
                        // 安装失败时沿用实际接管文件层的路径（若有）：第一次
                        // 安装的文件层仍在向它写入，has_log_file 以此为准；
                        // 进程内从未成功安装过则如实为 None（评审 #4/#12）
                        ACTIVE_LOG_PATH
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .clone()
                    };
                    Self { log_path: path }
                }
                None => {
                    // 文件层不可用：保持旧的单路 stderr 行为
                    let _ = tracing_subscriber::registry()
                        .with(filter)
                        .with(stderr_layer)
                        .try_init();
                    Self { log_path: None }
                }
            }
        }

        #[cfg(test)]
        {
            let opened = Self::open_log_file(base_dir).is_some();
            Self {
                log_path: opened.then(|| base_dir.join(LOG_RELATIVE_PATH)),
            }
        }
    }

    /// 清空日志内容，返回释放的字节数（文件层不可用时为 0）。
    ///
    /// 「重开 + 截断」而不是用日志句柄 `set_len`：后者在 Windows 上被拒（追加模式
    /// 只有 `FILE_APPEND_DATA`，没有 `FILE_WRITE_DATA`）。两个句柄的共享模式都允许
    /// 对方写入，故与 tracing 文件层不冲突；`append` 模式保证后续日志自动从 0 开始，
    /// 不会写出稀疏文件。
    ///
    /// 只清内容、**不删文件**：删了要等重启才会重建，中间这段排障信息就彻底没了。
    pub fn clear_log_file(&self) -> AppResult<u64> {
        let Some(path) = &self.log_path else {
            return Ok(0);
        };
        truncate_log_file(path).map_err(|e| AppError::Io(format!("清空日志文件失败: {}", e)))
    }

    /// 日志文件层是否可用。
    ///
    /// 用于区分「清空了日志」与「本来就没有文件层（磁盘只读等）」，让界面能如实
    /// 说明结果而不是笼统报成功。
    pub fn has_log_file(&self) -> bool {
        self.log_path.is_some()
    }

    /// 打开日志文件（追加模式）；超过大小上限先截断。失败返回 `None` 由调用方降级。
    fn open_log_file(base_dir: &Path) -> Option<File> {
        let log_path = base_dir.join(LOG_RELATIVE_PATH);
        if let Some(parent) = log_path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                stderr_line(&format!("创建日志目录失败 {}: {}", parent.display(), e));
                return None;
            }
        }

        // 启动时检查大小：超限即截断（rotate-by-truncate）
        if let Ok(meta) = fs::metadata(&log_path) {
            if meta.len() > MAX_LOG_FILE_BYTES {
                if let Err(e) = fs::write(&log_path, []) {
                    stderr_line(&format!("截断日志文件失败 {}: {}", log_path.display(), e));
                }
            }
        }

        match OpenOptions::new().create(true).append(true).open(&log_path) {
            Ok(file) => Some(file),
            Err(e) => {
                stderr_line(&format!("打开日志文件失败 {}: {}", log_path.display(), e));
                None
            }
        }
    }
}

/// 写一行到 stderr，写失败静默忽略。
///
/// `eprintln!` 在写失败时会 panic（"failed printing to stderr"），而
/// `windows_subsystem = "windows"` 的 release 构建没有 stderr 句柄。
/// `open_log_file` 的错误出口是日志系统自身失效时的最后报障途径，
/// 绝不能自己把启动炸掉。
fn stderr_line(msg: &str) {
    use std::io::Write as _;
    let _ = io::stderr().write_all(msg.as_bytes());
}

/// 把日志文件截断为 0，返回释放的字节数。
///
/// 抽成自由函数是为了可单测：`Logger::init` 会设置**全局** subscriber（每进程只能
/// 一次，与 `cargo test` 的输出捕获冲突），故截断逻辑必须能脱离 `init` 验证。
///
/// 用 `fs::write(path, [])`（`File::create` → truncate）而不是 `File::open` + `set_len`：
/// 语义等价且与启动时的截断式轮转同源，两处不会漂移。
fn truncate_log_file(path: &Path) -> io::Result<u64> {
    let freed = fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    fs::write(path, [])?;
    Ok(freed)
}

#[cfg(test)]
#[path = "tests/logger_tests.rs"]
mod tests;
