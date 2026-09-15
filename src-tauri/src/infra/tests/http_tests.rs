use super::*;

use crate::core::error::AppError;
use reqwest::StatusCode;

// ── status_error 的变体映射 ──
//
// 变体是前端 sessionGuard 与 AuthService::validate_session 判定会话失效的唯一依据，
// 映射错了整条兜底链路就断了：401 若归为 Network，token 过期时选手只会看到
// 「网络错误」，永远回不到登录页。

#[test]
fn http_401_maps_to_auth_variant() {
    let err = status_error("http://oj/api/get-user-auth-info", StatusCode::UNAUTHORIZED);
    assert!(
        matches!(err, AppError::Auth(_)),
        "HTTP 401 的标准语义是「未认证」，必须映射为 Auth 变体，实际 {:?}",
        err
    );
    // 消息需保留状态码与 URL，便于现场排障
    let msg = err.user_message();
    assert!(msg.contains("401"), "应含状态码: {}", msg);
    assert!(msg.contains("get-user-auth-info"), "应含 URL: {}", msg);
}

#[test]
fn http_403_stays_network_variant() {
    // 403 可能是「无权访问某场私有赛」这类业务限制而非会话问题，
    // 误判为 Auth 会把已登录选手踢回登录页
    let err = status_error("http://oj/api/get-contest-problem", StatusCode::FORBIDDEN);
    assert!(
        matches!(err, AppError::Network(_)),
        "HTTP 403 不应被判为会话失效，实际 {:?}",
        err
    );
}

#[test]
fn other_error_status_codes_stay_network_variant() {
    for status in [
        StatusCode::BAD_REQUEST,
        StatusCode::NOT_FOUND,
        StatusCode::INTERNAL_SERVER_ERROR,
        StatusCode::BAD_GATEWAY,
        StatusCode::SERVICE_UNAVAILABLE,
    ] {
        let err = status_error("http://oj/api/x", status);
        assert!(
            matches!(err, AppError::Network(_)),
            "HTTP {} 应为 Network 变体，实际 {:?}",
            status.as_u16(),
            err
        );
    }
}

#[test]
fn retry_delay_backs_off_exponentially() {
    assert_eq!(retry_delay(0), Duration::from_millis(1000));
    assert_eq!(retry_delay(1), Duration::from_millis(2000));
    assert_eq!(retry_delay(2), Duration::from_millis(4000));
}
