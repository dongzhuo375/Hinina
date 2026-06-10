use std::path::PathBuf;

/// 本地文件存储工具。
///
/// 提供底层的文件读写能力，不包含业务语义。
/// 业务层应通过 Repository trait 间接使用，不要直接依赖此模块。
pub struct Storage {
    base_dir: PathBuf,
}

impl Storage {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub fn base_dir(&self) -> &PathBuf {
        &self.base_dir
    }
}
