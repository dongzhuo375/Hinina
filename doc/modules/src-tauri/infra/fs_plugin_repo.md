# fs_plugin_repo

## 职责
`PluginRepository` trait 的文件系统实现，负责插件扫描与代码加载。扫描指定目录下的插件清单，加载插件代码。所有方法当前为 TODO 占位。

## 核心类型/函数
- **`FsPluginRepository`** — 文件系统插件仓库 struct，持有 `Arc<Storage>`
- **`FsPluginRepository::new(storage: Arc<Storage>)`** — 构造函数
- **`scan_plugins()`** — 扫描插件目录，返回 `Vec<PluginManifest>`（TODO）
- **`load_plugin_code(plugin_id)`** — 加载指定插件的代码字符串（TODO）
- **`plugin_dir_exists()`** — 检查插件目录是否存在（TODO）

## 直接依赖
- `std::sync::Arc`
- `core::error::AppResult`
- `core::repository::plugin_repo::PluginRepository`
- `infra::storage::Storage`
- `plugin::host::manifest::PluginManifest`

## 被依赖
暂无（未被 infra 外部模块直接引用，预期由 `plugin` 层通过 trait 使用）

## 逻辑流程
构造函数接收 `Arc<Storage>`。`scan_plugins` 遍历 `Storage` 中的插件目录，解析每个插件的 `plugin.toml`/`plugin.json` 清单文件为 `PluginManifest`。`load_plugin_code` 读取指定插件的代码文件内容。v0.x 阶段插件系统仅预留架构。
