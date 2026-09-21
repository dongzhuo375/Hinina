use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::core::error::{AppError, AppResult};

/// 原子写的临时文件序号：同进程内保证 tmp 文件名不冲突。
static ATOMIC_WRITE_SEQ: AtomicU64 = AtomicU64::new(0);

/// 文件名是否为 `target` 的原子写临时残片（`{target}.{数字}.tmp`）。
fn is_atomic_tmp(target: &str, name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".tmp") else {
        return false;
    };
    let Some(rest) = stem.strip_prefix(target) else {
        return false;
    };
    // rest 形如 `.{seq}`：以 `.` 起头且其后全为数字（空串不匹配）
    rest.len() > 1 && rest.starts_with('.') && rest[1..].bytes().all(|b| b.is_ascii_digit())
}

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

    /// 原子写入字符串到文件：先写临时文件、fsync 后 `rename` 覆盖目标。自动创建父目录。
    ///
    /// [`Self::write_string`] 是「截断 + 就地写」：进程在写入中途崩溃会留下半截
    /// 文件，下次读取只能当作损坏处理。凭据等「重启后必须可恢复」的数据不能
    /// 承受这一点。本方法先写 `{path}.{seq}.tmp` 并 `sync_all`，再 rename
    /// （同目录同卷，Windows 上 `std::fs::rename` 以 `MOVEFILE_REPLACE_EXISTING`
    /// 原子替换）：
    ///
    /// - **进程崩溃**：rename 是原子替换，目标要么是旧内容、要么是新内容；
    /// - **掉电**：数据已 fsync，但 rename 的目录项未额外 fsync（Windows 上需
    ///   `FILE_FLAG_BACKUP_SEMANTICS` 打开目录句柄，此处不做）—— 日志型文件
    ///   系统上最坏回退为**旧的完整文件**，同样不会出现半截。
    ///
    /// 每次写入前先清理同目标的临时残片（崩溃落在「写完临时文件」与「rename」
    /// 之间时会永久残留，rename 成功路径不会经过清理分支）—— 与
    /// `data_dir::move_entry` 清暂存同一模式。并发对**同一路径**的原子写可能
    /// 互相清掉对方的在途临时文件（表现为其中一方收到 Io 错误，不会损坏数据）；
    /// 需要并发安全时由调用方自行串行化（如 `FsSessionRepository` 的变更锁）。
    ///
    /// # Errors
    /// 路径不安全、临时文件写入/刷盘失败或改名失败时返回 `AppError::Io`。
    pub fn write_string_atomic(&self, relative_path: &str, content: &str) -> AppResult<()> {
        let path = self.resolve(relative_path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                AppError::Io(format!("创建父目录失败 {}: {}", relative_path, e))
            })?;
        }
        self.remove_stale_temps(&path);

        let seq = ATOMIC_WRITE_SEQ.fetch_add(1, Ordering::Relaxed);
        let mut tmp = path.clone().into_os_string();
        tmp.push(format!(".{seq}.tmp"));
        let tmp_path = PathBuf::from(tmp);

        let write_tmp = |e: std::io::Error| {
            let _ = fs::remove_file(&tmp_path); // best-effort 清理
            AppError::Io(format!("写入临时文件失败 {}: {}", relative_path, e))
        };
        let mut file = fs::File::create(&tmp_path).map_err(write_tmp)?;
        file.write_all(content.as_bytes()).map_err(write_tmp)?;
        // fsync 后再改名：保证 rename 生效时数据已在盘上（掉电语义见方法文档）
        file.sync_all().map_err(write_tmp)?;
        drop(file); // Windows 上 rename 前须先关闭句柄
        fs::rename(&tmp_path, &path).map_err(|e| {
            let _ = fs::remove_file(&tmp_path); // best-effort 清理
            AppError::Io(format!("原子替换文件失败 {}: {}", relative_path, e))
        })?;
        Ok(())
    }

    /// 删除 `target` 同前缀的原子写临时残片（`{target}.{数字}.tmp`）。
    ///
    /// 只认本模块的命名约定，不误伤其他文件；单个删除失败只跳过
    /// （清理失败不该让写入本身失败）。
    fn remove_stale_temps(&self, target: &Path) {
        let Some(dir) = target.parent() else { return };
        let Some(prefix) = target.file_name().and_then(|n| n.to_str()) else {
            return;
        };
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if is_atomic_tmp(prefix, &name) {
                let _ = fs::remove_file(entry.path());
            }
        }
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
#[path = "tests/storage_tests.rs"]
mod tests;
