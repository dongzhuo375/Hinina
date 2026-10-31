//! 应用级启动标志命令：把 `infra::launch_args` 的解析结果交给前端。
//!
//! 前端守卫据此做 `--settings` 的初始导航直落与登录门禁豁免，设置页据此决定
//! 分层锁。零状态命令（沿 `log_cmd::frontend_log` 先例，不取 `State<AppContext>`）
//! —— 标志在 `main()` 早期由 `launch_args::init` 固化，命令只读。

use crate::core::error::AppResult;
use crate::infra::launch_args::{self, LaunchFlags};

/// 前端 invoke 签名: `launch_flags`。
///
/// 返回**本次启动**的标志快照；IPC 失败时前端按「无参数」退化（正常门禁 + 选手层）。
#[tauri::command]
pub async fn launch_flags() -> AppResult<LaunchFlags> {
    Ok(launch_args::get())
}
