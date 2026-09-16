# submission

## 职责
定义提交 Provider trait `SubmissionProvider`，声明提交代码、评测结果轮询查询、比赛提交列表、提交详情与测试点结果五个异步方法。各 OJ Adapter 需实现此 trait。

## 核心类型/函数
- **`SubmissionProvider`** — 提交 Provider trait（`#[async_trait]`），方法：
  - `submit(&self, contest_id, problem_id, language, source_code) -> AppResult<String>` — 提交代码，返回提交 ID
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
