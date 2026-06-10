# problem

## 职责
定义题目 Provider trait `ProblemProvider`，声明获取单个题目详情与比赛题目列表两个异步方法。各 OJ Adapter 需实现此 trait。

## 核心类型/函数
- **`ProblemProvider`** — 题目 Provider trait（`#[async_trait]`），方法：
  - `get_problem(&self, contest_id, problem_id) -> AppResult<Problem>` — 获取题目详情
  - `list_problems(&self, contest_id) -> AppResult<Vec<Problem>>` — 获取题目列表

## 直接依赖
- `async_trait::async_trait`
- `core::entity::problem::Problem`
- `core::error::AppResult`

## 被依赖
- `core::provider::registry`（ProviderRegistry 注册/获取 ProblemProvider）
- `infra::provider_registry_impl`

## 逻辑流程
无（纯 trait 定义）。
