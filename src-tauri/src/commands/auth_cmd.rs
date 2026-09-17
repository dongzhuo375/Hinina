use tauri::State;
use tracing::{info, warn};

use crate::core::context::AppContext;
use crate::core::entity::user::User;
use crate::core::error::AppResult;
use crate::core::provider::oj_type::OJType;
use crate::service::auth::SessionValidity;

/// 从字符串解析 OJType（大小写不敏感）。
/// 提取为公开函数以支持 P40 测试。
pub fn parse_oj_type(s: &str) -> Option<OJType> {
    match s.to_uppercase().as_str() {
        "HOJ" => Some(OJType::HOJ),
        "QDUOJ" => Some(OJType::QDUOJ),
        "HUSTOJ" => Some(OJType::HUSTOJ),
        _ => None,
    }
}

/// 登录 Command。
///
/// 前端 invoke 签名: `login`({ username, password, ojType? })
///
/// 若传入 `ojType`，先切换 ProviderRegistry 的当前 OJ 再执行登录。
/// `ojType` 支持 "HOJ" / "QDUOJ" / "HUSTOJ"（大小写不敏感）。
/// 未传入时使用 Registry 当前配置的默认 OJ。
#[tauri::command]
pub async fn login(
    ctx: State<'_, AppContext>,
    username: String,
    password: String,
    oj_type: Option<String>,
) -> AppResult<User> {
    // 如果前端指定了 OJ 类型，先切换
    if let Some(ref ot) = oj_type {
        let oj = match parse_oj_type(ot) {
            Some(oj) => oj,
            None => {
                warn!(oj_type = ot, "未知的 OJ 类型，使用当前默认值");
                return ctx.auth.login(&username, &password).await;
            }
        };
        ctx.provider_registry.set_current_oj(oj);
        info!(oj_type = ot, "已切换 OJ 类型");
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
