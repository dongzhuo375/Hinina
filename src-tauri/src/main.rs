// release 构建走 GUI 子系统：不附控制台（用户可带 `--console` 重新附加，见
// `infra::console`）。debug/dev 构建保持控制台，`npm run tauri dev` 不受影响。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Hinina 入口点。
//
// 二进制根只保留 release 子系统属性与接线：进程级前置在 `bootstrap::early`，
// Builder/插件链与运行期回调在 `bootstrap`，setup 装配在 `bootstrap::setup`，
// 命令注册表在 `commands::invoke_handler`。启动顺序契约见 `bootstrap/mod.rs`。

fn main() {
    hinina_lib::bootstrap::early::prepare();

    let tauri_app = hinina_lib::bootstrap::builder()
        .setup(|app| hinina_lib::bootstrap::setup::install(app.handle()))
        .invoke_handler(hinina_lib::commands::invoke_handler())
        .build(tauri::generate_context!())
        .unwrap_or_else(|error| panic!("无法启动桌面应用：{error}"));

    tauri_app.run(hinina_lib::bootstrap::handle_run_event);
}
