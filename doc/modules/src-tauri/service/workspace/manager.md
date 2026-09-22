# manager

## 职责
Workspace 生命周期管理器。负责 Workspace 的创建、加载、切换、保存、自动保存、销毁以及崩溃恢复，是工作区服务的核心组件。

**落盘语义（debounce-to-memory）**：内存副本是唯一权威源 —— `update_file` 只写内存并标脏，磁盘写入只有两条路径：显式 `save()`（前端在切题 / 失焦 / 关窗时编排）与后台 auto-save 周期。因此「自动保存间隔」真正决定落盘频率，且**任何替换 `current` 的操作都必须先把旧工作区落盘**（`save_current_if_dirty`）。

## 核心类型/函数
- **`WorkspaceManager`** — 工作区管理器
  - `fn new(repo: Arc<dyn WorkspaceRepository>, event_bus: Arc<CoreEventBus>) -> Self` — 构造新实例，current 初始为 None
  - `fn create(&self, contest_id, problem_id, root_path) -> AppResult<Workspace>` — 创建新工作区：**先 `save_current_if_dirty()` 落盘旧工作区** → 调用 `Workspace::new()` → 持久化元数据到 `workspace.json` → 设为当前。**不发布事件**：工作区的创建由前端经 IPC 发起，返回值即真值；「已创建」不是需要其他观察者知晓的事实
  - `fn load(&self, workspace_id, _root_path) -> AppResult<Workspace>` — 从磁盘加载已有工作区：**先 `save_current_if_dirty()`**（同一工作区重载时先落盘再读回，避免读回旧内容）→ 读取 `workspace.json` 元数据 → 恢复所有用户文件（跳过 `workspace.json`）→ 设为当前。**不发布事件**（加载同样由前端经 IPC 发起，返回值即真值）
  - `fn save(&self) -> AppResult<()>` — 落盘的唯一同步入口：脏时**全量**写入 `files` 中的所有文件 + 一并持久化元数据（语言等字段不属于任何代码文件，只写文件会让它们永远停留在创建时的初值）；持有写锁完成写盘（保存期间不接受 `update_file`）；无 current 或非 dirty 时无操作且**不发布**事件 —— `WorkspaceSaved` 等价于「最新内容确已在磁盘上」，不能为一次空操作发布；成功时发布 `CoreEvent::WorkspaceSaved { workspace_id, revision, automatic: false }`
  - `fn set_language(&self, language) -> AppResult<Workspace>` — 设置当前工作区语言并**立即**持久化元数据（`persist_meta`）。语言不属于任何代码文件，`update_file` 与「仅写文件」的保存路径都带不上它，不单独落盘则切题/重启后退回默认语言（Java 代码被当 C++ 提交）。**不修改脏标记**（语言变更不需要重写代码文件），仅 `ws.touch()` 更新 updated_at；无当前工作区时报 `AppError::Workspace`
  - `fn persist_meta(&self, ws) -> AppResult<()>`（私有）— 把 `WorkspaceMeta` 序列化写入 `workspace.json`，被 `create` / `save` / `set_language` 三处共用。携带 `language` 与 **`active_file`**（后者 `#[serde(default)]`：历史 workspace.json 无该字段时降级为 `None`，由前端回退到「语言派生名 + 后缀探测」的老启发式）。**auto-save 刻意不写元数据**：`set_language` 会立即落盘语言，若 auto-save 用快照时刻的元数据回写，会把刚落盘的新语言覆盖回旧值
  - `fn auto_save_interval_secs(&self) -> Option<u64>` — 当前 auto-save 的间隔（`None` = 未运行）。单独记一份状态是为了让命令层「按配置同步」成为**幂等**操作：间隔未变则保持不动（免得每次 `load_workspace` 都重置计时器），关掉后也有状态可依、能重新启动（P48：旧实现用一次性 `static AtomicBool`，关掉后再也起不来）
  - `fn save_current_if_dirty(&self) -> ()`（私有）— 落盘当前工作区（脏时才写），失败仅 `warn`。`create` / `load` / `switch` 替换 `current` 前都必须调用：内存是唯一权威副本，不先落盘则未落盘的改动随替换静默消失
  - `fn find_or_create(&self, contest_id, problem_id, root_path) -> AppResult<Workspace>` — 按 contest_id + problem_id 扫描所有工作区的 `workspace.json` 元数据：命中 → `load()` 恢复代码；未命中 → `create()` 新建（P36 修复；元数据持久化正是为了避免从 workspace_id 字符串解析字段）
  - `fn start_auto_save(&self, interval_secs: u64)` — 启动后台自动保存（先停旧的再启动）。**「停旧的 → 起新的 → 登记句柄」在同一把锁内完成**：分三次取锁时，并发的两个调用会各自停掉「当时存在的」任务、各自 spawn，随后后登记者的句柄覆盖先登记者 —— 先起的循环成为无人可停的孤儿任务，`stop_auto_save` 之后仍在写盘（有并发用例锁定，修复前 4/4 失败）。循环语义：脏才写；**取快照与写盘整体在读锁内完成**（与 `save()` / `update_file` 的写锁互斥，保证「旧快照的写」不可能落在「更新的写」之后 —— 只锁「取快照」会让写盘期间到来的 `save()` 插进本次写盘之后被旧内容覆盖回退，且工作区已 clean → 新内容永不重写）；写盘失败保持脏、下轮重试且**不发布事件**（前端「已自动备份」必须表示最新内容确已落盘）；仅当修订号未变（快照之后无新改动）才 `mark_clean()` 并发布 `CoreEvent::WorkspaceSaved { workspace_id, revision: snapshot_revision, automatic: true }`
  - `fn stop_auto_save(&self) -> ()` — 停止自动保存：take 出 `auto_save_handle` 中的 JoinHandle 并 abort
  - `fn switch(&self, workspace_id, root_path) -> AppResult<Workspace>` — 切换工作区：先 `save_current_if_dirty()` → `load()` 目标。**不发布事件**：切换由前端经 IPC 发起，返回值即真值；切换前对旧工作区的显式保存本身会发布 `WorkspaceSaved`（前端据此清脏，并按 `workspace_id` / `revision` 过滤掉不属于当前工作区的过期事件）
  - `fn destroy(&self, workspace_id) -> AppResult<()>` — 销毁工作区：删除所有文件 → 若为当前则清空 current
  - `fn recover_all(&self) -> AppResult<Vec<Workspace>>` — 崩溃恢复（当前 stub，返回空 Vec，待 Storage 层补充目录扫描能力）
  - `fn update_file(&self, file_name, content) -> AppResult<u64>` — 更新当前工作区文件：**只写内存** HashMap + **把该文件记为 `active_file`（当前代码文件的权威源）** + mark_dirty + 在写锁内递增修订号（`revision`），**不落盘**（落盘由 `save` / auto-save 负责）。**返回本次内容被赋予的修订号**（`fetch_add(1) + 1`），供命令层原样回传前端：前端把它记为 `lastPushedRevision`，用于判断落盘事件是否落后于编辑器已推送的内容（落后则不清脏，避免假「已自动备份」）。写入路径的权威源由它承担后，调用方不必再按语言派生文件名（派生会让语言切换后的写入落到别的文件上）
  - `fn get_file(&self, file_name) -> AppResult<String>` — 获取文件内容：优先内存 HashMap，未命中回退磁盘读取
  - `fn delete_file(&self, file_name) -> AppResult<()>` — 删除当前工作区中的文件（P62：按文件删除能力，供旧代码文件清理）。**守卫**（比较一律经 `guard_equivalent` 归一 —— 小写化 + 剥离尾随点/空格：Windows 把 "Workspace.json" / "MAIN.JAVA" 等变体名解析到同一文件，精确字符串比较会被绕过）：拒绝删除 `active_file`（选手当前代码，清理只针对过期文件）与 `workspace.json`（元数据被删会让工作区无法加载）。**顺序刻意为先删磁盘、成功后再移除内存**：磁盘删除失败时内存保持原样、`save()` 仍会写出该文件，内存与磁盘始终一致（反序会出现「内存已无、磁盘残留」的幽灵文件，下次加载时复活）。不递增修订号、不置脏（删除是即时持久化操作，无待落盘内容）、不发布事件（与 create / load / switch 同一先例：前端发起、返回值即真值）；无当前工作区时报 `AppError::Workspace`
  - `fn current(&self) -> Option<Workspace>` — 获取当前活动工作区 clone
- **字段**：`repo: Arc<dyn WorkspaceRepository>`, `event_bus: Arc<CoreEventBus>`, `current: Arc<RwLock<Option<Workspace>>>`, `revision: Arc<AtomicU64>`（内容修订号，auto-save 判据）, `auto_save_handle: Mutex<Option<JoinHandle<()>>>`, `auto_save_interval_secs: Mutex<Option<u64>>`（`None` = 未运行）
- **`WorkspaceMeta`**（内部 struct） — 持久化在 `workspace.json` 中的元数据（contest_id, problem_id, root_path, language, created_at, updated_at）
- `impl Drop` — 析构时自动调用 `stop_auto_save()`

## 直接依赖
- `std::collections::HashMap`
- `std::path::PathBuf`
- `std::sync::atomic::{AtomicU64, Ordering}`
- `std::sync::{Arc, Mutex, RwLock}`
- `std::time::Duration`
- `core::entity::workspace::Workspace`
- `core::error::{AppError, AppResult}`
- `core::event::core_event::CoreEvent`
- `core::event::core_event_bus::CoreEventBus`
- `core::repository::workspace_repo::WorkspaceRepository`
- `serde`

## 被依赖
- `core::context`（`AppContext` 持有 `Option<Arc<WorkspaceManager>>`）
- `commands::workspace_cmd`（七个 Command 经 `ctx.workspace_manager` 调用生命周期与文件方法）

## 逻辑流程
Workspace 生命周期状态机：
1. **Created** — 通过 `create()` 创建新工作区，持久化 `workspace.json` 元数据
2. **Active** — 通过 `load()` 或 `create()` 后进入活动状态，期间可启用自动保存
3. **Saved** — 通过 `save()` 显式保存或 `switch()` 自动保存后；自动保存后台持续运行
4. **Destroyed** — 通过 `destroy()` 删除所有文件后移除

- `switch()` 先 `save_current_if_dirty()` 再加载目标；`save()` 写全部文件后一并 `persist_meta()`（语言等元数据随保存落盘）
- `set_language()` 只改内存语言字段 + `touch()` + 立即 `persist_meta()`，不动脏标记、不重写代码文件
- `find_or_create()` 按元数据匹配已有工作区（恢复代码），未命中才 `create()`
- `update_file()` 只写内存 + 记录 `active_file` + 递增修订号（不落盘）；`get_file()` 优先内存后回退磁盘
- `start_auto_save()` 启动 tokio task 定时扫描 dirty 标记；取快照与写盘在同一读锁内完成（与 `save()` 的写锁形成全序），落盘成功且快照之后无新改动才清脏并发布 `WorkspaceSaved { automatic: true }`；`Drop` 确保 task 被 abort

**唯一的事件发布点**：`save()`（`automatic: false`）与 auto-save 成功 tick（`automatic: true`），两者都是 `CoreEvent::WorkspaceSaved { workspace_id, revision, automatic }`。`create` / `load` / `switch` **都不发布事件** —— 它们由前端经 IPC 发起，返回值即真值；事件只用来把「后台 auto-save 已完成落盘」这件前端无法感知的事实告诉它（经 `main.rs` 的前端事件桥转为 `workspace-saved`）。

**`revision` 为什么在写锁内取**：释放锁之后 `update_file` 可能立刻递增它，那样事件会报出比磁盘内容更新的修订号，前端据此清脏 = 假 clean（最新内容仍在内存，直到下一次编辑才可能落盘）。auto-save 侧同理：修订号与快照必须在同一读锁作用域内取得，否则「取快照 → 取修订号」之间到来的改动会让修订号偏新，写盘的是旧内容却误判为可 clean。

## 测试
`src-tauri/src/service/workspace/tests/manager_tests.rs` 锁定：create 设为当前、`update_file` **不落盘**（`save` 才落盘）且**记录 `active_file`**、`save` 落盘并清脏、create / load 替换当前前先落盘旧工作区、get_file 内存优先、load 从磁盘恢复且**用元数据而非解析 workspace_id**、**`active_file` 跨实例恢复**（重启后仍指向语言切换后的那个文件）与**历史元数据无该字段时降级为 None**、destroy/switch/current 语义，语言持久化三契约——`set_language` 跨 Manager 实例存活、`save` 一并持久化语言元数据、无当前工作区时 `set_language` 报错，auto-save 五契约——落盘并发布一次事件、**写盘与显式 `save()` 全序**（`HookedRepo` 的写前钩子阻塞 auto-save 的写盘，另一线程完成 `update_file` + `save()`，放行后磁盘必须仍是新内容：修复前该用例失败于 `left: "v1"`）、tick 期间到来的改动最终必须落盘且不留「clean 但磁盘落后」终态、**写盘失败保留脏且静默**、**间隔可观测且支持「停掉再启动」**、**并发 `start_auto_save` 不泄漏孤儿循环**（修复前 4/4 失败：孤儿循环会在 `stop_auto_save` 之后把脏标记清掉）；`delete_file` 九契约——内存与磁盘一并移除且 active 文件不受影响、拒绝删除 active 文件、拒绝删除 `workspace.json`、**守卫按 Windows 等价类比较（大小写变体 / 尾随点与空格变体均不得绕过，另有 `guard_equivalent_*` 纯函数单测钉死归一规则）**、**ADS 变体名（`workspace.json::$DATA`）端到端拒绝（等价类守卫拦不住，由仓库层 Win32 非法字符校验收口）**、无当前工作区报错、内存与磁盘均无此文件时幂等成功、磁盘孤儿文件（内存无磁盘有）同样被删除；另有 `can_mark_clean_*` 逐条钉死清脏判据（同一工作区 + 修订号未变；集成测试无法确定性覆盖该判据的削弱 —— 写盘持读锁使「快照之后到来改动」的窗口不可注入，跨线程注入则胜负不定）。
