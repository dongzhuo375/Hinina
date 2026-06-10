# submission

## 职责
定义提交 Provider trait `SubmissionProvider`，声明提交代码与查询评测结果两个异步方法。各 OJ Adapter 需实现此 trait。

## 核心类型/函数
- **`SubmissionProvider`** — 提交 Provider trait（`#[async_trait]`），方法：
  - `submit(&self, contest_id, problem_id, language, source_code) -> AppResult<String>` — 提交代码，返回提交 ID
  - `get_judgement(&self, submission_id) -> AppResult<JudgementResult>` — 查询评测结果

## 直接依赖
- `async_trait::async_trait`
- `core::entity::submission::{JudgementResult, Submission}`
- `core::error::AppResult`

## 被依赖
- `core::provider::registry`（ProviderRegistry 注册/获取 SubmissionProvider）
- `infra::provider_registry_impl`

## 逻辑流程
无（纯 trait 定义）。
