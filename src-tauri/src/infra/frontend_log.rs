//! 前端日志落 Rust tracing（panic 钩子的前端半边）。
//!
//! `commands/log_cmd.rs` 的 `frontend_log` 是薄转发，收敛与落盘逻辑全部在此
//! （命令层零业务逻辑，CLAUDE.md「thin #[tauri::command] handlers」；落位与
//! 写法沿 `plugin_diagnostics`）。
//!
//! **安全口径**：scope 白名单化、message 长度与控制字符收敛 —— 该通道把前端
//! 世界（页面脚本、OJ 页面注入的 DOM、用户输入拼接的错误文案）写进本地日志，
//! 必须防日志伪造与无限膨胀；**不设载荷参数**，前端只传 level/scope/message
//! 三个标量，敏感面从一开始就不存在。
//!
//! 经 `tracing` 落现有 logger（stderr + `logs/hinina.log`，受 `RUST_LOG` 过滤
//! —— 与 panic 的 bootstrap 文件不同，这是**尽力而为**的通道：日志关闭时前端
//! 错误不留痕，但全局守卫本身不因转发失败而受影响）。

use std::sync::atomic::{AtomicU64, Ordering};

use super::text::truncate_ellipsis;

/// 单条前端日志的长度上限（字符数）。超长截断并加省略标记 —— 防御性上限，
/// 正常错误消息远小于此；栈帧可能很长，2000 字符足够定位又不至于把日志文件
/// 撑成单条巨行。
pub(crate) const MAX_MESSAGE_CHARS: usize = 2000;

/// 作用域长度上限。合法 scope 由前端 `createLogger` 的模块名产生（如
/// `workspaceStore`），64 字符远超实际用量。
const MAX_SCOPE_CHARS: usize = 64;

/// 会话级字节预算：`frontend_log` 全程最多向日志写入这么多**消息字节**。
///
/// 去重在前端按「完全相同的消息」进行，渲染循环里带变化计数器的消息每次都是
/// 新串 —— 没有预算时一次循环就能以每条 2000 字符的速度刷满日志（5 MiB 上限
/// 只在启动时生效）。预算按进程计（一次性，不随时间恢复）：64 KiB 对真实错误
/// 绰绰有余（多数消息 < 200 字符），又把最坏失控压到约 30 条量级。预算耗尽后
/// 静默丢弃并在**第一次丢弃时**告警一行（状态迁移值得看见，后续刷屏不值得）。
const MAX_SESSION_BYTES: u64 = 64 * 1024;

static BUDGET_REMAINING: AtomicU64 = AtomicU64::new(MAX_SESSION_BYTES);
static BUDGET_DROPPED: AtomicU64 = AtomicU64::new(0);

/// 前端日志入口（由 `commands/log_cmd.rs` 薄转发调用）。
pub(crate) fn emit(level: &str, scope: &str, message: &str) {
    let scope = sanitize_scope(scope);
    let message = sanitize_message(message);
    if !consume_budget(message.len() as u64) {
        return;
    }
    match sanitize_level(level) {
        "error" => tracing::error!(target: "frontend", scope = %scope, "{message}"),
        "info" => tracing::info!(target: "frontend", scope = %scope, "{message}"),
        "debug" => tracing::debug!(target: "frontend", scope = %scope, "{message}"),
        // 未知级别按 warn 落：可见但不制造 error 噪音
        _ => tracing::warn!(target: "frontend", scope = %scope, "{message}"),
    }
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
/// 终端转义注入），超长按字符截断加 `…`（中文不炸边界，共享 `text` 工具）。
fn sanitize_message(message: &str) -> String {
    let owned: String = message
        .chars()
        .map(|c| {
            if c.is_control() && c != '\n' && c != '\t' {
                ' '
            } else {
                c
            }
        })
        .collect();
    truncate_ellipsis(&owned, MAX_MESSAGE_CHARS)
}

/// 扣减会话预算；预算耗尽返回 `false`（消息被丢弃）。
fn consume_budget(bytes: u64) -> bool {
    let mut current = BUDGET_REMAINING.load(Ordering::Relaxed);
    loop {
        if current == 0 {
            let dropped = BUDGET_DROPPED.fetch_add(1, Ordering::Relaxed) + 1;
            if dropped == 1 {
                // 只在第一次丢弃时告警：预算已尽是状态迁移，值得一行；后续静默
                tracing::warn!(
                    target: "frontend",
                    budget_bytes = MAX_SESSION_BYTES,
                    "前端日志预算已用尽，本会话内后续消息不再落盘"
                );
            }
            return false;
        }
        let next = current.saturating_sub(bytes);
        match BUDGET_REMAINING.compare_exchange(current, next, Ordering::Relaxed, Ordering::Relaxed)
        {
            Ok(_) => return true,
            // 并发扣减失败：用最新值重试（CAS 循环）
            Err(actual) => current = actual,
        }
    }
}

#[cfg(test)]
/// 预算静态量按测试重置（仅测试编译；生产入口不暴露）。
pub(crate) fn set_budget_for_tests(remaining: u64) {
    BUDGET_REMAINING.store(remaining, Ordering::Relaxed);
    BUDGET_DROPPED.store(0, Ordering::Relaxed);
}

#[cfg(test)]
#[path = "tests/frontend_log_tests.rs"]
mod tests;
