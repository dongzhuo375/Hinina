use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::core::error::{AppError, AppResult};

/// 本地文件存储工具。
///
/// 提供底层的文件读写能力，不包含业务语义。
/// 所有路径相对于 `base_dir`，防止目录穿越攻击。
/// 业务层应通过 Repository trait 间接使用，不要直接依赖此模块。
pub struct Storage {
    base_dir: PathBuf,
}

impl Storage {
    pub fn new(base_dir: PathBuf) -> Self {
        // 确保存储根目录存在
        let _ = fs::create_dir_all(&base_dir);
        Self { base_dir }
    }

    pub fn base_dir(&self) -> &PathBuf {
        &self.base_dir
    }

    // ── 路径解析 ──

    /// 将相对路径解析为 `base_dir` 下的绝对路径，并拒绝任何 `..` 越权路径。
    ///
    /// 纯逻辑检查（遍历 `Path::components()`），不依赖文件系统状态，
    /// 因此对尚不存在的路径也能正确判断。
    fn resolve(&self, relative_path: &str) -> AppResult<PathBuf> {
        let path = Path::new(relative_path);
        for component in path.components() {
            if matches!(component, Component::ParentDir) {
                return Err(AppError::Io(format!("目录穿越攻击: {}", relative_path)));
            }
        }
        Ok(self.base_dir.join(relative_path))
    }

    // ── 文件操作 ──

    /// 读取文件内容（二进制）。
    ///
    /// # Errors
    /// 路径不安全或文件不存在时返回 `AppError::Io`。
    pub fn read(&self, relative_path: &str) -> AppResult<Vec<u8>> {
        let path = self.resolve(relative_path)?;
        fs::read(&path)
            .map_err(|e| AppError::Io(format!("读取文件失败 {}: {}", relative_path, e)))
    }

    /// 读取文件内容（UTF-8 字符串）。
    ///
    /// # Errors
    /// 路径不安全、文件不存在或非 UTF-8 时返回 `AppError::Io`。
    pub fn read_to_string(&self, relative_path: &str) -> AppResult<String> {
        let path = self.resolve(relative_path)?;
        fs::read_to_string(&path)
            .map_err(|e| AppError::Io(format!("读取文件失败 {}: {}", relative_path, e)))
    }

    /// 写入二进制数据到文件。自动创建父目录。
    ///
    /// # Errors
    /// 路径不安全或写入失败时返回 `AppError::Io`。
    pub fn write(&self, relative_path: &str, data: &[u8]) -> AppResult<()> {
        let path = self.resolve(relative_path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                AppError::Io(format!("创建父目录失败 {}: {}", relative_path, e))
            })?;
        }
        fs::write(&path, data)
            .map_err(|e| AppError::Io(format!("写入文件失败 {}: {}", relative_path, e)))
    }

    /// 写入字符串到文件。自动创建父目录。
    pub fn write_string(&self, relative_path: &str, content: &str) -> AppResult<()> {
        self.write(relative_path, content.as_bytes())
    }

    /// 检查文件或目录是否存在。
    pub fn exists(&self, relative_path: &str) -> bool {
        self.resolve(relative_path)
            .map(|p| p.exists())
            .unwrap_or(false)
    }

    /// 递归创建目录。
    ///
    /// # Errors
    /// 路径不安全或创建失败时返回 `AppError::Io`。
    pub fn create_dir(&self, relative_path: &str) -> AppResult<()> {
        let path = self.resolve(relative_path)?;
        fs::create_dir_all(&path)
            .map_err(|e| AppError::Io(format!("创建目录失败 {}: {}", relative_path, e)))
    }

    /// 删除文件或空目录。
    ///
    /// # Errors
    /// 路径不安全或删除失败时返回 `AppError::Io`。
    pub fn remove(&self, relative_path: &str) -> AppResult<()> {
        let path = self.resolve(relative_path)?;
        if path.is_dir() {
            fs::remove_dir(&path)
                .map_err(|e| AppError::Io(format!("删除目录失败 {}: {}", relative_path, e)))
        } else {
            fs::remove_file(&path)
                .map_err(|e| AppError::Io(format!("删除文件失败 {}: {}", relative_path, e)))
        }
    }

    /// 删除文件或目录（递归删除非空目录）。
    ///
    /// # Errors
    /// 路径不安全或删除失败时返回 `AppError::Io`。
    pub fn remove_all(&self, relative_path: &str) -> AppResult<()> {
        let path = self.resolve(relative_path)?;
        if path.is_dir() {
            fs::remove_dir_all(&path).map_err(|e| {
                AppError::Io(format!("递归删除目录失败 {}: {}", relative_path, e))
            })
        } else {
            fs::remove_file(&path)
                .map_err(|e| AppError::Io(format!("删除文件失败 {}: {}", relative_path, e)))
        }
    }

    /// 列出目录下的所有条目（仅直接子项），返回相对 base_dir 的路径。
    ///
    /// # Errors
    /// 路径不安全或读取失败时返回 `AppError::Io`。
    pub fn list(&self, relative_path: &str) -> AppResult<Vec<PathBuf>> {
        let path = self.resolve(relative_path)?;
        let entries: Vec<PathBuf> = fs::read_dir(&path)
            .map_err(|e| AppError::Io(format!("列出目录失败 {}: {}", relative_path, e)))?
            .filter_map(|entry| {
                entry.ok().and_then(|e| {
                    e.path()
                        .strip_prefix(&self.base_dir)
                        .ok()
                        .map(|p| p.to_path_buf())
                })
            })
            .collect();
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn storage() -> Storage {
        Storage::new(PathBuf::from("/tmp/hinina-test"))
    }

    #[test]
    fn resolve_normal_path() {
        let s = storage();
        assert!(s.resolve("foo/bar/baz").is_ok());
        assert!(s.resolve("a").is_ok());
        assert!(s.resolve("a/b/./c").is_ok());
    }

    #[test]
    fn resolve_rejects_parent_dir() {
        let s = storage();
        assert!(s.resolve("..").is_err());
        assert!(s.resolve("../etc").is_err());
        assert!(s.resolve("foo/../../../etc/passwd").is_err());
        assert!(s.resolve("a/../b/../c").is_err());
    }

    #[test]
    fn resolve_deep_nested() {
        let s = storage();
        assert!(s.resolve("a/b/c/d/e/f").is_ok());
    }
}
