# workspace_repo

## 职责
定义 Workspace 持久化仓库 trait `WorkspaceRepository`，抽象工作区文件的读写操作，解耦 Service 与底层存储实现。当前基于文件系统实现，未来可替换为其他存储后端。

## 核心类型/函数
- **`WorkspaceRepository`** — 工作区仓库 trait，方法：
  - `save_file(&self, workspace_id, path, content) -> AppResult<()>` — 保存文件
  - `read_file(&self, workspace_id, path) -> AppResult<String>` — 读取文件
  - `list_files(&self, workspace_id) -> AppResult<Vec<PathBuf>>` — 列出文件
  - `delete_file(&self, workspace_id, path) -> AppResult<()>` — 删除单个文件（文件不存在时为无操作，幂等；P62 旧代码文件清理）
  - `delete_workspace(&self, workspace_id) -> AppResult<()>` — 删除工作区
  - `exists(&self, workspace_id) -> bool` — 检查是否存在

## 直接依赖
- `std::path::{Path, PathBuf}`
- `core::error::AppResult`

## 被依赖
- `service::workspace::manager`（WorkspaceManager 通过此 trait 读写文件）
- `infra::fs_workspace_repo`

## 逻辑流程
无（纯 trait 定义）。
