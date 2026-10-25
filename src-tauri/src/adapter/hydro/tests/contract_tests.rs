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

/// 登录降级身份不进已验证用户缓存（M2 评审 P2）：探测失败的登录仍成功，
/// 但随后的 validate_session 必须真实回访服务端 —— 探测失败判 Unknown（Err）、
/// 服务端给出身份后恢复 Ok(true)，而不是被占位用户缓存无条件判有效。
#[tokio::test]
async fn validate_session_does_not_trust_degraded_cached_identity() {
    // 探测走 raw GET：5xx 由 infra 重试（首次 + 2 次重试 = 一次探测 3 个请求）
    let probe_500 = || ScriptedResponse::with(500, vec![], "boom");
    let server = LoopbackServer::start(vec![
        login_ok("sid-1"),
        // 登录后的用户上下文探测失败（3 个请求）→ 身份降级（登录成功，uid 空）
        probe_500(),
        probe_500(),
        probe_500(),
        // validate_session 的探测同样失败（3 个请求）→ 三态契约的「无法判定」（Err）
        probe_500(),
        probe_500(),
        probe_500(),
        // 服务端恢复：给出真实身份 → 判有效
        ScriptedResponse::json(r#"{"UserContext":{"_id":7,"uname":"alice"}}"#),
    ])
    .await;
    let adapter = fixture(server.url());

    adapter
        .login("alice", "pw")
        .await
        .expect("探测失败不应否定登录");
    // 登录 = POST + 1 次探测（探测 500 经 infra GET 重试，一次探测占多个请求，
    // 故这里只断言「探测确实发生过」，不与重试次数耦合）
    assert!(server.requests().len() >= 2);

    // 关键断言：若占位用户被写入了缓存，这里会直接 Ok(true) 且不发请求
    let requests_before_validate = server.requests().len();
    let verdict = adapter.validate_session().await;
    assert!(
        verdict.is_err(),
        "探测失败属「无法判定」，不得因降级身份缓存判有效: {:?}",
        verdict
    );
    assert!(
        server.requests().len() > requests_before_validate,
        "validate_session 必须真实回访服务端（缓存未命中）"
    );

    assert!(
        adapter
            .validate_session()
            .await
            .expect("服务端给出身份后应判有效"),
        "恢复路径应返回 Ok(true)"
    );

    server.shutdown();
}

// ── 会话校验与身份缓存（PR #52 评审 P0 两项）──

/// 已缓存身份的 validate_session 仍必须真实回访服务端（评审 P0-1）：
/// 身份缓存只服务展示查询；校验走缓存会让「服务器吊销 sid」在进程内不可见。
#[tokio::test]
async fn validate_session_revalidates_despite_cached_identity() {
    let server = LoopbackServer::start(vec![
        login_ok("sid-1"),
        // 登录探测成功：真实身份进缓存
        ScriptedResponse::json(r#"{"UserContext":{"_id":7,"uname":"alice"}}"#),
        // validate 的全新探测：服务端不可达（3 个请求 = 首次 + 2 次重试）
        ScriptedResponse::with(500, vec![], "boom"),
        ScriptedResponse::with(500, vec![], "boom"),
        ScriptedResponse::with(500, vec![], "boom"),
    ])
    .await;
    let adapter = fixture(server.url());

    let user = adapter.login("alice", "pw").await.unwrap();
    assert_eq!(user.id, "7", "登录探测成功后身份应已缓存");
    let requests_after_login = server.requests().len();

    // 若校验错误地走了缓存，这里会直接 Ok(true) 且零请求
    let verdict = adapter.validate_session().await;
    assert!(
        verdict.is_err(),
        "服务端不可达属「无法判定」（Err），不得因缓存身份判有效: {:?}",
        verdict
    );
    assert!(
        server.requests().len() > requests_after_login,
        "validate_session 必须回访服务端，不得被身份缓存短路"
    );

    server.shutdown();
}

/// 服务端已吊销（匿名）：缓存的旧身份必须被推翻，判失效并清除（评审 P0-1）。
#[tokio::test]
async fn validate_session_reports_invalid_when_server_says_anonymous() {
    let server = LoopbackServer::start(vec![
        login_ok("sid-1"),
        ScriptedResponse::json(r#"{"UserContext":{"_id":7,"uname":"alice"}}"#),
        // validate 的全新探测：服务端判定匿名（_id == 0）
        ScriptedResponse::json(r#"{"UserContext":{"_id":0}}"#),
    ])
    .await;
    let adapter = fixture(server.url());

    adapter.login("alice", "pw").await.unwrap();

    let verdict = adapter.validate_session().await;
    assert!(
        !verdict.unwrap(),
        "服务端匿名 = 会话已失效，缓存身份不得掩盖"
    );

    server.shutdown();
}

/// 换号登录：新身份不得沿用上一个账号的缓存（评审 P0-2）。
#[tokio::test]
async fn login_switching_accounts_refreshes_cached_identity() {
    let server = LoopbackServer::start(vec![
        login_ok("sid-1"),
        ScriptedResponse::json(r#"{"UserContext":{"_id":7,"uname":"alice"}}"#),
        login_ok("sid-2"),
        ScriptedResponse::json(r#"{"UserContext":{"_id":9,"uname":"bob"}}"#),
    ])
    .await;
    let adapter = fixture(server.url());

    let first = adapter.login("alice", "pw").await.unwrap();
    assert_eq!(first.id, "7");

    let second = adapter.login("bob", "pw").await.expect("换号登录应成功");
    assert_eq!(
        second.id, "9",
        "换号后身份必须来自新探测（旧缓存未清会返回 alice 的 7）"
    );
    assert_eq!(second.token, "sid-2");

    server.shutdown();
}
