# main

## 职责
二进制入口点（thin entry）。只保留 release 的 GUI 子系统属性与接线，不承载装配细节：进程级前置在 `bootstrap::early`，Builder/插件链与运行期回调在 `bootstrap`，`.setup()` 装配在 `bootstrap::setup`，命令注册表在 `commands::invoke_handler`。启动顺序契约见 `bootstrap/mod.md`；原住本文件的事件桥、`TauriPluginNotifications`、`report_data_dir`、43 条 `generate_handler!` 列表与插件注册链均已迁入对应模块。

## 核心类型/函数
- `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` — release 构建走 GUI 子系统（不附控制台，`--console` 可重新附加，见 `infra/console.md`）；debug/dev 保持控制台，`npm run tauri dev` 不受影响。**必须留在二进制根**（lib 无法为 bin 设置 crate 属性）。
- `fn main()` — 五步接线：
  1. `bootstrap::early::prepare()`；
  2. `bootstrap::builder()` 取 Builder（单实例 → dialog → opener）；
  3. `.setup(|app| bootstrap::setup::install(app.handle()))`；
  4. `.invoke_handler(commands::invoke_handler())`；
  5. `.build(tauri::generate_context!())`（失败 `panic!("无法启动桌面应用：{error}")`）→ `.run(bootstrap::handle_run_event)`。

## 直接依赖
- `hinina_lib::bootstrap::{early, builder, setup, handle_run_event}`
- `hinina_lib::commands::invoke_handler`
- `tauri::generate_context!`

## 被依赖
- 无 — 顶层入口点，不被其他模块引用

## 逻辑流程
`prepare()`（panic 钩子 → 启动错误框 arm → `--console` → `--settings`）→ `builder()`（单实例插件**第一个注册**；dialog；opener 关闭默认注入脚本）→ setup 装配（数据目录 + 一次性迁移 → `AppContext::init` → 诊断重放 → 通知出口 + `manage` → 事件消费者 → 插件 autostart）→ 注册 43 条 Command → build → run 回调（`RunEvent::Ready` → `startup_dialog::mark_started()`；`RunEvent::Exit` → `plugin_manager.cancel_all()`）。各步的约束与失败语义见 `bootstrap/mod.md`、`bootstrap/early.md`、`bootstrap/setup.md`。
