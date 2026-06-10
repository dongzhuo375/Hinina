use thiserror::Error;

#[derive(Debug, Error)]
pub enum HOJError {
    #[error("HOJ API 错误: {0}")]
    ApiError(String),
}
