# main

## 职责
应用程序入口点。按顺序初始化所有子系统，构造 `AppContext`，启动 Tauri 桌面窗口。

## 核心类型/函数
- `fn main()` — 程序入口：创建 Tokio runtime → 阻塞式调用 `AppContext::init()` → 以 `generate_handler!` 注册 25 个 Command → 启动 Tauri App

## 直接依赖
- `hinina_lib::commands`
- `hinina_lib::core::context::AppContext`
- `tauri`（隐式通过 `tauri::Builder` 等）
- `tokio::runtime::Runtime`

## 被依赖
- 无 — 顶层入口点，不被其他模块引用

## 逻辑流程
1. 创建 Tokio runtime（`tokio::runtime::Runtime::new()`）
2. `rt.block_on(AppContext::init(base_dir))` 阻塞式装配上下文 —— base_dir 当前为
   `std::env::temp_dir().join("hinina")`（源码 TODO：待接入 Tauri app_data_dir），
   目录在 `init()` 内自动创建；实际初始化序列见 `core/context.md`
   （Logger → Storage → EventBus → ConfigService → HttpClient → ProviderRegistry+HOJ 注册 →
   WorkspaceManager → 五个 Service → AppContext）
3. `tauri::Builder::default().manage(ctx)` 注入 `AppContext` 到 State
4. `.invoke_handler(tauri::generate_handler![...])` 注册全部 25 个 IPC Command
   （**不是** `setup` 中手动注册，`commands::register_commands()` 不存在；
   commands/mod.rs 注释亦说明选用 generate_handler 以避免 setup 手动注册的兼容性问题）：
   - `auth_cmd`：login / logout / get_session / validate_session
   - `contest_cmd`：list_contests / select_contest / load_configured_contest / **get_contest_rank**
   - `problem_cmd`：get_problem / list_problems / **get_user_problem_status** / **get_contest_problem_limits**
   - `submission_cmd`：submit_code / get_judgement
   - `workspace_cmd`：load_workspace / save_workspace / switch_workspace / current_workspace / update_workspace_file / **set_workspace_language**
   - `config_cmd`：get_config / reload_config / update_config
   - `theme_cmd`：get_theme / set_theme
5. `.run(tauri::generate_context!())` 启动 Tauri 桌面应用
