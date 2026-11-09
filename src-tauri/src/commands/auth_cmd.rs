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
/// 切 OJ 与登录是两个独立意图，混在一起会让 `OjSwitched` 事件发了也无人能观察。
#[tauri::command]
pub async fn login(
    ctx: State<'_, AppContext>,
    username: String,
    password: String,
) -> AppResult<User> {
    login_impl(&ctx, &username, &password).await
}

/// `login` 的真实编排（提取自命令本体，供命令与测试共用同一实现）。
pub(crate) async fn login_impl(
    ctx: &AppContext,
    username: &str,
    password: &str,
) -> AppResult<User> {
    // **无论登录成败都清进入记忆**：provider.login 一旦被服务端接受，适配器
    // 内存里已是新 token——即使随后会话落盘失败让整体返回 Err（`?` 会提前
    // 返回）。旧账号的访问结论必须失效，否则新 token 会继续复用旧账号的
    // Rejected 记忆（复用消息掩盖真实结果）或 Success 记忆（跳过复查）。
    // 失败登录（口令错误）时清理同样无害：旧会话仍在，下次加载重新复核
    // access 最多多一次 GET。
    let result = ctx.auth.login(username, password).await;
    ctx.contest.clear_access_attempts();
    result
}

/// 登出 Command。
///
/// 前端 invoke 签名: `logout`
///
/// 编排三件事：清除会话（`AuthService::logout`，其内部即使远端登出失败也返回 Ok），
/// 以及**清空用户域缓存**（终态提交详情/测试点，含源代码）—— 缓存是内存态，
/// 不清理会让同机换账号后仍能读到上一位选手的提交内容；以及**清空私有赛进入
/// 尝试记忆** —— 报名状态是账号级的，旧账号的记忆对新账号不再成立。
#[tauri::command]
pub async fn logout(ctx: State<'_, AppContext>) -> AppResult<()> {
    logout_impl(&ctx).await
}

/// `logout` 的真实编排（提取自命令本体，供命令与测试共用同一实现）。
pub(crate) async fn logout_impl(ctx: &AppContext) -> AppResult<()> {
    let result = ctx.auth.logout().await;
    ctx.submission.clear_user_caches();
    ctx.contest.clear_access_attempts();
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
    Ok(get_session_impl(&ctx))
}

/// `get_session` 的 VO 映射（Session → 前端 User 形状）。
pub(crate) fn get_session_impl(ctx: &AppContext) -> Option<User> {
    ctx.auth.get_session().map(|s| User {
        id: s.user_id,
        username: s.username,
        token: s.token,
    })
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

#[cfg(test)]
#[path = "tests/auth_cmd_tests.rs"]
mod tests;
