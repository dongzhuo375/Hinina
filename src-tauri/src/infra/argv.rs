//! 启动参数解析的共享原语。
//!
//! `console::requested`（`--console`）与 `launch_args::requested`（`--settings`）
//! 曾各自内联同一套扫描逻辑——注释声称「语义必须一致」，却没有任何机制保证。
//! 收敛到这里：**精确整参匹配、其余参数静默忽略、`OsString`
//! 入参与非 Unicode 参数共存**，新参数只加一行 `has_flag` 调用。

use std::ffi::OsString;

/// argv（**不含 argv[0]**，调用方负责 `skip(1)`）中是否存在与 `flag` 完全相同的参数。
///
/// 精确匹配：`--settings=x`、`-settings` 等近形一律不算；未知参数静默忽略（不能让
/// 未知参数把启动炸掉）。入参用 `OsString`：`std::env::args()` 遇非 Unicode 参数
/// 会 panic，`args_os()` 不会。
pub(crate) fn has_flag(args: &[OsString], flag: &str) -> bool {
    // OsString: PartialEq<str>：与非 Unicode 参数共存也不会 panic
    args.iter().any(|arg| arg == flag)
}

#[cfg(test)]
#[path = "tests/argv_tests.rs"]
mod tests;
