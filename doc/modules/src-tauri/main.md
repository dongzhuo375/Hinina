# main

## 职责
应用程序入口点与**组合根**。按顺序初始化所有子系统，构造 `AppContext`，启动全部事件消费者，启动 Tauri 桌面窗口。

## 核心类型/函数
- `fn main()` — 程序入口：创建 Tokio runtime → 注册 dialog 插件 → **在 `.setup()` 里**解析数据目录（`app_local_data_dir()`）、执行一次性迁移、初始化 `AppContext` 并 `manage` → 启动事件消费者 → 以 `generate_handler!` 注册全部 Command → 启动 Tauri App
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
- `hinina_lib::infra::data_dir::{self, DataDirPlan, DataDirSource, MigrateOutcome}`
- `tauri::{Emitter, Manager}`（`app.state::<AppContext>()` + `handle.emit`）
- `serde_json`（事件载荷）
- `tracing`（下发失败告警）
- `tokio::runtime::Runtime`

## 被依赖
- 无 — 顶层入口点，不被其他模块引用

## 逻辑流程
1. 创建 Tokio runtime（`tokio::runtime::Runtime::new()`）—— `AppContext::init` 是 async，
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
      WorkspaceManager → 五个 Service → PluginHost → AppContext）
   4. `report_data_dir(&plan, &migration)` —— **日志就绪后立刻汇报**：回退临时目录走
      `error` 级告警（数据随时会被系统清理）；迁移有失败项也走 `error` 并列出条目
   5. `app.manage(ctx)` 注入 `AppContext` 到 State
   6. `spawn_event_consumers(app.handle())` —— **消费者在 AppContext 就绪后启动**，
      且必须在 `AppContext::init` 之后：它们从 `app.state::<AppContext>()` 取总线与插件宿主，
      并只观察事实、不参与任何业务动作
4. `.invoke_handler(tauri::generate_handler![...])` 注册全部 IPC Command
   （**不是** `setup` 中手动注册，`commands::register_commands()` 不存在；
   commands/mod.rs 注释亦说明选用 generate_handler 以避免 setup 手动注册的兼容性问题）：
   - `auth_cmd`：login / logout / get_session / validate_session
   - `oj_cmd`：**switch_oj**（切换当前 OJ：补注册 → 校验已注册 → `set_current` → 显式清 contest/problem/submission 的 OJ 域缓存 → 持久化 `oj.active` → 发布 `CoreEvent::OjSwitched`）
   - `contest_cmd`：list_contests / select_contest / load_configured_contest / **get_contest_rank** / **list_contest_announcements** / **get_read_announcement_ids** / **mark_announcements_read**
   - `problem_cmd`：get_problem / **get_user_problem_status** / **get_contest_problem_limits**
   - `submission_cmd`：submit_code / get_judgement / **list_contest_submissions** / **get_submission_detail** / **get_submission_cases**
   - `workspace_cmd`：load_workspace / save_workspace / switch_workspace / current_workspace / update_workspace_file / **set_workspace_language**
   - `config_cmd`：get_config / reload_config / update_config / **get_storage_info**
   - `maintenance_cmd`：**reset_client**（设置页「重置客户端」：三层缓存 + 公告基线 + 公告已读状态，清完不重拉）/ **local_data_usage**（清理前的体积预览）/ **purge_local_data**（不可逆：清日志内容与过期提交留档）
   - `data_dir_cmd`：**get_data_dir** / **set_data_dir** / **reset_data_dir** / **pick_data_dir**（设置页「数据目录」：改动重启后生效，见 `commands/data_dir_cmd.md`）
   - `theme_cmd`：get_theme / set_theme
6. `.run(tauri::generate_context!())` 启动 Tauri 桌面应用
