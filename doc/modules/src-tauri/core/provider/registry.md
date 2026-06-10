# registry

## 职责
定义 Provider 注册中心 trait `ProviderRegistry`，管理各 OJ 的 AuthProvider、ContestProvider、ProblemProvider、SubmissionProvider 实例，支持注册、查找、OJ 切换与可用列表查询。

## 核心类型/函数
- **`ProviderRegistry`** — Provider 注册中心 trait，方法：
  - `register_auth(&mut self, oj_type, provider)` — 注册 AuthProvider
  - `register_contest(&mut self, oj_type, provider)` — 注册 ContestProvider
  - `register_problem(&mut self, oj_type, provider)` — 注册 ProblemProvider
  - `register_submission(&mut self, oj_type, provider)` — 注册 SubmissionProvider
  - `get_auth(&self, oj_type) -> AppResult<Arc<dyn AuthProvider>>` — 获取 AuthProvider
  - `get_contest(&self, oj_type) -> AppResult<Arc<dyn ContestProvider>>` — 获取 ContestProvider
  - `get_problem(&self, oj_type) -> AppResult<Arc<dyn ProblemProvider>>` — 获取 ProblemProvider
  - `get_submission(&self, oj_type) -> AppResult<Arc<dyn SubmissionProvider>>` — 获取 SubmissionProvider
  - `current_oj(&self) -> OJType` — 当前选中 OJ
  - `set_current_oj(&self, oj_type)` — 切换 OJ
  - `list_available(&self) -> Vec<OJType>` — 列出已注册 OJ

## 直接依赖
- `core::error::AppResult`
- `core::provider::auth::AuthProvider`
- `core::provider::contest::ContestProvider`
- `core::provider::oj_type::OJType`
- `core::provider::problem::ProblemProvider`
- `core::provider::submission::SubmissionProvider`
- `std::sync::Arc`

## 被依赖
- `core::context`（AppContext 持有 Arc<ProviderRegistry>）
- `infra::provider_registry_impl`

## 逻辑流程
无（纯 trait 定义）。
