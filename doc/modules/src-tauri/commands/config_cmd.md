# config_cmd

## 职责
应用配置与存储诊断相关 Tauri Command 模块。提供配置读取、热重载、更新持久化，以及存储位置/版本信息查询（排障用），面向前端暴露 IPC 接口。

## 核心类型/函数
- **`StorageInfo`** — 存储与版本信息 struct（仅 `Serialize`，camelCase），供前端「关于/诊断」面板展示日志位置与客户端版本：
  - `base_dir: String` — 应用数据根目录
  - `log_path: String` — 日志文件完整路径（`{base_dir}/logs/hinina.log`，与 `infra/logger.rs` 的落盘约定一致）
  - `version: String` — 客户端版本（`env!("CARGO_PKG_VERSION")` 构建时注入，与 HttpClient UA 同源）
- `pub async fn get_storage_info(ctx) -> AppResult<StorageInfo>` — 获取存储与版本信息。前端 invoke 签名 `get_storage_info`；从 `ctx.storage.base_dir()` 拼出日志路径，纯本地查询不发网络请求
- `pub async fn get_config(ctx) -> AppResult<AppConfig>` — 获取完整应用配置（user/oj/editor/theme/layout 五个子分组）。配置源自磁盘持久化文件，首次启动时自动生成默认值；加载路径已做旧值归一（见 `service/config/mod.md`）
- `pub async fn reload_config(ctx) -> AppResult<AppConfig>` — 从磁盘重新加载配置，发布 `SystemEvent::ConfigReloaded`，各 Service 可通过监听此事件热更新参数
- `pub async fn update_config(ctx, config: AppConfig) -> AppResult<()>` — 将前端传来的完整 `AppConfig` 写入磁盘（`ConfigService::update` 整体替换）。配置字段按需热生效（如主题切换需额外调用 `set_theme` 发布事件）

## 直接依赖
- `serde::Serialize`（`StorageInfo`）
- `tauri::State`
- `tracing::info`
- `crate::core::context::AppContext`
- `crate::core::entity::config::AppConfig`
- `crate::core::error::AppResult`

## 被依赖
- `src-tauri/src/main.rs`（`generate_handler!` 注册全部四个 Command）

## 逻辑流程
1. 前端调用 `invoke('get_config')` 获取当前配置用于设置页面展示
2. 用户修改设置后，前端调用 `invoke('update_config', { config })` 持久化新配置
3. `invoke('reload_config')` 强制从磁盘重读（手改配置文件后使用），归一旧值后覆盖内存并发布事件
4. 排障时前端调用 `invoke('get_storage_info')` 展示数据目录、日志文件路径与客户端版本，指导选手取日志
5. `AppConfig` / `StorageInfo` 通过 serde 序列化在前后端间传递（均 camelCase）
