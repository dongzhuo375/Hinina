use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::core::error::AppResult;
use crate::core::repository::workspace_repo::WorkspaceRepository;
use crate::infra::storage::Storage;

/// WorkspaceRepository 的文件系统实现
pub struct FsWorkspaceRepository {
    storage: Arc<Storage>,
}

impl FsWorkspaceRepository {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }
}

impl WorkspaceRepository for FsWorkspaceRepository {
    fn save_file(&self, workspace_id: &str, path: &Path, content: &str) -> AppResult<()> {
        let _ = (workspace_id, path, content);
        todo!("FsWorkspaceRepository::save_file()")
    }

    fn read_file(&self, workspace_id: &str, path: &Path) -> AppResult<String> {
        let _ = (workspace_id, path);
        todo!("FsWorkspaceRepository::read_file()")
    }

    fn list_files(&self, workspace_id: &str) -> AppResult<Vec<PathBuf>> {
        let _ = workspace_id;
        todo!("FsWorkspaceRepository::list_files()")
    }

    fn delete_workspace(&self, workspace_id: &str) -> AppResult<()> {
        let _ = workspace_id;
        todo!("FsWorkspaceRepository::delete_workspace()")
    }

    fn exists(&self, workspace_id: &str) -> bool {
        let _ = workspace_id;
        todo!("FsWorkspaceRepository::exists()")
    }
}
