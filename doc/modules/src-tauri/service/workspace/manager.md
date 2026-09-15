# manager

## 职责
Workspace 生命周期管理器。负责 Workspace 的创建、加载、切换、保存、自动保存、销毁以及崩溃恢复，是工作区服务的核心组件。所有文件操作先写内存再持久化磁盘；自动保存通过 tokio 后台任务实现。

## 核心类型/函数
- **`WorkspaceManager`** — 工作区管理器
  - `fn new(repo: Arc<dyn WorkspaceRepository>, event_bus: Arc<EventBus>) -> Self` — 构造新实例，current 初始为 None
  - `fn create(&self, contest_id, problem_id, root_path) -> AppResult<Workspace>` — 创建新工作区：调用 `Workspace::new()` → 持久化元数据到 `workspace.json` → 设为当前 → 发布 `WorkspaceEvent::Loaded`
  - `fn load(&self, workspace_id, _root_path) -> AppResult<Workspace>` — 从磁盘加载已有工作区：读取 `workspace.json` 元数据 → 恢复所有用户文件（跳过 `workspace.json`）→ 设为当前 → 发布 `WorkspaceEvent::Loaded`
  - `fn save(&self) -> AppResult<()>` — 保存当前工作区的脏文件到磁盘，**随后一并持久化元数据**（语言等字段不属于任何代码文件，只写文件会让它们永远停留在创建时的初值）；无 current 或非 dirty 时无操作；发布 `WorkspaceEvent::Saved`
  - `fn set_language(&self, language) -> AppResult<Workspace>` — 设置当前工作区语言并**立即**持久化元数据（`persist_meta`）。语言不属于任何代码文件，`update_file` 与「仅写文件」的保存路径都带不上它，不单独落盘则切题/重启后退回默认语言（Java 代码被当 C++ 提交）。**不修改脏标记**（语言变更不需要重写代码文件），仅 `ws.touch()` 更新 updated_at；无当前工作区时报 `AppError::Workspace`
  - `fn persist_meta(&self, ws) -> AppResult<()>`（私有）— 把 `WorkspaceMeta` 序列化写入 `workspace.json`，被 `create` / `save` / `set_language` 三处共用
  - `fn find_or_create(&self, contest_id, problem_id, root_path) -> AppResult<Workspace>` — 按 contest_id + problem_id 扫描所有工作区的 `workspace.json` 元数据：命中 → `load()` 恢复代码；未命中 → `create()` 新建（P36 修复；元数据持久化正是为了避免从 workspace_id 字符串解析字段）
  - `fn start_auto_save(&self, interval_secs: u64)` — 启动后台自动保存（先停旧的再启动）；tokio task 按 interval 检查脏标记并写入磁盘，发布 `WorkspaceEvent::AutoSaveTriggered`
  - `fn stop_auto_save(&self) -> ()` — 停止自动保存：take 出 `auto_save_handle` 中的 JoinHandle 并 abort
  - `fn switch(&self, workspace_id, root_path) -> AppResult<Workspace>` — 切换工作区：先 `save()` 当前 → `load()` 目标 → 发布 `WorkspaceEvent::Switched { from, to }`
  - `fn destroy(&self, workspace_id) -> AppResult<()>` — 销毁工作区：删除所有文件 → 若为当前则清空 current
  - `fn recover_all(&self) -> AppResult<Vec<Workspace>>` — 崩溃恢复（当前 stub，返回空 Vec，待 Storage 层补充目录扫描能力）
  - `fn update_file(&self, file_name, content) -> AppResult<()>` — 更新当前工作区文件：写入内存 HashMap + mark_dirty → 持久化磁盘
  - `fn get_file(&self, file_name) -> AppResult<String>` — 获取文件内容：优先内存 HashMap，未命中回退磁盘读取
  - `fn current(&self) -> Option<Workspace>` — 获取当前活动工作区 clone
- **字段**：`repo: Arc<dyn WorkspaceRepository>`, `event_bus: Arc<EventBus>`, `current: Arc<RwLock<Option<Workspace>>>`, `auto_save_handle: Mutex<Option<JoinHandle<()>>>`
- **`WorkspaceMeta`**（内部 struct） — 持久化在 `workspace.json` 中的元数据（contest_id, problem_id, root_path, language, created_at, updated_at）
- `impl Drop` — 析构时自动调用 `stop_auto_save()`

## 直接依赖
- `std::collections::HashMap`
- `std::path::PathBuf`
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

- `switch()` 先保存当前再加载目标，`save()` 写脏文件后一并 `persist_meta()`（语言等元数据随保存落盘）
- `set_language()` 只改内存语言字段 + `touch()` + 立即 `persist_meta()`，不动脏标记、不重写代码文件
- `find_or_create()` 按元数据匹配已有工作区（恢复代码），未命中才 `create()`
- `update_file()` 同时更新内存和磁盘，`get_file()` 优先内存后回退磁盘
- `start_auto_save()` 启动 tokio task 定时扫描 dirty 标记；`Drop` 确保 task 被 abort

## 测试
`src-tauri/src/service/workspace/tests/manager_tests.rs` 锁定：create 设为当前、save 落盘并清脏、update_file 标脏、get_file 内存优先、load 从磁盘恢复且**用元数据而非解析 workspace_id**、destroy/switch/current 语义，以及语言持久化三契约——`set_language` 跨 Manager 实例存活、`save` 一并持久化语言元数据、无当前工作区时 `set_language` 报错。
