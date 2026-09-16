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
pub struct Logger;

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
    pub fn init(base_dir: &Path) {
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
            }
            None => {
                // 文件层不可用：保持旧的单路 stderr 行为
                tracing_subscriber::registry()
                    .with(filter)
                    .with(stderr_layer)
                    .init();
            }
        }
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

#[cfg(test)]
#[path = "tests/logger_tests.rs"]
mod tests;
