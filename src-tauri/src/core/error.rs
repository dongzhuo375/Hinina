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

    #[error("未知错误: {0}")]
    Unknown(String),
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
            AppError::Unknown(msg) => msg,
        }
    }

    /// 是否应向用户展示此错误
    #[must_use]
    pub fn is_user_facing(&self) -> bool {
        true
    }
}

/// 应用全局 Result 类型别名
pub type AppResult<T> = Result<T, AppError>;
