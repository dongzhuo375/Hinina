# contest

## 职责
定义比赛 Provider trait `ContestProvider`，声明获取比赛列表与详情两个异步方法。各 OJ Adapter 需实现此 trait 以对接不同 OJ 的比赛数据。

## 核心类型/函数
- **`ContestProvider`** — 比赛 Provider trait（`#[async_trait]`），方法：
  - `list_contests(&self) -> AppResult<Vec<Contest>>` — 获取比赛列表
  - `get_contest(&self, contest_id) -> AppResult<Contest>` — 获取比赛详情

## 直接依赖
- `async_trait::async_trait`
- `core::entity::contest::Contest`
- `core::error::AppResult`

## 被依赖
- `core::provider::registry`（ProviderRegistry 注册/获取 ContestProvider）
- `infra::provider_registry_impl`

## 逻辑流程
无（纯 trait 定义）。
