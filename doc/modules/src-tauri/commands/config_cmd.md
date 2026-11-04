# config_cmd

## 职责
应用配置与存储诊断相关 Tauri Command 模块。提供配置读取、热重载、更新持久化，以及存储位置/版本信息查询（排障用），面向前端暴露 IPC 接口。

## 核心类型/函数
- **`StorageInfo`** — 存储与版本信息 struct（仅 `Serialize`，camelCase），供前端「关于/诊断」面板展示日志位置与客户端版本：
  - `base_dir: String` — 应用数据根目录
  - `log_path: String` — 日志文件完整路径（`base_dir.join(infra::logger::LOG_RELATIVE_PATH)` 拼出，与落盘位置同源常量，不存在二次硬编码）
  - `version: String` — 客户端版本（`env!("CARGO_PKG_VERSION")` 构建时注入，与 HttpClient UA 同源）
- `pub async fn get_storage_info(ctx) -> AppResult<StorageInfo>` — 获取存储与版本信息。前端 invoke 签名 `get_storage_info`；日志路径由 `ctx.storage.base_dir()` 拼 `infra::logger::LOG_RELATIVE_PATH` 得出，纯本地查询不发网络请求
- `pub async fn get_config(ctx) -> AppResult<AppConfig>` — 获取完整应用配置（user/oj/editor/theme/layout 五个子分组）。配置源自磁盘持久化文件，首次启动时自动生成默认值；加载路径已做旧值归一（见 `service/config/mod.md`）
- `pub async fn reload_config(ctx) -> AppResult<AppConfig>` — 从磁盘重新加载配置。**注意：前端零调用（bridge 导出但生产不使用），#87 曾对其加固并配套并发 fence 测试，属预留端点——删除与否待产品定夺（2026-11 清理时保留）**。**同一写事务内「读盘+归一 → 无条件检查 active 指向已启用实例 → 需要时补注册/可用性预检 → 提交内存/ConfigChanged → 切 Registry/OjSwitched」**（评审三十四/三十五/三十六）：即使 Registry 未变（如全禁用配置归一后回退 HOJ）也做结构检查，拒绝 `validate()` 会拒绝的配置；预检失败时内存、Registry、事件全部保持原样。提交与 Registry 切换由**外层 AccessTransition** 覆盖，插件读者不会看到「新配置 + 旧 Registry」的混合态。编排提取为 `reload_config_impl`；最后在事务内**显式**同步 auto-save（不依赖事件消费者）
- `pub async fn update_config(ctx, config: AppConfig) -> AppResult<AppConfig>` — **锁定语义由后端自持真值执行**：`maintainer_locked = !launch_args::get().settings`（`--settings` 由组合根在启动早期固化，普通启动锁定字段保留、钥匙启动维护者可写全部字段）；前端不传「是否锁定」——UI 层判断只影响可编辑性，没有任何 IPC 输入能改变后端的执行路径。将前端传来的完整 `AppConfig` 写入磁盘（`ConfigService::update_locked` 整体替换），**返回后端实际提交的配置**（active 取写锁内值，前端据此判定保存结果）。**整个「读锁内最新配置 → 合并/锁定 → 固定 active → 最终校验 → 持久化 → 同步 auto-save」在同一写事务锁内完成**（评审三十三 #2、三十六 P2）：`oj.active` 取锁内最新值（只有 `switch_oj` 能改），随后对**固定 active 后的最终候选**做兜底校验（M4，config.json 可被手改绕过前端）：普通模式先 `AppConfig::validate()`，服务器地址非法时返回 `AppError::Config("服务器地址必须以 http:// 或 https:// 开头")` 拒绝落盘；再 `AppConfig::sanitize()` 把越界字段钳制到与前端 SettingsView 一致的取值域，发生钳制时记 `warn!` 日志。**维护者层锁定（`maintainer_locked=true`，普通启动）走另一条语义**（评审二十六 #2）：先用 `preserve_locked_fields` 以旧配置覆盖锁定字段（OJ 选择、实例清单与地址、超时/轮询/缓存 TTL、题面缓存），再 `validate_preserving_locked`（只查 active 指向已启用实例）与 `sanitize_editable`（只钳制比赛引用/编辑器/主题/布局）——手改出的非法锁定值既不阻塞可编辑字段保存，也不被钳制或改写。编排提取为 `apply_config_update` 供测试直接调用（`apply_config_update_with_hook` / `apply_config_update_with_sync_hook` 只在测试中注入事务内交错点）。**配置变更后的必要动作在同一事务内显式完成**：落盘成功后立即 `sync_auto_save_from_context(&ctx)`（幂等）——同步若在锁外，U1 保存关闭与 U2 保存开启可交错成「磁盘开启、管理器停止」（评审三十六 P2）—— 旧实现把这件事挂在 `ConfigReloaded` 事件的订阅者上（订阅者内部还要 `tauri::async_runtime::spawn` 才拿得到 tokio 上下文），消费者一旦落后或缺失，设置页改了开关却不生效且无从察觉。主题域（`set_theme`/`get_theme`）已随死代码清理删除：前端只有编辑器主题，经 `update_config` 落盘并由消费方本地应用

## 直接依赖
- `serde::Serialize`（`StorageInfo`）
- `tauri::State`
- `tracing::{info, warn}`（warn 用于 sanitize 钳制留痕）
- `crate::core::context::AppContext`
- `crate::core::entity::config::AppConfig`
- `crate::core::error::{AppError, AppResult}`（`AppError::Config` 承载地址校验失败）
- `crate::infra::logger::LOG_RELATIVE_PATH`（日志展示路径的唯一事实来源）
- `crate::infra::launch_args`（`update_config` 的锁定执行真值：`maintainer_locked = !launch_args::get().settings`，2026-11-04 G1）

## 被依赖
- `src-tauri/src/main.rs`（`generate_handler!` 注册全部四个 Command）

## 逻辑流程
1. 前端调用 `invoke('get_config')` 获取当前配置用于设置页面展示
2. 用户修改设置后，前端调用 `invoke('update_config', { config })` 持久化新配置：在**一次写事务**内读取锁内最新配置 → `oj.active` 始终以它为准（只有 switch_oj 能改）→ 普通模式 `validate()`（地址非法直接拒绝）再 `sanitize()`（越界钳制 + warn 留痕），锁定模式改为 `preserve_locked_fields` + `validate_preserving_locked` + `sanitize_editable` → **对固定 active 后的最终候选校验通过才整体替换**（校验失败内存/磁盘均保持原样）→ 落盘成功后**显式**同步 auto-save（`sync_auto_save_from_context`，幂等）。与 `switch_oj` 共锁：先后顺序由锁决定，不会出现「校验用旧 active、落盘写新 active」的非法终态（评审三十三 #2）
3. `invoke('reload_config')` 强制从磁盘重读（手改配置文件后使用），归一旧值后覆盖内存、发布 `ConfigChanged`，并同样显式同步 auto-save
4. 排障时前端调用 `invoke('get_storage_info')` 展示数据目录、日志文件路径与客户端版本，指导选手取日志
5. `AppConfig` / `StorageInfo` 通过 serde 序列化在前后端间传递（均 camelCase）
