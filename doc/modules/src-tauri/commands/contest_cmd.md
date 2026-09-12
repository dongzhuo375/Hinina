# contest_cmd

## 职责
比赛相关 Tauri Command 模块。提供比赛列表查询、比赛选中及「加载配置比赛」功能，面向前端暴露 IPC 接口。

## 核心类型/函数
- `pub async fn list_contests(ctx) -> AppResult<Vec<Contest>>` — 获取比赛列表（TTL 从 `oj.cache_ttl_secs` 读取，默认 60 秒）
- `pub async fn select_contest(ctx, contest_id: String) -> AppResult<()>` — 选中指定比赛，发布 `ContestEvent::Selected`
- `pub async fn load_configured_contest(ctx) -> AppResult<ContestBundle>` — 从 `oj.contest_id` 读取默认比赛，加载详情 + 题目列表；`contest_id == 0` 时返回错误提示配置

## 直接依赖
- `tauri::State`
- `crate::core::context::AppContext`
- `crate::core::entity::contest::{Contest, ContestBundle}`
- `crate::core::error::{AppError, AppResult}`

## 被依赖
- `src-tauri/src/main.rs`（`generate_handler!` 注册）

## 逻辑流程
1. 前端调用 `invoke('list_contests')` / `invoke('select_contest', { contestId })` / `invoke('load_configured_contest')`
2. 各 command 接收 `AppContext`，调用 `ContestService` 对应方法
3. `load_configured_contest` 校验 `contest_id` 非 0 后调用 `load_contest_with_problems`，返回 `ContestBundle`（对象 `{ contest, problems }`，供前端直接解构）
