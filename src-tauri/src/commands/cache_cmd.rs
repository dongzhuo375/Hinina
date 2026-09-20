// 缓存维护 Command。
//
// 「清空缓存」是设置页的一个维护动作（入口默认隐藏，见 `src/utils/settings-access.ts`），
// 清的是**可以随时从服务端重新获取**的数据，不动任何本地事实。

use tauri::State;
use tracing::info;

use crate::core::context::AppContext;
use crate::core::error::AppResult;

/// 清空客户端缓存。
///
/// 前端 invoke 签名: `clear_cache`
///
/// **清什么**（三层，均按 OJ / 比赛维度分键，清空只是让当前会话立刻回到干净状态）：
/// - 比赛列表（内存）+ 比赛元信息（内存 + 磁盘 `cache/contest_meta/`）
/// - 题面（内存 + 磁盘 `cache/problem_statement/`）、题目 limits（内存 + 磁盘 `cache/problem_limits/`）
/// - 终态提交详情 / 测试点（内存；含源代码，属登录态数据，与登出清理同一方法）
///
/// **刻意不清什么**（本地事实，删掉不可恢复，且不属于「缓存」）：
/// - 工作区代码与元数据（`workspaces/`）
/// - 提交源码快照（`submissions/`）—— OJ 不回吐代码时它是唯一的本地来源
/// - 公告已读状态（`announcements_read/`）、配置（`config.json`）、日志（`logs/`）
///
/// 清完**不自动重拉**：何时补拉由前端决定（设置页清空后会立刻重拉当前比赛数据，
/// 否则界面会停在旧值上 —— 旧值来自前端 store 的内存副本，不在本次清理范围内）。
#[tauri::command]
pub async fn clear_cache(ctx: State<'_, AppContext>) -> AppResult<()> {
    info!("Command: 清空客户端缓存");
    ctx.contest.clear_caches();
    ctx.problem.clear_caches();
    // 用户域缓存（含源代码）与登出同一语义：换账号后不得复用，故共用同一方法
    ctx.submission.clear_user_caches();
    info!("客户端缓存已清空");
    Ok(())
}
