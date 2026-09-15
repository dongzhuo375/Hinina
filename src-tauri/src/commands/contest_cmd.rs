use tauri::State;
use tracing::info;

use crate::core::context::AppContext;
use crate::core::entity::contest::{Contest, ContestBundle};
use crate::core::entity::rank::{ContestRankPage, RankQuery};
use crate::core::error::{AppError, AppResult};

/// 榜单默认分页大小（HOJ 建议值：榜单为全量计算后分页，limit 越大单次越慢）
const DEFAULT_RANK_LIMIT: i64 = 50;

/// 获取比赛列表（带缓存）。
///
/// 前端 invoke 签名: `list_contests`
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
/// 前端 invoke 签名: `select_contest`({ contestId })
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

/// 从配置文件加载默认比赛（阶段 7 单比赛模式入口）。
///
/// 前端 invoke 签名: `load_configured_contest`
///
/// 从 `OjConfig.contest_id` 读取比赛 ID，自动加载比赛详情与题目列表。
/// 若 `contest_id == 0` 返回错误提示用户配置。
/// 返回 `ContestBundle`（`{ contest, problems }`），前端据此渲染题目侧边栏。
#[tauri::command]
pub async fn load_configured_contest(
    ctx: State<'_, AppContext>,
) -> AppResult<ContestBundle> {
    let contest_id = ctx.config.get().oj.contest_id;
    if contest_id == 0 {
        return Err(AppError::Contest(
            "未配置默认比赛 ID，请在 config.json 中设置 oj.contest_id".into(),
        ));
    }

    info!(contest_id = contest_id, "Command: 加载配置的比赛");
    let password = ctx.config.get().oj.contest_password.clone();
    ctx.contest
        .load_contest_with_problems(&contest_id.to_string(), password.as_deref())
        .await
}

/// 获取比赛排行榜（分页）。
///
/// 前端 invoke 签名: `get_contest_rank`({ contestId, currentPage?, limit?, keyword?, removeStar?, containsEnd? })
///
/// 除 `contestId` 外均可省略（默认第 1 页、每页 50 条、不过滤）。
///
/// 注意：返回的 `records` 可能包含服务端前置的「当前用户/关注用户」副本，
/// 前端渲染前需按 `uid` 去重；`total` 含这些前置条目，不能当作真实参赛人数。
#[tauri::command]
pub async fn get_contest_rank(
    ctx: State<'_, AppContext>,
    contest_id: String,
    current_page: Option<i64>,
    limit: Option<i64>,
    keyword: Option<String>,
    remove_star: Option<bool>,
    contains_end: Option<bool>,
) -> AppResult<ContestRankPage> {
    let query = RankQuery {
        current_page: current_page.unwrap_or(1).max(1),
        limit: limit.unwrap_or(DEFAULT_RANK_LIMIT),
        keyword,
        remove_star: remove_star.unwrap_or(false),
        contains_end: contains_end.unwrap_or(false),
    };

    info!(
        contest_id = %contest_id,
        page = query.current_page,
        limit = query.limit,
        "Command: 获取比赛榜单"
    );
    ctx.contest.get_rank(&contest_id, &query).await
}
