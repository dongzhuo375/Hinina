# main

## 职责
应用程序入口点。按顺序初始化所有子系统，构造 `AppContext`，启动 Tauri 桌面窗口。

## 核心类型/函数
- `fn main()` — 程序入口：创建 Tokio runtime → 注册 dialog 插件 → **在 `.setup()` 里**解析数据目录（`app_local_data_dir()`）、执行一次性迁移、初始化 `AppContext` 并 `manage` → 以 `generate_handler!` 注册 40 个 Command → 启动 Tauri App
- `const WORKSPACE_SAVED_EVENT: &str` — 工作区落盘事件的前端通道名（`"workspace-saved"`），与 `src/bridge/workspace.bridge.ts` 的 `listen` 对应
- `const ANNOUNCEMENTS_PUBLISHED_EVENT: &str` — 新公告事件的前端通道名（`"announcements-published"`），与 `src/bridge/announcement.bridge.ts` 的 `onAnnouncementsPublished` 对应
- `fn install_workspace_event_bridge(app: &tauri::AppHandle)` — 订阅 `EventCategory::Workspace`，把 `WorkspaceEvent::Saved`（显式保存）与 `AutoSaveTriggered`（auto-save 成功）emit 到 webview（载荷 `{ workspaceId, auto }`）。仅转发这两种「内容确已落盘」的事件；`Loaded` / `Switched` 不转发（前端是发起方，无需回环）
- `fn install_announcement_event_bridge(app: &tauri::AppHandle)` — 订阅 `EventCategory::Contest`，把 `ContestEvent::AnnouncementsPublished` emit 到 webview（载荷 `{ contestId, newIds }`）。公告红点因此是**事件驱动**的：前端虽仍按 60s 节拍拉取公告（拉取必须有人发起），但「有新公告」这一状态变更走 EventBus，事件到达即点亮红点，不必等下一次列表 diff。`ListLoaded` / `Selected` 不转发（前端是发起方）
- `fn install_auto_save_config_sync(app: &tauri::AppHandle)` — 订阅 `EventCategory::System` 的 `SystemEvent::ConfigReloaded`，按新配置调 `commands::workspace_cmd::sync_auto_save_with_config` 同步 auto-save 的启停与间隔（P48：设置页改开关/间隔**即时生效，无需重启**；判据 = 纯函数 `auto_save_action`）。事件由 `ConfigService::update` 与 `reload` 两条路径发布。同步动作经 `tauri::async_runtime::spawn` 执行 —— auto-save 的启停需要 tokio 上下文，**不依赖「发布方一定在 tokio 上下文里」这个隐含前提**。放在组合根是因为事件是应用级关注点，且只有这里能同时拿到 EventBus / ConfigService / WorkspaceManager

## 直接依赖
- `hinina_lib::commands`
- `hinina_lib::core::context::AppContext`
- `hinina_lib::core::event::{app_event, event_bus, event_category}`（事件桥）
- `tauri::{Emitter, Manager}`（`app.state::<AppContext>()` + `handle.emit`）
- `serde_json`（事件载荷）
- `tracing`（下发失败告警）
- `tokio::runtime::Runtime`

## 被依赖
- 无 — 顶层入口点，不被其他模块引用

## 逻辑流程
1. 创建 Tokio runtime（`tokio::runtime::Runtime::new()`）—— `AppContext::init` 是 async，
   而 `.setup()` 是同步回调，故 runtime 必须在 setup 之前建好
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
      （Logger（stderr + `{base_dir}/logs/hinina.log` 双路输出）→ Storage → EventBus →
      ConfigService → HttpClient（超时取自配置 `oj.timeout_secs`）→ ProviderRegistry（按
      `oj.instances` 实例注册全部内建 OJ，active 未注册回退首个已注册 OJ）→
      WorkspaceManager → 五个 Service → AppContext）
   4. `report_data_dir(&plan, &migration)` —— **日志就绪后立刻汇报**：回退临时目录走
      `error` 级告警（数据随时会被系统清理）；迁移有失败项也走 `error` 并列出条目
   5. `app.manage(ctx)` 注入 `AppContext` 到 State
   6. `install_workspace_event_bridge` / `install_announcement_event_bridge` /
      `install_auto_save_config_sync`
      （`app.state::<AppContext>()` 取 EventBus 订阅，`handle.emit("workspace-saved", …)` /
      `handle.emit("announcements-published", …)` 下发前端；auto-save 同步不回前端，直接调命令层）
4. `.invoke_handler(tauri::generate_handler![...])` 注册全部 39 个 IPC Command
   （**不是** `setup` 中手动注册，`commands::register_commands()` 不存在；
   commands/mod.rs 注释亦说明选用 generate_handler 以避免 setup 手动注册的兼容性问题）：
   - `auth_cmd`：login / logout / get_session / validate_session
   - `oj_cmd`：**switch_oj**（切换当前 OJ：校验注册 → 持久化 `oj.active` → 发布 `OJSwitched`）
   - `contest_cmd`：list_contests / select_contest / load_configured_contest / **get_contest_rank** / **list_contest_announcements** / **get_read_announcement_ids** / **mark_announcements_read**
   - `problem_cmd`：get_problem / **get_user_problem_status** / **get_contest_problem_limits**
   - `submission_cmd`：submit_code / get_judgement / **list_contest_submissions** / **get_submission_detail** / **get_submission_cases**
   - `workspace_cmd`：load_workspace / save_workspace / switch_workspace / current_workspace / update_workspace_file / **set_workspace_language**
   - `config_cmd`：get_config / reload_config / update_config / **get_storage_info**
   - `maintenance_cmd`：**reset_client**（设置页「重置客户端」：三层缓存 + 公告基线 + 公告已读状态，清完不重拉）/ **local_data_usage**（清理前的体积预览）/ **purge_local_data**（不可逆：清日志内容与过期提交留档）
   - `data_dir_cmd`：**get_data_dir** / **set_data_dir** / **reset_data_dir** / **pick_data_dir**（设置页「数据目录」：改动重启后生效，见 `commands/data_dir_cmd.md`）
   - `theme_cmd`：get_theme / set_theme
6. `.run(tauri::generate_context!())` 启动 Tauri 桌面应用
