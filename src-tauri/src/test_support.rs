//! 测试专用支撑（仅 `cfg(test)` 编译）。
//!
//! **为什么需要它**：单元测试普遍用 `std::env::temp_dir()` 建独立目录来隔离
//! Storage / 缓存 / 工作区 / 服务实例。此前这些目录**只在用例开始时**清理，
//! 结束时不管 —— 长期反复跑测试的机器会持续堆积（实测累积 1700+ 个 `hinina-*`
//! 目录，其中提交与比赛服务测试各占 700 以上）。
//!
//! `TempDir` 把「结束即回收」做成 RAII：`Drop` 在用例正常返回、断言失败乃至
//! panic 展开时都会执行。调用点只需**持有**它 —— 把它绑到一个变量上，或用
//! `_dir` 与其它返回值一起接住。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// 进程内自增序号：保证 `unique` 生成的目录名互不相同。
static SEQ: AtomicUsize = AtomicUsize::new(0);

/// 测试用临时目录。构造时清掉同名残留，`Drop` 时递归删除。
pub struct TempDir(PathBuf);

impl TempDir {
    /// 固定名字的临时目录（目录名 = `full_name`）。
    ///
    /// 同一用例重复运行会复用同一路径（构造时已清残留），适合按用例命名、
    /// 需要「跑完还能按名字认出来归属」的场景。
    pub fn named(full_name: &str) -> Self {
        Self::prepare(full_name.to_string())
    }

    /// 每次调用都唯一的临时目录（目录名 = `{prefix}-{pid}-{seq}`）。
    ///
    /// 并行用例共用固定路径会相互覆盖，故「一次用例里建多个目录」的场景用这个。
    pub fn unique(prefix: &str) -> Self {
        Self::prepare(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::SeqCst)
        ))
    }

    fn prepare(full_name: String) -> Self {
        let dir = std::env::temp_dir().join(full_name);
        let _ = std::fs::remove_dir_all(&dir);
        Self(dir)
    }

    /// 目录路径。
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// 目录路径（拥有型；交给 `Storage::new` 这类需要 `PathBuf` 的构造器时用）。
    pub fn to_path_buf(&self) -> PathBuf {
        self.0.clone()
    }

    /// 目录下的子路径（等价于 `self.path().join(rel)`）。
    pub fn join<P: AsRef<Path>>(&self, rel: P) -> PathBuf {
        self.0.join(rel)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 把一个测试对象与临时目录守卫绑在一起：对象被 Drop 时目录一起回收。
///
/// 用途是让既有测试**一行不改**地获得清理能力：`Deref` 把方法调用原样转发，
/// 调用点仍按原类型使用（`service.do_x()` / `mgr.create(...)`）。相比在每处
/// `let (service, _dir) = ...` 手工接住守卫，这个写法不会因为漏写 `_dir`
/// 而静默退回「不清理」。
pub struct Guarded<T> {
    inner: T,
    dir: TempDir,
}

impl<T> Guarded<T> {
    pub fn new(inner: T, dir: TempDir) -> Self {
        Self { inner, dir }
    }

    /// 被守卫目录的路径。
    pub fn dir_path(&self) -> &Path {
        self.dir.path()
    }

    /// 取出被守卫的对象，目录随之立即回收 —— 只在后续不再需要该目录时用。
    pub fn into_inner(self) -> T {
        let Self { inner, dir } = self;
        drop(dir);
        inner
    }
}

impl<T> std::ops::Deref for Guarded<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> std::ops::DerefMut for Guarded<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 守卫必须真的在 Drop 时删掉目录 —— 否则整套改造只是换了个写法。
    #[test]
    fn drop_removes_directory() {
        let path = {
            let dir = TempDir::named("hinina-test-support-drop");
            std::fs::create_dir_all(dir.path()).unwrap();
            std::fs::write(dir.path().join("f.txt"), "x").unwrap();
            assert!(dir.path().exists());
            dir.path().to_path_buf()
        };
        assert!(!path.exists(), "守卫 Drop 后目录应被删除");
    }

    /// 构造时清理同名残留：上次异常退出留下的目录不得污染本次用例。
    #[test]
    fn construction_clears_stale_residue() {
        let stale = std::env::temp_dir().join("hinina-test-support-residue");
        std::fs::create_dir_all(&stale).unwrap();
        std::fs::write(stale.join("old.txt"), "x").unwrap();

        let dir = TempDir::named("hinina-test-support-residue");
        assert!(!dir.path().join("old.txt").exists(), "同名残留应被清掉");
    }

    #[test]
    fn unique_names_do_not_collide() {
        let a = TempDir::unique("hinina-test-support-unique");
        let b = TempDir::unique("hinina-test-support-unique");
        assert_ne!(a.path(), b.path());
    }

    /// `Guarded` 必须把方法调用原样转发，并让目录活到被守卫对象一起被 Drop。
    #[test]
    fn guarded_forwards_calls_and_owns_directory() {
        let path = {
            let guarded = Guarded::new(
                String::from("payload"),
                TempDir::named("hinina-test-support-guarded"),
            );
            std::fs::create_dir_all(guarded.dir_path()).unwrap();
            // Deref 转发：String 的方法在被守卫对象上照常可用
            assert_eq!(guarded.len(), 7);
            guarded.dir_path().to_path_buf()
        };
        assert!(!path.exists(), "Guarded 被 Drop 后目录应被删除");
    }
}
