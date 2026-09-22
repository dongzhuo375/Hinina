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

/// 词法规范化：解析路径中的 `.` 与 `..`（不触文件系统、不解析符号链接）。
///
/// 供 `workspace_relative` 的前缀校验做纵深防御：输入已过组件白名单时是
/// 恒等变换，但保证「拼接结果以工作区根为前缀」这道闸对任何输入都成立。
fn lexically_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
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

    /// Win32 文件名组件内容校验（读写删三原语共用，收口在 `workspace_relative`）。
    ///
    /// 拒绝三类危险组件：
    /// 1. **非法字符** `< > : " | ? *` 与控制字符 —— 其中 `:` 是 NTFS ADS
    ///    分隔符：`workspace.json::$DATA` 即文件本身的默认数据流，
    ///    `fs::remove_file` 会删掉真正的 workspace.json（manager 守卫的
    ///    等价类比较拦不住它）；`save_file("workspace.json:evil")` 则会
    ///    造出 ADS 写入；
    /// 2. **保留设备名** CON/PRN/AUX/NUL/COM1-9/LPT1-9 —— Win32 按第一个点
    ///    前的词干解析，`NUL.txt` 与 `NUL` 同为设备；
    /// 3. **尾随点/空格** —— Win32 解析时剥离，`workspace.json.` 与
    ///    `workspace.json` 是同一文件；在仓库层拒绝可同时堵住
    ///    `save_file` 的变体名写路径。
    fn validate_win32_component(component: &str) -> AppResult<()> {
        if component
            .chars()
            .any(|c| matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*') || c.is_control())
        {
            return Err(AppError::Workspace(format!(
                "文件路径包含 Windows 非法字符: {}",
                component
            )));
        }
        if component.ends_with('.') || component.ends_with(' ') {
            return Err(AppError::Workspace(format!(
                "文件路径组件以点或空格结尾（Windows 下与剥离后的名字是同一文件）: {}",
                component
            )));
        }
        if Self::is_reserved_device_name(component) {
            return Err(AppError::Workspace(format!(
                "文件路径使用了 Windows 保留设备名: {}",
                component
            )));
        }
        Ok(())
    }

    /// 是否 Windows 保留设备名（按第一个点前的词干、大小写不敏感判定）。
    fn is_reserved_device_name(component: &str) -> bool {
        let stem = component.split('.').next().unwrap_or(component);
        const DEVICES: [&str; 22] = [
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6",
            "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7",
            "LPT8", "LPT9",
        ];
        DEVICES.contains(&stem.to_uppercase().as_str())
    }

    /// 构建工作区内文件的相对路径，并对 `file_path` 做路径安全校验。
    ///
    /// 防护分三层（`delete_file` 是破坏性原语，校验强度必须覆盖它）：
    /// 1. **组件白名单**：只接受 `Component::Normal` —— `..`、前导 `.`、
    ///    绝对路径（根目录 / Windows 盘符前缀）全部拒绝。黑名单式只拒
    ///    `ParentDir` 会放过 `C:`、`/` 等组件。
    /// 2. **组件内容校验**（[`Self::validate_win32_component`]）：Win32 非法
    ///    字符（含 NTFS ADS 分隔符 `:`）、保留设备名、尾随点/空格全部拒绝
    ///    —— 读写删三原语同时生效。
    /// 3. **拼接后规范化 + 前缀校验**：词法解析 `.`/`..` 后验证结果仍以
    ///    工作区根目录为前缀（组件级比较，非字符串前缀）。输入已过白名单
    ///    时这是恒等变换，作为纵深防御保留，兜底未来常量或校验的回归。
    fn workspace_relative(&self, workspace_id: &str, file_path: &Path) -> AppResult<String> {
        Self::validate_workspace_id(workspace_id)?;
        if file_path.as_os_str().is_empty() {
            return Err(AppError::Workspace("文件路径为空".into()));
        }
        for component in file_path.components() {
            match component {
                Component::Normal(name) => {
                    Self::validate_win32_component(&name.to_string_lossy())?;
                }
                _ => {
                    return Err(AppError::Workspace(format!(
                        "文件路径包含非法组件: {}（仅允许普通文件名/目录名）",
                        file_path.display()
                    )));
                }
            }
        }
        let root = Path::new(WORKSPACES_DIR).join(workspace_id);
        let relative = lexically_normalize(&root.join(file_path));
        if !relative.starts_with(&root) {
            return Err(AppError::Workspace(format!(
                "文件路径越界: {}",
                file_path.display()
            )));
        }
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
