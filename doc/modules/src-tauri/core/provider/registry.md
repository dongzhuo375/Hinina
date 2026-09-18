# registry

## 职责
定义 Provider 注册中心 trait `ProviderRegistry` 与能力聚合值 `ProviderSet`。管理各 OJ 的 Provider 实例，支持注册、按能力查询、OJ 切换与可用列表查询。OJ 身份是数据（`OjId`）：接入新 OJ 不需要修改本文件。

## 核心类型/函数
- **`ProviderSet`** — 一个 OJ 的 Provider 能力集合（注册侧的聚合值），字段 `auth / contest / problem / submission: Option<Arc<dyn …>>`。字段用 `Option`：新 Adapter 可以先只实现部分能力、逐步补齐（开发手册「新 Adapter 可先只实现部分接口」的扩展路径）—— 缺失的能力在查询侧返回 `ProviderNotFound`，而不是让注册整个失败。`#[derive(Default)]`（全空集合）
- **`ProviderSet::full(auth, contest, problem, submission)`** — 四项能力齐备的集合构造（内建 OJ 通常一步到位）
- **`ProviderRegistry`** — Provider 注册中心 trait，方法：
  - `register(&self, oj_id: OjId, set: ProviderSet)` — 注册一个 OJ 的能力集合（N 个 OJ = N 次调用，取代旧的 4×N 次单项注册）
  - `current_auth(&self) -> AppResult<Arc<dyn AuthProvider>>` — 当前 OJ 的 AuthProvider（未注册 / 未提供该能力 → `ProviderNotFound`）
  - `current_contest(&self) -> AppResult<Arc<dyn ContestProvider>>` — 当前 OJ 的 ContestProvider
  - `current_problem(&self) -> AppResult<Arc<dyn ProblemProvider>>` — 当前 OJ 的 ProblemProvider
  - `current_submission(&self) -> AppResult<Arc<dyn SubmissionProvider>>` — 当前 OJ 的 SubmissionProvider
  - `current_id(&self) -> OjId` — 当前选中的 OJ
  - `set_current(&self, oj_id: OjId)` — 切换当前 OJ
  - `list_available(&self) -> Vec<OjId>` — 列出所有已注册的 OJ

## 直接依赖
- `core::error::AppResult`
- `core::provider::auth::AuthProvider`
- `core::provider::contest::ContestProvider`
- `core::provider::oj_id::OjId`
- `core::provider::problem::ProblemProvider`
- `core::provider::submission::SubmissionProvider`
- `std::sync::Arc`

## 被依赖
- `core::context`（AppContext 持有 Arc<ProviderRegistry>）
- `infra::provider_registry_impl`

## 逻辑流程
无（纯 trait + 聚合值定义）。

## 设计要点
- **注册侧聚合、查询侧按能力（接口隔离）**：聚合值 `ProviderSet` 只用于一次性注册/构造；查询保持 `current_auth()` 这类单项能力方法 —— 刻意不提供返回聚合体的 `current()`，否则 Service 将拿到它不需要的三个能力，接口隔离就从接口层面退化成约定层面。
