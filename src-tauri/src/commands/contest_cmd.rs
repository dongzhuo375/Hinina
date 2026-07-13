use tauri::State;
use tracing::info;

use crate::core::context::AppContext;
use crate::core::entity::contest::Contest;
use crate::core::error::AppResult;

/// 获取比赛列表（带缓存）。
///
/// 前端 invoke 签名: `contest:list`
///
/// 缓存 TTL 从 Config 读取（`oj.cache_ttl_secs`），未配置时默认 60 秒。
/// 首次调用或缓存过期时从远程 OJ 拉取最新数据。
#[tauri::command]
pub async fn list_contests(ctx: State<'_, AppContext>) -> AppResult<Vec<Contest>> {
    let ttl = ctx.config.get().oj.cache_ttl_secs;
    ctx.contest.list_contests(ttl).await
}

/// 选中比赛。
///
/// 前端 invoke 签名: `contest:select`({ contest_id })
///
/// 选中后发布 `ContestEvent::Selected`，前端其他组件可监听此事件切换题目列表等。
#[tauri::command]
pub async fn select_contest(
    ctx: State<'_, AppContext>,
    contest_id: String,
) -> AppResult<()> {
    info!(contest_id = %contest_id, "Command: 选中比赛");
    ctx.contest.select_contest(&contest_id)
}
