use std::fs;
use std::path::PathBuf;

use crate::core::error::AppResult;

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
        Self { base_dir }
    }

    pub fn base_dir(&self) -> &PathBuf {
        &self.base_dir
    }

    // ── 路径解析 ──

    /// 将相对路径解析为 `base_dir` 下的绝对路径，并确保不超出 `base_dir`。
    fn resolve(&self, relative_path: &str) -> AppResult<PathBuf> {
        let full_path = self.base_dir.join(relative_path);

        // 规范化路径后检查是否仍在 base_dir 内
        let canonical = full_path.canonicalize().unwrap_or_else(|_| full_path.clone());
        let base_canonical = self
            .base_dir
            .canonicalize()
            .unwrap_or_else(|_| self.base_dir.clone());

        if !canonical.starts_with(&base_canonical) {
            // 如果路径尚不存在（如 create_dir 场景），检查父目录是否在 base_dir 内
            if let Some(parent) = full_path.parent() {
                let parent_canonical =
                    parent.canonicalize().unwrap_or_else(|_| parent.to_path_buf());
                if !parent_canonical.starts_with(&base_canonical) {
                    return Err(crate::core::error::AppError::Io(format!(
                        "目录穿越攻击: {}",
                        relative_path
                    )));
                }
            } else {
                return Err(crate::core::error::AppError::Io(format!(
                    "非法路径: {}",
                    relative_path
                )));
            }
        }
        Ok(full_path)
    }

    /// 检查 `relative_path` 是否指向 `base_dir` 内的路径（用于安全检查）。
    fn is_safe(&self, relative_path: &str) -> bool {
        let full_path = self.base_dir.join(relative_path);
        // 清理 .. 和 .
        match full_path.canonicalize() {
            Ok(canonical) => {
                match self.base_dir.canonicalize() {
                    Ok(base_canonical) => canonical.starts_with(&base_canonical),
                    Err(_) => false,
                }
            }
            Err(_) => {
                // 路径不存在时，检查父目录
                if let Some(parent) = full_path.parent() {
                    match (parent.canonicalize(), self.base_dir.canonicalize()) {
                        (Ok(p), Ok(b)) => p.starts_with(&b),
                        _ => false,
                    }
                } else {
                    false
                }
            }
        }
    }

    // ── 文件操作 ──

    /// 读取文件内容（二进制）。
    ///
    /// # Errors
    /// 路径不安全或文件不存在时返回 `AppError::Io`。
    pub fn read(&self, relative_path: &str) -> AppResult<Vec<u8>> {
        let path = self.resolve(relative_path)?;
        fs::read(&path).map_err(|e| {
            crate::core::error::AppError::Io(format!("读取文件失败 {}: {}", relative_path, e))
        })
    }

    /// 读取文件内容（UTF-8 字符串）。
    ///
    /// # Errors
    /// 路径不安全、文件不存在或非 UTF-8 时返回 `AppError::Io`。
    pub fn read_to_string(&self, relative_path: &str) -> AppResult<String> {
        let path = self.resolve(relative_path)?;
        fs::read_to_string(&path).map_err(|e| {
            crate::core::error::AppError::Io(format!(
                "读取文件失败 {}: {}",
                relative_path, e
            ))
        })
    }

    /// 写入二进制数据到文件。自动创建父目录。
    ///
    /// # Errors
    /// 路径不安全或写入失败时返回 `AppError::Io`。
    pub fn write(&self, relative_path: &str, data: &[u8]) -> AppResult<()> {
        let path = self.resolve(relative_path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                crate::core::error::AppError::Io(format!(
                    "创建父目录失败 {}: {}",
                    relative_path, e
                ))
            })?;
        }
        fs::write(&path, data).map_err(|e| {
            crate::core::error::AppError::Io(format!("写入文件失败 {}: {}", relative_path, e))
        })
    }

    /// 写入字符串到文件。自动创建父目录。
    pub fn write_string(&self, relative_path: &str, content: &str) -> AppResult<()> {
        self.write(relative_path, content.as_bytes())
    }

    /// 检查文件或目录是否存在。
    pub fn exists(&self, relative_path: &str) -> bool {
        self.is_safe(relative_path) && self.base_dir.join(relative_path).exists()
    }

    /// 递归创建目录。
    ///
    /// # Errors
    /// 路径不安全或创建失败时返回 `AppError::Io`。
    pub fn create_dir(&self, relative_path: &str) -> AppResult<()> {
        let path = self.resolve(relative_path)?;
        fs::create_dir_all(&path).map_err(|e| {
            crate::core::error::AppError::Io(format!(
                "创建目录失败 {}: {}",
                relative_path, e
            ))
        })
    }

    /// 删除文件或空目录。
    ///
    /// # Errors
    /// 路径不安全或删除失败时返回 `AppError::Io`。
    pub fn remove(&self, relative_path: &str) -> AppResult<()> {
        let path = self.resolve(relative_path)?;
        if path.is_dir() {
            fs::remove_dir(&path).map_err(|e| {
                crate::core::error::AppError::Io(format!("删除目录失败 {}: {}", relative_path, e))
            })
        } else {
            fs::remove_file(&path).map_err(|e| {
                crate::core::error::AppError::Io(format!("删除文件失败 {}: {}", relative_path, e))
            })
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
                crate::core::error::AppError::Io(format!(
                    "递归删除目录失败 {}: {}",
                    relative_path, e
                ))
            })
        } else {
            fs::remove_file(&path).map_err(|e| {
                crate::core::error::AppError::Io(format!("删除文件失败 {}: {}", relative_path, e))
            })
        }
    }

    /// 列出目录下的所有条目（仅直接子项）。
    ///
    /// # Errors
    /// 路径不安全或读取失败时返回 `AppError::Io`。
    pub fn list(&self, relative_path: &str) -> AppResult<Vec<PathBuf>> {
        let path = self.resolve(relative_path)?;
        let entries: Vec<PathBuf> = fs::read_dir(&path)
            .map_err(|e| {
                crate::core::error::AppError::Io(format!(
                    "列出目录失败 {}: {}",
                    relative_path, e
                ))
            })?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .collect();
        Ok(entries)
    }
}
