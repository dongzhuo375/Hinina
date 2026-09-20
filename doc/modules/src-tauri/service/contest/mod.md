# mod

## 职责
比赛服务模块入口。负责比赛列表获取、TTL 缓存管理、比赛元信息缓存（内存 + 磁盘，固定 TTL）、当前比赛切换、强制刷新、比赛排行榜获取、比赛公告获取及公告已读状态持久化。通过 `ProviderRegistry` 获取当前 OJ 的 ContestProvider，比赛列表带 TTL 缓存避免频繁网络请求；比赛**元信息**（标题/时间窗/封榜设置）另带固定 TTL 的内存 + 磁盘缓存 —— 题目总览页每 30s±5s 轮询 `load_configured_contest`（元信息 + 题目列表两次请求），缓存元信息可把轮询请求量减半，而 ac/total 仍在每次轮询实时拉取；榜单与公告则明确不缓存（实时计算数据 / 可能含裁判组临场规则变更）。已读状态是客户端本地特性（HOJ 无对应服务端接口），持久化在 `announcements_read/{cid}_{uid}.json`。

## 核心类型/函数
- `pub mod error` — 比赛错误类型模块声明
- **`ContestCache`**（内部 struct） — 比赛列表缓存（`contests: Vec<Contest>`, `fetched_at: Instant`）。**保留自持实现**：其 TTL 由配置 `oj.cacheTtlSecs` 逐调用传入（可变），而 `TtlCache` 的 TTL 在构造期固定
- **常量 `CONTEST_META_TTL = 120s`** — 比赛元信息缓存 TTL（覆盖总览页 30s 轮询的 4 个周期；把「管理员改时间/封榜设置」的滞后压到 2 分钟内。比赛是否结束由前端依 `endTime` 推导，不受影响）
- **常量 `CONTEST_META_CAPACITY = 8`** — 元信息内存缓存容量上限
- **常量 `CONTEST_META_NAMESPACE = "cache/contest_meta"`** — 元信息磁盘缓存目录
- **常量 `ANNOUNCEMENTS_READ_DIR = "announcements_read"`** — 公告已读状态持久化目录（相对 Storage 根）
- **`ReadAnnouncementState`** — 已读状态文件结构：`read_ids: Vec<String>`（JSON 键 `readIds`）
- **`ContestService`** — 比赛服务
  - `fn new(registry, event_bus, storage) -> Self` — 创建实例（`storage` 用于公告已读状态读写）
  - `async fn list_contests(&self, cache_ttl_secs: u64) -> AppResult<Vec<Contest>>` — 获取比赛列表，优先使用缓存（TTL 由 Config 传入）；缓存命中直接返回，未命中请求远端 → 更新缓存 → 发布 `ContestEvent::ListLoaded`
  - `fn select_contest(&self, contest_id) -> AppResult<()>` — 选中比赛并发布 `ContestEvent::Selected`
  - `fn current_contest_id(&self) -> Option<String>` — 获取当前选中的比赛 ID
  - `async fn refresh(&self) -> AppResult<Vec<Contest>>` — 清空比赛列表缓存 + **同时清空元信息缓存（内存 + 磁盘 namespace）** 并强制重新获取（等价 `list_contests(0)`）
  - `fn clear_caches(&self)` — 清空本服务的全部缓存（列表内存 + 元信息内存 + 元信息磁盘），**清完不重拉**（与 `refresh` 的区别：不重新获取，且不限于当前比赛）。供设置页「重置客户端」（`commands::maintenance_cmd::reset_client`）调用；**刻意保留公告基线**（它不是缓存，清掉会让清空后新发布的公告被当成「首次拉取」而漏报 —— 重置语义下要不要连它一起忘掉由调用方显式决定，见下）
  - `fn clear_announcement_baseline(&self)` — 忘掉公告基线（**仅「重置客户端」用**）。与 `clear_caches` 分开是有意的：基线不是缓存，两者语义相反 —— 清缓存保留它（否则新公告漏报），重置忘掉它（重置后一切皆未见，留着没有意义）。锁中毒时跳过而不让重置失败（基线下次拉取会自动重建）
  - `fn clear_announcement_read_state(&self) -> bool` — 清空全部公告已读状态（`announcements_read/`），返回是否确实删除了目录（不存在时 `false`，不算失败）。仅「重置客户端」用：表现是红点全部复亮，这正是重置应有的语义
  - `async fn load_contest_meta(&self, contest_id) -> AppResult<Contest>`（私有） — 比赛元信息：内存 → 磁盘 → 网络，命中即回填上游缓存；**只缓存成功结果**，Provider 错误（含 401/403）原样上抛
  - `async fn get_rank(&self, contest_id, query: &RankQuery) -> AppResult<ContestRankPage>` — 获取比赛排行榜（分页），经 registry 调 `ContestProvider::get_contest_rank`。**不做缓存**：HOJ 内榜每次实时计算（`doc/HOJ/HOJ-Contest-Rank-API.md` §4），缓存反而会给出过期名次；轮询节奏由前端控制（≥10s 且加抖动错峰、后台暂停）
  - `async fn load_contest_with_problems(&self, contest_id, password) -> AppResult<ContestBundle>` — 一次性获取比赛详情（走元信息缓存）+ 题目列表（**每次实时**）+ 自动选中（阶段 7 单比赛模式入口）
  - `async fn list_announcements(&self, contest_id, current_page: i64, limit: i64) -> AppResult<AnnouncementPage>` — 获取比赛公告（分页），经 registry 调 `ContestProvider::list_announcements`。**不做缓存**：公告可能包含裁判组临场发布的规则变更，必须每次拉取最新数据。每次拉取都会与上次结果比对，**出现新公告 ID 时发布 `ContestEvent::AnnouncementsPublished { contest_id, new_ids }`**（红点提醒属状态变更，由 `main.rs` 的事件桥转发到前端；**首次拉取只建基线不发事件** —— 没有基线可比，发了等于给每位选手一开机就亮红点）
  - `fn get_read_announcement_ids(&self, contest_id, uid) -> AppResult<Vec<String>>` — 读取某用户在某比赛下已读的公告 ID 列表。文件不存在视为「从未读过」；文件损坏只 `warn!` 并降级为空列表 —— 已读状态是纯 UI 便利特性，任何情况下都不应阻断公告展示
  - `fn mark_announcements_read(&self, contest_id, uid, ids: &[String]) -> AppResult<()>` — 标记公告为已读：与既有记录合并去重（保留首次出现顺序）后落盘；旧状态损坏时从空列表重建
  - `fn read_state_path(contest_id, uid) -> AppResult<String>`（私有） — 构造已读状态文件路径 `announcements_read/{cid}_{uid}.json`。cid / uid 来自会话与前端入参，**必须拒绝路径分隔符**（空串、`/`、`\`、`:`、`..` 均报 `AppError::Io`），防止写出存储根目录之外的文件
- **字段**：`registry: Arc<dyn ProviderRegistry>`, `event_bus: Arc<EventBus>`, `storage: Arc<Storage>`, `current_contest: RwLock<Option<String>>`, `cache: Arc<RwLock<Option<ContestCache>>>`, `meta_cache: Arc<TtlCache<String, Contest>>`, `meta_disk: Arc<JsonDiskCache>`, `announcement_baseline: Arc<RwLock<HashMap<String, Vec<String>>>>`（三处缓存 Arc 包装是为了共享进 `OJSwitched` 订阅闭包 —— 闭包只捕获缓存 Arc，不捕获 service/总线，无引用环）

## 直接依赖
- `serde::{Deserialize, Serialize}`（`ReadAnnouncementState` 持久化）
- `core::entity::announcement::AnnouncementPage`
- `core::entity::contest::{Contest, ContestBundle}`
- `core::entity::rank::{ContestRankPage, RankQuery}`
- `core::error::{AppError, AppResult}`（传播 Provider 错误仍只用 `e.context(...)` 不改写变体；`AppError` 仅用于本地已读状态自身的 `Io` / `Serialization` 错误构造）
- `core::event::app_event::{AppEvent, ContestEvent}`
- `core::event::event_bus::EventBus`
- `core::provider::registry::ProviderRegistry`
- `infra::cache::{JsonDiskCache, TtlCache}`（比赛元信息内存 + 磁盘缓存）
- `infra::storage::Storage`（公告已读状态文件读写 + 元信息磁盘缓存底层）

## 被依赖

- `core::context`（`AppContext` 持有 `Arc<ContestService>`，构造时注入 `Arc<Storage>`）
- `commands::contest_cmd`（经 AppContext 转发 `list_contests` / `select_contest` / `load_configured_contest` / `get_contest_rank` / `list_contest_announcements` / `get_read_announcement_ids` / `mark_announcements_read`）
- `commands::maintenance_cmd`（`reset_client` → `clear_caches` / `clear_announcement_baseline` / `clear_announcement_read_state`）

## 逻辑流程
- **list_contests(ttl)**：检查 `ContestCache::fetched_at` 是否在 TTL 内 → 命中则直接返回缓存；未命中调 `ContestProvider::list_contests()`（失败 `warn!` + `e.context("获取比赛列表失败")`，变体不改写）→ 更新缓存 → 发布 `ListLoaded` → 返回结果
- **select_contest(id)**：写入 `current_contest` → 发布 `Selected` 事件
- **refresh**：清空比赛列表缓存 + 清空元信息内存缓存与磁盘 namespace → 调 `list_contests(0)` 跳过缓存检查
- **clear_caches**：清空 `cache`（列表内存）+ `meta_cache`（元信息内存）+ `meta_disk.clear_namespace()`（元信息磁盘）→ 返回（**不重拉**、不动 `announcement_baseline`）
- **clear_announcement_baseline**：`announcement_baseline.clear()` → 返回（锁中毒时 `warn` 并跳过）
- **clear_announcement_read_state**：`storage.exists("announcements_read")` 守卫 → `storage.remove_all(...)` → 返回是否删除（不存在或失败均为 `false`）
- **load_contest_meta(contest_id)**：内存缓存命中 → 直接返回；否则磁盘缓存（`read` 按 `fetchedAt` 判 TTL）命中 → 回填内存并返回；仍未命中 → `ContestProvider::get_contest()`（失败 `e.context("获取比赛详情失败")`）→ **成功才**回写内存 + 磁盘
- **get_rank(contest_id, query)**：`registry.current_contest()` → `ContestProvider::get_contest_rank()` → 失败先 `warn!` 再 `e.context("获取比赛榜单失败")` 上抛（**变体原样穿透**）；成功直接透传 `ContestRankPage`（records 前置副本的去重与真实人数推导由前端处理，Service 不加工）
- **load_contest_with_problems(id, password)**：`load_contest_meta()` 获取详情（命中缓存时零请求）→（私有赛校验密码）→ `list_contest_problems()` 获取题目（**每次实时**，失败 `e.context("获取比赛题目列表失败")`）→ `select_contest()` 自动选中 → 返回 `ContestBundle { contest, problems }`
- **list_announcements(contest_id, page, limit)**：`registry.current_contest()` → `ContestProvider::list_announcements()` → 失败先 `warn!` 再 `e.context("获取比赛公告")` 上抛（**变体原样穿透**）；成功先 `publish_new_announcements(contest_id, &page)` 再透传 `AnnouncementPage`，不缓存
- **get_read_announcement_ids(cid, uid)**：`read_state_path()` 校验并拼路径 → `storage.read_to_string()` 失败（不存在/读取错误）按未读返回空列表 → JSON 解析失败 `warn!` 后降级空列表
- **mark_announcements_read(cid, uid, ids)**：读既有已读列表（损坏时为空）→ 合并去重（保留首次出现顺序）→ `serde_json::to_string_pretty` 序列化（失败归 `AppError::Serialization`）→ `storage.write_string()` 落盘（失败 `e.context("写入公告已读状态失败")`）

## 设计要点
- **错误处理约定：用 `context()` 而不是重新包装**。向上传播 Provider 错误一律 `e.context("环节名")`（保留变体、仍补环节名、`warn!` 日志保留），**禁止** `AppError::Contest(format!("…: {}", e))` —— 那会把 401 改写成 `Contest` 变体，而变体是前端 `isAuthError` 分流与 `stores/sessionGuard.ts` 会话失效兜底的**唯一依据**（见 `core/error.md` 与 `doc/Architecture.md`「错误变体是分流依据，后端不得改写」）。改写后的现场表现：token 过期时榜单静默 stale、提交只弹一条错误文案、选手不被带回登录页，反复重试全部失败。
- **`get_rank` 是全场最高频的认证调用**（前端每 10s 轮询一次），因此它的变体穿透最关键：一旦改写，会话失效兜底链路等于整场失效。`list_contests`（登录页匿名简报）与 `load_contest_with_problems`（进场链路，外壳 `loadContest` 走的就是它）同样必须保留 `Auth` 变体，前端才能区分「连不上」与「凭证无效」。
- **公告不缓存**：与比赛列表（TTL 缓存）不同，公告可能包含裁判组临场发布的规则变更（澄清、封榜时间调整），拿到过期公告的代价远高于一次额外请求，故每次拉取最新数据，刷新节奏由前端控制。
- **新公告检测（`announcement_baseline: Arc<RwLock<HashMap<contest_id, Vec<String>>>>` + `publish_new_announcements`）**：公告是外部状态（裁判组在服务端发布），客户端唯一能感知的方式仍是拉取，但「有新公告」这件事必须走事件而不是让各视图各自比对列表 —— 红点提醒属状态变更，按项目约定归 EventBus（查询走 Service、状态变更走 EventBus）。三条不变量：① **首次拉取只建基线不发事件**（没有基线可比，发了等于给每位选手一开机就亮红点）；② **基线按比赛隔离**（同一进程内先看 1011 再看 1012 是常态，只留一份会让切回旧比赛时把既有公告误判成新公告 —— 红点误报）；③ **失败时保留旧基线**（拉取失败不推进基线，否则失败期间发布的公告会被永久漏报）。锁中毒（持有者 panic）时跳过检测而不让公告查询失败 —— 公告本身已经拿到了，只是红点提醒失效。事件由 `main.rs` 的 `install_announcement_event_bridge` 转发到前端 `announcements-published`。
- **比赛元信息缓存（内存 + 磁盘，TTL 120s）**：题目总览页每 30s±5s 轮询 `load_configured_contest`，其中 `get_contest`（标题/时间窗/封榜设置/allow_end_submit）几乎不变、`list_contest_problems`（含 ac/total）才需要新鲜 —— 只缓存前者可把该轮询的请求量减半，且**不牺牲任何计数新鲜度**。磁盘层带 `fetchedAt`，重启后继续计时（不会「重启即永久命中」）；`refresh()` 同时清两层，保证「强制刷新」拿到的是服务端真值。**缓存键带 OJ 维度**（`{oj}/{cid}`）：跨 OJ 同 cid 在结构上不可能互相命中，因此构造时订阅 `OJSwitched` 只是让当前会话回到干净状态 —— 内存段同步清、**磁盘段延迟清**（`subscribe_deferred`，I/O 不阻塞 `switch_oj`）。**错误永不入缓存**：首次 401 不得写缓存，否则会话恢复后仍返回旧错误。
- **已读状态永远不阻断公告展示**：读取路径上「文件不存在」「读取失败」「JSON 损坏」三种情况全部降级为空列表（损坏时 `warn!` 留痕），只有路径非法（分隔符注入）才报错 —— 已读标记是纯 UI 便利特性，不值得为它牺牲公告可达性。写入路径的合并去重保证多次标记幂等。
- **文件名净化是安全边界**：`{cid}_{uid}.json` 的两个组成部分分别来自前端入参与会话文件，`read_state_path` 拒绝空串、`/`、`\`、`:` 与 `..`，防止构造出存储根目录之外的写入路径。
- **`clear_caches` 与 `refresh` 是两件事**：`refresh` 是「我要服务端真值」→ 清两层 + 立即重拉；`clear_caches` 是「把本地缓存腾空」→ 只清不拉（补拉时机归调用方，设置页重置后立刻重拉当前比赛数据）。两者都不动 `announcement_baseline`：它不是缓存而是「已经告诉过用户哪些公告」的记忆，清掉会让清空之后新发布的公告在下次拉取时被当成「首次拉取」而**漏报**（有单测锁定）。
- **「重置」与「清缓存」对公告基线的处理相反，故分成两个方法**：`clear_caches` **保留**基线，`clear_announcement_baseline` **清空**它。合成一个方法会让其中一种语义出错（保留则重置不彻底；清空则清缓存后漏报新公告），两条用例成对锁定这个区别。
- **锁中毒统一 `into_inner` 取回内部数据**（与 `TtlCache` / `provider_registry_impl` 同款约定，见 `provider_registry_impl.rs` 头注释）：本文件的 `cache` / `current_contest` / `announcement_baseline` 三处全部如此。相关容器是 `Option` / `HashMap`，panic 不会让它们结构不一致 —— 而「静默跳过」或「降级不发事件」的后果更糟：重置会留下脏缓存，公告基线则会在中毒后**永久**不再报新公告（每次调用都走 Err 分支提前返回）。`clear_announcement_read_state` 是唯一返回 `bool` 的清理方法（返回「是否确实删除了」，目录不存在时为 `false`），那是「本就没有」而非错误。

## 测试
`src-tauri/src/service/contest/tests/contest_tests.rs`（由 `mod.rs` 底部 `#[cfg(test)] #[path = "tests/contest_tests.rs"] mod tests;` 引用）以 `StubContestProvider` 锁定错误变体穿透与正常路径。Stub 用 `StubMode::{Ok, Auth, Network}` 让全部五个 trait 方法按同一模式响应（`Auth` 模拟 token 过期，即 HTTP 401 或 HOJ 体内 403「请您先登录」；`Network` 模拟断网），便于逐方法断言变体是否被保留；`mode` 可在测试中途切换（模拟「缓存命中后服务端开始 401」等时序），`calls` 计数用于断言缓存真的省掉了请求。服务经 `ProviderRegistryImpl::new(OjId::new("HOJ"))` + `register(OjId, ProviderSet)`（只挂 contest 能力）注册后构造（`make_service` 基于独立临时目录注入 `Storage`），异步用例各自建 current-thread runtime 避免嵌套 panic。

覆盖：`get_rank` 保留 `Auth` 变体且消息含「获取比赛榜单失败」环节名、`get_rank` 保留 `Network` 变体（不被改写成 `Contest`）、`list_contests` 保留 `Auth`、`load_contest_with_problems` 保留 `Auth`（进场时 401 必须触发会话守卫）、`list_announcements` 保留 `Auth` / `Network` 变体；成功路径 `get_rank` 返回分页（records/uid 正确）、`load_contest_with_problems` 返回 `ContestBundle` 并**自动选中比赛**（`current_contest_id()` 为 `Some("1011")`）、`list_announcements` 返回分页。

**比赛列表 TTL 缓存语义**：缓存命中时第二次调用不再触达 Provider（`call_count` 为 1）、TTL 过期后重新拉取、`refresh` 在 TTL 内也强制触达 Provider、刷新失败不留 stale 缓存。

**比赛元信息缓存（内存 + 磁盘）**：二次 `load_contest_with_problems` 不再触达 `get_contest`（`meta_call_count` 为 1）而 `list_contest_problems` 仍每次实时（`problems_call_count` 为 2）；新实例共用同一临时目录时元信息命中磁盘缓存（`meta_call_count` 为 0，模拟重启）；不同 `contest_id` 互不命中（按比赛隔离）；`refresh` 后必须重新请求（两层同时清）；首次 401 不入缓存（恢复后 `meta_call_count` 为 2，且变体保持 `Auth`）。

**公告已读状态（客户端本地特性）**：写入-读取往返且多次标记合并去重（保留首次出现顺序）、损坏文件降级为空列表而不报错、路径含分隔符（`/`、`\`、`..` 等）被拒绝（`AppError::Io`）。

**新公告检测（红点提醒的事件源）**：首次拉取只建基线不发事件、第二次多一条公告时**只发一次**且载荷只带新增 ID、列表不变时重复拉取不再发事件、基线按比赛隔离（只看过 1011 时首次看 1012 不误报）、拉取失败保留旧基线（恢复后仍能报出失败期间新增的公告）。Stub 的 `announcements` 可中途覆盖（`set_announcements`）以模拟裁判组发布；`make_service_with_bus` 额外交出 `EventBus` 供 `collect_contest_events` 断言事件契约。

**清空缓存（设置页维护动作）**：`clear_caches` 后列表内存 / 元信息内存 / 元信息磁盘三层全空、清理本身不发任何请求、清后再查必须回源；`clear_caches` 保留公告基线（清空后新发的公告仍能被报出 —— 丢基线则漏报）。

**重置客户端**：`clear_announcement_baseline` 后下次拉取重新建基线（等价「首次拉取」，不报新公告 —— 与上一条成对，锁定「清缓存保留 / 重置忘掉」的区别）；`clear_announcement_read_state` 删净已读状态文件（表现 = 红点复亮）、目录不存在时返回 `false` 且不报错（幂等）。
