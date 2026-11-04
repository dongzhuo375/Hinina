//! 应用级命令：启动标志下发 + 外链转交系统浏览器。
//!
//! 前端守卫据此做 `--settings` 的初始导航直落与登录门禁豁免，设置页据此决定
//! 分层锁。零状态命令（沿 `log_cmd::frontend_log` 先例，不取 `State<AppContext>`）
//! —— 标志在 `main()` 早期由 `launch_args::init` 固化，命令只读。
//!
//! 外链（`open_external`）只经 Rust 官方 `tauri-plugin-opener` 的 **Rust API**
//! 转交系统默认浏览器：插件注册为 Rust-only（不装 npm 包、不声明 capability，
//! 插件自身的 IPC 命令对 webview 完全不暴露），前端经本命令消费——
//! WebView 永不导航离开应用（分层审计 2026-10「外链无出路」缺口的闭合）。

use crate::core::error::{AppError, AppResult};
use crate::infra::launch_args::{self, LaunchFlags};
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;
use tracing::info;

/// 前端 invoke 签名: `launch_flags`。
///
/// 返回**本次启动**的标志快照；IPC 失败时前端按「无参数」退化（正常门禁 + 选手层）。
#[tauri::command]
pub async fn launch_flags() -> AppResult<LaunchFlags> {
    Ok(launch_args::get())
}

/// 前端 invoke 签名: `open_external`（`{ url }`）。
///
/// 把 http(s) 外链转交**系统默认浏览器**（题面/公告 markdown 里的链接）。
/// WebView 内点这类链接的默认行为是把整个客户端导航走——这里拦下转交，
/// 应用界面保持不动（分层审计 2026-10「外链无出路」缺口）。
///
/// **scheme 白名单由本命令守卫**：`Opener` 的 Rust API 不做任何 scope 校验
/// （插件文档明示），而本插件未声明 capability、插件自身的 IPC 也不暴露——
/// 本命令是唯一入口，非 http/https 一律拒绝。失败只 `Unknown` 上抛，
/// 前端 catch 后仅记日志（与窗口控制同款「无补救动作」）。
#[tauri::command]
pub async fn open_external(app: AppHandle, url: String) -> AppResult<()> {
    let parsed = validate_external_url(&url).map_err(AppError::Config)?;
    app.opener()
        .open_url(parsed.as_str(), None::<&str>)
        .map_err(|e| AppError::Unknown(format!("打开外链失败: {e}")))?;
    // 只记 host 不记完整 URL：query 可能带签名令牌（沿 globalErrorGuard 先例）
    info!(host = ?parsed.host_str(), "open_external: 已转交系统浏览器");
    Ok(())
}

/// scheme 白名单校验：只放行 http/https，返回解析后的绝对 URL。
///
/// 独立成纯函数以便单测（真实 open 不做单测——tauri mock runtime 会污染
/// 测试二进制，沿 single_instance 先例，真机人工验收）。**错误信息刻意
/// 不含原始 URL**：题面/公告的链接 query 可能带签名令牌，进日志/UI 都有泄漏面。
pub(crate) fn validate_external_url(url: &str) -> Result<url::Url, String> {
    let parsed = url::Url::parse(url).map_err(|_| "非法外链：仅支持 http/https".to_string())?;
    match parsed.scheme() {
        "http" | "https" => Ok(parsed),
        _ => Err("非法外链：仅支持 http/https".to_string()),
    }
}
