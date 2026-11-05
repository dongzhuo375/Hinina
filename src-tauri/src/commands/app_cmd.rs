//! 应用级命令：启动标志下发 + 外链转交系统浏览器。
//!
//! 前端守卫据此做 `--settings` 的初始导航直落与登录门禁豁免，设置页据此决定
//! 分层锁。零状态命令（沿 `log_cmd::frontend_log` 先例，不取 `State<AppContext>`）
//! —— 标志在 `main()` 早期由 `launch_args::init` 固化，命令只读。
//!
//! 外链（`open_external`）只经 Rust 官方 `tauri-plugin-opener` 的 **Rust API**
//! 转交系统默认浏览器：插件注册为 Rust-only（不装 npm 包、不声明 opener
//! capability——插件的 IPC 命令虽已在 invoke handler 注册，但 webview 调用一律被
//! ACL 拒绝；且 `open_js_links_on_click` 已关闭，没有注入的点击脚本这条旁路），
//! 前端经本命令消费——WebView 永不导航离开应用（分层审计 2026-10「外链无出路」
//! 缺口的闭合）。

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
/// （插件文档明示）；插件的 IPC 命令无 capability 授权（webview 调用被 ACL
/// 拒绝），注入点击脚本也已关闭——本命令是唯一入口，非 http/https 一律拒绝。
///
/// **Windows 不存在 shell 拼接面**：插件启用 `open` crate 的
/// `shellexecute-on-windows` feature，`that_detached` 直接走 `ShellExecuteExW`
/// （URL 作为单个宽字符串参数，不经 cmd.exe；`&`/`^` 无特殊语义）。即便回退到
/// PowerShell 启动路径，目标也是经 `OPEN_RS_TARGET` 环境变量传给固定脚本，
/// crate 自带测试断言元字符不会被拼进命令文本。因此本命令只需守 scheme 白名单。
///
/// **失败信息绝不携带底层错误文本**：`tauri_plugin_opener::Error` 的
/// `ForbiddenUrl` 分支会把 URL 原样带出，io 错误文本也可能含目标；query 里可能
/// 有签名令牌，一律丢弃，只回固定文案（前端 catch 后仅记日志，无补救动作）。
#[tauri::command]
pub async fn open_external(app: AppHandle, url: String) -> AppResult<()> {
    let parsed = validate_external_url(&url).map_err(AppError::Config)?;
    app.opener()
        .open_url(parsed.as_str(), None::<&str>)
        .map_err(open_failure)?;
    // 只记 host 不记完整 URL：query 可能带签名令牌（沿 globalErrorGuard 先例）
    info!(host = ?parsed.host_str(), "open_external: 已转交系统浏览器");
    Ok(())
}

/// 打开失败 → 固定文案错误。**刻意丢弃底层错误文本**（可能含 URL/签名令牌），
/// 独立成函数以便单测锁定「错误信息不含目标地址」。
pub(crate) fn open_failure<E>(_error: E) -> AppError {
    AppError::Unknown("打开外链失败".to_string())
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
