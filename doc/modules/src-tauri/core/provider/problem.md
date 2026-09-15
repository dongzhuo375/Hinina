# problem

## 职责
定义题目 Provider trait `ProblemProvider`，声明题目详情、比赛题目列表与用户题目状态三个异步方法。各 OJ Adapter 需实现此 trait。

## 核心类型/函数
- **`ProblemProvider`** — 题目 Provider trait（`#[async_trait]`），方法：
  - `get_problem(&self, contest_id, problem_id) -> AppResult<Problem>` — 获取题目详情
  - `list_problems(&self, contest_id) -> AppResult<Vec<Problem>>` — 获取题目列表
  - `get_user_problem_status(&self, contest_id, problem_ids: &[String]) -> AppResult<HashMap<String, i32>>` — 批量获取当前用户对指定题目的提交状态。返回 map 的 key 为题目真实 ID（pid）字符串，value 为 `0=未提交 / 1=已AC / 2=尝试过`；未出现在 map 中的题目视为未提交

## 直接依赖
- `std::collections::HashMap`
- `async_trait::async_trait`
- `core::entity::problem::Problem`
- `core::error::AppResult`

## 被依赖
- `core::provider::registry`（ProviderRegistry 注册/获取 ProblemProvider）
- `infra::provider_registry_impl`
- `adapter::hoj`（实现全部三个方法）
- `service::problem`（`get_user_problem_status` / `load_problem_limits` 经 registry 调用；后者复用 `get_problem` 逐题取 limits）

## 逻辑流程
无（纯 trait 定义）。
