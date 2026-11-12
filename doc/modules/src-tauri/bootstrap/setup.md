# bootstrap/setup

## 职责
`.setup()` 内的应用装配：解析数据目录并执行一次性迁移、初始化 `AppContext`、在日志就绪后重放数据目录诊断、接上插件通知出口并 `manage` 状态、启动事件消费者、spawn 插件 autostart。数据目录/迁移**必须早于** `AppContext::init`（init 会打开日志文件并占住旧目录）。

## 核心类型/函数
- `fn install(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>>` — 装配入口，步骤：
  1. 在 setup 内创建 tokio runtime（只服务 `AppContext::init` 的 `block_on`；**事件消费者不跑在这个运行时上**，它随 setup 结束销毁）。
  2. `app_local_data_dir()`：解析失败不静默顶替——原因交给 `prepare_startup_with_default_error` 显式降级为 `FallbackTemp`；release GUI 下 stderr 写失败用忽略的 `writeln!`（`eprintln!` 写失败会 panic）。`legacy_dir()` 只取一次。
  3. `data_dir::prepare_startup_with_default_error(...)`：解析 base_dir + 一次性迁移。
  4. `rt.block_on(AppContext::init(...))`：失败时**先** `report_data_dir` 再 `panic!("无法初始化应用数据：{...}")`（失败路径也不丢「为什么」）。
  5. `report_data_dir`：日志就绪后立刻汇报来源与迁移结果。
  6. `plugin_manager.set_output(Arc::new(TauriPluginNotifications(...)))` → `app.manage(ctx)`。
  7. `events::spawn_event_consumers(app)`：消费者在 State 就绪后启动。
  8. `tauri::async_runtime::spawn(plugins.autostart())`：配置不可用时 `warn!`（`operation=host.autostart`），不阻断窗口。
- `fn report_data_dir(plan: &DataDirPlan, migration: &MigrateOutcome)`（私有）— `replay(plan.warnings)`；`Default`/`Custom` 走 `info`，`FallbackTemp` 走 `error` 强告警；迁移成功 `info`、失败 `error` 并列条目（下次启动自动重试）。

## 直接依赖
- `crate::core::context::AppContext`
- `crate::bootstrap::{events, notification::TauriPluginNotifications}`
- `crate::infra::data_dir::{self, DataDirPlan, DataDirSource, MigrateOutcome}`
- `tauri::{AppHandle, Manager}` / `tokio::runtime::Runtime`

## 被依赖
- `bootstrap::builder` 的调用方 `src-tauri/src/main.rs`（经 `.setup(...)`）

## 逻辑流程
见上「步骤 1–8」。顺序即契约：数据目录 → 迁移 → `AppContext::init`（内部 1–12 步，见 `core/context.rs`）→ 诊断 → 通知出口 + `manage` → 消费者 → autostart。失败路径只有 `AppContext::init` 一处 panic，其余降级都记录后继续。
