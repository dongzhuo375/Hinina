use thiserror::Error;

#[derive(Debug, Error)]
pub enum QDUOJError {
    #[error("QDUOJ API 错误: {0}")]
    ApiError(String),
}
