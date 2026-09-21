# mod

## 职责
题目服务模块入口。负责题目详情打开、题面缓存（内存 + 磁盘，受配置开关控制）、用户题目状态批量查询、题目 limits 批量获取（内存 + 磁盘双层缓存）。打开题目只发布 `CoreEvent::ProblemOpened`（**事实通知**）：本服务**不**持有 `WorkspaceManager`，工作区的创建/切换由调用方编排（前端 `ProblemSolveView` → `workspaceStore.loadWorkspace`），题目详情与工作区是两个独立关注点。

**OJ 切换后的缓存清理走 `on_oj_switched()` 显式调用**（由 `commands/oj_cmd::switch_oj` 编排），**不订阅任何事件**。

## 核心类型/函数
- `pub mod error` — 题目错误类型模块声明
- 常量：`LIMITS_CACHE_DIR = "cache/problem_limits"`（磁盘缓存目录，limits 只能从题目详情接口取得且对同一题基本不变，跨重启复用可省掉整批详情请求）、`LIMITS_CONCURRENCY = 4`（并发扇出上限：既要让首屏尽快补齐，也不能让单客户端瞬间打爆 OJ）
- 常量（公开）：`PROBLEM_CACHE_TTL = 30min`（题面缓存 TTL —— 题面在比赛期间基本不变但并非永不变化，管理员可能中途修正，30 分钟是「切题来回/重进应用几乎总能命中」与「修正最多滞后半小时」的折中）、`PROBLEM_CACHE_CAPACITY = 200`、`PROBLEM_CACHE_NAMESPACE = "cache/problem_statement"`
- 私有函数：`statement_key(oj_id, contest_id, display_id) -> String` — 题面缓存键 `{oj}/{contest_id}/{display_id}`（三重作用域：OJ / 比赛 / 题目；磁盘按目录分层；键的安全性由 `JsonDiskCache::key_path` 统一把关）
- 私有自由函数：`clear_problem_disk_caches(statement_disk, storage) -> (bool, bool)` — 清空题面与 limits 的**磁盘**缓存，返回 `(题面已清, limits 已清)`。抽成自由函数的原因：`on_oj_switched`（后台任务）与 `clear_caches`（用户显式点击、需同步完成并回报结果）必须清同一批目录 —— 两处各写一遍迟早漂移，而漂移的后果是「切 OJ 清了、点按钮没清」这类难以复现的脏读。limits 目录带**存在性守卫**（从未缓存过时 `remove_all` 会返回 NotFound，不该当成失败告警）
- **`ProblemService`** — 题目服务
  - `fn new(registry, event_bus, storage) -> Self` — 创建实例（`storage` 用于 limits 与题面磁盘缓存）
  - `fn on_oj_switched(&self)` — OJ 切换后的缓存清理（**由 `switch_oj` 命令显式调用，不订阅事件**）。两段语义不同：**内存段同步清**（limits 内存表 + 题面内存缓存，便宜且让当前会话立刻干净）；**磁盘段延迟清**（纯空间回收 —— 三层缓存的键都带 OJ 维度，跨 OJ 撞号在结构上不可能，延迟清理不影响正确性）—— 有 tokio 上下文则 `Handle::spawn`，否则同步兜底。为什么不走事件消费者：`switch_oj` 返回后紧接着就可能有新 OJ 的题目查询进来，「清缓存」是切换正确性的一部分；旧实现靠同步投递保证「发布即已清」，而那个保证建立在「发布方与订阅者同栈」的隐含前提上，换成 broadcast 后该前提不再成立
  - `async fn open_problem(&self, contest_id, problem_id, cache_enabled: bool) -> AppResult<Problem>` — 获取题目详情并发布 `CoreEvent::ProblemOpened { contest_id, problem_id }`。题面内容**不进入事件**：它是大载荷，且真实数据由 IPC 返回值承载；事件只让其他观察者（插件、前端其他页面）知道「有题目被打开了」。`cache_enabled` 来自配置 `oj.cacheProblemStatement`，**无论命中与否都照常发布事件**
  - `async fn load_problem_statement(&self, contest_id, problem_id, cache_enabled) -> AppResult<Problem>`（私有） — 题面缓存编排：内存 → 磁盘 → 网络，命中即回填上游；`cache_enabled=false` 时完全直连
  - `async fn get_user_problem_status(&self, contest_id, problem_ids: &[String]) -> AppResult<HashMap<String, i32>>` — 批量查询当前用户提交状态（key=pid，`0=未提交 / 1=已AC / 2=尝试过`，未出现的题视为未提交）；空列表直接返回空 map，不发请求
  - `async fn load_problem_limits(&self, contest_id, display_ids: &[String]) -> AppResult<Vec<ProblemLimits>>` — 批量获取题目 limits（时间 ms / 内存 MB），带内存 + 磁盘双层缓存；返回顺序与入参一致，获取失败的题在结果中**缺失**
  - `fn clear_caches(&self)` — 清空本服务的全部缓存（题面内存 + 题面磁盘 + limits 内存 + limits 磁盘），**清完不重拉**（下次打开题目/拉 limits 自然回源）。供设置页「重置客户端」（`commands::maintenance_cmd::reset_client`）调用；与 `on_oj_switched` 的清理同源（同一批缓存，共用 `clear_problem_disk_caches`），区别只是**磁盘段也同步执行** —— 用户显式点了按钮就该等到真清完再看到「已清空」，而不是投进后台任务后立即返回
  - 内部：`fetch_limits()`（分批并发拉详情）、`limits_key()` / `limits_cache_path()` / `read_limits_cache()` / `write_limits_cache()`
- **字段**：`registry: Arc<dyn ProviderRegistry>`, `event_bus: Arc<CoreEventBus>`, `storage: Arc<Storage>`, `limits_cache: Arc<RwLock<HashMap<{oj}/{contest_id}, HashMap<display_id, ProblemLimits>>>>`, `statement_cache: Arc<TtlCache<String, Problem>>`, `statement_disk: Arc<JsonDiskCache>`。**三层缓存的键都带 OJ 维度**（`{oj}/{cid}/{display_id}`、`{oj}/{cid}`、`cache/problem_limits/{oj}/{cid}.json`）—— 跨 OJ 撞号在结构上不可能，故「切 OJ」的清理只是空间回收

## 直接依赖
- `std::collections::HashMap`
- `std::time::Duration`（题面缓存 TTL 常量）
- `tokio::task::JoinSet`（limits 分批并发）、`tokio::runtime::Handle`（切换清理的后台投递）
- `core::entity::problem::Problem`
- `core::entity::rank::ProblemLimits`
- `core::error::{AppError, AppResult}`
- `core::event::core_event::CoreEvent`
- `core::event::core_event_bus::CoreEventBus`
- `core::provider::registry::ProviderRegistry`
- `infra::cache::{JsonDiskCache, TtlCache}`（题面内存 + 磁盘缓存）
- `infra::storage::Storage`（limits / 题面磁盘缓存底层）

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<ProblemService>`，装配时注入 `Arc<Storage>`）
- `commands::problem_cmd`（经 AppContext 转发 `get_problem` / `get_user_problem_status` / `get_contest_problem_limits`）
- `commands::oj_cmd`（`switch_oj` → `on_oj_switched()` 显式清缓存）
- `commands::maintenance_cmd`（`reset_client` → `clear_caches`）

## 逻辑流程
- ~~**list_problems(contest_id)**~~：已删除（P70）—— 该链路零调用方，题目列表实际走 `ContestProvider::list_contest_problems`（返回 `ContestProblem`）；即便被调用也是语义滥用（同一端点却映射成 description 为空、limits 为 0 的 `Problem`）
- **open_problem(contest_id, problem_id, cache_enabled)**：`load_problem_statement`（内存 → 磁盘 → 网络，命中即回填；`cache_enabled=false` 直连）→ 发布 `CoreEvent::ProblemOpened` → 返回题目。**只缓存成功结果**：Provider 错误原样上抛，不入缓存
- **on_oj_switched**：`limits_cache.clear()` + `statement_cache.clear()`（内存段同步）→ `clear_problem_disk_caches(...)`（有 tokio 上下文则 `Handle::spawn`，否则同步兜底）
- **get_user_problem_status(contest_id, problem_ids)**：空列表短路 → `registry.current_problem()` → `ProblemProvider::get_user_problem_status()` → 失败 `e.context("获取用户题目状态失败")`，变体不改写
- **load_problem_limits(contest_id, display_ids)**：

```
1. 内存缓存 limits_cache[{oj}/{contest_id}] 整表克隆
2. 内存无该比赛任何记录 → 读磁盘 cache/problem_limits/{oj}/{cid}.json
   （不存在/损坏返回空表，损坏文件会被后续回写覆盖）
3. missing = display_ids 中未命中的项
   → fetch_limits：按 LIMITS_CONCURRENCY=4 分块，每块用 JoinSet 并发调
     ProblemProvider::get_problem(cid, display_id) 取 time_limit/memory_limit
4. 错误策略：
   - 部分失败 → 逐题 warn 并跳过，该题在结果中缺失（前端显示占位而不是假默认值）
   - 全部失败 → 上抛首个错误 —— 401/403 这类会话或权限问题必须让前端明确提示，
     不能被静默吞成「拿不到 limits」（HOJ-Problem-Limits-API.md §9.5）
   - Provider 不可用 → 整批失败，交由上抛
5. 有成功项 → 合并进 resolved → 回写内存缓存 + 磁盘缓存
   （磁盘写失败只 warn：缓存是优化，不影响正确性）
6. 按 display_ids 顺序 filter_map 输出（失败题自然缺失）
```

## 设计要点
- **错误处理约定：用 `context()` 而不是重新包装**。传播 Provider 错误一律 `e.context("环节名")`（保留变体、仍补环节名、`warn!` 日志保留），**禁止** `AppError::Problem(format!("…: {}", e))` —— 变体是前端 `isAuthError` 分流与 `guards/sessionGuard.ts` 会话失效兜底的唯一依据（见 `core/error.md`）。改写成 `Problem` 后，token 过期时选手只会看到「题目错误」文案而不会被带回登录页，反复重试全部失败。
- **`load_problem_limits` 同样遵守**：401/403 原样上抛，既不回退默认值，也不改写成 `Problem` 变体。`fetch_limits` 内部只在 JoinSet 任务 join 失败时自行构造 `AppError::Unknown`（那是本层自己的错误，不存在改写下游变体的问题）。
- **`get_user_problem_status` 空入参短路**：`problem_ids` 为空时直接返回空 map，不发请求 —— 因此针对它的错误路径测试**必须传非空列表**才能真正走到 Provider。
- **题面缓存（内存 + 磁盘，TTL 30min，受开关控制）**：题面是「几乎不变但并非永不变化」的公共数据 —— 管理员可能中途修正题面/样例。故 TTL 取 30 分钟（不是永久），并给出配置开关 `oj.cacheProblemStatement`（默认开启）供「要立刻看真值」时关闭；关闭时**不读不写**（连磁盘目录都不创建）。磁盘层带 `fetchedAt`，重启后继续计时；过期文件懒删除。缓存键含 `contest_id`：同一 `display_id` 在不同比赛是不同题目。**只缓存成功结果**，401/403 不入缓存（否则会话失效被缓存掩盖）。limits 仍走自己的双层缓存，两套缓存互不干扰（`limits` 也可从题面实体派生，但保持既有实现避免重复改造）。
- **`clear_caches` 同步清、不清无关目录**：题面磁盘走 `clear_namespace()`（整目录删，含布局标记 —— 下次构造重扫一遍，目录已空是 no-op），limits 磁盘先做存在性守卫再 `remove_all`（从未缓存过 limits 是常见情形，直接删会得到 NotFound 告警）；两段共用自由函数 `clear_problem_disk_caches`，与 `on_oj_switched` 同源。**只清 `cache/` 下的这两处**：工作区代码、提交源码快照、公告已读状态都不是缓存，删掉不可恢复。

## 测试
`src-tauri/src/service/problem/tests/problem_tests.rs` 以桩 Provider + 临时目录锁定：limits 首次全量拉取并落盘、二次调用命中内存缓存零请求、磁盘缓存跨 Service 实例复用、部分失败只返回成功子集、**全部失败上抛错误而非回退假默认值**、损坏缓存文件重新获取、空入参短路不发请求；`get_user_problem_status` 空列表短路与按 pid 映射。

**题面缓存**（5 项）：二次 `open_problem` 命中缓存零请求、开关关闭时每次都请求且**不落盘**、磁盘缓存跨实例复用（模拟重启零请求）、按 `contest_id + display_id` 隔离（同题号不同比赛不互命中）、**错误不入缓存**（连续两次失败各请求一次，变体保持 `Auth`）。

**清空缓存（设置页维护动作）**：`clear_caches` 后题面内存 / 题面磁盘 / limits 内存 / limits 磁盘四处全空、清理本身不发请求、清后再取必须回源；从未产生过磁盘缓存时清空不报错（存在性守卫）。

**OJ 切换清理**：`on_oj_switched_clears_problem_scoped_caches` —— 题面内存 / limits 内存 / 题面磁盘 / limits 磁盘四处全空（无 tokio 上下文时磁盘段同步执行，用例据此断言），且不发任何请求。

**事件契约**：`open_problem_publishes_problem_opened_without_statement` —— 打开题目发布 `ProblemOpened`，且**载荷不含题面内容**（大载荷不进事件）。

**错误变体穿透**（3 项）：`open_problem_preserves_auth_variant`（`Auth` 变体保留且消息含「获取题目详情失败」环节名）、`get_user_problem_status_preserves_auth_variant`（题目总览的「我的状态」每 30s 轮询一次，变体被改写会让守卫失灵；用例传非空 pid 列表以绕开空入参短路）。

为支撑这组用例，`StubProblemProvider` 新增 `fail_all: bool` 字段（为 true 时 `get_user_problem_status` 返回 `Auth` 错误），并新增 `build_service_with(dir, calls, failing, fail_all)` 构造函数；`build_service` 保持原签名（内部转调 `build_service_with(…, false)`），以免影响既有 limits 测试。`open_problem` 的失败路径复用既有 `failing` 列表机制（Stub 对列表内 displayId 返回 `Auth`，模拟 401 或私有赛未注册）。
