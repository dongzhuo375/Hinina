use tauri::State;
use tracing::{info, warn};

use crate::core::context::AppContext;
use crate::core::entity::user::User;
use crate::core::error::AppResult;
use crate::core::provider::oj_id::OjId;
use crate::service::auth::SessionValidity;

/// 登录 Command。
///
/// 前端 invoke 签名: `login`({ username, password, ojType? })
///
/// 若传入 `ojType`，先切换 ProviderRegistry 的当前 OJ 再执行登录。
/// OJ 身份是数据（字符串 id）：不再经闭集枚举解析，改为校验该 id 是否已注册
/// （未注册只告警并沿用当前 OJ，不阻断登录）。
#[tauri::command]
pub async fn login(
    ctx: State<'_, AppContext>,
    username: String,
    password: String,
    oj_type: Option<String>,
) -> AppResult<User> {
    // 如果前端指定了 OJ，先切换（校验是否已注册，取代旧的闭集枚举解析）
    if let Some(ref ot) = oj_type {
        let id = OjId::new(ot);
        if ctx.provider_registry.list_available().contains(&id) {
            ctx.provider_registry.set_current(id);
            info!(oj_id = ot, "已切换 OJ");
        } else {
            warn!(oj_id = ot, "未注册的 OJ，沿用当前默认值");
        }
    }

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
