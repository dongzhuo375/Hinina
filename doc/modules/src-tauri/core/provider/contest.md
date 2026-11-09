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
  - `is_credential_rejection(&self, error: &AppError) -> bool`（同步方法，带默认实现） — 判断题目列表的业务失败是否属「需凭证进入」类，是错误驱动进入流程的**精确触发判据**。「需凭证的失败以 `Contest` 变体表达」只是必要条件不充分（「比赛还未开始」「该比赛不存在」等同样落在该变体），是否属需凭证只有适配器能可靠解读自家服务端的消息措辞。返回 true 才可走凭证进入流程；false 则错误原样上抛——**漏判是安全降级方向**（进入流程不触发，而不是给用户看错的引导）。默认实现恒 false：未声明该能力的 OJ 不触发进入流程
  - `classify_access_rejection(&self, error: &AppError) -> AccessRejectionKind`（同步方法，带默认实现） — 分类进入流程业务失败（`AccessRejectionKind::{Rejected, Other}`），决定防重试记忆写回策略。是否「注册被拒」只有适配器能可靠解读自家消息（查询阶段失败与注册被拒从黑盒里出来都是 `Contest` 变体）。默认实现把 `Contest` 归为 `Rejected`（保守缓存防未知措辞的密码错误每周期重发报名）、非 `Contest` 归 `Other`

## 直接依赖
- `async_trait::async_trait`
- `core::entity::announcement::AnnouncementPage`
- `core::entity::contest::{Contest, ContestProblem}`
- `core::entity::rank::{ContestRankPage, RankQuery}`
- `core::error::{AppError, AppResult}`（默认实现构造不支持错误）

## 被依赖
- `core::provider::registry`（ProviderRegistry 注册/获取 ContestProvider）
- `infra::provider_registry_impl`
- `adapter::hoj`（实现全部方法，含覆盖 `ensure_contest_access` / `is_credential_rejection` / `classify_access_rejection` 三个默认实现；Hydro 沿用默认实现）
- `service::contest`（`get_rank` 经 registry 调用 `get_contest_rank`；`list_announcements` 经 registry 调用同名方法；`ensure_contest_access_core` 调用 `ensure_contest_access`，错误驱动触发判据调用 `is_credential_rejection`，记忆写回策略调用 `classify_access_rejection`）

## 逻辑流程
本文件绝大部分是纯 trait 声明；仅三个**默认实现**自带行为：
- `ensure_contest_access` 默认实现：返回 `AppError::Contest("当前 OJ 不支持凭密码/凭证进入私有赛")`——能力缺失必须是 `Contest` 变体而不是 `Auth`（否则前端 sessionGuard 会把「不支持」误判成会话失效把选手踢回登录页）；
- `is_credential_rejection` 默认实现：恒 `false`——未声明「凭证进入」能力的 OJ 不触发错误驱动的进入流程（安全降级方向）；
- `classify_access_rejection` 默认实现：`Contest` 变体归 `Rejected`（保守缓存——防未知措辞的密码错误每周期重发报名 POST）、非 `Contest` 归 `Other`。

三者的默认行为由 `core/provider/tests/contest_tests.rs` 锁定。
