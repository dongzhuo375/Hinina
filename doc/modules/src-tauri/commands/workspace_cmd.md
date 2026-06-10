# workspace_cmd

## 职责
工作区相关 Tauri Command 模块。管理代码编辑工作区的加载、保存、切换和状态查询，面向前端暴露 `workspace:*` 命名空间的 IPC 接口。

## 核心类型/函数
- `pub async fn load_workspace(ctx, contest_id, problem_id) -> AppResult<Workspace>` — 加载或创建指定比赛/题目的工作区
- `pub async fn save_workspace(ctx) -> AppResult<()>` — 保存当前工作区文件
- `pub async fn switch_workspace(ctx, workspace_id) -> AppResult<Workspace>` — 切换到指定工作区
- `pub async fn current_workspace(ctx) -> AppResult<Option<Workspace>>` — 获取当前工作区

## 直接依赖
- `tauri::State`
- `crate::core::context::AppContext`
- `crate::core::entity::workspace::Workspace`
- `crate::core::error::AppResult`

## 被依赖
- `src-tauri/src/commands/mod.rs`（通过 `pub mod workspace_cmd` 声明）

## 逻辑流程
1. 用户选择比赛/题目后，前端调用 `load_workspace` 初始化编辑环境
2. 编辑过程中通过 `save_workspace` 触发持久化（配合自动保存策略）
3. `switch_workspace` 和 `current_workspace` 支持多工作区管理
4. 当前为占位实现（`todo!()`），待接入 WorkspaceManager
