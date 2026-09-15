# mod

## 职责
比赛服务模块入口。负责比赛列表获取、TTL 缓存管理、当前比赛切换、强制刷新及比赛排行榜获取。通过 `ProviderRegistry` 获取当前 OJ 的 ContestProvider，比赛列表带 TTL 缓存避免频繁网络请求；榜单则明确不缓存（实时计算数据）。

## 核心类型/函数
- `pub mod error` — 比赛错误类型模块声明
- **`ContestCache`**（内部 struct） — 比赛列表缓存（`contests: Vec<Contest>`, `fetched_at: Instant`）
- **`ContestService`** — 比赛服务
  - `fn new(registry, event_bus) -> Self` — 创建实例
  - `async fn list_contests(&self, cache_ttl_secs: u64) -> AppResult<Vec<Contest>>` — 获取比赛列表，优先使用缓存（TTL 由 Config 传入）；缓存命中直接返回，未命中请求远端 → 更新缓存 → 发布 `ContestEvent::ListLoaded`
  - `fn select_contest(&self, contest_id) -> AppResult<()>` — 选中比赛并发布 `ContestEvent::Selected`
  - `fn current_contest_id(&self) -> Option<String>` — 获取当前选中的比赛 ID
  - `async fn refresh(&self) -> AppResult<Vec<Contest>>` — 清空缓存并强制重新获取（等价 `list_contests(0)`）
  - `async fn get_rank(&self, contest_id, query: &RankQuery) -> AppResult<ContestRankPage>` — 获取比赛排行榜（分页），经 registry 调 `ContestProvider::get_contest_rank`。**不做缓存**：HOJ 内榜每次实时计算（`doc/HOJ/HOJ-Contest-Rank-API.md` §4），缓存反而会给出过期名次；轮询节奏由前端控制（≥10s 且加抖动错峰、后台暂停）
  - `async fn load_contest_with_problems(&self, contest_id, password) -> AppResult<ContestBundle>` — 一次性获取比赛详情 + 题目列表 + 自动选中（阶段 7 单比赛模式入口）
- **字段**：`registry: Arc<dyn ProviderRegistry>`, `event_bus: Arc<EventBus>`, `current_contest: RwLock<Option<String>>`, `cache: RwLock<Option<ContestCache>>`

## 直接依赖
- `core::entity::contest::{Contest, ContestBundle}`
- `core::entity::rank::{ContestRankPage, RankQuery}`
- `core::error::AppResult`（**不再直接引用 `AppError`**：传播 Provider 错误只用 `e.context(...)`，不构造新变体）
- `core::event::app_event::{AppEvent, ContestEvent}`
- `core::event::event_bus::EventBus`
- `core::provider::registry::ProviderRegistry`

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<ContestService>`）
- `commands::contest_cmd`（经 AppContext 转发 `list_contests` / `select_contest` / `load_configured_contest` / `get_contest_rank`）

## 逻辑流程
- **list_contests(ttl)**：检查 `ContestCache::fetched_at` 是否在 TTL 内 → 命中则直接返回缓存；未命中调 `ContestProvider::list_contests()`（失败 `warn!` + `e.context("获取比赛列表失败")`，变体不改写）→ 更新缓存 → 发布 `ListLoaded` → 返回结果
- **select_contest(id)**：写入 `current_contest` → 发布 `Selected` 事件
- **refresh**：清空缓存 → 调 `list_contests(0)` 跳过缓存检查
- **get_rank(contest_id, query)**：`registry.get_contest()` → `ContestProvider::get_contest_rank()` → 失败先 `warn!` 再 `e.context("获取比赛榜单失败")` 上抛（**变体原样穿透**）；成功直接透传 `ContestRankPage`（records 前置副本的去重与真实人数推导由前端处理，Service 不加工）
- **load_contest_with_problems(id, password)**：`get_contest()` 获取详情（失败 `e.context("获取比赛详情失败")`）→（私有赛校验密码）→ `list_contest_problems()` 获取题目（失败 `e.context("获取比赛题目列表失败")`）→ `select_contest()` 自动选中 → 返回 `ContestBundle { contest, problems }`

## 设计要点
- **错误处理约定：用 `context()` 而不是重新包装**。向上传播 Provider 错误一律 `e.context("环节名")`（保留变体、仍补环节名、`warn!` 日志保留），**禁止** `AppError::Contest(format!("…: {}", e))` —— 那会把 401 改写成 `Contest` 变体，而变体是前端 `isAuthError` 分流与 `stores/sessionGuard.ts` 会话失效兜底的**唯一依据**（见 `core/error.md` 与 `doc/Architecture.md`「错误变体是分流依据，后端不得改写」）。改写后的现场表现：token 过期时榜单静默 stale、提交只弹一条错误文案、选手不被带回登录页，反复重试全部失败。
- **`get_rank` 是全场最高频的认证调用**（前端每 10s 轮询一次），因此它的变体穿透最关键：一旦改写，会话失效兜底链路等于整场失效。`list_contests`（登录页匿名简报）与 `load_contest_with_problems`（进场链路，外壳 `loadContest` 走的就是它）同样必须保留 `Auth` 变体，前端才能区分「连不上」与「凭证无效」。

## 测试
`src-tauri/src/service/contest/tests/contest_tests.rs`（由 `mod.rs` 底部 `#[cfg(test)] #[path = "tests/contest_tests.rs"] mod tests;` 引用）以 `StubContestProvider` 锁定错误变体穿透与正常路径。Stub 用 `StubMode::{Ok, Auth, Network}` 让四个 trait 方法按同一模式响应（`Auth` 模拟 token 过期，即 HTTP 401 或 HOJ 体内 403「请您先登录」；`Network` 模拟断网），便于逐方法断言变体是否被保留；服务经 `ProviderRegistryImpl::new(OJType::HOJ)` 注册后构造，异步用例各自建 current-thread runtime 避免嵌套 panic。

覆盖：`get_rank` 保留 `Auth` 变体且消息含「获取比赛榜单失败」环节名、`get_rank` 保留 `Network` 变体（不被改写成 `Contest`）、`list_contests` 保留 `Auth`、`load_contest_with_problems` 保留 `Auth`（进场时 401 必须触发会话守卫）；成功路径 `get_rank` 返回分页（records/uid 正确）、`load_contest_with_problems` 返回 `ContestBundle` 并**自动选中比赛**（`current_contest_id()` 为 `Some("1011")`）。
