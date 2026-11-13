// Tauri Command 模块树与 invoke handler 注册表。
//
// 全部 Command 由本模块的 `invoke_handler()` 一次注册（标准
// `tauri::generate_handler!`），由 main.rs 接线到 `bootstrap::builder()`
// 产出的 Builder；不在 `.setup()` 中手动注册，避免兼容性问题。

pub mod app_cmd;
pub mod auth_cmd;
pub mod config_cmd;
pub mod contest_cmd;
pub mod data_dir_cmd;
pub mod log_cmd;
pub mod maintenance_cmd;
pub mod oj_cmd;
pub mod plugin_cmd;
pub mod problem_cmd;
pub mod submission_cmd;
pub mod workspace_cmd;

/// 构造全部 Command 的 invoke handler。
///
/// 列表顺序无关语义；每个条目由 `generate_handler!` 在编译期解析，路径写错或
/// 漏掉 `#[tauri::command]` 都会直接编译失败。新增 Command 时在此登记。
///
/// handler 固定为 `Wry`：`open_external` / `pick_data_dir` 的命令参数是具体
/// `AppHandle`（默认 `Wry`），而 `CommandArg` 只为 `AppHandle<R>` 实现——若这里
/// 泛型化，展开处会要求 `AppHandle<Wry>: CommandArg<'_, 任意 R>` 而不成立。
pub fn invoke_handler() -> impl Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        crate::commands::app_cmd::launch_flags,
        crate::commands::app_cmd::open_external,
        crate::commands::plugin_cmd::plugin_list,
        crate::commands::plugin_cmd::plugin_start,
        crate::commands::plugin_cmd::plugin_stop,
        crate::commands::plugin_cmd::plugin_reload,
        crate::commands::plugin_cmd::plugin_resync,
        crate::commands::auth_cmd::login,
        crate::commands::auth_cmd::logout,
        crate::commands::auth_cmd::get_session,
        crate::commands::auth_cmd::validate_session,
        crate::commands::oj_cmd::switch_oj,
        crate::commands::contest_cmd::list_contests,
        crate::commands::contest_cmd::load_configured_contest,
        crate::commands::contest_cmd::get_contest_rank,
        crate::commands::contest_cmd::list_contest_announcements,
        crate::commands::contest_cmd::get_read_announcement_ids,
        crate::commands::contest_cmd::mark_announcements_read,
        crate::commands::problem_cmd::get_problem,
        crate::commands::problem_cmd::get_user_problem_status,
        crate::commands::problem_cmd::get_contest_problem_limits,
        crate::commands::submission_cmd::submit_code,
        crate::commands::submission_cmd::get_judgement,
        crate::commands::submission_cmd::list_contest_submissions,
        crate::commands::submission_cmd::get_submission_detail,
        crate::commands::submission_cmd::get_submission_cases,
        crate::commands::workspace_cmd::load_workspace,
        crate::commands::workspace_cmd::save_workspace,
        crate::commands::workspace_cmd::update_workspace_file,
        crate::commands::workspace_cmd::set_workspace_language,
        crate::commands::workspace_cmd::delete_workspace_file,
        crate::commands::config_cmd::get_config,
        crate::commands::config_cmd::reload_config,
        crate::commands::config_cmd::update_config,
        crate::commands::config_cmd::get_storage_info,
        crate::commands::log_cmd::frontend_log,
        crate::commands::maintenance_cmd::reset_client,
        crate::commands::maintenance_cmd::local_data_usage,
        crate::commands::maintenance_cmd::purge_local_data,
        crate::commands::data_dir_cmd::get_data_dir,
        crate::commands::data_dir_cmd::set_data_dir,
        crate::commands::data_dir_cmd::reset_data_dir,
        crate::commands::data_dir_cmd::pick_data_dir,
    ]
}

#[cfg(test)]
#[path = "tests/app_cmd_tests.rs"]
mod app_cmd_tests;

#[cfg(test)]
#[path = "tests/mod_tests.rs"]
mod tests;
