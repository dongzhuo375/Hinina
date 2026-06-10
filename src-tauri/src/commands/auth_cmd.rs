use tauri::State;

use crate::core::context::AppContext;
use crate::core::entity::user::User;
use crate::core::error::AppResult;

/// 登录 Command
/// invoke('auth:login', { username, password, oj_type })
#[tauri::command]
pub async fn login(
    ctx: State<'_, AppContext>,
    username: String,
    password: String,
    oj_type: String,
) -> AppResult<User> {
    let _ = (ctx, username, password, oj_type);
    todo!("auth_cmd::login()")
}

/// 登出 Command
/// invoke('auth:logout')
#[tauri::command]
pub async fn logout(ctx: State<'_, AppContext>) -> AppResult<()> {
    let _ = ctx;
    todo!("auth_cmd::logout()")
}

/// 获取当前会话
/// invoke('auth:get_session')
#[tauri::command]
pub async fn get_session(ctx: State<'_, AppContext>) -> AppResult<Option<User>> {
    let _ = ctx;
    todo!("auth_cmd::get_session()")
}
