use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("配置加载失败: {0}")]
    LoadFailed(String),
    #[error("配置保存失败: {0}")]
    SaveFailed(String),
}
