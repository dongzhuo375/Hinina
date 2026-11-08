# mod

## 职责
比赛服务模块入口。负责比赛列表获取、TTL 缓存管理、比赛元信息缓存（内存 + 磁盘，固定 TTL）、当前比赛切换、强制刷新、比赛排行榜获取、比赛公告获取及公告已读状态持久化、**私有赛（auth==1）错误驱动进入（凭证报名 + 防轮询重复尝试记忆）**。通过 `ProviderRegistry` 获取当前 OJ 的 ContestProvider，比赛列表带 TTL 缓存避免频繁网络请求；比赛**元信息**（标题/时间窗/封榜设置）另带固定 TTL 的内存 + 磁盘缓存 —— 题目总览页每 30s±5s 轮询 `load_configured_contest`（元信息 + 题目列表两次请求），缓存元信息可把轮询请求量减半，而 ac/total 仍在每次轮询实时拉取；榜单与公告则明确不缓存（实时计算数据 / 可能含裁判组临场规则变更）。已读状态是客户端本地特性（HOJ 无对应服务端接口），持久化在 `announcements_read/{cid}_{uid}.json`。

**OJ 切换后的缓存清理走 `on_oj_switched()` 显式调用**（由 `commands/oj_cmd::switch_oj` 编排），**不订阅任何事件**。

## 核心类型/函数
- **`ContestCache`**（内部 struct） — 比赛列表缓存（`contests: Vec<Contest>`, `fetched_at: Instant`）。**保留自持实现**：其 TTL 由配置 `oj.cacheTtlSecs` 逐调用传入（可变），而 `TtlCache` 的 TTL 在构造期固定
- **常量 `CONTEST_META_TTL = 120s`** — 比赛元信息缓存 TTL（覆盖总览页 30s 轮询的 4 个周期；把「管理员改时间/封榜设置」的滞后压到 2 分钟内。比赛是否结束由前端依 `endTime` 推导，不受影响）
- **常量 `CONTEST_META_CAPACITY = 8`** — 元信息内存缓存容量上限
- **常量 `CONTEST_META_NAMESPACE = "cache/contest_meta"`** — 元信息磁盘缓存目录
- **常量 `ANNOUNCEMENTS_READ_DIR = "announcements_read"`** — 公告已读状态持久化目录（相对 Storage 根）
- **`ReadAnnouncementState`** — 已读状态文件结构：`read_ids: Vec<String>`（JSON 键 `readIds`）
- **`ContestService`** — 比赛服务
  - `fn new(registry, event_bus, storage, access) -> Self` — 创建实例（`storage` 用于公告已读状态读写）
  - `fn on_oj_switched(&self)` — OJ 切换后的缓存清理（**由 `switch_oj` 命令显式调用，不订阅事件**）。两段语义不同：**内存段同步清**（比赛列表缓存 + 元信息内存缓存 + 私有赛进入尝试记忆（走 `clear_access_attempts`），便宜且让当前会话立刻回到干净状态）；**磁盘段延迟清**（元信息 namespace 是纯空间回收 —— 缓存键自带 OJ 维度，跨 OJ 撞号在结构上不可能，故延迟清理不影响正确性）—— 有 tokio 上下文则 `Handle::spawn`，否则（纯同步调用 / 单元测试）同步兜底
  - `async fn list_contests(&self, cache_ttl_secs: u64) -> AppResult<Vec<Contest>>` — 获取比赛列表，优先使用缓存（TTL 由 Config 传入）；缓存命中直接返回，未命中请求远端 → 更新缓存 → 返回。**不再发布任何事件**：比赛列表是查询结果，真实数据由 IPC 返回给调用方；把查询结果伪装成事件既浪费一次全量克隆，又制造「前端可能靠事件拿数据」的错误预期
  - `fn select_contest(&self, contest_id) -> AppResult<()>` — 选中比赛（写 `current_contest`）并发布 `CoreEvent::ContestSelected { contest_id }`（事实通知，供审计 / 插件 / 前端刷新）
  - `fn current_contest_id(&self) -> Option<String>` — 获取当前选中的比赛 ID
  - `async fn refresh(&self) -> AppResult<Vec<Contest>>` — 清空比赛列表缓存 + **同时清空元信息缓存（内存 + 磁盘 namespace）** 并强制重新获取（等价 `list_contests(0)`）
  - `fn clear_caches(&self)` — 清空本服务的全部缓存（列表内存 + 元信息内存 + 元信息磁盘），**清完不重拉**（与 `refresh` 的区别：不重新获取，且不限于当前比赛）。供设置页「重置客户端」（`commands::maintenance_cmd::reset_client`）调用；**刻意保留公告基线**（它不是缓存，清掉会让清空后新发布的公告被当成「首次拉取」而漏报 —— 重置语义下要不要连它一起忘掉由调用方显式决定，见下）
  - `fn clear_announcement_baseline(&self)` — 忘掉公告基线（**仅「重置客户端」用**）。与 `clear_caches` 分开是有意的：基线不是缓存，两者语义相反 —— 清缓存保留它（否则新公告漏报），重置忘掉它（重置后一切皆未见，留着没有意义）。锁中毒时取回内部数据继续（基线下次拉取会自动重建）
  - `fn clear_announcement_read_state(&self) -> bool` — 清空全部公告已读状态（`announcements_read/`），返回是否确实删除了目录（不存在时 `false`，不算失败）。仅「重置客户端」用：表现是红点全部复亮，这正是重置应有的语义
  - `async fn load_contest_meta(&self, contest_id) -> AppResult<Contest>`（私有） — 比赛元信息：内存 → 磁盘 → 网络，命中即回填上游缓存；**只缓存成功结果**，Provider 错误（含 401/403）原样上抛
  - `async fn get_rank(&self, contest_id, query: &RankQuery) -> AppResult<ContestRankPage>` — 获取比赛排行榜（分页），经 registry 调 `ContestProvider::get_contest_rank`。**不做缓存**：HOJ 内榜每次实时计算（`doc/HOJ/HOJ-Contest-Rank-API.md` §4），缓存反而会给出过期名次；轮询节奏由前端控制（≥10s 且加抖动错峰、后台暂停）
  - `async fn list_problems(&self, contest_id) -> AppResult<Vec<ContestProblem>>` — 只读比赛题目列表，经 registry 调 `ContestProvider::list_contest_problems`。**插件查询路径专用**：不选中比赛、不落缓存（与 `load_contest_with_problems` 的自动选中 + 元信息缓存是刻意分开的两条取数路径——插件查询绑定运行中的作用域，不应有选中副作用，也不消费为页面轮询服务的缓存）
  - `async fn load_contest_with_problems(&self, contest_id, password) -> AppResult<ContestBundle>` — 一次性获取比赛详情（走元信息缓存）+ 题目列表（**每次实时**）+ 自动选中（阶段 7 单比赛模式入口）。**私有赛（`auth == 1`）错误驱动进入**：题目列表先行，仅当其失败且适配器判定属「需凭证进入」类（`ContestProvider::is_credential_rejection`——`Contest` 变体只是必要条件，「比赛还未开始」等业务失败同样落在该变体，不得误触发；首个错误以 `debug!` 留痕）时才走 `ensure_contest_access_core` 并重试一次 —— 已报名用户、比赛管理员、开放赛外榜单（HOJ 对这些用户不校验报名）**同步进入流程零额外请求**（配置密码时后台兜底仍会发起一次**不阻塞**的权限查询，见下）；`password` 来自配置 `oj.contestPassword`（命令层 `load_configured_contest` 传入）。**提交侧兜底**：题目拉取成功但未报名的选手（openRank 私有赛可见不可交——HOJ 提交通道对 `auth == 1` 始终要求报名，与 openRank 无关）在配置了密码时**后台报名（`tokio::spawn` 真后台，不阻塞本次加载返回——首载/记忆过期时的一次权限查询不再拖慢 30s 轮询）**，失败只留日志不影响查看——查看侧不得被它挡住
  - `async fn ensure_contest_access_core(memo, epoch_counter, provider, oj_id, contest_id, password, failure_log) -> AccessCoreResult`（文件级自由函数） — 私有赛进入校验的独立执行体：调 `ContestProvider::ensure_contest_access`（凭证语义由适配器解释，HOJ=比赛密码报名），带进程内防重复尝试记忆。**状态全部 owned**（memo/纪元为 Arc、provider 为 Arc、键字段 owned），错误驱动路径与后台兜底任务共用同一实现。返回 `AccessCoreResult` 三变体：`Attempted(AppResult<()>)`（真实发起了 provider 请求）、`Memoized(AppResult<()>)`（记忆命中复用，本轮未发起请求）、`Skipped`（距上次尝试不足最小重发间隔，本轮未发起任何请求）。记忆条目三形态（见 `AccessAttemptMemo`）：Success / Rejected(msg)（保留期 `ACCESS_MEMO_RETENTION` 10 分钟）/ Attempted（仅时间戳，最小重发间隔 `ACCESS_RETRY_MIN_INTERVAL` 45s）。**查记忆与 Miss 时记录 Attempted 在同一临界区完成**（并发的第二个加载必然看到 Attempted → Busy，双双发起注册 POST 的窗口结构性闭合；纪元校验同样生效）；完成后按 `ContestProvider::classify_access_rejection` 写回——仅注册被拒升级为 Rejected，其余（瞬态/查询阶段失败/网络/序列化/会话）保持 Attempted 退避。**纪元守卫在锁内复核**（见 `clear_access_attempts`）。失败日志按 `failure_log`（`tracing::Level`：错误驱动=WARN、后台=DEBUG）分级。错误经 `e.context("私有赛进入校验失败")` 上抛，变体穿透
  - `async fn run_best_effort_backfill(...)`（文件级自由函数） — 提交侧兜底的真后台执行体：包一层 `ensure_contest_access_core`（传 `tracing::Level::DEBUG`）——结果按形态分级：**Attempted 且分类为 Rejected 的失败升 `warn!`**（release 默认 info 下 debug 不落日志，「真实原因可追溯」的披露依赖这一条；Rejected 记忆已把重试压到保留期一次，不刷屏）；其余（Network 类退避、Memoized 复用、Skipped 跳过、成功）保持 `debug!`——查看侧不受任何影响。由 `load_contest_with_problems` 在有 tokio 上下文时 `Handle::spawn`、无上下文时同步退化
  - `fn credential_fingerprint(password) -> Option<u64>`（文件级自由函数） — 凭证的进程内指纹（`DefaultHasher` 固定键种子）：**记忆键不落明文密码**；同密码同指纹、不跨重启、不做安全用途，u64 碰撞在条目量级下可忽略
  - `fn clear_access_attempts(&self)` — 清空进入尝试记忆。报名状态是**账号级**的：同 OJ 换账号后，旧账号的成功记忆会让新账号跳过进入校验（题目接口报「请先到比赛首页输入比赛密码进行注册」——不含登录关键词，不触发会话守卫，页面停在无法自愈的错误上），失败记忆则让本可访问的新账号继续报旧错。四个调用点：成功 `login`（`commands::auth_cmd`）与 `logout`（同处，登出即进程内信号）、`reset_client`（`commands::maintenance_cmd`）、`on_oj_switched`。**先持 memo 锁再递增纪元**：写回侧在锁内复核纪元后才插回，清理与「检查+写回」互斥，纳秒级交错窗口彻底闭合
  - `async fn list_announcements(&self, contest_id, current_page: i64, limit: i64) -> AppResult<AnnouncementPage>` — 获取比赛公告（分页），经 registry 调 `ContestProvider::list_announcements`。**不做缓存**：公告可能包含裁判组临场发布的规则变更，必须每次拉取最新数据。每次拉取都会与上次结果比对，**出现新公告 ID 时发布 `CoreEvent::AnnouncementChanged { contest_id, new_ids }`**（红点提醒属状态变更，由 `main.rs` 的前端事件桥转发到 webview；**首次拉取只建基线不发事件** —— 没有基线可比，发了等于给每位选手一开机就亮红点）
  - `fn get_read_announcement_ids(&self, contest_id, uid) -> AppResult<Vec<String>>` — 读取某用户在某比赛下已读的公告 ID 列表。文件不存在视为「从未读过」；文件损坏只 `warn!` 并降级为空列表 —— 已读状态是纯 UI 便利特性，任何情况下都不应阻断公告展示
  - `fn mark_announcements_read(&self, contest_id, uid, ids: &[String]) -> AppResult<()>` — 标记公告为已读：与既有记录合并去重（保留首次出现顺序）后落盘；旧状态损坏时从空列表重建
  - `fn read_state_path(contest_id, uid) -> AppResult<String>`（私有） — 构造已读状态文件路径 `announcements_read/{cid}_{uid}.json`。cid / uid 来自会话与前端入参，**必须拒绝路径分隔符**（空串、`/`、`\`、`:`、`..` 均报 `AppError::Io`），防止写出存储根目录之外的文件
- **字段**：`registry: Arc<dyn ProviderRegistry>`, `event_bus: Arc<CoreEventBus>`, `storage: Arc<Storage>`, `current_contest: RwLock<Option<String>>`, `cache: Arc<RwLock<Option<ContestCache>>>`, `meta_cache: Arc<TtlCache<String, Contest>>`, `meta_disk: Arc<JsonDiskCache>`, `announcement_baseline: Arc<RwLock<HashMap<String, Vec<String>>>>`, `access_attempts: Arc<Mutex<AccessAttemptMemo>>`（私有赛进入尝试记忆：键 `(oj_id, contest_id, password 指纹)`、条目三形态 Success / Rejected(msg) / Attempted，时钟注入可测，见 `ensure_contest_access_core`；**Arc 包装供后台兜底任务 owned 持有**）, `access_epoch: Arc<AtomicU64>`（进入记忆的纪元：清理时**先持 memo 锁再递增**，写回侧**锁内复核**——在途写回不落新会话且清理不可能插在检查与插回之间）

## 直接依赖
- `serde::{Deserialize, Serialize}`（`ReadAnnouncementState` 持久化）
- `core::entity::announcement::AnnouncementPage`
- `core::entity::contest::{Contest, ContestBundle}`
- `core::entity::rank::{ContestRankPage, RankQuery}`
- `core::error::{AppError, AppResult}`（传播 Provider 错误仍只用 `e.context(...)` 不改写变体；`AppError` 仅用于本地已读状态自身的 `Io` / `Serialization` 错误构造）
- `core::event::core_event::CoreEvent`
- `core::event::core_event_bus::CoreEventBus`
- `core::provider::contest::ContestProvider`（进入校验调用的能力 trait）
- `core::provider::registry::ProviderRegistry`
- `infra::cache::{JsonDiskCache, TtlCache}`（比赛元信息内存 + 磁盘缓存）
- `infra::storage::Storage`（公告已读状态文件读写 + 元信息磁盘缓存底层）

## 被依赖

- `core::context`（`AppContext` 持有 `Arc<ContestService>`，构造时注入 `Arc<Storage>`）
- `commands::contest_cmd`（经 AppContext 转发 `list_contests` / `load_configured_contest` / `get_contest_rank` / `list_contest_announcements` / `get_read_announcement_ids` / `mark_announcements_read`；`select_contest` 命令已删除，`ContestService::select_contest` 由服务层内部复用）
- `commands::auth_cmd`（成功 `login` 与 `logout` → `clear_access_attempts()`：报名状态是账号级的，换账号后旧记忆不再成立）
- `commands::oj_cmd`（`switch_oj` → `on_oj_switched()` 显式清缓存，含进入尝试记忆）
- `commands::maintenance_cmd`（`reset_client` → `clear_caches` / `clear_access_attempts` / `clear_announcement_baseline` / `clear_announcement_read_state`）

## 逻辑流程
- **list_contests(ttl)**：检查 `ContestCache::fetched_at` 是否在 TTL 内 → 命中则直接返回缓存；未命中调 `ContestProvider::list_contests()`（失败 `warn!` + `e.context("获取比赛列表失败")`，变体不改写）→ 更新缓存 → 返回结果（**不发事件**）
- **on_oj_switched**：`cache = None` + `clear_access_attempts()` + `meta_cache.clear()`（内存段同步）→ `meta_disk.clear_namespace()`（有 tokio 上下文则 `Handle::spawn`，否则同步兜底）
- **select_contest(id)**：写入 `current_contest` → 发布 `CoreEvent::ContestSelected { contest_id }`
- **refresh**：清空比赛列表缓存 + 清空元信息内存缓存与磁盘 namespace → 调 `list_contests(0)` 跳过缓存检查
- **clear_caches**：清空 `cache`（列表内存）+ `meta_cache`（元信息内存）+ `meta_disk.clear_namespace()`（元信息磁盘）→ 返回（**不重拉**、不动 `announcement_baseline`）
- **clear_announcement_baseline**：`announcement_baseline.clear()` → 返回（锁中毒时 `into_inner` 取回内部数据继续）
- **clear_announcement_read_state**：`storage.exists("announcements_read")` 守卫 → `storage.remove_all(...)` → 返回是否删除（不存在或失败均为 `false`）
- **load_contest_meta(contest_id)**：内存缓存命中 → 直接返回；否则磁盘缓存（`read` 按 `fetchedAt` 判 TTL）命中 → 回填内存并返回；仍未命中 → `ContestProvider::get_contest()`（失败 `e.context("获取比赛详情失败")`）→ **成功才**回写内存 + 磁盘
- **get_rank(contest_id, query)**：`registry.current_contest()` → `ContestProvider::get_contest_rank()` → 失败先 `warn!` 再 `e.context("获取比赛榜单失败")` 上抛（**变体原样穿透**）；成功直接透传 `ContestRankPage`（records 前置副本的去重与真实人数推导由前端处理，Service 不加工）
- **list_problems(contest_id)**：`registry.current_contest()` → `ContestProvider::list_contest_problems()` → 失败 `warn!` + `e.context("获取比赛题目列表失败")`；成功透传，不选中比赛、不落缓存
- **load_contest_with_problems(id, password)**：provider 与 `oj_id` 经 `registry.current_contest_snapshot()` **同源快照**（捕获 id 一次、按该 id 反查 provider——两次独立读取在切换竞态下会拿到 A 的 provider 配 B 的 id，记忆键跨 OJ 错配）→ `load_contest_meta()` 获取详情（命中缓存时零请求）→ `list_contest_problems()` 拉题目（**每次实时**）：成功直接走兜底；失败且 `auth == 1` 且 `provider.is_credential_rejection(&err)`（适配器精确判据，首个错误 `debug!` 留痕）→ `ensure_contest_access_core()`：`Attempted(Ok)` / `Memoized(Ok)`（成功或保留期内的记忆命中）→ **重试题目列表一次**（重试失败上抛其 `e.context("获取比赛题目列表失败")`）；`Attempted(Err)` / `Memoized(Err)` → 上抛进入的引导/复用错误；**`Skipped`（距上次尝试不足最小重发间隔）→ 上抛原始题目错误**（本轮未发起任何请求，45s 窗口内的暂态展示，下周期自愈）；其余失败（其它变体、其它 Contest 业务失败如「比赛还未开始」、`auth != 1`）→ 原样 `e.context("获取比赛题目列表失败")` 上抛 → 题目成功且 `auth == 1` 且配置了密码 → **`tokio::spawn(run_best_effort_backfill(...))` 真后台兜底**（不阻塞返回；无 tokio 上下文时同步退化；结果按形态分级——Attempted 且 Rejected 失败升 `warn!`，其余 `debug!`）→ `select_contest()` 自动选中 → 返回 `ContestBundle { contest, problems }`
- **ensure_contest_access_core(memo, epoch, provider, oj_id, id, password, failure_log)**：拼记忆键（密码取 `credential_fingerprint` 指纹）→ 查记忆：Success（保留期内）直接放行 / Rejected 保留期内复用首次消息 / **Attempted 在最小重发间隔内 → Skipped** / 过期或无记录视为未尝试 → **发起请求前先记 Attempted**（纪元校验同样生效）→ 调 `provider.ensure_contest_access(id, password)` → 锁内复核纪元（清理持同一把锁先递增再清）后按 `classify_access_rejection` 写回：`Ok` → Success（保留期内跳过复查——服务端可能撤销报名，不能永久可信）；`Contest` 且分类 Rejected → 记失败消息；其余（瞬态/查询阶段失败/网络/序列化/会话）**不升级结论**（Attempted 条目退避）→ 结果经 `e.context("私有赛进入校验失败")` 上抛（日志级别按 `failure_log`：错误驱动 WARN / 后台 DEBUG）
- **list_announcements(contest_id, page, limit)**：`registry.current_contest()` → `ContestProvider::list_announcements()` → 失败先 `warn!` 再 `e.context("获取比赛公告")` 上抛（**变体原样穿透**）；成功先 `publish_new_announcements(contest_id, &page)` 再透传 `AnnouncementPage`，不缓存
- **get_read_announcement_ids(cid, uid)**：`read_state_path()` 校验并拼路径 → `storage.read_to_string()` 失败（不存在/读取错误）按未读返回空列表 → JSON 解析失败 `warn!` 后降级空列表
- **mark_announcements_read(cid, uid, ids)**：读既有已读列表（损坏时为空）→ 合并去重（保留首次出现顺序）→ `serde_json::to_string_pretty` 序列化（失败归 `AppError::Serialization`）→ `storage.write_string()` 落盘（失败 `e.context("写入公告已读状态失败")`）

## 设计要点
- **错误处理约定：用 `context()` 而不是重新包装**。向上传播 Provider 错误一律 `e.context("环节名")`（保留变体、仍补环节名、`warn!` 日志保留），**禁止** `AppError::Contest(format!("…: {}", e))` —— 那会把 401 改写成 `Contest` 变体，而变体是前端 `isAuthError` 分流与 `guards/sessionGuard.ts` 会话失效兜底的**唯一依据**（见 `core/error.md` 与 `doc/Architecture.md`「错误变体是分流依据，后端不得改写」）。改写后的现场表现：token 过期时榜单静默 stale、提交只弹一条错误文案、选手不被带回登录页，反复重试全部失败。
- **`get_rank` 是全场最高频的认证调用**（前端每 10s 轮询一次），因此它的变体穿透最关键：一旦改写，会话失效兜底链路等于整场失效。`list_contests`（登录页匿名简报）与 `load_contest_with_problems`（进场链路，外壳 `loadContest` 走的就是它）同样必须保留 `Auth` 变体，前端才能区分「连不上」与「凭证无效」。
- **公告不缓存**：与比赛列表（TTL 缓存）不同，公告可能包含裁判组临场发布的规则变更（澄清、封榜时间调整），拿到过期公告的代价远高于一次额外请求，故每次拉取最新数据，刷新节奏由前端控制。
- **新公告检测（`announcement_baseline: Arc<RwLock<HashMap<contest_id, Vec<String>>>>` + `publish_new_announcements`）**：公告是外部状态（裁判组在服务端发布），客户端唯一能感知的方式仍是拉取，但「有新公告」这件事必须走事件而不是让各视图各自比对列表 —— 红点提醒属状态变更，按项目约定归事件总线（查询走 Service、状态变更走事件总线）。三条不变量：① **首次拉取只建基线不发事件**（没有基线可比，发了等于给每位选手一开机就亮红点）；② **基线按比赛隔离**（同一进程内先看 1011 再看 1012 是常态，只留一份会让切回旧比赛时把既有公告误判成新公告 —— 红点误报）；③ **失败时保留旧基线**（拉取失败不推进基线，否则失败期间发布的公告会被永久漏报）。锁中毒（持有者 panic）时取回内部数据继续 —— 公告本身已经拿到了，只是红点提醒可能失效。事件由 `main.rs` 的前端事件桥（`spawn_frontend_event_bridge`）转发到前端 `announcements-published`。
- **比赛元信息缓存（内存 + 磁盘，TTL 120s）**：题目总览页每 30s±5s 轮询 `load_configured_contest`，其中 `get_contest`（标题/时间窗/封榜设置/allow_end_submit）几乎不变、`list_contest_problems`（含 ac/total）才需要新鲜 —— 只缓存前者可把该轮询的请求量减半，且**不牺牲任何计数新鲜度**。磁盘层带 `fetchedAt`，重启后继续计时（不会「重启即永久命中」）；`refresh()` 同时清两层，保证「强制刷新」拿到的是服务端真值。**缓存键带 OJ 维度**（`{oj}/{cid}`）：跨 OJ 同 cid 在结构上不可能互相命中，因此 `on_oj_switched()` 只是让当前会话回到干净状态 —— 内存段同步清、**磁盘段延迟清**（纯空间回收，不阻塞 `switch_oj`）。**错误永不入缓存**：首次 401 不得写缓存，否则会话恢复后仍返回旧错误。
- **已读状态永远不阻断公告展示**：读取路径上「文件不存在」「读取失败」「JSON 损坏」三种情况全部降级为空列表（损坏时 `warn!` 留痕），只有路径非法（分隔符注入）才报错 —— 已读标记是纯 UI 便利特性，不值得为它牺牲公告可达性。写入路径的合并去重保证多次标记幂等。
- **文件名净化是安全边界**：`{cid}_{uid}.json` 的两个组成部分分别来自前端入参与会话文件，`read_state_path` 拒绝空串、`/`、`\`、`:` 与 `..`，防止构造出存储根目录之外的写入路径。
- **`clear_caches` 与 `refresh` 是两件事**：`refresh` 是「我要服务端真值」→ 清两层 + 立即重拉；`clear_caches` 是「把本地缓存腾空」→ 只清不拉（补拉时机归调用方，设置页重置后立刻重拉当前比赛数据）。两者都不动 `announcement_baseline`：它不是缓存而是「已经告诉过用户哪些公告」的记忆，清掉会让清空之后新发布的公告在下次拉取时被当成「首次拉取」而**漏报**（有单测锁定）。
- **「重置」与「清缓存」对公告基线的处理相反，故分成两个方法**：`clear_caches` **保留**基线，`clear_announcement_baseline` **清空**它。合成一个方法会让其中一种语义出错（保留则重置不彻底；清空则清缓存后漏报新公告），两条用例成对锁定这个区别。
- **私有赛进入是错误驱动的**（而非「先查权限再拉题」）：HOJ 的题目访问判据比「报名」宽 —— `openRank != true` 才校验报名、管理员/创建者豁免（客户端没有建模 openRank/author），先查权限会把「私有赛 + 开放赛外榜单」与「管理员未报名」这两类能正常拉题的用户误挡在密码错误页；题目列表先行让这些用户**同步进入流程零额外请求**（配置密码时的后台兜底仍会发起一次不阻塞的权限查询），且已报名用户同样省掉每轮询一次的权限查询。触发判据**收在适配器侧**（`ContestProvider::is_credential_rejection`，HOJ 按自家消息特征片段分类，见 `adapter/hoj/mod.md`）：「需凭证的失败以 `Contest` 变体表达」只是必要条件不充分——「比赛还未开始」「该比赛不存在」等同样落在该变体，误触发会让用户看到密码引导而非真实原因；`auth == 1` 收窄到私有赛；触发前的首个题目错误以 `debug!` 留痕（进入失败时用户看到的是引导文案，原始原因只能靠日志追溯）。判据刻意保守：漏判（HOJ 改文案）→ 进入流程不触发、原始错误原样上抛——安全降级方向。
- **提交侧兜底（best-effort，真后台）补齐 openRank 缺口**：HOJ 的 `validateJudgeAuth` 对 `auth == 1` **始终要求报名，与 openRank 无关**——题目可拉≠能交。查看侧错误驱动修好后，未报名且配置了密码的 openRank 私有赛选手在题目拉取成功后**后台报名**（`tokio::spawn` 真后台 + owned 状态的独立执行体 `run_best_effort_backfill`，同一记忆防重复；无 tokio 上下文时同步退化）——不阻塞加载返回，首载/记忆过期时的一次权限查询不再拖慢 30s 轮询；失败只留日志不影响查看：查看侧不被它挡住，管理员未配密码也不受影响。**已知限制（有意接受并披露）**：openRank 私有赛 + 错误密码时查看成功、提交仍被 HOJ 拒（「请先注册该比赛」），真实原因「密码错误」以 **warn 级**落在日志（release 默认级别即可见——后台兜底对「真实发起过请求且被拒」的失败升 warn，记忆命中复用与退避跳过保持 debug，Rejected 记忆压到保留期一次不刷屏）——错误密码不触发错误驱动流程（题目能拉），兜底失败也不打断加载；自愈路径是用户在设置页更正密码后，下次 30s 轮询的兜底以新密码（新记忆 key）自动报名，无需重启。缺凭证引导同样归 Rejected（确定性结果，补填前恒同）：结论记忆让引导文案**持续显示**，不会与 Attempted 退避交错成「引导/裸错误」每两轮交替。
- **防重复尝试记忆是轮询副作用的对冲**：题目总览每 30s±5s 轮询进场链路，密码缺失/错误时若无记忆会对 OJ 每周期重复提交报名请求（可能触发服务端限频/锁定，选手反而被锁在场外）。条目三形态：**Success**（保留期内跳过复查——服务端可能撤销报名，成功不能永久可信，过期重新校验）；**Rejected(msg)**（`classify_access_rejection` 判定的注册被拒，保留期 `ACCESS_MEMO_RETENTION` 10 分钟——把错误密码下的报名 POST 压到每 10 分钟一次，期内复用首次引导消息）；**Attempted**（请求发起前记录的时间戳：网络/序列化/会话失败、瞬态与查询阶段业务失败——结论不升级，最小重发间隔 `ACCESS_RETRY_MIN_INTERVAL` 45s 内拦住同 key 重发，覆盖「注册在途超过轮询间隔」的防重发与「OJ 故障」的退避，过期放行自愈——「请稍后再试」因此真正可自愈）。记忆键的密码位是指纹（明文不落表），「设置页补填/更正密码」天然换 key 重试；`oj_id` 与 provider 经同源快照进键，切 OJ 竞态下不会错配。**账号切换有进程内信号**（登出/登录/重置都是显式命令编排），故 `clear_access_attempts()` 在成功 `login` / `logout` / `reset_client` / `on_oj_switched` 四处调用——换账号后旧记忆立即失效，不依赖重启。**清理是时点操作**：logout/login 期间仍在途的旧会话加载可能在清理后才完成，进入流程以纪元（`access_epoch`，清理时递增）守卫——发起请求前捕获、**Attempted 记录与写回前都校验**（否则旧账号的时间戳/结果会落进新会话，新账号首次尝试被无辜拦 45s）；读侧（记忆命中）的同类窗口只有微秒级且下次轮询自愈，不做守卫。**条目无限增长的对冲**：每次写回（Attempted/Success/Rejected）顺带清扫全表过期条目（表为常量级小）。`auth == 2`（保护赛）不走此路径：HOJ 保护赛报名语义未取证，保持原 TODO 范围。
- **锁中毒统一 `into_inner` 取回内部数据**（与 `TtlCache` / `provider_registry_impl` 同款约定，见 `provider_registry_impl.rs` 头注释）：本文件的 `cache` / `current_contest` / `announcement_baseline` / `access_attempts` 四处全部如此。相关容器是 `Option` / `HashMap`，panic 不会让它们结构不一致 —— 而「静默跳过」或「降级不发事件」的后果更糟：重置会留下脏缓存，公告基线则会在中毒后**永久**不再报新公告（每次调用都走 Err 分支提前返回）。`clear_announcement_read_state` 是唯一返回 `bool` 的清理方法（返回「是否确实删除了」，目录不存在时为 `false`），那是「本就没有」而非错误。

## 测试
`src-tauri/src/service/contest/tests/contest_tests.rs`（由 `mod.rs` 底部 `#[cfg(test)] #[path = "tests/contest_tests.rs"] mod tests;` 引用）以 `StubContestProvider` 锁定错误变体穿透与正常路径。控制面分三块：**前五个方法**由 `StubMode::{Ok, Auth, Network, Contest, ContestAlways}` 按同一模式响应（`Auth` 模拟 token 过期，即 HTTP 401 或 HOJ 体内 403「请您先登录」；`Network` 模拟断网；`Contest` 模拟「未报名/需凭证」类业务拒绝——is_credential_rejection 判定为 true 的形态；`ContestAlways` 无视报名状态恒失败，走通服务级成功记忆路径）；**第六个方法 `ensure_contest_access`** 由 `AccessMode::{Grant, WrongPassword, Network, Auth}` 单独控制；**`list_contest_problems`** 可被独立 `problems_mode` 覆盖（错误驱动用例需要「元信息成功 + 题目列表业务失败」的组合，全局 `mode` 会连 `get_contest` 一起失败），其中 `Contest` 语义在 `registered` 置位（模拟「报名解锁题目列表」）后放行、`ContestAlways` 不理会。`mode` 可在测试中途切换（模拟「缓存命中后服务端开始 401」等时序），`calls` 计数用于断言缓存真的省掉了请求。服务经 `ProviderRegistryImpl::new(OjId::new("HOJ"), access.clone())` + `register(OjId, ProviderSet)`（只挂 contest 能力）注册后构造（`make_service` 基于独立临时目录注入 `Storage`），异步用例各自建 current-thread runtime 避免嵌套 panic。

覆盖：`get_rank` 保留 `Auth` 变体且消息含「获取比赛榜单失败」环节名、`get_rank` 保留 `Network` 变体（不被改写成 `Contest`）、`list_contests` 保留 `Auth`、`load_contest_with_problems` 保留 `Auth`（进场时 401 必须触发会话守卫）、`list_announcements` 保留 `Auth` / `Network` 变体；成功路径 `get_rank` 返回分页（records/uid 正确）、`load_contest_with_problems` 返回 `ContestBundle` 并**自动选中比赛**（`current_contest_id()` 为 `Some("1011")`）、`list_announcements` 返回分页。

**比赛列表 TTL 缓存语义**：缓存命中时第二次调用不再触达 Provider（`call_count` 为 1）、TTL 过期后重新拉取、`refresh` 在 TTL 内也强制触达 Provider、刷新失败不留 stale 缓存。

**比赛元信息缓存（内存 + 磁盘）**：二次 `load_contest_with_problems` 不再触达 `get_contest`（`meta_call_count` 为 1）而 `list_contest_problems` 仍每次实时（`problems_call_count` 为 2）；新实例共用同一临时目录时元信息命中磁盘缓存（`meta_call_count` 为 0，模拟重启）；不同 `contest_id` 互不命中（按比赛隔离）；`refresh` 后必须重新请求（两层同时清）；首次 401 不入缓存（恢复后 `meta_call_count` 为 2，且变体保持 `Auth`）。

**私有赛进入与防重试记忆**：公开赛（auth=0）题目失败不走进入流程（原样上抛、`access_call_count` 为 0）；私有赛题目可拉且无密码时零进入请求（错误驱动的关键不变量，`password=None` 隔离提交侧兜底）；题目「需凭证类」失败时发起进入、配置密码透传到 provider（`last_access_credential` 断言）、进入成功后重试题目列表（stub 以 `registered` 标志模拟「报名解锁题目列表」，进入失败不重试）；**提交侧 best-effort 兜底**：题目可拉 + 配置密码 → **真后台报名一次**（`open_rank_private_contest_backfills_registration_best_effort`，用例内以 `yield_now` 轮询等 spawn 任务完成——后台化后断言不能直接跟在 load 后），报名失败不影响加载（`best_effort_backfill_failure_does_not_break_loading`）；业务失败（`AccessMode::WrongPassword`）保留 `Contest` 变体 + 环节名且上抛进入的引导错误、第二次加载命中记忆不再触达 provider 且消息与首次一致；网络失败（`AccessMode::Network`）与会话失败（`AccessMode::Auth`）不升级结论（条目保持 Attempted：最小重发间隔内被拦、过期重试；变体保持——首轮失败即 Authentic/Network 上抛触发会话守卫/网络提示，退避窗口内的次轮上抛原始题目错误）；**成功记忆的服务级路径**由 `success_memo_hits_when_problems_still_fail`（`ContestAlways` 使报名后题目仍失败）锁定——删掉成功记忆分支该用例会红；密码更正后新 key 重新发起；`on_oj_switched()` 与 `clear_access_attempts()` 清空记忆（切换/换账号后必须重新发起）；**在途写回**由 `in_flight_access_attempt_is_not_written_back_after_clear` 以带延迟的 stub 复现竞态锁定（变异验证：去掉纪元守卫该用例变红）；**并发防重发**由 `concurrent_loads_do_not_double_register`（多线程 runtime 双加载同时进入 core）锁定——同临界区 lookup+begin 下第二个加载必然 Busy（结构性正确由单临界区实现保证，变异在单线程调度下不可复现）；**命令接线**（logout / login / reset_client 三处的清空调用）由 `commands::contest_cmd::tests` 的三条全真栈用例锁定（logout 后重新触达被 require_token 拦为 Auth、login/reset 后 access 查询再次出现，logout 变异验证红）。memo 保留期由注入时钟单测：Rejected 项保留期内命中、期满即 Miss 且条目被清；**Success 同受保留期约束**（`access_memo_success_expires_for_recheck` 断言期满即 Miss——服务端可能撤销报名，不能永久可信）；Attempted 条目最小间隔内 Busy、过期放行；写回时顺带清扫全表过期条目（`access_memo_record_sweeps_expired_entries`）；密码以指纹进键（`credential_fingerprint_distinguishes_passwords`）。stub 的 `set_auth` / `set_problems_mode` / `set_access_mode` / `set_ensure_delay_ms` / `access_calls` / `access_credential` 支撑以上断言。

**公告已读状态（客户端本地特性）**：写入-读取往返且多次标记合并去重（保留首次出现顺序）、损坏文件降级为空列表而不报错、路径含分隔符（`/`、`\`、`..` 等）被拒绝（`AppError::Io`）。

**新公告检测（红点提醒的事件源）**：首次拉取只建基线不发事件、第二次多一条公告时**只发一次**且载荷只带新增 ID、列表不变时重复拉取不再发事件、基线按比赛隔离（只看过 1011 时首次看 1012 不误报）、拉取失败保留旧基线（恢复后仍能报出失败期间新增的公告）。Stub 的 `announcements` 可中途覆盖（`set_announcements`）以模拟裁判组发布；`make_service_with_bus` 额外交出 `Arc<CoreEventBus>` 供 `collect_contest_events` 断言事件契约。

**OJ 切换清理**：`on_oj_switched_clears_contest_scoped_caches`（列表内存 / 元信息内存 / 元信息磁盘三层全空且不发请求）、`on_oj_switched_disk_cleanup_runs_on_runtime`（有 tokio 上下文时磁盘段走后台任务，断言最终清空）。

**事件契约**：`select_contest_publishes_contest_selected`（选中比赛发布 `ContestSelected`）、`list_contests_publishes_nothing`（**查询结果不发任何事件** —— 锁定「列表不再伪装成事件」这条重构决定）。

**清空缓存（设置页维护动作）**：`clear_caches` 后列表内存 / 元信息内存 / 元信息磁盘三层全空、清理本身不发任何请求、清后再查必须回源；`clear_caches` 保留公告基线（清空后新发的公告仍能被报出 —— 丢基线则漏报）。

**重置客户端**：`clear_announcement_baseline` 后下次拉取重新建基线（等价「首次拉取」，不报新公告 —— 与上一条成对，锁定「清缓存保留 / 重置忘掉」的区别）；`clear_announcement_read_state` 删净已读状态文件（表现 = 红点复亮）、目录不存在时返回 `false` 且不报错（幂等）。

## 只读接口与作用域

read_metadata只查远端元信息，不选择比赛、不走持久缓存。select_contest只有ID实际改变才更新共享revision；反复刷新同一比赛不撤销。on_oj_switched显式撤销并清空current_contest，避免跨OJ复用比赛上下文。

访问栅栏为构造必填参数，无私有默认栅栏或with_access后置绑定。组合根创建一次共享Arc<AccessRevision>，测试夹具也显式指定。
