use std::path::{Path, PathBuf};

use crate::core::error::AppResult;

/// Workspace 持久化仓库。
///
/// 抽象工作区文件的读写操作，解耦 Service 与底层存储实现。
/// 当前基于文件系统实现，未来可替换为 SQLite 或云同步。
pub trait WorkspaceRepository: Send + Sync {
    /// 保存文件到工作区
    fn save_file(&self, workspace_id: &str, path: &Path, content: &str) -> AppResult<()>;

    /// 读取工作区中的文件
    fn read_file(&self, workspace_id: &str, path: &Path) -> AppResult<String>;

    /// 列出工作区中所有文件
    fn list_files(&self, workspace_id: &str) -> AppResult<Vec<PathBuf>>;

    /// 删除整个工作区目录
    fn delete_workspace(&self, workspace_id: &str) -> AppResult<()>;

    /// 检查工作区是否存在
    fn exists(&self, workspace_id: &str) -> bool;
}
