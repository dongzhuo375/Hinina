# submission_cmd

## 职责
提交与评测相关 Tauri Command 模块。提供代码提交、评测结果单次查询、比赛提交列表、提交详情与测试点结果查询功能，面向前端暴露 IPC 接口。

## 核心类型/函数
- 常量：`DEFAULT_SUBMISSION_LIMIT: i64 = 20` — 提交列表默认分页大小
- `pub async fn submit_code(ctx, contest_id, problem_id, display_id, language, source_code) -> AppResult<String>` — 提交代码到 OJ，返回 submission_id。前端 invoke 签名 `submit_code`({ contestId, problemId, displayId, language, sourceCode })。**`problemId` 与 `displayId` 都必传**：前者是题目真实 ID（工作区隔离与状态查询的键），后者是比赛内展示题号（如 `"A"`）—— HOJ 的提交接口只认后者，传错会得到 HTTP 500（详见 `core/provider/submission.md`）。发布 `SubmissionEvent::Created`
- `pub async fn get_judgement(ctx, submission_id) -> AppResult<JudgementResult>` — **单次**查询评测结果（不阻塞、不循环）。前端 invoke 签名 `get_judgement`({ submissionId })；轮询节拍 / 总超时 / 终态停止全部由前端 submissionStore 编排（createPoller，抖动 ±20% 封顶 500ms），`oj.poll_interval_secs` / `poll_timeout_secs` 由前端经 get_config 消费，本命令不再读取 Config；终态发布 `SubmissionEvent::Judged`，非终态原样透传、不发事件
- `pub async fn list_contest_submissions(ctx, contest_id, current_page?, limit?, problem_display_id?, status?) -> AppResult<SubmissionPage>` — 获取比赛提交列表（分页）。前端 invoke 签名 `list_contest_submissions`({ contestId, currentPage?, limit?, problemDisplayId?, status? })，默认第 1 页、每页 20 条（`.max(1)` 收敛）。组装 `SubmissionQuery` 时 **`only_mine` 硬编码为 `true`** —— 产品决策「提交记录只显示本人」，由后端强制，前端不可绕过；`problem_display_id` 经空白过滤（空串/纯空白视为不筛选，不会拼进 HOJ 查询串）。不做缓存，刷新节奏由前端控制
- `pub async fn get_submission_detail(ctx, submission_id) -> AppResult<SubmissionDetail>` — 获取提交详情（含源代码与错误信息）。前端 invoke 签名 `get_submission_detail`({ submissionId })
- `pub async fn get_submission_cases(ctx, submission_id) -> AppResult<SubmissionCases>` — 获取提交的全部测试点结果。前端 invoke 签名 `get_submission_cases`({ submissionId })

## 直接依赖
- `tauri::State`
- `tracing::info`
- `crate::core::context::AppContext`
- `crate::core::entity::submission::{JudgementResult, SubmissionCases, SubmissionDetail, SubmissionPage, SubmissionQuery}`
- `crate::core::error::AppResult`

## 被依赖
- `src-tauri/src/main.rs`（`generate_handler!` 注册全部五个 Command）

## 逻辑流程
1. 前端调用 `invoke('submit_code', {...})` 提交代码，获得 submission_id
2. 前端 submissionStore 的 createPoller 按节拍反复调用 `invoke('get_judgement', { submissionId })`，Command 直接交 `SubmissionService::get_judgement` 单次查询：终态发 `Judged` 事件并由前端停止轮询，非终态原样返回、前端下一拍再查；总超时由前端 deadline 判定
3. 提交记录页调用 `invoke('list_contest_submissions', {...})`：Command 收敛分页参数、强制 `only_mine = true`、过滤空白筛选后组装 `SubmissionQuery`，交 `SubmissionService::list_contest_submissions` 透传 `SubmissionPage`
4. 点击单条记录调用 `get_submission_detail`（源码/CE 信息）与 `get_submission_cases`（测试点面板），分别透传 `SubmissionDetail` / `SubmissionCases`
5. 错误变体由 Service 层 `e.context()` 补环节名后原样穿透（`Auth` 变体驱动前端会话守卫，见 `service/submission/mod.md`）
