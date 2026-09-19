# main

## 职责
应用程序入口点。按顺序初始化所有子系统，构造 `AppContext`，启动 Tauri 桌面窗口。

## 核心类型/函数
- `fn main()` — 程序入口：创建 Tokio runtime → 阻塞式调用 `AppContext::init()` → 以 `generate_handler!` 注册 33 个 Command → 启动 Tauri App
- `const WORKSPACE_SAVED_EVENT: &str` — 工作区落盘事件的前端通道名（`"workspace-saved"`），与 `src/bridge/workspace.bridge.ts` 的 `listen` 对应
- `fn install_workspace_event_bridge(app: &tauri::AppHandle)` — 订阅 `EventCategory::Workspace`，把 `WorkspaceEvent::Saved`（显式保存）与 `AutoSaveTriggered`（auto-save 成功）emit 到 webview（载荷 `{ workspaceId, auto }`）。仅转发这两种「内容确已落盘」的事件；`Loaded` / `Switched` 不转发（前端是发起方，无需回环）

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
1. 创建 Tokio runtime（`tokio::runtime::Runtime::new()`）
2. `rt.block_on(AppContext::init(base_dir))` 阻塞式装配上下文 —— base_dir 当前为
   `std::env::temp_dir().join("hinina")`（源码 TODO：待接入 Tauri app_data_dir），
   目录在 `init()` 内自动创建；实际初始化序列见 `core/context.md`
   （Logger（stderr + `{base_dir}/logs/hinina.log` 双路输出）→ Storage → EventBus →
   ConfigService → HttpClient（超时取自配置 `oj.timeout_secs`）→ ProviderRegistry（按 `oj.instances`
   实例注册全部内建 OJ，active 未注册回退 HOJ）→
   WorkspaceManager → 五个 Service → AppContext）
3. `tauri::Builder::default().manage(ctx)` 注入 `AppContext` 到 State
4. `.setup(|app| { install_workspace_event_bridge(app.handle()); Ok(()) })` 装配工作区落盘事件桥
   （`app.state::<AppContext>()` 取 EventBus 订阅，`handle.emit("workspace-saved", …)` 下发前端）
5. `.invoke_handler(tauri::generate_handler![...])` 注册全部 33 个 IPC Command
   （**不是** `setup` 中手动注册，`commands::register_commands()` 不存在；
   commands/mod.rs 注释亦说明选用 generate_handler 以避免 setup 手动注册的兼容性问题）：
   - `auth_cmd`：login / logout / get_session / validate_session
   - `oj_cmd`：**switch_oj**（切换当前 OJ：校验注册 → 持久化 `oj.active` → 发布 `OJSwitched`）
   - `contest_cmd`：list_contests / select_contest / load_configured_contest / **get_contest_rank** / **list_contest_announcements** / **get_read_announcement_ids** / **mark_announcements_read**
   - `problem_cmd`：get_problem / list_problems / **get_user_problem_status** / **get_contest_problem_limits**
   - `submission_cmd`：submit_code / get_judgement / **list_contest_submissions** / **get_submission_detail** / **get_submission_cases**
   - `workspace_cmd`：load_workspace / save_workspace / switch_workspace / current_workspace / update_workspace_file / **set_workspace_language**
   - `config_cmd`：get_config / reload_config / update_config / **get_storage_info**
   - `theme_cmd`：get_theme / set_theme
6. `.run(tauri::generate_context!())` 启动 Tauri 桌面应用
