use thiserror::Error;

use crate::core::error::AppError;

/// Hydro 适配器专用错误。
///
/// 与 `HOJError` 同构：Adapter 内部只描述「Hydro 报了什么」，
/// 翻译成 `AppError` 时才决定变体。翻译有一条硬约束 ——
/// **会话失效（`Auth`）与业务错误的边界绝不能错**：前端 `sessionGuard`
/// 依据 `variant === 'Auth'` 判定会话失效并清会话回登录页，
/// 而 `AuthService::validate_session` 的三态契约也依赖它。
#[derive(Debug, Error)]
pub enum HydroError {
    /// Hydro 业务错误：响应体 `{"error":{"name","params","code"}}`。
    ///
    /// Hydro 的错误**不带 message**（`message` 是原型上的非枚举 getter，
    /// 不参与 JSON 序列化），只有 `name` + `params`，故必须原样带上，
    /// 由调用方（或前端）自行组织文案。
    #[error("Hydro API 错误 {name}(code={code}) params={params:?}")]
    ApiError {
        code: i32,
        name: String,
        params: Vec<serde_json::Value>,
    },

    /// HTTP 传输失败（含 infra 层映射出的状态码错误）
    #[error("Hydro 网络请求失败: {0}")]
    HttpError(String),

    /// JSON 解析失败
    #[error("Hydro 响应解析失败: {0}")]
    JsonError(String),

    /// 未认证 / 会话已失效
    #[error("Hydro 未授权: {0}")]
    Unauthorized(String),

    /// 评测状态码不在 Hydro 已知码表内
    #[error("Hydro 未知评测状态码: {0}")]
    UnknownStatus(i64),
}

/// 判断 Hydro 错误名是否属于「会话失效」。
///
/// 判定刻意保守（与 HOJ 侧 `auth_failure_from_body` 同一思路）：
/// 只有明确表示「没登录 / 登录失败」的名字才算会话问题。
/// - `PrivilegeError`：未登录（域权限校验失败），`params` 里带所需的 PRIV 常量；
/// - `LoginError` / `BuiltinLoginError`：登录失败（用户名或密码错误）。
///
/// **`PermissionError` 不算**：它表示「已登录但无权限」（如未报名私有赛），
/// 误判成会话失效会把已登录选手踢回登录页 —— 赛场上这是最坏的失败方式。
/// 同理 `CsrfTokenError`（Referer 校验）、`OpcountExceededError`（限流）、
/// `BlacklistedError`（IP 黑名单）都是业务/环境问题，不是会话过期。
pub fn is_auth_error_name(name: &str) -> bool {
    matches!(name, "PrivilegeError" | "LoginError" | "BuiltinLoginError")
}

/// 按错误名的领域前缀归类业务错误。
///
/// Hydro 的错误名遵循 `<领域><条件>Error` 约定（`ProblemNotFoundError`、
/// `ContestNotLiveError`、`RecordNotFoundError` …），据此归入既有的
/// `Problem` / `Contest` / `Submission` 变体，与 HOJ Adapter 的分类粒度对齐。
///
/// **这是尽力而为的启发式**：Hydro 未公开完整错误名清单，未命中前缀的一律落
/// `Unknown`（而不是猜一个领域）—— `AppError` 变体是前端的展示分流依据，
/// 猜错比不分类更糟；而控制流只依赖 `Auth`，不受此处影响。
fn classify_business_error(name: &str, code: i32, detail: &str) -> AppError {
    let msg = format!("Hydro {} (code={}) {}", name, code, detail);
    if name.starts_with("Problem") {
        return AppError::Problem(msg);
    }
    if name.starts_with("Contest") || name.starts_with("Homework") {
        return AppError::Contest(msg);
    }
    if name.starts_with("Record") {
        return AppError::Submission(msg);
    }
    AppError::Unknown(msg)
}

impl From<HydroError> for AppError {
    fn from(e: HydroError) -> Self {
        match e {
            HydroError::ApiError {
                code,
                name,
                params,
            } => {
                // 会话失效优先于领域归类：`PrivilegeError` 也带 403，
                // 但它的语义是「未登录」，必须让前端能据此登出。
                if is_auth_error_name(&name) {
                    return AppError::Auth(format!(
                        "Hydro {} 登录状态已失效（code={}）",
                        name, code
                    ));
                }
                classify_business_error(&name, code, &format!("params={:?}", params))
            }
            HydroError::HttpError(msg) => AppError::Network(format!("Hydro: {}", msg)),
            HydroError::JsonError(msg) => AppError::Serialization(format!("Hydro: {}", msg)),
            HydroError::Unauthorized(msg) => AppError::Auth(format!("Hydro: {}", msg)),
            HydroError::UnknownStatus(code) => {
                AppError::Unknown(format!("Hydro 未知评测状态码: {}", code))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api(code: i32, name: &str) -> AppError {
        HydroError::ApiError {
            code,
            name: name.into(),
            params: vec![],
        }
        .into()
    }

    #[test]
    fn privilege_error_is_session_failure() {
        let e: AppError = HydroError::ApiError {
            code: 403,
            name: "PrivilegeError".into(),
            params: vec![serde_json::json!("PRIV_USER_PROFILE")],
        }
        .into();
        assert!(matches!(e, AppError::Auth(_)), "未登录必须映射为 Auth");
    }

    #[test]
    fn permission_error_is_not_session_failure() {
        // 已登录但无权限（未报名私有赛）：绝不能踢人
        assert!(!matches!(api(403, "PermissionError"), AppError::Auth(_)));
    }

    #[test]
    fn csrf_and_rate_limit_are_not_session_failure() {
        for name in ["CsrfTokenError", "OpcountExceededError", "BlacklistedError"] {
            assert!(
                !matches!(api(403, name), AppError::Auth(_)),
                "{} 不应触发登出",
                name
            );
        }
    }

    #[test]
    fn business_errors_are_classified_by_domain_prefix() {
        assert!(matches!(
            api(404, "ProblemNotFoundError"),
            AppError::Problem(_)
        ));
        assert!(matches!(api(403, "ContestNotLiveError"), AppError::Contest(_)));
        assert!(matches!(
            api(404, "RecordNotFoundError"),
            AppError::Submission(_)
        ));
        assert!(matches!(api(404, "HomeworkNotFoundError"), AppError::Contest(_)));
    }

    #[test]
    fn unknown_name_falls_back_to_unknown_not_a_guess() {
        assert!(matches!(api(500, "SystemError"), AppError::Unknown(_)));
    }

    #[test]
    fn transport_and_parse_failures_keep_their_variants() {
        let net: AppError = HydroError::HttpError("timeout".into()).into();
        assert!(matches!(net, AppError::Network(_)));
        let ser: AppError = HydroError::JsonError("bad json".into()).into();
        assert!(matches!(ser, AppError::Serialization(_)));
        let auth: AppError = HydroError::Unauthorized("no sid".into()).into();
        assert!(matches!(auth, AppError::Auth(_)));
    }
}
