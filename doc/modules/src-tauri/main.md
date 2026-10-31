# main

## 职责
应用程序入口点与**组合根**。按顺序初始化所有子系统，构造 `AppContext`，启动全部事件消费者，启动 Tauri 桌面窗口。release 构建经 `#![windows_subsystem = "windows"]` 走 GUI 子系统（不带控制台，`--console` 启动参数可重新附加，见 `infra/console.md`）；崩溃经 panic 钩子落日志（见 `infra/panic_report.md`），启动期失败还会在记录落盘后弹一次系统错误框（见 `infra/startup_dialog.md`）。

## 核心类型/函数
- `fn main()` — 程序入口：**先安装 panic 钩子（`infra::panic_report::install`）→ `startup_dialog::arm()`（启用启动期错误框，记主线程；测试进程不 arm）→ `--console` 附加调试控制台（`infra::console::requested/attach`，幂等，`args_os()` 收集参数）→** 创建 Tokio runtime → 注册 dialog 插件 → **在 `.setup()` 里**解析数据目录（`app_local_data_dir()`）、执行一次性迁移、初始化 `AppContext`、接上插件通知出口并 `manage` → 启动事件消费者 → spawn 插件自启动 → 以 `generate_handler!` 注册全部 Command → 启动 Tauri App；`run` 回调收到 `RunEvent::Ready` 时 `startup_dialog::mark_started()`（此后运行期 panic 只落日志），收到 `RunEvent::Exit` 时收尾全部插件。控制台重绑必须早于 `Logger::init` 与任何 stdio 使用。三处启动不可恢复点（Tokio runtime / Tauri Builder / `AppContext::init`）用 `unwrap_or_else(|e| panic!("中文上下文：{e}"))` 保留 panic 语义（钩子照常落盘），payload 会显示在启动期错误框里
- `const WORKSPACE_SAVED_EVENT: &str = "workspace-saved"` — 工作区落盘事件的前端通道名，与 `src/bridge/workspace.bridge.ts` 的 `listen` 对应
- `const ANNOUNCEMENTS_PUBLISHED_EVENT: &str = "announcements-published"` — 新公告事件的前端通道名，与 `src/bridge/announcement.bridge.ts` 的 `onAnnouncementsPublished` 对应
- `const FRONTEND_BRIDGE_CONSUMER: &str = "tauri-frontend-bridge"` — 前端事件桥的消费者名（日志字段）
- `fn spawn_event_consumers(app: &tauri::AppHandle)` — **启动全部事件消费者**。三类消费者挂在同一条底层事件流上：
  ```text
  CoreEventBus（broadcast）
    ├── tauri-frontend-bridge → emit 到 webview（前端 Store 只做 UI 刷新）
    ├── audit                 → 只读审计日志（infra::audit）
    └── plugin-host           → CoreEvent → PluginEvent（白名单 / 脱敏 / 版本）
  ```
  **内部 Service 之间没有任何事件订阅**：工作区落盘、OJ 切换清缓存、配置落盘、会话持久化、登出清理全部由 Service / Command 显式完成。事件只承载「已经发生的事实」，因此消费者落后、崩溃或缺失都不影响任何业务动作。
  **整段包在 `tauri::async_runtime::spawn` 里**：`spawn_consumer` 内部用 `tokio::spawn`，必须有 tokio runtime 上下文；而 `.setup()` 回调运行在 Tauri 的主线程上、不在运行时里。命令层（异步命令）天然在运行时内，组合根则需要显式投递一次 —— 也**不能**用 `main()` 里那个 `rt`（它在 setup 结束时被销毁，消费者会开机即死）。
- `fn spawn_frontend_event_bridge(bus, app)` — 用 `spawn_consumer` 启动前端桥；`handle` 调 `forward_to_webview`，`resync` 只在 `Lagged` 时 `warn!`（前端本来就有轮询，下一周期即补齐）
- `fn forward_to_webview(app, event)` — 单条事件的转发逻辑，只转发两种「前端需要即时感知」的事实：
  - `CoreEvent::WorkspaceSaved { workspace_id, revision, automatic }` → `workspace-saved`，载荷 `{ workspaceId, revision, auto }`。后台 auto-save 由 Rust 触发，前端无从感知（这是唯一非前端发起的落盘路径），故用它清除「编辑中…」指示；`revision` 让前端能幂等过滤重复 / 过期事件
  - `CoreEvent::AnnouncementChanged { contest_id, new_ids }` → `announcements-published`，载荷 `{ contestId, newIds }`。红点即时点亮，不必等下一轮 diff
  - 其余事件（OJ 切换、配置变更、提交创建/判定……）**不桥接**：前端都能从 IPC 返回值直接得到，转发只会制造「前端靠事件拿状态」的错误预期 —— 刷新触发之外的一切真实状态仍必须经 IPC 查询（评测状态、榜单、公告内容、工作区真值）
- `fn emit_or_warn(app, channel, payload, what)` — `emit` 失败只 `warn!`：前端事件是刷新触发，丢了不影响正确性（轮询会补齐）
- `fn report_data_dir(plan, migration)` — 汇报数据目录方案与迁移结果（在日志就绪后调用），见「逻辑流程」
- **已删除的三个安装函数**：`install_workspace_event_bridge` / `install_announcement_event_bridge`（旧 `EventBus` 的同步订阅桥）与 `install_auto_save_config_sync`（订阅 `SystemEvent::ConfigReloaded` 同步 auto-save）—— 前两者被单个 `spawn_event_consumers` + 前端桥取代；后者改为命令层**显式**调用 `commands::workspace_cmd::sync_auto_save_from_context`（见 `commands/config_cmd.md`）

## 直接依赖
- `hinina_lib::commands`
- `hinina_lib::core::context::AppContext`
- `hinina_lib::core::event::consumer::{spawn_consumer, ResyncReason}`（前端桥消费者）
- `hinina_lib::core::event::core_event::CoreEvent`（转发分支）
- `hinina_lib::core::event::core_event_bus::CoreEventBus`
- `hinina_lib::infra::audit::spawn_audit_consumer`
- `hinina_lib::infra::console::{requested, attach}`（`--console` 启动参数，见 `infra/console.md`）
- `hinina_lib::infra::data_dir::{self, DataDirPlan, DataDirSource, MigrateOutcome}`
- `hinina_lib::infra::panic_report::install`（panic 钩子，见 `infra/panic_report.md`）
- `hinina_lib::infra::startup_dialog::{arm, mark_started}`（启动期错误框，见 `infra/startup_dialog.md`）
- `hinina_lib::infra::plugin_notification::TauriPluginNotifications`（插件通知的前端出口）
- `tauri::{Emitter, Manager}`（`app.state::<AppContext>()` + `handle.emit`）
- `serde_json`（事件载荷）
- `tracing`（下发失败告警）
- `tokio::runtime::Runtime`

## 被依赖
- 无 — 顶层入口点，不被其他模块引用

## 逻辑流程
0. **panic 钩子最先安装**（此后包括参数解析在内的任何 panic 都有落点）→ `startup_dialog::arm()`（启用启动期错误框；测试进程不 arm）→ `--console` 附加调试控制台（release GUI 构建默认无控制台：从终端启动挂父控制台、双击启动新开窗口、已有控制台幂等跳过；**只有新开的窗口**才切 UTF-8 输出代码页并打开 VT 处理（否则中文乱码、ANSI 以字面量刷屏），挂父控制台/dev 终端不改配置——那是用户日常会话的状态；参数用 `args_os()` 收集，非 Unicode 参数不会 panic）→ 此后的 stderr 兜底与 Logger 的 stderr 层都接得住。**每次 panic 都有持久落点**：无条件落 `%TEMP%/hinina-panic.log`（不经 tracing 过滤器，`RUST_LOG=off`、日志文件打不开都不影响）；日志未被 `RUST_LOG` 关闭时同一条报告另落 `logs/hinina.log`（见 `infra/panic_report.md`）。**启动期的 panic 还会弹一次错误框**：钩子在记录落盘后调用 `startup_dialog`（主线程、启动未完成、每进程一次），把「双击无反应」变成可读的失败原因
1. 创建 Tokio runtime（`tokio::runtime::Runtime::new()`，失败时 `panic!("无法创建后台运行时：{e}")`）—— `AppContext::init` 是 async，
   而 `.setup()` 是同步回调，故 runtime 必须在 setup 之前建好。**注意：事件消费者不跑在这个
   运行时上**（它在 setup 结束时随 `rt` 一起销毁），而是由 `spawn_event_consumers` 投到
   Tauri 的运行时 —— 与异步命令同一个
2. `tauri::Builder::default().plugin(tauri_plugin_dialog::init())` 注册目录选择器插件
   （本项目**第一个真正注册的插件**：`tauri-plugin-fs` / `-http` 的依赖与 capability
   虽存在但从未注册，全部 I/O 都走 Rust）
3. `.setup(move |app| { … })` —— **整个初始化都在这里**，因为只有 setup 能拿到
   `AppHandle`，也就只有那里能解析 `app_local_data_dir()`：
   1. `default_dir = app.path().app_local_data_dir()`（`%LOCALAPPDATA%/{identifier}`，
      解析失败时退到 `data_dir::legacy_dir()` 让回退链继续）
   2. `data_dir::prepare_startup(&default_dir, &legacy)` → 解析 base_dir + **一次性迁移**
      （**必须在 `AppContext::init` 之前**：init 会打开日志文件，之后旧目录就被占住）
   3. `rt.block_on(AppContext::init(base_dir, default_dir, source))` 装配上下文
      （Logger（stderr + `{base_dir}/logs/hinina.log` 双路输出）→ Storage → CoreEventBus →
      SessionRepository → ConfigService → HttpClient（超时取自配置 `oj.timeout_secs`）→
      ProviderRegistry（按 `oj.instances` 实例注册全部内建 OJ，active 未注册回退首个已注册 OJ）→
      WorkspaceManager → 五个 Service → PluginHost → `PluginManager`（装配文件仓库
      `FsPluginRepository` 与 API 工厂 `FsPluginApiFactory` + `ServiceQueries`）→ AppContext）。
      失败时 `panic!("无法初始化应用数据：{}", error.user_message())`——最常见的是
      `%TEMP%/hinina` 回退目录也建不出来（`create_dir_all(base_dir)`）
   4. `report_data_dir(&plan, &migration)` —— **日志就绪后立刻汇报**：回退临时目录走
      `error` 级告警（数据随时会被系统清理）；迁移有失败项也走 `error` 并列出条目
   5. `ctx.plugin_manager.set_output(Arc<TauriPluginNotifications>)` —— 插件通知的
      前端出口（emit 到 `plugin-notification` 通道）在 `manage` 之前接上
   6. `app.manage(ctx)` 注入 `AppContext` 到 State
   7. `spawn_event_consumers(app.handle())` —— **消费者在 AppContext 就绪后启动**，
      且必须在 `AppContext::init` 之后：它们从 `app.state::<AppContext>()` 取总线与插件宿主，
      并只观察事实、不参与任何业务动作
   8. `tauri::async_runtime::spawn(plugins.autostart())` —— 异步扫描 `plugins.json`
      并启动全部 enabled 插件；配置不可用时 `warn!`「automatic loading skipped」，
      不阻断窗口启动
4. `.invoke_handler(tauri::generate_handler![...])` 注册全部 IPC Command
   （**不是** `setup` 中手动注册，`commands::register_commands()` 不存在；
   commands/mod.rs 注释亦说明选用 generate_handler 以避免 setup 手动注册的兼容性问题）：
   - `plugin_cmd`：plugin_list / plugin_start / plugin_stop / plugin_reload / plugin_resync
   - `auth_cmd`：login / logout / get_session / validate_session
   - `oj_cmd`：**switch_oj**（切换当前 OJ：补注册 → 校验已注册 → `set_current` → 显式清 contest/problem/submission 的 OJ 域缓存 → 持久化 `oj.active` → 发布 `CoreEvent::OjSwitched`）
   - `contest_cmd`：list_contests / select_contest / load_configured_contest / **get_contest_rank** / **list_contest_announcements** / **get_read_announcement_ids** / **mark_announcements_read**
   - `problem_cmd`：get_problem / **get_user_problem_status** / **get_contest_problem_limits**
   - `submission_cmd`：submit_code / get_judgement / **list_contest_submissions** / **get_submission_detail** / **get_submission_cases**
   - `workspace_cmd`：load_workspace / save_workspace / switch_workspace / current_workspace / update_workspace_file / **set_workspace_language** / **delete_workspace_file**
   - `config_cmd`：get_config / reload_config / update_config / **get_storage_info**
   - `log_cmd`：**frontend_log**（前端全局错误 → tracing 落 `logs/hinina.log`；级别/scope 白名单 + 消息收敛，见 `commands/log_cmd.md`）
   - `maintenance_cmd`：**reset_client**（设置页「重置客户端」：三层缓存 + 公告基线 + 公告已读状态，清完不重拉）/ **local_data_usage**（清理前的体积预览）/ **purge_local_data**（不可逆：清日志内容与崩溃记录文件、过期提交留档）
   - `data_dir_cmd`：**get_data_dir** / **set_data_dir** / **reset_data_dir** / **pick_data_dir**（设置页「数据目录」：改动重启后生效，见 `commands/data_dir_cmd.md`）
   - `theme_cmd`：get_theme / set_theme
5. `.build(tauri::generate_context!())` 构建应用（失败时 `panic!("无法启动桌面应用：{e}")`，触发启动期错误框），`.run(|app, event| …)` 进入事件循环：
   - `RunEvent::Ready` → `startup_dialog::mark_started()`。Tauri 的 `fn setup` 先按配置创建窗口（WebView2 在此初始化）、再调用用户的 setup 闭包，Ready 在 setup 返回后发出；因此窗口/WebView2 创建失败时标志尚未置位，启动错误框仍会弹出。
   - `RunEvent::Exit` 时取 `AppContext` 调 `plugin_manager.cancel_all()` 收尾全部插件 worker


### 插件装配现状

插件子系统已在组合根**完整装配**：`AppContext::init` 构造 `PluginHost` 与 `PluginManager`
——文件仓库 `FsPluginRepository` 扫描 `plugins/` 目录读取 `manifest.json` 与入口 JS 源码，
API 工厂 `FsPluginApiFactory` 提供存储 / HTTP 端口（通知出口由 `set_output` 注入 `PluginManager`，
见 `infra/fs_plugin_repo.md`、`infra/plugin_api_factory.md`）。`main.rs` 接上通知出口后 `manage`，事件消费者含
`plugin-host`（CoreEvent → PluginEvent 白名单投影），随后 spawn `autostart()`：扫描
`plugins.json` 并逐个启动 enabled 插件（QuickJS worker 加载入口 JS）。默认不启用：
`plugins.json` 不存在视为空配置，是否启用由用户配置决定；运行期控制走
`commands/plugin_cmd` 五个命令（开发期另有 DEV-only `window.hininaPlugins`）。
