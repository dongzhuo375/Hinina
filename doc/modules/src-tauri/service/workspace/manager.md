# manager

## 职责
Workspace 生命周期管理器。负责 Workspace 的创建、加载、切换、保存、自动保存、销毁以及崩溃恢复，是工作区服务的核心组件。

## 核心类型/函数
- `struct WorkspaceManager` — 工作区管理器，持有 Repository、EventBus、当前 Workspace 等状态
  - `fn new(repo, event_bus, workspaces_dir)` — 构造新实例
  - `fn create(&self, contest_id) -> AppResult<Workspace>` — 创建新工作区（初始化目录结构、模板文件）
  - `fn load(&self, workspace_id) -> AppResult<Workspace>` — 从磁盘加载已有工作区
  - `fn switch(&self, workspace_id) -> AppResult<Workspace>` — 切换工作区（保存当前 → 加载目标）
  - `fn save(&self) -> AppResult<()>` — 保存当前工作区所有文件
  - `fn start_auto_save(&self, interval)` — 启动自动保存
  - `fn stop_auto_save(&self)` — 停止自动保存
  - `fn destroy(&self, workspace_id) -> AppResult<()>` — 销毁工作区（删除所有文件）
  - `fn recover_all(&self) -> AppResult<Vec<Workspace>>` — 崩溃恢复：扫描所有未正常关闭的工作区
  - `fn current(&self) -> Option<Workspace>` — 获取当前活动工作区
  - `fn workspaces_dir(&self) -> &PathBuf` — 获取工作区根目录
- `struct AutoSaveHandle` — 自动保存句柄（内部类型，当前为空占位）

## 直接依赖
- `std::collections::HashMap`
- `std::path::PathBuf`
- `std::sync::{Arc, Mutex, RwLock}`
- `std::time::Duration`
- `crate::core::entity::workspace::Workspace`
- `crate::core::error::AppResult`
- `crate::core::event::event_bus::EventBus`
- `crate::core::repository::workspace_repo::WorkspaceRepository`

## 被依赖
- `src-tauri/src/core/context.rs`（`AppContext` 持有 `Arc<WorkspaceManager>`）

## 逻辑流程
Workspace 生命周期状态机：
1. **Created** — 通过 `create()` 创建新工作区
2. **Active** — 通过 `load()` 或 `recover_all()` 加载后进入活动状态，期间可启用自动保存
3. **Saved** — 通过 `save()` 显式保存或 `switch()` 自动保存后进入空闲状态
4. **Destroyed** — 通过 `destroy()` 删除所有文件后移除

启动时 `recover_all()` 扫描磁盘恢复未正常关闭的工作区。`switch()` 操作会先保存当前再加载目标。
