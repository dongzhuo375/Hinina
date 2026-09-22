use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use tracing::{debug, warn};

use crate::core::error::{AppError, AppResult};
use crate::core::repository::workspace_repo::WorkspaceRepository;
use crate::infra::storage::Storage;

/// Workspace 数据的文件系统根目录名。
const WORKSPACES_DIR: &str = "workspaces";

/// WorkspaceRepository 的文件系统实现。
///
/// 目录结构：`{storage.base_dir}/workspaces/{workspace_id}/...`
pub struct FsWorkspaceRepository {
    storage: Arc<Storage>,
}

impl FsWorkspaceRepository {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    /// 校验 workspace_id 合法性：仅允许字母、数字、短横线和下划线。
    fn validate_workspace_id(id: &str) -> AppResult<()> {
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(AppError::Workspace(format!(
                "无效的 workspace_id: {}（仅允许字母、数字、-、_）",
                id
            )));
        }
        Ok(())
    }

    /// 构建 workspace 内文件的相对路径，并对 `file_path` 做路径穿越校验。
    fn workspace_relative(&self, workspace_id: &str, file_path: &Path) -> AppResult<String> {
        Self::validate_workspace_id(workspace_id)?;
        for component in file_path.components() {
            if matches!(component, Component::ParentDir) {
                return Err(AppError::Workspace(format!(
                    "文件路径包含非法字符: {}",
                    file_path.display()
                )));
            }
        }
        let relative = Path::new(WORKSPACES_DIR)
            .join(workspace_id)
            .join(file_path);
        Ok(relative.to_string_lossy().into_owned())
    }

    /// 构建 workspace 根目录的相对路径。
    fn workspace_root(&self, workspace_id: &str) -> String {
        Path::new(WORKSPACES_DIR)
            .join(workspace_id)
            .to_string_lossy()
            .into_owned()
    }

    /// 递归遍历目录，收集所有文件相对于 `dir_root` 的路径。
    fn walk_dir(dir: &Path, dir_root: &Path, files: &mut Vec<PathBuf>) -> AppResult<()> {
        for entry in std::fs::read_dir(dir).map_err(|e| {
            AppError::Workspace(format!("读取目录失败 {}: {}", dir.display(), e))
        })? {
            let entry = entry.map_err(|e| {
                AppError::Workspace(format!("读取目录条目失败: {}", e))
            })?;
            let path = entry.path();
            let relative = path.strip_prefix(dir_root).map_err(|e| {
                AppError::Workspace(format!("路径前缀剥离失败 {}: {}", path.display(), e))
            })?;
            if path.is_dir() {
                Self::walk_dir(&path, dir_root, files)?;
            } else {
                files.push(relative.to_path_buf());
            }
        }
        Ok(())
    }
}

impl WorkspaceRepository for FsWorkspaceRepository {
    fn save_file(&self, workspace_id: &str, path: &Path, content: &str) -> AppResult<()> {
        let rel = self.workspace_relative(workspace_id, path)?;
        let result = self.storage.write_string(&rel, content);
        if let Err(ref e) = result {
            warn!(
                workspace_id = workspace_id,
                file = %path.display(),
                error = %e,
                "工作区文件保存失败"
            );
        } else {
            debug!(
                workspace_id = workspace_id,
                file = %path.display(),
                size = content.len(),
                "保存工作区文件"
            );
        }
        result
    }

    fn read_file(&self, workspace_id: &str, path: &Path) -> AppResult<String> {
        let rel = self.workspace_relative(workspace_id, path)?;
        self.storage.read_to_string(&rel).map_err(|e| {
            warn!(
                workspace_id = workspace_id,
                file = %path.display(),
                error = %e,
                "工作区文件读取失败"
            );
            AppError::Workspace(format!(
                "读取文件失败: workspace={}, path={}, 错误: {}",
                workspace_id,
                path.display(),
                e
            ))
        })
    }

    fn list_files(&self, workspace_id: &str) -> AppResult<Vec<PathBuf>> {
        let root = self.workspace_root(workspace_id);
        // 工作区不存在时返回空列表，而非错误
        if !self.storage.exists(&root) {
            debug!(workspace_id = workspace_id, "工作区不存在，返回空文件列表");
            return Ok(Vec::new());
        }
        // 递归遍历工作区目录，返回所有文件的相对路径（去掉 workspace 根前缀）。
        let root_abs = self.storage.base_dir().join(&root);
        let mut files = Vec::new();
        Self::walk_dir(&root_abs, &root_abs, &mut files)?;
        // 过滤内部元数据文件，避免暴露给调用方
        files.retain(|p| p != &PathBuf::from("workspace.json"));
        debug!(
            workspace_id = workspace_id,
            count = files.len(),
            "列出工作区文件"
        );
        Ok(files)
    }

    fn delete_file(&self, workspace_id: &str, path: &Path) -> AppResult<()> {
        let rel = self.workspace_relative(workspace_id, path)?;
        // 幂等：文件不存在视为成功（清理场景下重复调用、内存与磁盘状态
        // 不一致时的重试都不应报错）
        if !self.storage.exists(&rel) {
            debug!(
                workspace_id = workspace_id,
                file = %path.display(),
                "文件不存在，跳过删除"
            );
            return Ok(());
        }
        let result = self.storage.remove(&rel);
        if let Err(ref e) = result {
            warn!(
                workspace_id = workspace_id,
                file = %path.display(),
                error = %e,
                "工作区文件删除失败"
            );
        } else {
            debug!(
                workspace_id = workspace_id,
                file = %path.display(),
                "删除工作区文件"
            );
        }
        result
    }

    fn delete_workspace(&self, workspace_id: &str) -> AppResult<()> {
        let root = self.workspace_root(workspace_id);
        let result = self.storage.remove_all(&root);
        if let Err(ref e) = result {
            warn!(
                workspace_id = workspace_id,
                error = %e,
                "工作区删除失败"
            );
        } else {
            debug!(workspace_id = workspace_id, "删除工作区");
        }
        result
    }

    fn exists(&self, workspace_id: &str) -> bool {
        let root = self.workspace_root(workspace_id);
        self.storage.exists(&root)
    }

    fn list_workspace_ids(&self) -> AppResult<Vec<String>> {
        let workspaces_dir = self.storage.base_dir().join(WORKSPACES_DIR);
        if !workspaces_dir.exists() {
            return Ok(Vec::new());
        }

        let mut ids = Vec::new();
        for entry in std::fs::read_dir(&workspaces_dir).map_err(|e| {
            AppError::Workspace(format!("读取 workspaces 目录失败: {}", e))
        })? {
            let entry = entry.map_err(|e| {
                AppError::Workspace(format!("读取目录条目失败: {}", e))
            })?;
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                if let Some(name) = entry.file_name().to_str() {
                    ids.push(name.to_string());
                }
            }
        }
        Ok(ids)
    }
}

#[cfg(test)]
#[path = "tests/fs_workspace_repo_tests.rs"]
mod tests;
