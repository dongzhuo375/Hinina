# contest_cmd

## 职责
比赛相关 Tauri Command 模块。提供比赛列表查询和比赛选中功能，面向前端暴露 `contest:*` 命名空间的 IPC 接口。

## 核心类型/函数
- `pub async fn list_contests(ctx) -> AppResult<Vec<Contest>>` — 获取比赛列表
- `pub async fn select_contest(ctx, contest_id) -> AppResult<Contest>` — 选中指定比赛

## 直接依赖
- `tauri::State`
- `crate::core::context::AppContext`
- `crate::core::entity::contest::Contest`
- `crate::core::error::AppResult`

## 被依赖
- `src-tauri/src/commands/mod.rs`（通过 `pub mod contest_cmd` 声明）

## 逻辑流程
1. 前端调用 `invoke('contest:list')` 或 `invoke('contest:select', {...})`
2. 各 command 接收 `AppContext`，调用对应的 ContestProvider 实现
3. 当前为占位实现（`todo!()`），待接入具体 OJ Adapter
