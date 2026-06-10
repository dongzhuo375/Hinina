use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContestError {
    #[error("获取比赛失败: {0}")]
    FetchFailed(String),
    #[error("比赛未找到: {0}")]
    NotFound(String),
}
