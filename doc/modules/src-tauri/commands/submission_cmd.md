# submission_cmd

## 职责
提交与评测相关 Tauri Command 模块。提供代码提交和评测结果查询功能，面向前端暴露 `submission:*` 命名空间的 IPC 接口。

## 核心类型/函数
- `pub async fn submit_code(ctx, contest_id, problem_id, language, source_code) -> AppResult<String>` — 提交代码到 OJ，返回 submission_id
- `pub async fn get_judgement(ctx, submission_id) -> AppResult<JudgementResult>` — 查询评测结果

## 直接依赖
- `tauri::State`
- `crate::core::context::AppContext`
- `crate::core::entity::submission::JudgementResult`
- `crate::core::error::AppResult`

## 被依赖
- `src-tauri/src/commands/mod.rs`（通过 `pub mod submission_cmd` 声明）

## 逻辑流程
1. 前端调用 `invoke('submission:submit', {...})` 提交代码，获得 submission_id
2. 前端轮询或等待后调用 `invoke('submission:get_judgement', {...})` 查询评测结果
3. 当前为占位实现（`todo!()`），待接入具体 SubmissionProvider
