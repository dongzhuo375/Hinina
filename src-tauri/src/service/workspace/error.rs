use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("工作区未找到: {0}")]
    NotFound(String),
    #[error("文件读写错误: {0}")]
    IoError(String),
}
