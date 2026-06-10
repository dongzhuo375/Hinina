use thiserror::Error;

#[derive(Debug, Error)]
pub enum HUSTOJError {
    #[error("HUSTOJ API 错误: {0}")]
    ApiError(String),
}
