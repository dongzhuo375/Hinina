// Tauri Command 注册入口。
// 所有 Command 在此模块注册到 Tauri App。
//
// 使用标准 `tauri::generate_handler!` + `invoke_handler()` 注册方式，
// 避免在 `.setup()` 中手动注册引入的兼容性问题。

pub mod auth_cmd;
pub mod config_cmd;
pub mod contest_cmd;
pub mod oj_cmd;
pub mod problem_cmd;
pub mod submission_cmd;
pub mod theme_cmd;
pub mod workspace_cmd;

#[cfg(test)]
#[path = "tests/mod_tests.rs"]
mod tests;
