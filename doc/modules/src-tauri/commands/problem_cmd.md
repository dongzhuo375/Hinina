# problem_cmd

## 职责
题目相关 Tauri Command 模块。提供题目详情获取、题目列表查询、用户题目状态与题目 limits 批量查询，面向前端暴露 IPC 接口。

## 核心类型/函数
- `pub async fn get_problem(ctx, contest_id, problem_id) -> AppResult<Problem>` — 获取指定题目详情（调 `ProblemService::open_problem`，同时发布 `ProblemEvent::Opened` 供前端 Workspace 切换）
- `pub async fn list_problems(ctx, contest_id) -> AppResult<Vec<Problem>>` — 获取某比赛下所有题目（摘要，不含完整题面）
- `pub async fn get_user_problem_status(ctx, contest_id, problem_ids: Vec<String>) -> AppResult<HashMap<String, i32>>` — 批量获取当前用户提交状态。前端 invoke 签名 `get_user_problem_status`({ contestId, problemIds })；返回 `{ pid: 0|1|2 }`（0=未提交，1=已AC，2=尝试过），未出现的 pid 视为未提交。用于题目卡片状态标记与「解题进度」统计
- `pub async fn get_contest_problem_limits(ctx, contest_id, display_ids: Vec<String>) -> AppResult<Vec<ProblemLimits>>` — 批量获取题目 limits（时间 ms / 内存 MB）。前端 invoke 签名 `get_contest_problem_limits`({ contestId, displayIds })；比赛题目列表接口不返回 limits，只能按题拉详情，因此服务端做了内存 + 磁盘双层缓存（`cache/problem_limits/{cid}.json`）并限制并发扇出。返回顺序与入参一致，**获取失败的题目不会出现在结果里**（前端应显示占位而非假默认值）

## 直接依赖
- `std::collections::HashMap`
- `tauri::State`
- `crate::core::context::AppContext`
- `crate::core::entity::problem::Problem`
- `crate::core::entity::rank::ProblemLimits`
- `crate::core::error::AppResult`

## 被依赖
- `src-tauri/src/commands/mod.rs`（通过 `pub mod problem_cmd` 声明）
- `src-tauri/src/main.rs`（`generate_handler!` 注册全部四个 command）

## 逻辑流程
1. 前端调用 `invoke('get_problem', { contestId, problemId })` / `invoke('list_problems', { contestId })` / `invoke('get_user_problem_status', { contestId, problemIds })` / `invoke('get_contest_problem_limits', { contestId, displayIds })`
2. 各 command 接收 `AppContext`，转发给 `ProblemService` 对应方法（`open_problem` / `list_problems` / `get_user_problem_status` / `load_problem_limits`），由 Service 经 ProviderRegistry 调用当前 OJ 的 ProblemProvider
