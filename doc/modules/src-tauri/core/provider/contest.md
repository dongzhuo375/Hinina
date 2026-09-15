# contest

## 职责
定义比赛 Provider trait `ContestProvider`，声明比赛列表、详情、题目列表与排行榜四个异步方法。各 OJ Adapter 需实现此 trait 以对接不同 OJ 的比赛数据。

## 核心类型/函数
- **`ContestProvider`** — 比赛 Provider trait（`#[async_trait]`），方法：
  - `list_contests(&self) -> AppResult<Vec<Contest>>` — 获取比赛列表
  - `get_contest(&self, contest_id) -> AppResult<Contest>` — 获取比赛详情
  - `list_contest_problems(&self, contest_id) -> AppResult<Vec<ContestProblem>>` — 获取比赛题目列表（仅摘要，不含完整题面）
  - `get_contest_rank(&self, contest_id, query: &RankQuery) -> AppResult<ContestRankPage>` — 获取比赛排行榜（分页）。实现约定：
    - 返回的行可能包含服务端前置的「当前用户/关注用户」副本，调用方渲染前需按 `uid` 去重
    - `total` 含这些前置条目，**不能**当作真实参赛人数
    - 榜单为服务端实时计算，本方法不做缓存，轮询节奏由调用方控制（建议 ≥10s 且加抖动错峰）

## 直接依赖
- `async_trait::async_trait`
- `core::entity::contest::{Contest, ContestProblem}`
- `core::entity::rank::{ContestRankPage, RankQuery}`
- `core::error::AppResult`

## 被依赖
- `core::provider::registry`（ProviderRegistry 注册/获取 ContestProvider）
- `infra::provider_registry_impl`
- `adapter::hoj`（实现全部四个方法）
- `service::contest`（`get_rank` 经 registry 调用 `get_contest_rank`）

## 逻辑流程
无（纯 trait 定义）。
