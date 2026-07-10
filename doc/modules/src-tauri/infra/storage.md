# storage

## 职责
本地文件存储工具，提供底层的文件读写能力。不包含业务语义，业务层应通过 Repository trait 间接使用，不应直接依赖此模块。所有路径相对于 `base_dir`，内建目录穿越防护。

## 核心类型/函数
- **`Storage`** — 文件存储 struct，持有基础目录 `base_dir: PathBuf`
- **`Storage::new(base_dir: PathBuf)`** — 构造函数，指定存储根目录
- **`Storage::base_dir()`** — 返回基础目录的不可变引用
- **`Storage::read(relative_path) -> AppResult<Vec<u8>>`** — 读取二进制文件
- **`Storage::read_to_string(relative_path) -> AppResult<String>`** — 读取 UTF-8 文件
- **`Storage::write(relative_path, data) -> AppResult<()>`** — 写入二进制数据，自动创建父目录
- **`Storage::write_string(relative_path, content) -> AppResult<()>`** — 写入字符串
- **`Storage::exists(relative_path) -> bool`** — 检查路径是否存在
- **`Storage::create_dir(relative_path) -> AppResult<()>`** — 递归创建目录
- **`Storage::remove(relative_path) -> AppResult<()>`** — 删除文件或空目录
- **`Storage::remove_all(relative_path) -> AppResult<()>`** — 递归删除文件或目录
- **`Storage::list(relative_path) -> AppResult<Vec<PathBuf>>`** — 列出目录直接子项，返回相对路径

## 直接依赖
- `std::path::{Path, PathBuf, Component}`
- `std::fs`
- `core::error::{AppError, AppResult}`

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<Storage>` 注入各模块）
- `infra::fs_workspace_repo`（`FsWorkspaceRepository` 依赖 `Storage` 存取工作区文件）
- `infra::fs_config_repo`（`FsConfigRepository` 依赖 `Storage` 存取配置文件）
- `infra::fs_plugin_repo`（`FsPluginRepository` 依赖 `Storage` 存取插件文件）

## 逻辑流程
构造时接收 `base_dir` 路径并调用 `create_dir_all` 确保目录存在。所有文件操作通过 `resolve()` 方法将相对路径拼接到 `base_dir` 下，遍历 `Path::components()` 拒绝任何 `Component::ParentDir`（纯逻辑检查，不依赖文件系统）。write 系列方法自动创建不存在的父目录。`list()` 返回 `strip_prefix` 后的相对路径。

## 测试
测试代码位于 `tests/storage_tests.rs`，通过 `#[cfg(test)] #[path = "tests/storage_tests.rs"]` 引用。
包含 3 个目录穿越防护测试：正常路径通过、`..` 拒绝、深层嵌套合法路径通过。
