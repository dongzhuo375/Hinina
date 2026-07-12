# manager

## 职责
Workspace 生命周期管理器。负责 Workspace 的创建、加载、切换、保存、自动保存、销毁以及崩溃恢复，是工作区服务的核心组件。所有文件操作先写内存再持久化磁盘；自动保存通过 tokio 后台任务实现。

## 核心类型/函数
- **`WorkspaceManager`** — 工作区管理器
  - `fn new(repo: Arc<dyn WorkspaceRepository>, event_bus: Arc<EventBus>) -> Self` — 构造新实例，current 初始为 None
  - `fn create(&self, contest_id, problem_id, root_path) -> AppResult<Workspace>` — 创建新工作区：调用 `Workspace::new()` → 持久化元数据到 `workspace.json` → 设为当前 → 发布 `WorkspaceEvent::Loaded`
  - `fn load(&self, workspace_id, _root_path) -> AppResult<Workspace>` — 从磁盘加载已有工作区：读取 `workspace.json` 元数据 → 恢复所有用户文件（跳过 `workspace.json`）→ 设为当前 → 发布 `WorkspaceEvent::Loaded`
  - `fn save(&self) -> AppResult<()>` — 保存当前工作区的脏文件到磁盘；无 current 或非 dirty 时无操作；发布 `WorkspaceEvent::Saved`
  - `fn start_auto_save(&self, interval_secs: u64)` — 启动后台自动保存（先停旧的再启动）；tokio task 按 interval 检查脏标记并写入磁盘，发布 `WorkspaceEvent::AutoSaveTriggered`
  - `fn stop_auto_save(&self)` — 停止自动保存：设置 `auto_save_running = false` 并 abort 后台 task
  - `fn switch(&self, workspace_id, root_path) -> AppResult<Workspace>` — 切换工作区：先 `save()` 当前 → `load()` 目标 → 发布 `WorkspaceEvent::Switched { from, to }`
  - `fn destroy(&self, workspace_id) -> AppResult<()>` — 销毁工作区：删除所有文件 → 若为当前则清空 current
  - `fn recover_all(&self) -> AppResult<Vec<Workspace>>` — 崩溃恢复（当前 stub，返回空 Vec，待 Storage 层补充目录扫描能力）
  - `fn update_file(&self, file_name, content) -> AppResult<()>` — 更新当前工作区文件：写入内存 HashMap + mark_dirty → 持久化磁盘
  - `fn get_file(&self, file_name) -> AppResult<String>` — 获取文件内容：优先内存 HashMap，未命中回退磁盘读取
  - `fn current(&self) -> Option<Workspace>` — 获取当前活动工作区 clone
- **字段**：`repo: Arc<dyn WorkspaceRepository>`, `event_bus: Arc<EventBus>`, `current: Arc<RwLock<Option<Workspace>>>`, `auto_save_running: AtomicBool`, `auto_save_handle: Mutex<Option<JoinHandle<()>>>`
- **`WorkspaceMeta`**（内部 struct） — 持久化在 `workspace.json` 中的元数据（contest_id, problem_id, root_path, language, created_at, updated_at）
- `impl Drop` — 析构时自动调用 `stop_auto_save()`

## 直接依赖
- `std::collections::HashMap`
- `std::path::PathBuf`
- `std::sync::{Arc, Mutex, RwLock, atomic::{AtomicBool, Ordering}}`
- `std::time::Duration`
- `core::entity::workspace::Workspace`
- `core::error::{AppError, AppResult}`
- `core::event::app_event::{AppEvent, WorkspaceEvent}`
- `core::event::event_bus::EventBus`
- `core::repository::workspace_repo::WorkspaceRepository`
- `serde`

## 被依赖
- `core::context`（`AppContext` 持有 `Option<Arc<WorkspaceManager>>`）

## 逻辑流程
Workspace 生命周期状态机：
1. **Created** — 通过 `create()` 创建新工作区，持久化 `workspace.json` 元数据
2. **Active** — 通过 `load()` 或 `create()` 后进入活动状态，期间可启用自动保存
3. **Saved** — 通过 `save()` 显式保存或 `switch()` 自动保存后；自动保存后台持续运行
4. **Destroyed** — 通过 `destroy()` 删除所有文件后移除

- `switch()` 先保存当前再加载目标，`save()` 仅写脏文件
- `update_file()` 同时更新内存和磁盘，`get_file()` 优先内存后回退磁盘
- `start_auto_save()` 启动 tokio task 定时扫描 dirty 标记；`Drop` 确保 task 被 abort
