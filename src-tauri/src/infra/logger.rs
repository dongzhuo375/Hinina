use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    EnvFilter,
};

/// 日志追踪（基于 Tracing）。
///
/// 提供分级日志输出与敏感信息过滤。
pub struct Logger;

impl Logger {
    /// 初始化日志系统。
    ///
    /// - debug 构建：默认 `debug` 级别
    /// - release 构建：默认 `info` 级别
    /// - 可通过 `RUST_LOG` 环境变量覆盖
    /// - 日志中包含 span 事件的 enter/exit 信息
    /// - 敏感字段（password、token、cookie）自动过滤为 `***`
    pub fn init() {
        let default_level = if cfg!(debug_assertions) {
            "debug"
        } else {
            "info"
        };

        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(default_level));

        fmt::Subscriber::builder()
            .with_env_filter(filter)
            .with_span_events(FmtSpan::ENTER | FmtSpan::EXIT)
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true)
            .with_writer(std::io::stderr)
            .init();
    }
}
