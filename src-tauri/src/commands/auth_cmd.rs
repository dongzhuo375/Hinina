use tauri::State;
use tracing::{info, warn};

use crate::core::context::AppContext;
use crate::core::entity::user::User;
use crate::core::error::AppResult;
use crate::core::provider::oj_type::OJType;

/// 登录 Command。
///
/// 前端 invoke 签名: `auth:login`({ username, password, oj_type? })
///
/// 若传入 `oj_type`，先切换 ProviderRegistry 的当前 OJ 再执行登录。
/// `oj_type` 支持 "HOJ" / "QDUOJ" / "HUSTOJ"（大小写不敏感）。
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
        let oj = match ot.to_uppercase().as_str() {
            "HOJ" => OJType::HOJ,
            "QDUOJ" => OJType::QDUOJ,
            "HUSTOJ" => OJType::HUSTOJ,
            other => {
                warn!(oj_type = other, "未知的 OJ 类型，使用当前默认值");
                // 未知类型不回退设置，直接进入登录流程
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
/// 前端 invoke 签名: `auth:logout`
#[tauri::command]
pub async fn logout(ctx: State<'_, AppContext>) -> AppResult<()> {
    ctx.auth.logout().await
}

/// 获取本地保存的会话信息。
///
/// 前端 invoke 签名: `auth:get_session`
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
