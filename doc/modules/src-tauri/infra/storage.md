# storage

## 职责
本地文件存储工具，提供底层的文件读写能力。不包含业务语义，业务层应通过 Repository trait 间接使用，不应直接依赖此模块。

## 核心类型/函数
- **`Storage`** — 文件存储 struct，持有基础目录 `base_dir: PathBuf`
- **`Storage::new(base_dir: PathBuf)`** — 构造函数，指定存储根目录
- **`Storage::base_dir()`** — 返回基础目录的不可变引用

## 直接依赖
- `std::path::PathBuf`

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<Storage>` 注入各模块）
- `infra::fs_workspace_repo`（`FsWorkspaceRepository` 依赖 `Storage` 存取工作区文件）
- `infra::fs_config_repo`（`FsConfigRepository` 依赖 `Storage` 存取配置文件）
- `infra::fs_plugin_repo`（`FsPluginRepository` 依赖 `Storage` 存取插件文件）

## 逻辑流程
构造时接收 `base_dir` 路径，所有后续文件操作均在此目录下进行，提供统一的文件读写基础能力。
