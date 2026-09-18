# manager

## 职责
Workspace 生命周期管理器。负责 Workspace 的创建、加载、切换、保存、自动保存、销毁以及崩溃恢复，是工作区服务的核心组件。

**落盘语义（debounce-to-memory）**：内存副本是唯一权威源 —— `update_file` 只写内存并标脏，磁盘写入只有两条路径：显式 `save()`（前端在切题 / 失焦 / 关窗时编排）与后台 auto-save 周期。因此「自动保存间隔」真正决定落盘频率，且**任何替换 `current` 的操作都必须先把旧工作区落盘**（`save_current_if_dirty`）。

## 核心类型/函数
- **`WorkspaceManager`** — 工作区管理器
  - `fn new(repo: Arc<dyn WorkspaceRepository>, event_bus: Arc<EventBus>) -> Self` — 构造新实例，current 初始为 None
  - `fn create(&self, contest_id, problem_id, root_path) -> AppResult<Workspace>` — 创建新工作区：**先 `save_current_if_dirty()` 落盘旧工作区** → 调用 `Workspace::new()` → 持久化元数据到 `workspace.json` → 设为当前 → 发布 `WorkspaceEvent::Loaded`
  - `fn load(&self, workspace_id, _root_path) -> AppResult<Workspace>` — 从磁盘加载已有工作区：**先 `save_current_if_dirty()`**（同一工作区重载时先落盘再读回，避免读回旧内容）→ 读取 `workspace.json` 元数据 → 恢复所有用户文件（跳过 `workspace.json`）→ 设为当前 → 发布 `WorkspaceEvent::Loaded`
  - `fn save(&self) -> AppResult<()>` — 落盘的唯一同步入口：脏时**全量**写入 `files` 中的所有文件 + 一并持久化元数据（语言等字段不属于任何代码文件，只写文件会让它们永远停留在创建时的初值）；持有写锁完成写盘（保存期间不接受 `update_file`）；无 current 或非 dirty 时无操作且**不发布**事件；成功时发布 `WorkspaceEvent::Saved`
  - `fn set_language(&self, language) -> AppResult<Workspace>` — 设置当前工作区语言并**立即**持久化元数据（`persist_meta`）。语言不属于任何代码文件，`update_file` 与「仅写文件」的保存路径都带不上它，不单独落盘则切题/重启后退回默认语言（Java 代码被当 C++ 提交）。**不修改脏标记**（语言变更不需要重写代码文件），仅 `ws.touch()` 更新 updated_at；无当前工作区时报 `AppError::Workspace`
  - `fn persist_meta(&self, ws) -> AppResult<()>`（私有）— 把 `WorkspaceMeta` 序列化写入 `workspace.json`，被 `create` / `save` / `set_language` 三处共用。**auto-save 刻意不写元数据**：`set_language` 会立即落盘语言，若 auto-save 用快照时刻的元数据回写，会把刚落盘的新语言覆盖回旧值
  - `fn save_current_if_dirty(&self) -> ()`（私有）— 落盘当前工作区（脏时才写），失败仅 `warn`。`create` / `load` / `switch` 替换 `current` 前都必须调用：内存是唯一权威副本，不先落盘则未落盘的改动随替换静默消失
  - `fn find_or_create(&self, contest_id, problem_id, root_path) -> AppResult<Workspace>` — 按 contest_id + problem_id 扫描所有工作区的 `workspace.json` 元数据：命中 → `load()` 恢复代码；未命中 → `create()` 新建（P36 修复；元数据持久化正是为了避免从 workspace_id 字符串解析字段）
  - `fn start_auto_save(&self, interval_secs: u64)` — 启动后台自动保存（先停旧的再启动）。循环语义：脏才写；**取快照与写盘整体在读锁内完成**（与 `save()` / `update_file` 的写锁互斥，保证「旧快照的写」不可能落在「更新的写」之后 —— 只锁「取快照」会让写盘期间到来的 `save()` 插进本次写盘之后被旧内容覆盖回退，且工作区已 clean → 新内容永不重写）；写盘失败保持脏、下轮重试且不发布事件；仅当修订号未变（快照之后无新改动）才 `mark_clean()` 并发布 `WorkspaceEvent::AutoSaveTriggered`
  - `fn stop_auto_save(&self) -> ()` — 停止自动保存：take 出 `auto_save_handle` 中的 JoinHandle 并 abort
  - `fn switch(&self, workspace_id, root_path) -> AppResult<Workspace>` — 切换工作区：先 `save_current_if_dirty()` → `load()` 目标 → 发布 `WorkspaceEvent::Switched { from, to }`
  - `fn destroy(&self, workspace_id) -> AppResult<()>` — 销毁工作区：删除所有文件 → 若为当前则清空 current
  - `fn recover_all(&self) -> AppResult<Vec<Workspace>>` — 崩溃恢复（当前 stub，返回空 Vec，待 Storage 层补充目录扫描能力）
  - `fn update_file(&self, file_name, content) -> AppResult<()>` — 更新当前工作区文件：**只写内存** HashMap + mark_dirty + 在写锁内递增修订号（`revision`），**不落盘**（落盘由 `save` / auto-save 负责）
  - `fn get_file(&self, file_name) -> AppResult<String>` — 获取文件内容：优先内存 HashMap，未命中回退磁盘读取
  - `fn current(&self) -> Option<Workspace>` — 获取当前活动工作区 clone
- **字段**：`repo: Arc<dyn WorkspaceRepository>`, `event_bus: Arc<EventBus>`, `current: Arc<RwLock<Option<Workspace>>>`, `revision: Arc<AtomicU64>`（内容修订号，auto-save 判据）, `auto_save_handle: Mutex<Option<JoinHandle<()>>>`
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
- `core::event::app_event::{AppEvent, WorkspaceEvent}`
- `core::event::event_bus::EventBus`
- `core::repository::workspace_repo::WorkspaceRepository`
- `serde`

## 被依赖
- `core::context`（`AppContext` 持有 `Option<Arc<WorkspaceManager>>`）
- `commands::workspace_cmd`（六个 Command 经 `ctx.workspace_manager` 调用生命周期与文件方法）

## 逻辑流程
Workspace 生命周期状态机：
1. **Created** — 通过 `create()` 创建新工作区，持久化 `workspace.json` 元数据
2. **Active** — 通过 `load()` 或 `create()` 后进入活动状态，期间可启用自动保存
3. **Saved** — 通过 `save()` 显式保存或 `switch()` 自动保存后；自动保存后台持续运行
4. **Destroyed** — 通过 `destroy()` 删除所有文件后移除

- `switch()` 先 `save_current_if_dirty()` 再加载目标；`save()` 写全部文件后一并 `persist_meta()`（语言等元数据随保存落盘）
- `set_language()` 只改内存语言字段 + `touch()` + 立即 `persist_meta()`，不动脏标记、不重写代码文件
- `find_or_create()` 按元数据匹配已有工作区（恢复代码），未命中才 `create()`
- `update_file()` 只写内存 + 递增修订号（不落盘）；`get_file()` 优先内存后回退磁盘
- `start_auto_save()` 启动 tokio task 定时扫描 dirty 标记；取快照与写盘在同一读锁内完成（与 `save()` 的写锁形成全序），落盘成功且快照之后无新改动才清脏并发布事件；`Drop` 确保 task 被 abort

## 测试
`src-tauri/src/service/workspace/tests/manager_tests.rs` 锁定：create 设为当前、`update_file` **不落盘**（`save` 才落盘）、`save` 落盘并清脏、create / load 替换当前前先落盘旧工作区、get_file 内存优先、load 从磁盘恢复且**用元数据而非解析 workspace_id**、destroy/switch/current 语义，语言持久化三契约——`set_language` 跨 Manager 实例存活、`save` 一并持久化语言元数据、无当前工作区时 `set_language` 报错，以及 auto-save 四契约——落盘并发布一次事件、**写盘与显式 `save()` 全序**（`HookedRepo` 的写前钩子阻塞 auto-save 的写盘，另一线程完成 `update_file` + `save()`，放行后磁盘必须仍是新内容：修复前该用例失败于 `left: "v1"`）、tick 期间到来的改动最终必须落盘且不留「clean 但磁盘落后」终态、**写盘失败保留脏且静默**；另有 `can_mark_clean_*` 逐条钉死清脏判据（同一工作区 + 修订号未变；集成测试无法确定性覆盖该判据的削弱 —— 写盘持读锁使「快照之后到来改动」的窗口不可注入，跨线程注入则胜负不定）。
