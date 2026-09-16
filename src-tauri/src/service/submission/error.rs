use thiserror::Error;

#[derive(Debug, Error)]
pub enum SubmissionError {
    #[error("提交失败: {0}")]
    SubmitFailed(String),
}
