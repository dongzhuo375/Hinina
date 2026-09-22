# fs_workspace_repo

## 职责
`WorkspaceRepository` trait 的文件系统实现，负责工作区代码的持久化存储（文件读写、递归列表、删除、存在性检查）。所有方法已完整实现并通过单元测试，基于 `Storage` 底层文件操作能力。

## 核心类型/函数
- **`FsWorkspaceRepository`** — 文件系统工作区仓库 struct，持有 `Arc<Storage>`
- **`FsWorkspaceRepository::new(storage: Arc<Storage>)`** — 构造函数
- **`workspace_root(workspace_id)`** — 构建 workspace 根目录相对路径 `workspaces/{id}`
- **`workspace_relative(workspace_id, file_path)`** — 构建文件相对路径并做**两层路径安全校验**（`delete_file` 是破坏性原语，校验强度必须覆盖它）：① **组件白名单** —— 只接受 `Component::Normal`，`..`、前导 `.`、绝对路径（根目录 / Windows 盘符前缀）与空路径全部拒绝（黑名单式只拒 `ParentDir` 会放过 `C:`、`/` 等组件）；② **拼接后词法规范化（`lexically_normalize`）+ 前缀校验** —— 验证结果仍以工作区根目录为前缀（组件级比较，非字符串前缀），输入已过白名单时是恒等变换，作为纵深防御兜底未来回归
- **`lexically_normalize(path)`** — 模块级纯函数：词法解析路径中的 `.` 与 `..`（不触文件系统、不解析符号链接）
- **`walk_dir(dir, prefix, files)`** — 递归遍历目录，收集所有文件相对路径
- **`save_file(workspace_id, path, content)`** — 保存工作区文件，自动创建父目录
- **`read_file(workspace_id, path)`** — 读取工作区文件，不存在时返回 `AppError::Workspace`
- **`list_files(workspace_id)`** — 递归列出工作区所有文件，不存在返回空列表
- **`delete_file(workspace_id, path)`** — 删除单个工作区文件（P62 旧代码文件清理）：经 `workspace_relative()` 路径穿越校验后委托 `Storage::remove()`；文件不存在时返回 `Ok(())`（幂等，清理路径可安全重试）
- **`delete_workspace(workspace_id)`** — 递归删除整个工作区目录
- **`exists(workspace_id)`** — 检查工作区目录是否存在

## 直接依赖
- `std::path::{Component, Path, PathBuf}`
- `std::sync::Arc`
- `core::error::{AppError, AppResult}`
- `core::repository::workspace_repo::WorkspaceRepository`
- `infra::storage::Storage`

## 被依赖
暂无（未被 infra 外部模块直接引用，预期由 `service::workspace` 通过 trait 使用）

## 逻辑流程
目录结构为 `{storage.base_dir}/workspaces/{workspace_id}/...`。构造函数接收 `Arc<Storage>` 作为底层存储。

- **save_file / read_file / delete_file**：通过 `workspace_relative()` 构建 `workspaces/{id}/{file_path}` 格式的相对路径，传入前对 `file_path` 做组件白名单 + 规范化前缀校验（见核心类型/函数），然后委托 Storage 读写删。
- **list_files**：先检查 workspace 根目录是否存在（不存在返回空列表），再通过 `walk_dir()` 递归遍历 `std::fs::read_dir`，用字符串前缀剥离得到业务层可用的相对路径（兼容 Windows `\` 分隔符）。
- **delete_workspace**：委托 `Storage::remove_all()` 递归删除。
- **delete_file**：经 `workspace_relative()` 校验后委托 `Storage::remove()`；先查 `Storage::exists()`，不存在直接 `Ok(())`（幂等）。
- **exists**：委托 `Storage::exists()` 检查目录存在性。

## 测试覆盖（12 项）
测试代码位于 `tests/fs_workspace_repo_tests.rs`。
- `save_and_read_file` — 保存与读取往返
- `read_nonexistent_file_returns_error` — 读取不存在的文件
- `list_files_after_save` — 递归列出含子目录的文件
- `list_files_nonexistent_workspace_returns_empty` — 不存在的工作区返回空列表
- `exists_detects_workspace` — 存在性检查
- `delete_workspace_removes_all` — 删除后文件和存在性均清除
- `rejects_path_traversal` — 拒绝 `..`、`../../`、`a/../b` 等路径穿越
- `delete_file_removes_disk_file` — 删除单个文件，其余文件不受影响
- `delete_file_missing_is_noop` — 文件不存在时幂等成功
- `delete_file_rejects_path_traversal` — 删除路径同样拒绝穿越，合法文件不受波及
- `rejects_non_normal_path_components` — 组件白名单：绝对路径、前导 `.`、空路径全部拒绝（读写删三个原语一致）
- `rejects_drive_letter_paths`（仅 Windows）— 盘符前缀路径拒绝
