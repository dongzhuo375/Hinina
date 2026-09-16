use tauri::State;
use tracing::{info, warn};

use crate::core::context::AppContext;
use crate::core::entity::announcement::AnnouncementPage;
use crate::core::entity::contest::{Contest, ContestBundle};
use crate::core::entity::rank::{ContestRankPage, RankQuery};
use crate::core::error::{AppError, AppResult};

/// 榜单默认分页大小（HOJ 建议值：榜单为全量计算后分页，limit 越大单次越慢）
const DEFAULT_RANK_LIMIT: i64 = 50;

/// 公告默认分页大小
const DEFAULT_ANNOUNCEMENT_LIMIT: i64 = 50;

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

/// 获取比赛公告（分页）。
///
/// 前端 invoke 签名: `list_contest_announcements`({ contestId, currentPage?, limit? })
///
/// 默认第 1 页、每页 50 条。**不做缓存**：公告可能包含裁判组临场发布的规则变更。
#[tauri::command]
pub async fn list_contest_announcements(
    ctx: State<'_, AppContext>,
    contest_id: String,
    current_page: Option<i64>,
    limit: Option<i64>,
) -> AppResult<AnnouncementPage> {
    let page = current_page.unwrap_or(1).max(1);
    let limit = limit.unwrap_or(DEFAULT_ANNOUNCEMENT_LIMIT).max(1);

    info!(contest_id = %contest_id, page = page, limit = limit, "Command: 获取比赛公告");
    ctx.contest.list_announcements(&contest_id, page, limit).await
}

/// 从当前会话解析公告已读状态使用的 uid。
///
/// 优先用 HOJ 的用户 UUID（user_id）；旧版会话文件可能缺失该字段，回退 username。
fn session_uid(ctx: &AppContext) -> Option<String> {
    ctx.auth.get_session().map(|s| {
        if s.user_id.is_empty() {
            s.username
        } else {
            s.user_id
        }
    })
}

/// 获取当前用户在某比赛下已读的公告 ID 列表。
///
/// 前端 invoke 签名: `get_read_announcement_ids`({ contestId })
///
/// 已读状态是客户端本地特性（HOJ 无对应接口），按会话 uid 隔离存储。
/// 无会话时返回空列表而不是报错 —— 登录页也可能预渲染公告。
#[tauri::command]
pub async fn get_read_announcement_ids(
    ctx: State<'_, AppContext>,
    contest_id: String,
) -> AppResult<Vec<String>> {
    let Some(uid) = session_uid(&ctx) else {
        return Ok(Vec::new());
    };
    ctx.contest.get_read_announcement_ids(&contest_id, &uid)
}

/// 标记公告为已读（与既有记录合并去重）。
///
/// 前端 invoke 签名: `mark_announcements_read`({ contestId, ids })
///
/// 无会话时静默跳过（warn 日志）：已读状态是纯 UI 便利特性，不值得为此报错。
#[tauri::command]
pub async fn mark_announcements_read(
    ctx: State<'_, AppContext>,
    contest_id: String,
    ids: Vec<String>,
) -> AppResult<()> {
    let Some(uid) = session_uid(&ctx) else {
        warn!(contest_id = %contest_id, "无会话，跳过公告已读标记");
        return Ok(());
    };
    ctx.contest.mark_announcements_read(&contest_id, &uid, &ids)
}
