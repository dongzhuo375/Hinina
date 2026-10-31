use tracing::error as tracing_error;

use crate::core::error::AppResult;

/// 单条前端日志的长度上限（字符数）。超长截断并加省略标记 —— 防御性上限，
/// 正常错误消息远小于此；栈帧可能很长，2000 字符足够定位又不至于把日志文件
/// 撑成单条巨行。
const MAX_MESSAGE_CHARS: usize = 2000;

/// 作用域长度上限。合法 scope 由前端 `createLogger` 的模块名产生（如
/// `workspaceStore`），64 字符远超实际用量。
const MAX_SCOPE_CHARS: usize = 64;

/// 前端日志落 Rust tracing（panic 钩子的前端半边）。
///
/// 前端 invoke 签名: `frontend_log`({ level, scope, message })
///
/// **安全口径沿 `infra/plugin_diagnostics`**：scope 白名单化、message 只做长度
/// 与控制字符收敛 —— 该命令把前端世界（页面脚本、OJ 页面注入的 DOM、用户输入
/// 拼接的错误文案）写进本地日志，必须防日志伪造与无限膨胀；**不新增载荷参数**，
/// 前端不传 payload/args，敏感面从一开始就不存在。
///
/// 经 `tracing` 落现有 logger（stderr + `logs/hinina.log`，受 `RUST_LOG` 过滤
/// —— 与 panic 的 bootstrap 文件不同，这是**尽力而为**的通道：日志关闭时前端
/// 错误不留痕，但全局守卫本身不因转发失败而受影响）。
#[tauri::command]
pub async fn frontend_log(level: String, scope: String, message: String) -> AppResult<()> {
    let scope = sanitize_scope(&scope);
    let message = sanitize_message(&message);
    match sanitize_level(&level) {
        "error" => tracing_error!(target: "frontend", scope = %scope, "{message}"),
        "info" => tracing::info!(target: "frontend", scope = %scope, "{message}"),
        "debug" => tracing::debug!(target: "frontend", scope = %scope, "{message}"),
        // 未知级别按 warn 落：可见但不制造 error 噪音
        _ => tracing::warn!(target: "frontend", scope = %scope, "{message}"),
    }
    Ok(())
}

/// 级别白名单：只认 `error` / `warn` / `info` / `debug`，其余一律按 `warn`。
fn sanitize_level(level: &str) -> &'static str {
    match level {
        "error" => "error",
        "warn" => "warn",
        "info" => "info",
        "debug" => "debug",
        _ => "warn",
    }
}

/// 作用域白名单：`[A-Za-z0-9._-]` 且 ≤ 64 字符，否则 `"<invalid>"`。
/// 同 `plugin_diagnostics::safe_id`：日志里只允许出现可归因的受控标识。
fn sanitize_scope(scope: &str) -> &str {
    let valid = !scope.is_empty()
        && scope.len() <= MAX_SCOPE_CHARS
        && scope
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-');
    if valid {
        scope
    } else {
        "<invalid>"
    }
}

/// 消息收敛：保留换行/制表（栈可读），剥掉其它控制字符（防 `\r` 伪造行首、
/// 终端转义注入），超长截断加 `…`。按字符而非字节截断，中文消息不炸边界。
fn sanitize_message(message: &str) -> String {
    let mut owned: String = message
        .chars()
        .map(|c| {
            if c.is_control() && c != '\n' && c != '\t' {
                ' '
            } else {
                c
            }
        })
        .collect();
    if owned.chars().count() > MAX_MESSAGE_CHARS {
        owned = owned.chars().take(MAX_MESSAGE_CHARS).collect();
        owned.push('…');
    }
    owned
}

#[cfg(test)]
#[path = "tests/log_cmd_tests.rs"]
mod tests;
