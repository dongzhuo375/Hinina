use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};

use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

use crate::core::error::{AppError, AppResult};

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
struct SharedFileWriter(Arc<Mutex<File>>);

impl io::Write for SharedFileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut file = self
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        io::Write::write(&mut *file, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::Write::flush(&mut *self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

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
    pub fn init(base_dir: &Path) -> Self {
        let default_level = if cfg!(debug_assertions) {
            "debug"
        } else {
            "info"
        };

        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(default_level));

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
                tracing_subscriber::registry()
                    .with(filter)
                    .with(stderr_layer)
                    .with(file_layer)
                    .init();
                Self {
                    log_path: Some(base_dir.join(LOG_RELATIVE_PATH)),
                }
            }
            None => {
                // 文件层不可用：保持旧的单路 stderr 行为
                tracing_subscriber::registry()
                    .with(filter)
                    .with(stderr_layer)
                    .init();
                Self { log_path: None }
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
                eprintln!("创建日志目录失败 {}: {}", parent.display(), e);
                return None;
            }
        }

        // 启动时检查大小：超限即截断（rotate-by-truncate）
        if let Ok(meta) = fs::metadata(&log_path) {
            if meta.len() > MAX_LOG_FILE_BYTES {
                if let Err(e) = fs::write(&log_path, []) {
                    eprintln!("截断日志文件失败 {}: {}", log_path.display(), e);
                }
            }
        }

        match OpenOptions::new().create(true).append(true).open(&log_path) {
            Ok(file) => Some(file),
            Err(e) => {
                eprintln!("打开日志文件失败 {}: {}", log_path.display(), e);
                None
            }
        }
    }
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
