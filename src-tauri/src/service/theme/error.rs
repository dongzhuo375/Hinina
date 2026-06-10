use thiserror::Error;

#[derive(Debug, Error)]
pub enum ThemeError {
    #[error("主题未找到: {0}")]
    NotFound(String),
}
