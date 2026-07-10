use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

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

    /// 构建 workspace 内文件的相对路径，并对 `file_path` 做路径穿越校验。
    fn workspace_relative(&self, workspace_id: &str, file_path: &Path) -> AppResult<String> {
        for component in file_path.components() {
            if matches!(component, Component::ParentDir) {
                return Err(AppError::Workspace(format!(
                    "文件路径包含非法字符: {}",
                    file_path.display()
                )));
            }
        }
        Ok(format!("{}/{}/{}", WORKSPACES_DIR, workspace_id, file_path.display()))
    }

    /// 构建 workspace 根目录的相对路径。
    fn workspace_root(&self, workspace_id: &str) -> String {
        format!("{}/{}", WORKSPACES_DIR, workspace_id)
    }

    /// 递归遍历目录，收集所有文件相对于 workspace 根目录的路径。
    fn walk_dir(dir: &Path, prefix: &str, files: &mut Vec<PathBuf>) -> AppResult<()> {
        for entry in std::fs::read_dir(dir).map_err(|e| {
            AppError::Workspace(format!("读取目录失败 {}: {}", dir.display(), e))
        })? {
            let entry = entry.map_err(|e| {
                AppError::Workspace(format!("读取目录条目失败: {}", e))
            })?;
            let path = entry.path();
            let relative = path
                .to_string_lossy()
                .strip_prefix(prefix)
                .map(PathBuf::from)
                .ok_or_else(|| {
                    AppError::Workspace(format!("路径前缀剥离失败: {}", path.display()))
                })?;
            if path.is_dir() {
                Self::walk_dir(&path, prefix, files)?;
            } else {
                files.push(relative);
            }
        }
        Ok(())
    }
}

impl WorkspaceRepository for FsWorkspaceRepository {
    fn save_file(&self, workspace_id: &str, path: &Path, content: &str) -> AppResult<()> {
        let rel = self.workspace_relative(workspace_id, path)?;
        self.storage.write_string(&rel, content)
    }

    fn read_file(&self, workspace_id: &str, path: &Path) -> AppResult<String> {
        let rel = self.workspace_relative(workspace_id, path)?;
        self.storage.read_to_string(&rel).map_err(|_| {
            AppError::Workspace(format!(
                "文件不存在: workspace={}, path={}",
                workspace_id,
                path.display()
            ))
        })
    }

    fn list_files(&self, workspace_id: &str) -> AppResult<Vec<PathBuf>> {
        let root = self.workspace_root(workspace_id);
        // 工作区不存在时返回空列表，而非错误
        if !self.storage.exists(&root) {
            return Ok(Vec::new());
        }
        // 递归遍历工作区目录，返回所有文件的相对路径（去掉 workspace 根前缀）。
        let root_abs = self.storage.base_dir().join(&root);
        let mut prefix = root_abs.to_string_lossy().to_string();
        // 确保前缀以路径分隔符结尾，使 strip_prefix 后得到干净的相对路径
        if !prefix.ends_with(std::path::MAIN_SEPARATOR) {
            prefix.push(std::path::MAIN_SEPARATOR);
        }
        let mut files = Vec::new();
        Self::walk_dir(&root_abs, &prefix, &mut files)?;
        Ok(files)
    }

    fn delete_workspace(&self, workspace_id: &str) -> AppResult<()> {
        let root = self.workspace_root(workspace_id);
        self.storage.remove_all(&root)
    }

    fn exists(&self, workspace_id: &str) -> bool {
        let root = self.workspace_root(workspace_id);
        self.storage.exists(&root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn repo(name: &str) -> FsWorkspaceRepository {
        let dir = std::env::temp_dir().join(format!("hinina-test-ws-{}", name));
        let _ = std::fs::remove_dir_all(&dir);
        FsWorkspaceRepository::new(Arc::new(Storage::new(dir)))
    }

    #[test]
    fn save_and_read_file() {
        let r = repo("save-read");
        let ws = "test-ws-1";
        let path = Path::new("main.cpp");
        let content = "#include <iostream>\nint main() { return 0; }";

        r.save_file(ws, path, content).unwrap();
        assert!(r.exists(ws));

        let read = r.read_file(ws, path).unwrap();
        assert_eq!(read, content);
    }

    #[test]
    fn read_nonexistent_file_returns_error() {
        let r = repo("no-file");
        let result = r.read_file("no-such-ws", Path::new("x.cpp"));
        assert!(result.is_err());
    }

    #[test]
    fn list_files_after_save() {
        let r = repo("list-files");
        let ws = "test-ws-2";
        r.save_file(ws, Path::new("a.cpp"), "// a").unwrap();
        r.save_file(ws, Path::new("sub/b.py"), "# b").unwrap();

        let mut files = r.list_files(ws).unwrap();
        files.sort();
        assert_eq!(files.len(), 2);
        assert!(files.contains(&PathBuf::from("a.cpp")));
        assert!(files.contains(&PathBuf::from("sub/b.py")));
    }

    #[test]
    fn list_files_nonexistent_workspace_returns_empty() {
        let r = repo("empty-list");
        let files = r.list_files("no-such-ws").unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn exists_detects_workspace() {
        let r = repo("exists");
        assert!(!r.exists("ws-x"));
        r.save_file("ws-x", Path::new("f.txt"), "hello").unwrap();
        assert!(r.exists("ws-x"));
    }

    #[test]
    fn delete_workspace_removes_all() {
        let r = repo("delete");
        let ws = "ws-to-delete";
        r.save_file(ws, Path::new("f.txt"), "data").unwrap();
        assert!(r.exists(ws));

        r.delete_workspace(ws).unwrap();
        assert!(!r.exists(ws));
        assert!(r.list_files(ws).unwrap().is_empty());
    }

    #[test]
    fn rejects_path_traversal() {
        let r = repo("traversal");
        let ws = "ws-safe";
        // 确保 workspace 存在
        r.save_file(ws, Path::new("legit.txt"), "ok").unwrap();

        assert!(r.save_file(ws, Path::new("../escape.txt"), "bad").is_err());
        assert!(r.read_file(ws, Path::new("../../etc/passwd")).is_err());
        assert!(r.save_file(ws, Path::new("a/../b"), "bad").is_err());
    }
}
