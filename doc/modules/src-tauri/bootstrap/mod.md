# bootstrap/mod

## 职责
组合根装配分块。声明主侧启动顺序契约，构造 Tauri Builder 并注册三个内建插件，处理运行期事件（`Ready` / `Exit`）。`main.rs` 只保留二进制入口必须自己做的事（`windows_subsystem` 属性、`generate_context!`、接线）；其余装配逻辑都在 lib 内，因而可被 `cargo test` / `clippy --all-targets` / 覆盖率口径触达。

**启动顺序契约**（`AppContext::init` 的 1–12 步以 `core/context.rs` 为唯一编号来源，本模块不复制）：
1. 前置（Builder 之前）：`early::prepare` —— panic 钩子 → 启动错误框 arm → `--console` → `--settings`
2. `.setup()`：`setup::install` —— 数据目录 + 一次性迁移 → `AppContext::init` → 诊断重放 → 通知出口 + `manage` → 事件消费者 → 插件 autostart
3. 收尾：`commands::invoke_handler` 注册全部 Command → `build` → `run`（`Ready` 标记启动完成、`Exit` 收尾插件）

## 核心类型/函数
- `fn builder() -> tauri::Builder<tauri::Wry>` — 构造 Builder 并注册三个插件：`single_instance`（**必须第一个注册**，重复启动在 setup 之前退出）、`dialog`（设置页数据目录选择器）、`opener`（Rust-only 外链转交，`open_js_links_on_click(false)` 关闭默认注入脚本，保证 `open_external` 是唯一转交通道）。
- `fn handle_run_event(app: &tauri::AppHandle, event: tauri::RunEvent)` — `Ready` → `startup_dialog::mark_started()`（此前窗口/WebView2 创建失败仍会弹启动错误框）；`Exit` → `AppContext::plugin_manager.cancel_all()` 收尾全部插件 worker。

## 直接依赖
- `crate::core::context::AppContext`（Exit 时取 State）
- `crate::infra::{single_instance, startup_dialog}`
- `tauri::{Builder, Manager, RunEvent, Wry}`
- `tauri_plugin_dialog` / `tauri_plugin_opener`

## 被依赖
- `src-tauri/src/main.rs` — 接线：`early::prepare()` → `builder()` → `.setup(setup::install)` → `.invoke_handler(commands::invoke_handler())` → `.build(generate_context!())` → `.run(handle_run_event)`

## 逻辑流程
1. `main()` 先调 `early::prepare`，再取本模块的 `builder()`。
2. `.setup(|app| setup::install(app.handle()))`：装配失败直接 panic（钩子落盘 + 启动框语义不变）。
3. `.invoke_handler(commands::invoke_handler())`：43 条 Command 的注册表在 `commands/mod.rs`。
4. `.build(tauri::generate_context!())` 失败 → `panic!("无法启动桌面应用：{error}")`。
5. `.run(handle_run_event)`：`Ready` 后运行期 panic 不再弹框；`Exit` 同步取消插件。
