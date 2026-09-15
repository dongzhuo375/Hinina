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

// ── classify_status（GET 重试决策）──
//
// 回归背景：5xx 在重试耗尽后曾落到 `return Ok(response)`，把网关的 HTML 错误页
// 当成正常响应交给上层，最终报成「响应不是合法 JSON」而不是「HTTP 502」，
// 把排障引向错误方向；同时 retry_get 末尾的 Err(last_error) 成了永不可达的死代码。

#[test]
fn classify_accepts_success_on_any_attempt() {
    for status in [StatusCode::OK, StatusCode::CREATED, StatusCode::NO_CONTENT] {
        for attempt in 0..=MAX_RETRIES {
            assert_eq!(
                classify_status(status, attempt),
                StatusDecision::Accept,
                "HTTP {} 应被接受",
                status.as_u16()
            );
        }
    }
}

#[test]
fn classify_retries_server_error_while_budget_remains() {
    for status in [
        StatusCode::INTERNAL_SERVER_ERROR,
        StatusCode::BAD_GATEWAY,
        StatusCode::SERVICE_UNAVAILABLE,
        StatusCode::GATEWAY_TIMEOUT,
    ] {
        for attempt in 0..MAX_RETRIES {
            assert_eq!(
                classify_status(status, attempt),
                StatusDecision::Retry,
                "HTTP {} 第 {} 次应重试",
                status.as_u16(),
                attempt
            );
        }
    }
}

#[test]
fn classify_fails_server_error_once_retry_budget_exhausted() {
    // 这条是上述回归的正面锁定：耗尽后必须报错，不能把错误页当成功响应
    assert_eq!(
        classify_status(StatusCode::BAD_GATEWAY, MAX_RETRIES),
        StatusDecision::Fail
    );
    assert_eq!(
        classify_status(StatusCode::INTERNAL_SERVER_ERROR, MAX_RETRIES),
        StatusDecision::Fail
    );
}

#[test]
fn classify_fails_client_error_without_retry() {
    // 客户端错误重试只会得到同样的结果；401 还必须保持 Auth 变体以触发会话守卫
    for status in [
        StatusCode::BAD_REQUEST,
        StatusCode::UNAUTHORIZED,
        StatusCode::FORBIDDEN,
        StatusCode::NOT_FOUND,
    ] {
        for attempt in 0..=MAX_RETRIES {
            assert_eq!(
                classify_status(status, attempt),
                StatusDecision::Fail,
                "HTTP {} 不应重试",
                status.as_u16()
            );
        }
    }
}

#[test]
fn classify_fails_redirect_leftovers() {
    // reqwest 默认自动跟随重定向，能走到这里说明重定向次数耗尽，同样不该当成功
    assert_eq!(
        classify_status(StatusCode::MOVED_PERMANENTLY, 0),
        StatusDecision::Fail
    );
}
