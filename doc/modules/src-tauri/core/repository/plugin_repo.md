# plugin_repo

## 职责
定义插件仓库 trait `PluginRepository`，抽象插件目录扫描与代码加载，解耦 PluginHost 与底层文件系统。用于扫描插件目录、加载插件入口代码。

## 核心类型/函数
- **`PluginRepository`** — 插件仓库 trait，方法：
  - `scan_plugins(&self) -> AppResult<Vec<PluginManifest>>` — 扫描插件目录，返回清单列表
  - `load_plugin_code(&self, plugin_id) -> AppResult<String>` — 加载插件入口代码
  - `plugin_dir_exists(&self) -> bool` — 检查插件目录是否存在

## 直接依赖
- `core::error::AppResult`
- `plugin::host::manifest::PluginManifest`

## 被依赖
- `infra::fs_plugin_repo`

## 逻辑流程
无（纯 trait 定义）。
