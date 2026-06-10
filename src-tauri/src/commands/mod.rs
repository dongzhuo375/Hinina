// Tauri Command 注册入口。
// 所有 Command 在此模块注册到 Tauri App。
pub mod auth_cmd;
pub mod contest_cmd;
pub mod problem_cmd;
pub mod submission_cmd;
pub mod workspace_cmd;
pub mod config_cmd;
pub mod theme_cmd;

use tauri::Manager;

/// 注册所有 Command 到 Tauri App。
/// 在 main.rs 的 `.invoke_handler()` 中调用此函数。
pub fn register_commands(app: &mut tauri::App) {
    // TODO: 注册所有 Command handler
    let _ = app;
    todo!("register_commands()")
}
