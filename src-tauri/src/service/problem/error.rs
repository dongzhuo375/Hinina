use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProblemError {
    #[error("获取题目失败: {0}")]
    FetchFailed(String),
    #[error("题目未找到: {0}")]
    NotFound(String),
}
