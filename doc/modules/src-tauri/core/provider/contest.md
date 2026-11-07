# contest

## 职责
定义比赛 Provider trait `ContestProvider`，声明比赛列表、详情、题目列表、排行榜、比赛公告与**私有赛凭证进入**六个异步方法（`ensure_contest_access` 带默认实现）。各 OJ Adapter 需实现此 trait 以对接不同 OJ 的比赛数据。

## 核心类型/函数
- **`ContestProvider`** — 比赛 Provider trait（`#[async_trait]`），方法：
  - `list_contests(&self) -> AppResult<Vec<Contest>>` — 获取比赛列表
  - `get_contest(&self, contest_id) -> AppResult<Contest>` — 获取比赛详情
  - `list_contest_problems(&self, contest_id) -> AppResult<Vec<ContestProblem>>` — 获取比赛题目列表（仅摘要，不含完整题面）
  - `get_contest_rank(&self, contest_id, query: &RankQuery) -> AppResult<ContestRankPage>` — 获取比赛排行榜（分页）。实现约定：
    - 返回的行可能包含服务端前置的「当前用户/关注用户」副本，调用方渲染前需按 `uid` 去重
    - `total` 含这些前置条目，**不能**当作真实参赛人数
    - 榜单为服务端实时计算，本方法不做缓存，轮询节奏由调用方控制（建议 ≥10s 且加抖动错峰）
  - `list_announcements(&self, contest_id, current_page: i64, limit: i64) -> AppResult<AnnouncementPage>` — 获取比赛公告（分页）。实现约定：公告可能包含裁判组临场发布的规则变更，实现方**不得缓存**，刷新节奏由调用方控制
  - `ensure_contest_access(&self, contest_id, credential: Option<&str>) -> AppResult<()>` — 确保当前会话可访问指定比赛（私有赛/需报名赛），必要时用凭证报名进入。「凭证进入比赛」是各 OJ 的公因数：HOJ 为比赛密码（register-contest），其它 OJ 可为邀请码（如 Hydro 的 attend），凭证语义由适配器自行解释。实现约定：已可访问时幂等返回 Ok；需要凭证而未提供/被拒绝返回 `AppError::Contest`（消息引导设置页补填）；会话失效返回 `AppError::Auth`（变体是前端 sessionGuard 的判定依据）。**默认实现返回不支持错误**——当前仅 HOJ 覆盖；Hydro 的比赛可见性由域权限/报名表达、Hinina 侧 `auth` 恒映射 0，不会触发

## 直接依赖
- `async_trait::async_trait`
- `core::entity::announcement::AnnouncementPage`
- `core::entity::contest::{Contest, ContestProblem}`
- `core::entity::rank::{ContestRankPage, RankQuery}`
- `core::error::{AppError, AppResult}`（默认实现构造不支持错误）

## 被依赖
- `core::provider::registry`（ProviderRegistry 注册/获取 ContestProvider）
- `infra::provider_registry_impl`
- `adapter::hoj`（实现全部方法，含覆盖 `ensure_contest_access` 默认实现；Hydro 沿用默认实现）
- `service::contest`（`get_rank` 经 registry 调用 `get_contest_rank`；`list_announcements` 经 registry 调用同名方法；`ensure_private_contest_access` 调用 `ensure_contest_access`）

## 逻辑流程
无（纯 trait 定义）。
