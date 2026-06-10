# problem_cmd

## 职责
题目相关 Tauri Command 模块。提供题目详情获取和题目列表查询，面向前端暴露 `problem:*` 命名空间的 IPC 接口。

## 核心类型/函数
- `pub async fn get_problem(ctx, contest_id, problem_id) -> AppResult<Problem>` — 获取指定题目详情
- `pub async fn list_problems(ctx, contest_id) -> AppResult<Vec<Problem>>` — 获取某比赛下所有题目

## 直接依赖
- `tauri::State`
- `crate::core::context::AppContext`
- `crate::core::entity::problem::Problem`
- `crate::core::error::AppResult`

## 被依赖
- `src-tauri/src/commands/mod.rs`（通过 `pub mod problem_cmd` 声明）

## 逻辑流程
1. 前端调用 `invoke('problem:get', {...})` 或 `invoke('problem:list', {...})`
2. 各 command 接收 `AppContext`，调用对应的 ProblemProvider 实现
3. 当前为占位实现（`todo!()`），待接入具体 OJ Adapter
