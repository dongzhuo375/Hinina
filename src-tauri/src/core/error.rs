use serde::Serialize;
use thiserror::Error;

/// 统一应用错误枚举，可跨 Tauri IPC 序列化传递
#[derive(Debug, Error, Serialize)]
pub enum AppError {
    #[error("认证错误: {0}")]
    Auth(String),

    #[error("比赛错误: {0}")]
    Contest(String),

    #[error("题目错误: {0}")]
    Problem(String),

    #[error("提交错误: {0}")]
    Submission(String),

    #[error("工作区错误: {0}")]
    Workspace(String),

    #[error("IO 错误: {0}")]
    Io(String),

    #[error("网络错误: {0}")]
    Network(String),

    #[error("配置错误: {0}")]
    Config(String),

    #[error("Provider 未找到: {0}")]
    ProviderNotFound(String),

    #[error("序列化错误: {0}")]
    Serialization(String),

    #[error("未知错误: {0}")]
    Unknown(String),
}

// ── From 转换：让 ? 操作符自动将底层错误转为 AppError ──

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Network(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Serialization(e.to_string())
    }
}

impl AppError {
    /// 返回用户可读的错误信息
    #[must_use]
    pub fn user_message(&self) -> &str {
        match self {
            AppError::Auth(msg) => msg,
            AppError::Contest(msg) => msg,
            AppError::Problem(msg) => msg,
            AppError::Submission(msg) => msg,
            AppError::Workspace(msg) => msg,
            AppError::Io(msg) => msg,
            AppError::Network(msg) => msg,
            AppError::Config(msg) => msg,
            AppError::ProviderNotFound(msg) => msg,
            AppError::Serialization(msg) => msg,
            AppError::Unknown(msg) => msg,
        }
    }

    /// 是否应向用户展示此错误
    #[must_use]
    pub fn is_user_facing(&self) -> bool {
        true
    }

    /// 补上「哪个环节失败」的上下文，**保留原始变体**。
    ///
    /// 为什么不用 `AppError::Network(format!("xx 请求失败: {}", e))` 重新包装：
    /// 那样会把反序列化失败、认证失败等一律改写成「网络错误」，现场看到的是
    /// 「网络错误: … 序列化错误: …」这类自相矛盾的嵌套消息，把排障引向错误方向
    /// （例如把 DTO 字段不匹配当成断网去查）。变体是前端 `isAuthError` 分流的依据，
    /// 改写变体还会让会话失效兜底失灵。
    #[must_use]
    pub fn context(self, ctx: &str) -> Self {
        // 逐变体展开而非统一取 user_message()：Display 会带上「网络错误:」等前缀，
        // 与变体本身重复；这里只在原始消息前拼接调用方给出的环节名。
        match self {
            AppError::Auth(msg) => AppError::Auth(prepend(ctx, msg)),
            AppError::Contest(msg) => AppError::Contest(prepend(ctx, msg)),
            AppError::Problem(msg) => AppError::Problem(prepend(ctx, msg)),
            AppError::Submission(msg) => AppError::Submission(prepend(ctx, msg)),
            AppError::Workspace(msg) => AppError::Workspace(prepend(ctx, msg)),
            AppError::Io(msg) => AppError::Io(prepend(ctx, msg)),
            AppError::Network(msg) => AppError::Network(prepend(ctx, msg)),
            AppError::Config(msg) => AppError::Config(prepend(ctx, msg)),
            AppError::ProviderNotFound(msg) => AppError::ProviderNotFound(prepend(ctx, msg)),
            AppError::Serialization(msg) => AppError::Serialization(prepend(ctx, msg)),
            AppError::Unknown(msg) => AppError::Unknown(prepend(ctx, msg)),
        }
    }
}

/// 把环节名拼到消息前面（`ctx: msg`）。
fn prepend(ctx: &str, msg: String) -> String {
    format!("{}: {}", ctx, msg)
}

/// 应用全局 Result 类型别名
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
#[path = "tests/error_tests.rs"]
mod tests;
