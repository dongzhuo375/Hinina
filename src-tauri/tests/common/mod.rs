//! 测试共用：把进程的临时目录重定向到用例专属沙箱。
//!
//! panic 报告的持久落点固定在 `std::env::temp_dir()` 下的 `hinina-panic.log`。
//! 测试**不得**读写或清理真实临时目录里的这个文件——开发机上它可能保存着
//! 真实崩溃记录，删除不可逆。每个测试二进制独占进程，因此在用例开始时把
//! `TMPDIR` / `TMP` / `TEMP` 指向沙箱即可让被测代码只看到沙箱；`Drop` 时
//! 恢复原环境并清理。

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static SEQ: AtomicU64 = AtomicU64::new(0);

pub struct SandboxTemp {
    dir: PathBuf,
    previous: Vec<(&'static str, Option<OsString>)>,
}

impl SandboxTemp {
    /// 创建沙箱并重定向临时目录环境变量（必须在调用 `std::env::temp_dir()`
    /// 的被测代码之前构造）。
    pub fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "hinina-test-sandbox-{tag}-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
            SEQ.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir_all(&dir).expect("创建沙箱临时目录失败");

        let keys = ["TMPDIR", "TMP", "TEMP"];
        let previous = keys
            .iter()
            .map(|key| (*key, std::env::var_os(key)))
            .collect();
        for key in keys {
            std::env::set_var(key, &dir);
        }
        Self { dir, previous }
    }

    /// 沙箱内路径（等价于沙箱根下的相对路径；沙箱根即进程此刻的
    /// `std::env::temp_dir()`）。
    pub fn join(&self, rel: &str) -> PathBuf {
        self.dir.join(rel)
    }
}

impl Drop for SandboxTemp {
    fn drop(&mut self) {
        for (key, value) in &self.previous {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
