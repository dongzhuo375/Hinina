//! `--settings` 启动参数（管理方钥匙）：携带该参数启动时跳过登录门禁直落设置页，
//! 且设置页的维护者层字段解锁。
//!
//! 死锁场景：配置指向连不上的服务器 → 登录失败 → 进不了设置 → 改不了地址。
//! `--settings` 让管理方直接进入设置页修复连接配置；正常路径（含连点版本号的
//! 隐藏入口）只开选手层。命名取「打开设置页」——它不设置任何值，区别于
//! `--config` 那个会让人以为要传配置的误称。
//!
//! 管道：`main()` 收集 argv 后 `init()` 一次性固化进 OnceLock →
//! `commands/app_cmd.rs` 的 `launch_flags` 命令把它交给前端（守卫据此做初始
//! 导航与门禁豁免、设置页据此决定分层锁）。定位是**防误触护栏而非安全边界**：
//! config.json 明文可手改，前端锁不设后端强制。
//!
//! 解析语义与 `console::requested` 一致：精确匹配（`--settings=x`、`-settings`
//! 等一律不算），其余参数静默忽略；`OsString` 入参与非 Unicode 参数共存。

use std::sync::OnceLock;

use serde::Serialize;

/// 启动标志（按需扩展字段；未出现的语义一律按 false）。
#[derive(Debug, Clone, Copy, Serialize)]
pub struct LaunchFlags {
    /// `--settings`：管理方模式（初始直落设置页 + 维护者层解锁）。
    pub settings: bool,
}

static FLAGS: OnceLock<LaunchFlags> = OnceLock::new();

/// argv（**不含 argv[0]**，调用方负责 `skip(1)`）是否请求设置页。
///
/// 精确匹配 `--settings`：`--settings=x`、`-settings` 等一律不算；其余参数
/// 静默忽略，留给未来扩展（避免未知参数把启动炸掉）。入参用 `OsString`：
/// `std::env::args()` 遇非 Unicode 参数会 panic，`args_os()` 不会。
fn requested(args: &[std::ffi::OsString]) -> bool {
    // OsString: PartialEq<str>：与非 Unicode 参数共存也不会 panic
    args.iter().any(|arg| arg == "--settings")
}

/// `main()` 在收集 argv 后调用一次，固化启动标志。
///
/// 重复调用不生效（OnceLock 首次定型后忽略后续 set）——生产入口只调一次。
pub fn init(args: &[std::ffi::OsString]) {
    let _ = FLAGS.set(LaunchFlags {
        settings: requested(args),
    });
}

/// 读取启动标志。`main` 尚未 `init`（测试进程不经过 main）时返回全 false 的
/// 默认值 —— 与「无参数启动」同义，绝不 panic。**非提交式读取**：不能在这里
/// `get_or_init` 写入默认值，否则会把 OnceLock 提前定型、让随后真正的 `init`
/// 静默失效（测试抓住过）。
pub fn get() -> LaunchFlags {
    FLAGS
        .get()
        .copied()
        .unwrap_or(LaunchFlags { settings: false })
}

#[cfg(test)]
#[path = "tests/launch_args_tests.rs"]
mod tests;
