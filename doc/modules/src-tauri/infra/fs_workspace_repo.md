# fs_workspace_repo

## 职责
`WorkspaceRepository` trait 的文件系统实现，负责工作区代码的持久化存储（文件读写、列表、删除、存在性检查）。所有方法当前为 TODO 占位。

## 核心类型/函数
- **`FsWorkspaceRepository`** — 文件系统工作区仓库 struct，持有 `Arc<Storage>`
- **`FsWorkspaceRepository::new(storage: Arc<Storage>)`** — 构造函数
- **`save_file(workspace_id, path, content)`** — 保存工作区文件（TODO）
- **`read_file(workspace_id, path)`** — 读取工作区文件（TODO）
- **`list_files(workspace_id)`** — 列出工作区所有文件（TODO）
- **`delete_workspace(workspace_id)`** — 删除整个工作区（TODO）
- **`exists(workspace_id)`** — 检查工作区是否存在（TODO）

## 直接依赖
- `std::path::{Path, PathBuf}`
- `std::sync::Arc`
- `core::error::AppResult`
- `core::repository::workspace_repo::WorkspaceRepository`
- `infra::storage::Storage`

## 被依赖
暂无（未被 infra 外部模块直接引用，预期由 `service::workspace` 通过 trait 使用）

## 逻辑流程
构造函数接收 `Arc<Storage>` 作为底层存储。所有方法通过 `workspace_id` 定位工作区目录，在 `Storage::base_dir()` 下进行文件的 CRUD 操作，全部通过 `WorkspaceRepository` trait 接口对外暴露。
