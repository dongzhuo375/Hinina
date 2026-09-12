# mod

## 职责
比赛服务模块入口。负责比赛列表获取、TTL 缓存管理、当前比赛切换及强制刷新。通过 `ProviderRegistry` 获取当前 OJ 的 ContestProvider，比赛列表带 TTL 缓存避免频繁网络请求。

## 核心类型/函数
- `pub mod error` — 比赛错误类型模块声明
- **`ContestCache`**（内部 struct） — 比赛列表缓存（`contests: Vec<Contest>`, `fetched_at: Instant`）
- **`ContestService`** — 比赛服务
  - `fn new(registry, event_bus) -> Self` — 创建实例
  - `async fn list_contests(&self, cache_ttl_secs: u64) -> AppResult<Vec<Contest>>` — 获取比赛列表，优先使用缓存（TTL 由 Config 传入）；缓存命中直接返回，未命中请求远端 → 更新缓存 → 发布 `ContestEvent::ListLoaded`
  - `fn select_contest(&self, contest_id) -> AppResult<()>` — 选中比赛并发布 `ContestEvent::Selected`
  - `fn current_contest_id(&self) -> Option<String>` — 获取当前选中的比赛 ID
  - `async fn refresh(&self) -> AppResult<Vec<Contest>>` — 清空缓存并强制重新获取（等价 `list_contests(0)`）
  - `async fn load_contest_with_problems(&self, contest_id, password) -> AppResult<ContestBundle>` — 一次性获取比赛详情 + 题目列表 + 自动选中（阶段 7 单比赛模式入口）
- **字段**：`registry: Arc<dyn ProviderRegistry>`, `event_bus: Arc<EventBus>`, `current_contest: RwLock<Option<String>>`, `cache: RwLock<Option<ContestCache>>`

## 直接依赖
- `core::entity::contest::{Contest, ContestProblem, ContestBundle}`
- `core::error::{AppError, AppResult}`
- `core::event::app_event::{AppEvent, ContestEvent}`
- `core::event::event_bus::EventBus`
- `core::provider::registry::ProviderRegistry`

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<ContestService>`）

## 逻辑流程
- **list_contests(ttl)**：检查 `ContestCache::fetched_at` 是否在 TTL 内 → 命中则直接返回缓存；未命中调 `ContestProvider::list_contests()` → 更新缓存 → 发布 `ListLoaded` → 返回结果
- **select_contest(id)**：写入 `current_contest` → 发布 `Selected` 事件
- **refresh**：清空缓存 → 调 `list_contests(0)` 跳过缓存检查
- **load_contest_with_problems(id, password)**：`get_contest()` 获取详情 →（私有赛校验密码）→ `list_contest_problems()` 获取题目 → `select_contest()` 自动选中 → 返回 `ContestBundle { contest, problems }`
