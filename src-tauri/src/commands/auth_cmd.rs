use tauri::State;

use crate::core::context::AppContext;
use crate::core::entity::user::User;
use crate::core::error::AppResult;
use crate::service::auth::SessionValidity;

/// 登录 Command。
///
/// 前端 invoke 签名: `login`({ username, password })
///
/// OJ 切换走显式 `switch_oj`（应用级状态），不再作为登录的副作用 ——
/// 切 OJ 与登录是两个独立意图，混在一起会让 `OJSwitched` 事件发了也无人能观察。
#[tauri::command]
pub async fn login(
    ctx: State<'_, AppContext>,
    username: String,
    password: String,
) -> AppResult<User> {
    ctx.auth.login(&username, &password).await
}

/// 登出 Command。
///
/// 前端 invoke 签名: `logout`
///
/// 编排两件事：清除会话（`AuthService::logout`，其内部即使远端登出失败也返回 Ok），
/// 以及**清空用户域缓存**（终态提交详情/测试点，含源代码）—— 缓存是内存态，
/// 不清理会让同机换账号后仍能读到上一位选手的提交内容。
#[tauri::command]
pub async fn logout(ctx: State<'_, AppContext>) -> AppResult<()> {
    let result = ctx.auth.logout().await;
    ctx.submission.clear_user_caches();
    result
}

/// 获取本地保存的会话信息。
///
/// 前端 invoke 签名: `get_session`
///
/// 返回 `None` 表示无已保存的会话（从未登录或已登出）。
/// 返回的 `User.token` 可用于恢复 API 认证。
#[tauri::command]
pub async fn get_session(ctx: State<'_, AppContext>) -> AppResult<Option<User>> {
    let session = ctx.auth.get_session();
    Ok(session.map(|s| User {
        id: s.user_id,
        username: s.username,
        token: s.token,
    }))
}

/// 校验当前会话是否仍然有效 Command。
///
/// 前端 invoke 签名: `validate_session`
///
/// 三态返回 `valid` / `invalid` / `unknown`：
/// - `invalid`：本地无会话或服务端已判定失效（磁盘会话已被清除），前端须回到登录页
/// - `unknown`：网络异常等无法判定，本地会话保留，前端应稍后重试而非踢出用户
///
/// 供登录页赛前预检与全局会话守卫使用。
#[tauri::command]
pub async fn validate_session(ctx: State<'_, AppContext>) -> AppResult<SessionValidity> {
    Ok(ctx.auth.validate_session().await)
}
