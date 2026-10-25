// Hydro 适配器的本地 scripted HTTP 契约测试（M2 工作包 B）。
//
// 锁定 Hydro 的会话语义：登录响应体无 token，会话靠 `Set-Cookie: sid` 下发；
// 带 Accept: application/json 时重定向被序列化为 200+{"url":...}（不 302），
// Set-Cookie 留在同一次响应里；后续请求以 Cookie sid + `Authorization: Bearer`
// 双形态携带会话（该头一旦出现即覆盖 Cookie）。

use std::sync::Arc;

use crate::adapter::hydro::HydroAdapter;
use crate::core::provider::auth::AuthProvider;
use crate::infra::http::HttpClient;
use crate::test_support::loopback::{LoopbackServer, ScriptedResponse};

fn fixture(base_url: &str) -> Arc<HydroAdapter> {
    Arc::new(HydroAdapter::new(
        Arc::new(HttpClient::with_timeout(std::time::Duration::from_secs(5)).unwrap()),
        base_url.to_string(),
    ))
}

/// 登录成功响应：Set-Cookie 下发 sid；body 是被序列化的重定向 {"url":"/"}。
fn login_ok(sid: &str) -> ScriptedResponse {
    ScriptedResponse::with(
        200,
        vec![(
            "Set-Cookie".into(),
            format!("sid={}; Path=/; HttpOnly", sid),
        )],
        r#"{"url":"/"}"#,
    )
}

/// 登录：POST /login（rememberme 开启），sid 取自 Set-Cookie；
/// 登录后的用户上下文探测失败时身份降级（uname 用输入值、id 空、token 为裸 sid）。
#[tokio::test]
async fn login_extracts_sid_and_next_request_carries_session() {
    let server = LoopbackServer::start(vec![
        login_ok("sid-abc-123"),
        // 探测 GET / 失败（500）：身份降级路径 —— 不让一次探测失败否定已建立的会话
        ScriptedResponse::with(500, vec![], "boom"),
    ])
    .await;
    let adapter = fixture(server.url());

    let user = adapter
        .login("alice", "pw")
        .await
        .expect("探测失败不应否定登录");

    assert_eq!(user.username, "alice");
    assert_eq!(user.id, "", "探测失败时身份降级为空 id");
    assert_eq!(user.token, "sid-abc-123", "User.token 保存裸 sid");

    let reqs = server.requests();
    let login = &reqs[0];
    assert_eq!(login.method, "POST");
    assert_eq!(login.path, "/login");
    let body: serde_json::Value = serde_json::from_str(&login.body).unwrap();
    assert_eq!(body["uname"], "alice");
    assert_eq!(body["password"], "pw");
    assert_eq!(
        body["rememberme"], true,
        "赛场场景必须开启记住我（3h → 30d）"
    );
    assert!(
        login
            .header("accept")
            .unwrap_or("")
            .contains("application/json"),
        "Accept: application/json 是让重定向序列化为 200 的前提（否则丢 Set-Cookie）"
    );

    let probe = &reqs[1];
    assert_eq!(probe.method, "GET");
    assert_eq!(probe.path, "/");
    assert!(
        probe
            .header("cookie")
            .unwrap_or("")
            .contains("sid=sid-abc-123"),
        "后续请求应经 Cookie store 携带 sid"
    );
    assert_eq!(
        probe.header("authorization"),
        Some("Bearer sid-abc-123"),
        "持会话时附加 Bearer 头（该头一旦出现即覆盖 Cookie）"
    );

    server.shutdown();
}

/// 响应无 Set-Cookie: sid：认证失败（不是把无会话当成功）。
#[tokio::test]
async fn login_without_sid_cookie_fails_as_auth() {
    let server = LoopbackServer::start(vec![ScriptedResponse::json(r#"{"url":"/"}"#)]).await;
    let adapter = fixture(server.url());

    let err = adapter.login("alice", "pw").await.unwrap_err();
    assert!(
        matches!(err, crate::core::error::AppError::Auth(ref m) if m.contains("sid") || m.contains("Cookie")),
        "缺 sid 应为 Auth 错误，实得 {:?}",
        err
    );

    server.shutdown();
}
