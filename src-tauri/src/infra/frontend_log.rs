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

/// 滚动窗口字节预算：**每个 60 秒滚动窗口**内 `frontend_log` 最多向日志写入这么多
/// **消息字节**，窗口滚动即重置。
///
/// 去重在前端按「完全相同的消息」进行，渲染循环里带变化计数器的消息每次都是
/// 新串 —— 没有预算时持续循环可刷出 MB 级日志（2000 字符的消息按 UTF-8 编码
/// 最大约 8 KB/条——4 字节/码点，与「每条 2000 字节」差 4 倍，勿混用两种
/// 口径）。窗口制
/// （而非进程一次性总量）：失控循环被压到每窗口 64 KiB（持续刷屏最坏约
/// 3.8 MB/小时，对照 5 MiB 启动轮转），而**后续窗口里的真实错误恢复预算**，
/// 不会因早期的失控一次性殃及整个会话。每个耗尽的窗口只在**第一次丢弃时**
/// 告警一行（状态迁移值得看见，后续静默计数）。
const BUDGET_WINDOW_BYTES: u64 = 64 * 1024;
const BUDGET_WINDOW_SECS: u64 = 60;

static WINDOW_START: AtomicU64 = AtomicU64::new(0);
static WINDOW_BYTES: AtomicU64 = AtomicU64::new(0);
static WINDOW_DROPPED: AtomicU64 = AtomicU64::new(0);

/// 预算判定结果。`Drop.first` 标记本窗口第一次丢弃（驱动那一行告警）。
enum BudgetDecision {
    Allow,
    Drop { first: bool },
}

/// 前端日志入口（由 `commands/log_cmd.rs` 薄转发调用）。
pub(crate) fn emit(level: &str, scope: &str, message: &str) {
    let scope = sanitize_scope(scope);
    let message = sanitize_message(message);
    match consume_budget(message.len() as u64) {
        BudgetDecision::Allow => {}
        BudgetDecision::Drop { first: true } => {
            // 本窗口首次超限：只发那一行状态迁移告警，**消息本身照常丢弃**
            //（缺 return 会把首条超预算消息泄漏进日志，评审抓出）
            tracing::warn!(
                target: "frontend",
                budget_bytes = BUDGET_WINDOW_BYTES,
                budget_window_secs = BUDGET_WINDOW_SECS,
                "前端日志预算已用尽（本窗口内后续消息丢弃，下一窗口恢复）"
            );
            return;
        }
        BudgetDecision::Drop { first: false } => return,
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

/// 窗口过期则滚动（重置记账），随后扣减本窗口用量。
fn consume_budget(bytes: u64) -> BudgetDecision {
    roll_window_if_expired(unix_secs());
    let mut used = WINDOW_BYTES.load(Ordering::Relaxed);
    loop {
        let next = used.saturating_add(bytes);
        if next > BUDGET_WINDOW_BYTES {
            let dropped = WINDOW_DROPPED.fetch_add(1, Ordering::Relaxed) + 1;
            return BudgetDecision::Drop {
                first: dropped == 1,
            };
        }
        match WINDOW_BYTES.compare_exchange(used, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return BudgetDecision::Allow,
            // 并发扣减失败：用最新值重试（CAS 循环）
            Err(actual) => used = actual,
        }
    }
}

/// 窗口起点已过期（距上次滚动 ≥ 60s）则把起点推到当前时间并清零记账。
/// CAS 失败 = 别的线程刚滚动过：字节清零由它负责，这里不重复（重复清零会
/// 复活已耗尽的预算）。竞态窗口内的记账是近似值 —— 预算是**闸门**而非精确计量。
fn roll_window_if_expired(now: u64) {
    let start = WINDOW_START.load(Ordering::Relaxed);
    if now.saturating_sub(start) >= BUDGET_WINDOW_SECS
        && WINDOW_START
            .compare_exchange(start, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
    {
        WINDOW_BYTES.store(0, Ordering::Relaxed);
        WINDOW_DROPPED.store(0, Ordering::Relaxed);
    }
}

fn unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
/// 预算静态量按测试重置（仅测试编译；生产入口不暴露）。
///
/// `seconds_ago` 决定窗口起点距现在的秒数：0 = 新窗口（不滚动），
/// `BUDGET_WINDOW_SECS` = 已过期（下一次 consume 先滚动清账）。
pub(crate) fn set_budget_for_tests(seconds_ago: u64, used: u64, dropped: u64) {
    WINDOW_START.store(unix_secs().saturating_sub(seconds_ago), Ordering::Relaxed);
    WINDOW_BYTES.store(used, Ordering::Relaxed);
    WINDOW_DROPPED.store(dropped, Ordering::Relaxed);
}

#[cfg(test)]
/// 快照（本窗口已用字节, 本窗口丢弃条数），供测试断言内部记账。
pub(crate) fn budget_snapshot_for_tests() -> (u64, u64) {
    (
        WINDOW_BYTES.load(Ordering::Relaxed),
        WINDOW_DROPPED.load(Ordering::Relaxed),
    )
}

#[cfg(test)]
#[path = "tests/frontend_log_tests.rs"]
mod tests;
