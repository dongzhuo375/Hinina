# contest_cmd

## 职责
比赛相关 Tauri Command 模块。提供比赛列表查询、比赛选中、「加载配置比赛」及比赛排行榜查询功能，面向前端暴露 IPC 接口。

## 核心类型/函数
- 常量：`DEFAULT_RANK_LIMIT: i64 = 50` — 榜单默认分页大小（HOJ 建议值：榜单为全量计算后分页，limit 越大单次越慢）
- `pub async fn list_contests(ctx) -> AppResult<Vec<Contest>>` — 获取比赛列表（TTL 从 `oj.cache_ttl_secs` 读取，默认 60 秒）
- `pub async fn select_contest(ctx, contest_id: String) -> AppResult<()>` — 选中指定比赛，发布 `ContestEvent::Selected`
- `pub async fn load_configured_contest(ctx) -> AppResult<ContestBundle>` — 从 `oj.contest_id` 读取默认比赛，加载详情 + 题目列表；`contest_id == 0` 时返回错误提示配置
- `pub async fn get_contest_rank(ctx, contest_id, current_page?, limit?, keyword?, remove_star?, contains_end?) -> AppResult<ContestRankPage>` — 获取比赛排行榜（分页）。前端 invoke 签名 `get_contest_rank`({ contestId, currentPage?, limit?, keyword?, removeStar?, containsEnd? })，除 contestId 外均可省略（默认第 1 页、每页 50 条、不过滤）；`current_page` 经 `.max(1)` 收敛。注意：返回的 `records` 可能含服务端前置的「当前用户/关注用户」副本，前端渲染前需按 `uid` 去重；`total` 含这些前置条目，不能当作真实参赛人数

## 直接依赖
- `tauri::State`
- `crate::core::context::AppContext`
- `crate::core::entity::contest::{Contest, ContestBundle}`
- `crate::core::entity::rank::{ContestRankPage, RankQuery}`
- `crate::core::error::{AppError, AppResult}`

## 被依赖
- `src-tauri/src/main.rs`（`generate_handler!` 注册）

## 逻辑流程
1. 前端调用 `invoke('list_contests')` / `invoke('select_contest', { contestId })` / `invoke('load_configured_contest')` / `invoke('get_contest_rank', { contestId, ... })`
2. 各 command 接收 `AppContext`，调用 `ContestService` 对应方法
3. `load_configured_contest` 校验 `contest_id` 非 0 后调用 `load_contest_with_problems`，返回 `ContestBundle`（对象 `{ contest, problems }`，供前端直接解构）
4. `get_contest_rank` 把可选参数收敛为 `RankQuery`（缺省第 1 页 / 50 条 / 不过滤）后调用 `ContestService::get_rank`，透传 `ContestRankPage`
