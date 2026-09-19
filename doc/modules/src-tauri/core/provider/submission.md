# submission

## 职责
定义提交 Provider trait `SubmissionProvider`，声明提交代码、评测结果轮询查询、比赛提交列表、提交详情与测试点结果五个异步方法。各 OJ Adapter 需实现此 trait。

## 核心类型/函数
- **`SubmissionProvider`** — 提交 Provider trait（`#[async_trait]`），方法：
  - `submit(&self, contest_id, problem_id, display_id, language, source_code) -> AppResult<String>` — 提交代码，返回提交 ID。**`problem_id` 与 `display_id` 是同一道题的两个标识，各 OJ 认的不是同一个，故两个都传入由 Adapter 各取所需**：
    - `problem_id`：题目真实 ID（HOJ 数字 pid / Hydro ObjectId），也是工作区隔离与 `get_user_problem_status` 的键
    - `display_id`：比赛内展示题号（如 `"A"`）。HOJ 的 `POST /submit-problem-judge` 收的 `pid` 是**展示题号** —— 服务端拿它查 `contest_problem.display_id`，查不到直接 NPE 返回 **HTTP 500**（实测传数字 pid 必 500）；Hydro 的 `/p/{id}/submit` 收的则是真实 ID
    - 不要在某一家里「猜」另一家的语义（Hydro 侧刻意不做字母换算：万一某题的 pid 恰好是 `"A"`，换算会把提交打到另一道题上）
  - `get_judgement(&self, submission_id) -> AppResult<JudgementResult>` — 查询评测结果（轮询用轻量投影）
  - `list_contest_submissions(&self, query: &SubmissionQuery) -> AppResult<SubmissionPage>` — 查询比赛提交列表（分页）
  - `get_submission_detail(&self, submit_id) -> AppResult<SubmissionDetail>` — 查询提交详情（含源代码与错误信息）
  - `get_submission_cases(&self, submit_id) -> AppResult<SubmissionCases>` — 查询提交的全部测试点结果

## 直接依赖
- `async_trait::async_trait`
- `core::entity::submission::{JudgementResult, SubmissionCases, SubmissionDetail, SubmissionPage, SubmissionQuery}`
- `core::error::AppResult`

## 被依赖
- `core::provider::registry`（ProviderRegistry 注册/获取 SubmissionProvider）
- `infra::provider_registry_impl`
- `adapter::hoj`（实现全部五个方法）
- `service::submission`（`submit` / `get_judgement` / `list_contest_submissions` / `get_submission_detail` / `get_submission_cases` 经 registry 调用）

## 逻辑流程
无（纯 trait 定义）。
