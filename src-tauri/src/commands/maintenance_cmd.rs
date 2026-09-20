// 客户端维护 Command（设置页「重置与清理」）。
//
// 两个动作的定位刻意分开：
//
// - **重置客户端**（`reset_client`）：把客户端拉回「干净状态」—— 清掉一切**可以
//   重新从服务端获取**的东西（三层缓存 + 公告基线 + 公告已读标记）。安全、可反复点。
// - **清理本地数据**（`local_data_usage` / `purge_local_data`）：删除**不可重建**的
//   本地事实（日志内容、过期提交留档）。不可逆，故先给预览、再由用户显式勾选确认。
//
// 两者都**不动**：工作区代码（`workspaces/`，选手唯一作品本体）、配置（`config.json`）、
// 登录会话（`sessions/`，重置不该把选手踢回登录页 —— 换账号有独立的登出路径）。

use serde::Serialize;
use tauri::State;
use tracing::{info, warn};

use crate::core::context::AppContext;
use crate::core::error::AppResult;
use crate::infra::cache::CACHE_ROOT_DIR;
use crate::infra::logger::LOG_RELATIVE_PATH;
use crate::service::submission::snapshot::{self, SNAPSHOT_KEEP_DAYS};

/// 重置客户端（安全档）。
///
/// 前端 invoke 签名: `reset_client`
///
/// **清什么**：
/// - 比赛列表（内存）+ 比赛元信息（内存 + 磁盘 `cache/contest_meta/`）
/// - 题面（内存 + 磁盘 `cache/problem_statement/`）、题目 limits（内存 + 磁盘 `cache/problem_limits/`）
/// - 终态提交详情 / 测试点（内存；含源代码，属登录态数据，与登出清理同一方法）
/// - 公告基线（`announcement_baseline`）与公告已读状态（`announcements_read/`）
///
/// **刻意不清什么**（本地事实，删掉不可恢复，且不属于「可重新获取」的范畴）：
/// - 工作区代码与元数据（`workspaces/`）
/// - 提交源码快照（`submissions/`）—— OJ 不回吐代码时它是唯一的本地来源，
///   过期留档的回收走 `purge_local_data`
/// - 配置（`config.json`）、登录会话（`sessions/`）、日志（`logs/`）
///
/// 清完**不自动重拉**：何时补拉由前端决定（设置页清空后会立刻重拉当前比赛数据，
/// 否则界面会停在前端 store 的旧内存副本上）。
#[tauri::command]
pub async fn reset_client(ctx: State<'_, AppContext>) -> AppResult<()> {
    info!("Command: 重置客户端");

    ctx.contest.clear_caches();
    ctx.problem.clear_caches();
    // 用户域缓存（含源代码）与登出同一语义：换账号/重置后都不得复用上一位选手的提交内容
    ctx.submission.clear_user_caches();

    // 重置语义下「已告知过哪些公告」也该忘掉：重置后一切皆未见，留着基线没有意义
    ctx.contest.clear_announcement_baseline();
    ctx.contest.clear_announcement_read_state();

    // 兜底清扫整个缓存目录：各 Service 只清自己那部分，这里保证**将来新增的
    // namespace 也会被重置覆盖**（否则「重置」会静默漏掉新缓存，用户以为干净了）
    if ctx.storage.exists(CACHE_ROOT_DIR) {
        if let Err(e) = ctx.storage.remove_all(CACHE_ROOT_DIR) {
            warn!(error = %e, "清扫缓存目录失败（各 Service 的清理已完成）");
        }
    }

    info!("客户端重置完成");
    Ok(())
}

/// 可清理的本地数据占用（清理前的预览，供界面展示「将删除什么」）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalDataUsage {
    /// 日志文件字节数
    pub log_bytes: u64,
    /// 日志文件完整路径（展示用）
    pub log_path: String,
    /// 提交留档：总条数 / 总字节数 / 其中过期的条数与字节数
    pub snapshot_total_count: usize,
    pub snapshot_total_bytes: u64,
    pub snapshot_stale_count: usize,
    pub snapshot_stale_bytes: u64,
    /// 留档保留窗口（天）：早于该窗口的留档视为过期（判据同 [`purge_local_data`]）
    pub keep_days: u64,
}

/// 统计可清理的本地数据。
///
/// 前端 invoke 签名: `local_data_usage`
///
/// 存在的意义是**先看再删**：不可逆的删除动作必须让用户看到确切的范围与体积，
/// 而不是点下去才知道删了什么。
#[tauri::command]
pub async fn local_data_usage(ctx: State<'_, AppContext>) -> AppResult<LocalDataUsage> {
    let log_path = ctx.storage.base_dir().join(LOG_RELATIVE_PATH);
    let log_bytes = std::fs::metadata(&log_path).map(|m| m.len()).unwrap_or(0);
    let usage = snapshot::inspect_snapshots(&ctx.storage, SNAPSHOT_KEEP_DAYS);

    Ok(LocalDataUsage {
        log_bytes,
        log_path: log_path.to_string_lossy().into_owned(),
        snapshot_total_count: usage.total_count,
        snapshot_total_bytes: usage.total_bytes,
        snapshot_stale_count: usage.stale_count,
        snapshot_stale_bytes: usage.stale_bytes,
        keep_days: SNAPSHOT_KEEP_DAYS,
    })
}

/// 清理结果。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurgeReport {
    /// 释放的总字节数
    pub freed_bytes: u64,
    /// 删除的留档条数
    pub removed_snapshots: usize,
    /// 日志内容是否**确实**被清空。
    ///
    /// `false` 有三种成因：文件层不可用（磁盘只读）、截断失败、或本次没勾选日志。
    /// 界面据此如实提示，而不是笼统报「已清理」。
    pub log_cleared: bool,
}

/// 逐项容错地执行清理（**两个勾选项互不牵连**）。
///
/// 抽成纯函数是为了可单测（命令本体依赖 `tauri::State`，测试里拿不到）——
/// 而「一项失败不该连带取消另一项」正是这里最需要锁定的行为。
///
/// 日志清理失败（文件被占用、权限不足）时**降级为 `log_cleared=false` 并继续**，
/// 而不是 `?` 冒泡中断整个命令：用户勾了留档清理，就该拿到留档清理的结果。
pub(crate) fn run_purge(
    logs: bool,
    stale_snapshots: bool,
    clear_log: impl FnOnce() -> AppResult<(u64, bool)>,
    purge_snapshots: impl FnOnce() -> (usize, u64),
) -> PurgeReport {
    let mut report = PurgeReport::default();

    if logs {
        match clear_log() {
            Ok((freed, cleared)) => {
                report.freed_bytes += freed;
                report.log_cleared = cleared;
                info!(freed_bytes = freed, log_cleared = cleared, "日志内容已清空");
            }
            Err(e) => warn!(error = %e, "清空日志内容失败，继续执行其余清理项"),
        }
    }

    if stale_snapshots {
        let (removed, freed) = purge_snapshots();
        report.removed_snapshots = removed;
        report.freed_bytes += freed;
    }

    report
}

/// 清理本地数据（**不可逆**）。
///
/// 前端 invoke 签名: `purge_local_data`({ logs, staleSnapshots })
///
/// - `logs`：清空日志**内容**（不删文件 —— 删了要等重启才重建，中间这段排障信息
///   就彻底没了；用「重开 + 截断」而非句柄 `set_len`，原因见 `infra/logger.rs`）
/// - `staleSnapshots`：删除早于 `SNAPSHOT_KEEP_DAYS` 的提交源码留档
///
/// **逐项容错**：任何一项失败都不影响另一项（见 [`run_purge`]），故本命令只在
/// 参数层面之外几乎不会失败 —— `log_cleared=false` 如实表示「日志没被清掉」。
/// 两个开关都关时是 no-op（前端不会这么调，此处仍如实处理）。
/// **不动**工作区代码、配置、会话与公告已读状态。
#[tauri::command]
pub async fn purge_local_data(
    ctx: State<'_, AppContext>,
    logs: bool,
    stale_snapshots: bool,
) -> AppResult<PurgeReport> {
    info!(logs, stale_snapshots, "Command: 清理本地数据");

    let report = run_purge(
        logs,
        stale_snapshots,
        // 文件层不可用（磁盘只读等）时 clear_log_file 返回 Ok(0)：此时并没有清掉
        // 任何东西，故用 has_log_file 如实回报「是否真的清了」
        || {
            ctx.logger
                .clear_log_file()
                .map(|freed| (freed, ctx.logger.has_log_file()))
        },
        || snapshot::purge_stale_snapshots(&ctx.storage, SNAPSHOT_KEEP_DAYS),
    );

    Ok(report)
}
